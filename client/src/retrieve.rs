#[allow(unused)]
use crate::misc::{console_log, log};
use crate::{app::PollState, misc::Pollable, SERVER_URL};
use areyougoing_shared::PollQueryResult;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

#[derive(Debug)]
pub enum RetrievingState {
    None,
    Fetching(JsFuture),
    Converting(JsFuture),
}

impl Default for RetrievingState {
    fn default() -> Self {
        Self::None
    }
}

impl RetrievingState {
    pub fn process(&mut self, next_poll_state: &mut Option<PollState>, poll_key: u64) {
        let mut next_retreiving_state = None;
        match self {
            RetrievingState::None => {
                let mut opts = RequestInit::new();
                opts.method("GET");
                opts.mode(RequestMode::Cors);
                let url = format!("{SERVER_URL}/{poll_key}");
                let request = Request::new_with_str_and_init(&url, &opts).unwrap();
                let window = web_sys::window().unwrap();
                next_retreiving_state = Some(RetrievingState::Fetching(JsFuture::from(
                    window.fetch_with_request(&request),
                )));
            }
            RetrievingState::Fetching(js_future) => {
                if let Some(result) = js_future.poll() {
                    next_retreiving_state = Some(RetrievingState::None);
                    if let Ok(resp_value) = result {
                        assert!(resp_value.is_instance_of::<Response>());
                        let resp: Response = resp_value.dyn_into().unwrap();

                        // Convert this other `Promise` into a rust `Future`.
                        if let Ok(json) = resp.json() {
                            next_retreiving_state =
                                Some(RetrievingState::Converting(JsFuture::from(json)));
                        }
                    }
                }
            }
            RetrievingState::Converting(js_future) => {
                if let Some(result) = js_future.poll() {
                    if let Ok(json) = result {
                        if let Ok(poll_query_result) = serde_wasm_bindgen::from_value(json) {
                            match poll_query_result {
                                PollQueryResult::Found(poll) => {
                                    *next_poll_state = Some(PollState::Found {
                                        poll,
                                        key: poll_key,
                                    });
                                }
                                PollQueryResult::NotFound => {
                                    *next_poll_state = Some(PollState::NotFound { key: poll_key });
                                }
                            }
                        } else {
                            next_retreiving_state = Some(RetrievingState::None);
                        }
                    }
                }
            }
        }
        if let Some(next_state) = next_retreiving_state {
            *self = next_state;
        }
    }
}
