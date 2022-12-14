#![warn(clippy::all, rust_2018_idioms)]

mod app;
mod time;
pub use app::App;
pub mod misc;
pub mod new_poll;
pub mod participation;
pub mod retrieve;

// pub const SERVER_URL: &str = "http://127.0.0.1:3000";
pub const SERVER_URL: &str = "http://ec2-34-216-22-62.us-west-2.compute.amazonaws.com:3000";
