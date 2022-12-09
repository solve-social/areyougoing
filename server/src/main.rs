use std::{collections::HashMap, fs, net::SocketAddr, sync::Arc};

use areyougoing_shared::{Poll, PollQueryResult, PollStatus};
use axum::{extract::Path, response::IntoResponse, routing::get, Extension, Json, Router};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
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
        .layer(
            // logging
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(Extension(config))
        .layer(Extension(Arc::new(db)));

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
}

#[derive(Deserialize, Serialize, Default)]
struct Db {
    polls: HashMap<u64, PollData>,
}

const DB_PATH: &str = "data.ron";

impl Db {
    fn new() -> Self {
        if let Ok(string) = fs::read_to_string(DB_PATH) {
            if let Ok(db) = ron::de::from_str(&string) {
                return db;
            }
        }
        let mut db = Self::default();
        db.polls.insert(
            0,
            PollData {
                poll: Poll {
                    title: "Test Poll".to_string(),
                    announcement: None,
                    description: "Today, 3pm, you know where".to_string(),
                    expiration: None,
                    results: vec![],
                    status: PollStatus::SeekingResponses,
                    questions: vec![],
                },
            },
        );
        fs::write(
            DB_PATH,
            ron::ser::to_string_pretty(&db, PrettyConfig::new()).unwrap(),
        )
        .unwrap();
        db
    }
}

async fn get_poll(
    Extension(db): Extension<Arc<Db>>,
    Path(poll_id): Path<u64>,
) -> impl IntoResponse {
    Json(if let Some(poll_data) = db.polls.get(&poll_id) {
        PollQueryResult::Found(poll_data.poll.clone())
    } else {
        PollQueryResult::NotFound
    })
}
