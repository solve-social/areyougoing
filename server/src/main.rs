use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use areyougoing_shared::{
    CreatePollResult, Form, FormResponse, Metric, MetricTracker, Poll, PollProgress,
    PollQueryResult, PollResponse, PollResult, PollStatus, PollSubmissionResult, Progress,
    ProgressReportResult, Question, Requirement, ResultState,
};
use axum::{
    extract::Query,
    http::Method,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use local_ip_address::local_ip;
use ron::{extensions::Extensions, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{Any, CorsLayer},
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
        .route("/", get(get_poll))
        .route("/submit", post(submit))
        .route("/new_poll", post(new_poll))
        .route("/progress", post(get_progress))
        .layer(
            // see https://docs.rs/tower-http/latest/tower_http/cors/index.html
            // for more details
            //
            // pay attention that for some request types like posting content-type: application/json
            // it is required to add ".allow_headers([http::header::CONTENT_TYPE])"
            // or see this issue https://github.com/tokio-rs/axum/issues/849
            CorsLayer::new()
                .allow_origin(Any)
                // .allow_origin("http://127.0.0.1:5000".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET])
                // .allow_credentials(true)
                .allow_headers([http::header::CONTENT_TYPE]),
        )
        .layer(
            // logging
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(Extension(config))
        .layer(Extension(Arc::new(Mutex::new(db))));

    // configure certificate and private key used by https
    let tls_config = RustlsConfig::from_pem_file(
        PathBuf::from("/etc/letsencrypt/live/areyougoingserver.solve.social/fullchain.pem"),
        PathBuf::from("/etc/letsencrypt/live/areyougoingserver.solve.social/privkey.pem"),
    )
    .await
    .unwrap();

    // let addr = SocketAddr::from(([127, 0, 0, 1], 3000)); // for offline use
    // let addr = SocketAddr::from((local_ip().expect("Failed to get local ip address"), 3000));
    let addr = SocketAddr::from((local_ip().expect("Failed to get local ip address"), 443));
    // println!("Listening on http://{addr}");
    println!("Listening on https://{addr}");
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .unwrap();
    // axum::Server::bind(&addr)
    //     .serve(app.into_make_service_with_connect_info::<SocketAddr>())
    //     .await
    //     .unwrap();
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
            poll_data.update_results();
            db.write();
            PollSubmissionResult::Success
        } else {
            PollSubmissionResult::Error
        }
    } else {
        PollSubmissionResult::Error
    })
}

fn get_unused_key(db: &MutexGuard<Db>) -> u64 {
    let mut key = 1;
    loop {
        if !db.0.contains_key(&key) {
            return key;
        }
        key += 1;
    }
}

async fn new_poll(
    Extension(db): Extension<Arc<Mutex<Db>>>,
    Json(poll): Json<Poll>,
) -> impl IntoResponse {
    Json(if let Ok(mut db) = db.lock() {
        let key = get_unused_key(&db);
        println!("New Poll at {key}: {poll:?}");
        db.0.insert(
            key,
            PollData {
                result_states: poll.results.iter().map(ResultState::from_result).collect(),
                progresses: poll
                    .metric_trackers
                    .iter()
                    .map(|t| match t.metric {
                        Metric::SpecificResponses { .. } => Progress::Count(0),
                    })
                    .collect(),
                poll,
                responses: Default::default(),
            },
        );
        CreatePollResult::Success { key }
    } else {
        CreatePollResult::Error
    })
}

#[derive(Debug, Deserialize, Serialize)]
struct GetPollQuery {
    poll_key: u64,
}

async fn get_poll(
    Extension(db): Extension<Arc<Mutex<Db>>>,
    Query(get_poll_query): Query<GetPollQuery>,
) -> impl IntoResponse {
    Json(
        if let Some(poll_data) = db.lock().unwrap().0.get(&get_poll_query.poll_key) {
            PollQueryResult::Found(poll_data.poll.clone())
        } else {
            PollQueryResult::NotFound
        },
    )
}

async fn get_progress(
    Extension(db): Extension<Arc<Mutex<Db>>>,
    Json(key): Json<u64>,
) -> impl IntoResponse {
    Json(if let Ok(db) = db.lock() {
        let poll_data = db.0.get(&key).unwrap();

        ProgressReportResult::Success {
            progress: PollProgress {
                result_states: poll_data.result_states.clone(),
                metric_progresses: poll_data
                    .poll
                    .metric_trackers
                    .iter()
                    .zip(poll_data.progresses.iter())
                    .map(|(t, p)| {
                        if t.publicly_visible {
                            Some(p.clone())
                        } else {
                            None
                        }
                    })
                    .collect(),
            },
        }
    } else {
        ProgressReportResult::Error
    })
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
    progresses: Vec<Progress>,
    result_states: Vec<ResultState>,
}

impl PollData {
    pub fn update_results(&mut self) {
        self.progresses = self
            .poll
            .metric_trackers
            .iter()
            .map(|t| t.metric.calculate_progress(&self.responses))
            .collect();
        self.result_states = self
            .poll
            .results
            .iter()
            .map(|r| {
                r.requirements
                    .iter()
                    .map(|r| r.evaluate(&self.progresses))
                    .collect::<Vec<_>>()
            })
            .map(|requirements_met| ResultState {
                overall_met: requirements_met.iter().all(|x| *x),
                requirements_met,
            })
            .collect();
    }
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

    fn get_from_file() -> Option<Self> {
        if let Ok(string) = fs::read_to_string(DB_PATH) {
            if let Ok(db) = ron::de::from_str(&string) {
                return Some(db);
            } else {
                panic!("Failed to parse the data file that was found!");
            }
        }
        None
    }

    fn new() -> Self {
        let mut db = Self::get_from_file().unwrap_or_else(|| {
            let mut db = Self::default();
            let default_questions = vec![
                Question {
                    prompt: "Are you going?".to_string(),
                    form: Form::OneOrNone {
                        options: vec!["Yes".to_string(), "No".to_string()],
                    },
                },
                Question {
                    prompt: "How are you arriving?".to_string(),
                    form: Form::OneOrNone {
                        options: vec![
                            "Driving own car".to_string(),
                            "Walking".to_string(),
                            "Uber".to_string(),
                        ],
                    },
                },
                Question {
                    prompt: "Which restaurant would you prefer?".to_string(),
                    form: Form::OneOrNone {
                        options: vec![
                            "Chilis".to_string(),
                            "Burger King".to_string(),
                            "Cheddars".to_string(),
                            "Papasitos".to_string(),
                            "Taco Bell".to_string(),
                        ],
                    },
                },
            ];
            db.0.insert(
                0,
                PollData {
                    poll: Poll {
                        title: "Test Poll".to_string(),
                        announcement: None,
                        description: "Today, 3pm, you know where".to_string(),
                        expiration: None,
                        results: vec![PollResult {
                            requirements: vec![Requirement::AtLeast {
                                metric_index: 0,
                                minimum: 2,
                            }],
                            desc: "The party happens".to_string(),
                        }],
                        metric_trackers: vec![MetricTracker::init_from_questions(
                            &default_questions,
                        )
                        .unwrap()],
                        status: PollStatus::SeekingResponses,
                        questions: default_questions,
                    },
                    responses: Default::default(),
                    progresses: Vec::new(),
                    result_states: Vec::new(),
                },
            );
            db
        });

        db.update_all_results();
        db.write();
        db
    }

    fn update_all_results(&mut self) {
        for poll_data in self.0.values_mut() {
            poll_data.update_results();
        }
    }
}
