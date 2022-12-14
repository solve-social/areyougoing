#![warn(clippy::all, rust_2018_idioms)]

mod app;
mod time;
pub use app::App;
pub mod misc;
pub mod new_poll;
pub mod participation;
pub mod poll;
pub mod results_ui;
pub mod retrieve;
pub mod toggle_switch;

// pub const SERVER_URL: &str = "http://127.0.0.1:3000";
pub const SERVER_URL: &str = "https://areyougoingserver.solve.social";
