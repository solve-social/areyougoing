use std::sync::atomic::{AtomicBool, Ordering};

use futures_lite::{future, Future};
use gloo::events::EventListener;
use url::Url;
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};
use web_sys::{Event, Window};

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
            .push_state_with_url(&JsValue::NULL, "", Some(&self.get().to_string()))
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
