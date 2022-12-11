use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use areyougoing_shared::{
    Form, FormResponse, Poll, PollQueryResult, PollResponse, PollStatus, PollSubmissionResult,
    Question,
};
use axum::{
    extract::Path,
    http::Method,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use headers::HeaderValue;
use ron::{extensions::Extensions, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "tower_http=warn".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::new();
    let db = Db::new();

    let app = Router::new()
        // .route("/", get(get_page))
        .route("/:poll_id", get(get_poll))
        .route("/submit", post(submit))
        .route("/new_poll", post(new_poll))
        .layer(
            // see https://docs.rs/tower-http/latest/tower_http/cors/index.html
            // for more details
            //
            // pay attention that for some request types like posting content-type: application/json
            // it is required to add ".allow_headers([http::header::CONTENT_TYPE])"
            // or see this issue https://github.com/tokio-rs/axum/issues/849
            CorsLayer::new()
                .allow_origin("http://127.0.0.1:5000".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET])
                .allow_credentials(true)
                .allow_headers([http::header::CONTENT_TYPE]),
        )
        .layer(
            // logging
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(Extension(config))
        .layer(Extension(Arc::new(Mutex::new(db))));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000)); // for offline use
                                                         // let addr = SocketAddr::from((
                                                         //     local_ip().expect("Failed to get local ip address"),
                                                         //     BIND_PORT,
                                                         // ));
    println!("Listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn submit(
    Extension(db): Extension<Arc<Mutex<Db>>>,
    Json(poll_response): Json<PollResponse>,
) -> impl IntoResponse {
    println!("{poll_response:?}");
    Json(if let Ok(mut db) = db.lock() {
        if let Some(poll_data) = db.0.get_mut(&poll_response.poll_id) {
            poll_data
                .responses
                .insert(poll_response.user.clone(), poll_response.responses);
            db.write();
            PollSubmissionResult::Success
        } else {
            PollSubmissionResult::Error
        }
    } else {
        PollSubmissionResult::Error
    })
}

async fn new_poll(
    Extension(db): Extension<Arc<Mutex<Db>>>,
    Json(poll_response): Json<PollResponse>,
) -> impl IntoResponse {
    println!("{poll_response:?}");
    Json(if let Ok(mut db) = db.lock() {
        if let Some(poll_data) = db.0.get_mut(&poll_response.poll_id) {
            poll_data
                .responses
                .insert(poll_response.user.clone(), poll_response.responses);
            db.write();
            PollSubmissionResult::Success
        } else {
            PollSubmissionResult::Error
        }
    } else {
        PollSubmissionResult::Error
    })
}

async fn get_poll(
    Extension(db): Extension<Arc<Mutex<Db>>>,
    Path(poll_id): Path<u64>,
) -> impl IntoResponse {
    Json(
        if let Some(poll_data) = db.lock().unwrap().0.get(&poll_id) {
            PollQueryResult::Found(poll_data.poll.clone())
        } else {
            PollQueryResult::NotFound
        },
    )
}

#[derive(Clone)]
struct Config {}

impl Config {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Deserialize, Serialize)]
struct PollData {
    poll: Poll,
    responses: HashMap<String, Vec<FormResponse>>,
}

#[derive(Deserialize, Serialize, Default)]
struct Db(HashMap<u64, PollData>);

const DB_PATH: &str = "data.ron";

impl Db {
    pub fn write(&self) {
        fs::write(
            DB_PATH,
            ron::ser::to_string_pretty(
                self,
                PrettyConfig::new()
                    .enumerate_arrays(true)
                    .extensions(Extensions::all())
                    .compact_arrays(true),
            )
            .unwrap(),
        )
        .unwrap();
    }
    fn new() -> Self {
        if let Ok(string) = fs::read_to_string(DB_PATH) {
            if let Ok(db) = ron::de::from_str(&string) {
                return db;
            }
        }
        let mut db = Self::default();
        db.0.insert(
            0,
            PollData {
                poll: Poll {
                    title: "Test Poll".to_string(),
                    announcement: None,
                    description: "Today, 3pm, you know where".to_string(),
                    expiration: None,
                    results: vec![],
                    status: PollStatus::SeekingResponses,
                    questions: vec![
                        Question {
                            prompt: "Are you going?".to_string(),
                            form: Form::ChooseOneorNone {
                                options: vec!["Yes".to_string(), "No".to_string()],
                            },
                        },
                        Question {
                            prompt: "How are you arriving?".to_string(),
                            form: Form::ChooseOneorNone {
                                options: vec![
                                    "Driving own car".to_string(),
                                    "Walking".to_string(),
                                    "Uber".to_string(),
                                ],
                            },
                        },
                        Question {
                            prompt: "Which restaurant would you prefer?".to_string(),
                            form: Form::ChooseOneorNone {
                                options: vec![
                                    "Chilis".to_string(),
                                    "Burger King".to_string(),
                                    "Cheddars".to_string(),
                                    "Papasitos".to_string(),
                                    "Taco Bell".to_string(),
                                ],
                            },
                        },
                    ],
                },
                responses: Default::default(),
            },
        );
        db.write();
        db
    }
}
