use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_lite::{future, Future};
use gloo::events::EventListener;
use gloo::{console::__macro::JsValue, net::http::RequestMode};
use serde::{Deserialize, Serialize};
use url::Url;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, Window};
use web_sys::{Request, RequestInit, Response};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

#[allow(unused)]
macro_rules! console_log {
    ($($t:tt)*) => (
        #[allow(unused_unsafe)]
        unsafe{log(&format_args!($($t)*).to_string())}
    )
}
pub(crate) use console_log;

pub trait Pollable
where
    Self: Future + Sized + Unpin,
{
    fn poll(&mut self) -> Option<<Self as Future>::Output> {
        future::block_on(future::poll_once(&mut *self))
    }
}

impl<T> Pollable for T where T: Future + Sized + Unpin {}

pub trait UrlExt {
    fn get(&self) -> &Url;
    fn with_path<T: Into<String>>(&self, path: T) -> Url {
        let mut new_link = self.get().clone();
        let path: String = path.into();
        new_link.set_path(&path);
        new_link
    }

    fn push_to_window(&self) {
        web_sys::window()
            .expect("no global `window` exists")
            .history()
            .expect("Failed to access browser history")
            .push_state_with_url(&JsValue::NULL, "", Some(self.get().as_ref()))
            .expect("Failed to set URL");
    }
}

impl UrlExt for Url {
    fn get(&self) -> &Url {
        self
    }
}

impl UrlExt for Option<Url> {
    fn get(&self) -> &Url {
        self.as_ref().unwrap()
    }
}

pub fn get_window() -> Window {
    web_sys::window().expect("no global `window` exists")
}

pub fn listen_in_window<F>(event_type: &'static str, callback: F)
where
    F: FnMut(&Event) + 'static,
{
    let listener = EventListener::new(&get_window(), event_type, callback);
    listener.forget();
}

pub trait AtomicBoolExt {
    fn toggle(&self);
    fn set(&self, value: bool);
    fn get(&self) -> bool;
}

impl AtomicBoolExt for AtomicBool {
    fn toggle(&self) {
        self.store(!self.load(Ordering::SeqCst), Ordering::SeqCst);
    }

    fn set(&self, value: bool) {
        self.store(value, Ordering::SeqCst);
    }

    fn get(&self) -> bool {
        self.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
enum SubmitterState {
    None,
    Submitting(JsFuture),
    Converting(JsFuture),
}

#[derive(Debug)]
pub struct Submitter<SendT, ReceiveT> {
    path: String,
    data: SendT,
    state: SubmitterState,
    receive_t: PhantomData<ReceiveT>,
}

use crate::SERVER_URL;

impl<SendT: Serialize, ReceiveT: Debug + for<'de> Deserialize<'de>> Submitter<SendT, ReceiveT> {
    pub fn new(path: &str, data: SendT) -> Self {
        Self {
            path: path.to_string(),
            state: SubmitterState::None,
            data,
            receive_t: Default::default(),
        }
    }

    pub fn poll(&mut self) -> Option<ReceiveT> {
        let mut next_state = None;
        match &mut self.state {
            SubmitterState::None => {
                let mut opts = RequestInit::new();
                opts.method("POST");
                opts.body(Some(&JsValue::from(
                    serde_json::to_string(&self.data).unwrap(),
                )));
                opts.credentials(web_sys::RequestCredentials::Include);
                opts.mode(RequestMode::Cors);
                let url = format!("{SERVER_URL}/{}", self.path);
                let request = Request::new_with_str_and_init(&url, &opts).unwrap();
                request
                    .headers()
                    .set("Content-Type", "application/json")
                    .unwrap();
                next_state = Some(SubmitterState::Submitting(JsFuture::from(
                    get_window().fetch_with_request(&request),
                )));
            }
            SubmitterState::Submitting(ref mut future) => {
                if let Some(result) = future.poll() {
                    next_state = Some(SubmitterState::None);
                    if let Ok(response) = result {
                        assert!(response.is_instance_of::<Response>());
                        let resp: Response = response.dyn_into().unwrap();
                        if let Ok(json) = resp.json() {
                            next_state = Some(SubmitterState::Converting(JsFuture::from(json)));
                        }
                    }
                }
            }
            SubmitterState::Converting(ref mut future) => {
                if let Some(result) = future.poll() {
                    next_state = Some(SubmitterState::None);
                    if let Ok(json) = result {
                        if let Ok(submission_result) = serde_wasm_bindgen::from_value(json) {
                            console_log!("Received from server: {submission_result:?}");
                            return Some(submission_result);
                        }
                    }
                }
            }
        }
        if let Some(next_state) = next_state {
            self.state = next_state;
        }
        None
    }
}
