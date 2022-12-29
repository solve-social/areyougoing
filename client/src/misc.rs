use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use egui::{pos2, vec2, Align, Layout, NumExt, Rect, RichText, Sense, Ui, Vec2};
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
        unsafe{$crate::misc::log(&format_args!($($t)*).to_string())}
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

    fn with_query(&self, path: Option<&str>) -> Url {
        let mut new_link = self.get().clone();
        new_link.set_query(path);
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

use crate::time::Instant;
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
                // opts.credentials(web_sys::RequestCredentials::Include);
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

pub trait UiExt {
    fn unequal_columns<R>(
        &mut self,
        column_widths: &[f32],
        add_contents: impl FnOnce(&mut [Self]) -> R,
    ) -> R;
    #[allow(clippy::type_complexity)]
    fn unequal_columns_dyn<'c, R>(
        &mut self,
        column_widths: &[f32],
        add_contents: Box<dyn FnOnce(&mut [Self]) -> R + 'c>,
    ) -> R
    where
        Self: std::marker::Sized;

    fn standard_width(&self) -> f32;

    fn indicate_loading(&mut self, last_time: &Option<Instant>);
}

impl UiExt for Ui {
    fn unequal_columns<R>(
        &mut self,
        column_widths: &[f32],
        add_contents: impl FnOnce(&mut [Self]) -> R,
    ) -> R {
        self.unequal_columns_dyn(column_widths, Box::new(add_contents))
    }

    fn unequal_columns_dyn<'c, R>(
        &mut self,
        column_widths: &[f32],
        add_contents: Box<dyn FnOnce(&mut [Self]) -> R + 'c>,
    ) -> R {
        // TODO(emilk): ensure there is space
        let spacing = self.spacing().item_spacing.x;
        let total_spacing = spacing * (column_widths.len() as f32 - 1.0);
        // let column_width = (self.available_width() - total_spacing) / (num_columns as f32);
        let top_left = self.cursor().min;

        let mut pos = top_left;
        let mut columns: Vec<Self> = column_widths
            .iter()
            .enumerate()
            .map(|(_col_idx, column_width)| {
                // let pos = top_left + vec2((col_idx as f32) * (column_width + spacing), 0.0);
                let child_rect = Rect::from_min_max(
                    pos,
                    pos2(pos.x + column_width, self.max_rect().right_bottom().y),
                );
                let mut column_ui =
                    self.child_ui(child_rect, Layout::top_down_justified(Align::LEFT));
                column_ui.set_width(*column_width);
                pos += vec2(column_width + spacing, 0.0);
                column_ui
            })
            .collect();

        let result = add_contents(&mut columns[..]);

        let mut max_column_widths = column_widths.to_vec();
        let mut max_height = 0.0;
        for (column, max_column_width) in columns.iter().zip(max_column_widths.iter_mut()) {
            *max_column_width = max_column_width.max(column.min_rect().width());
            max_height = column.min_size().y.max(max_height);
        }

        // Make sure we fit everything next frame:
        let total_required_width = total_spacing + max_column_widths.iter().sum::<f32>();

        let size = vec2(self.available_width().max(total_required_width), max_height);
        self.allocate_rect(Rect::from_min_size(top_left, size), Sense::hover());
        // self.advance_cursor_after_rect(Rect::from_min_size(top_left, size));
        result
    }

    fn standard_width(&self) -> f32 {
        const MIN_WIDTH: f32 = 24.0;
        let available_width = self.available_width().at_least(MIN_WIDTH);
        self.spacing().text_edit_width.min(available_width)
    }

    fn indicate_loading(&mut self, last_time: &Option<Instant>) {
        let mut ui = self.child_ui(self.ctx().available_rect(), Layout::bottom_up(Align::Min));
        if let Some(last_time) = last_time {
            ui.label(
                RichText::new(last_time.elapsed().as_secs().to_string())
                    .small()
                    .weak()
                    .color(ui.style().visuals.weak_text_color().linear_multiply(0.1)),
            );
        } else {
            ui.spinner();
        }
    }
}

pub struct ArrangeableList<'a, T> {
    items: &'a mut Vec<T>,
    inner: ArrangeableListInner,
}

#[derive(Default)]
pub struct ArrangeableListInner {
    num_items: usize,
    pub current_index: usize,
    min_items: usize,
    new_index: Option<usize>,
    delete_index: Option<usize>,
    swap_indices: Option<(usize, usize)>,
    item_description: String,
    item_spacing: Option<Vec2>,
    add_button_is_at_bottom: bool,
}

impl ArrangeableListInner {
    pub fn show_controls(&mut self, ui: &mut Ui) {
        let spacing = ui.spacing().clone();
        ui.spacing_mut().button_padding = vec2(0., 0.);
        if let Some(spacing) = self.item_spacing {
            ui.spacing_mut().item_spacing = spacing;
        } else {
            ui.spacing_mut().item_spacing = vec2(3., 1.);
        }

        ui.add_enabled_ui(self.num_items > self.min_items, |ui| {
            if ui
                .small_button("🗑")
                .on_hover_text(format!("Delete {}", self.item_description))
                .clicked()
            {
                self.delete_index = Some(self.current_index);
            }
        });

        ui.add_enabled_ui(self.current_index < self.num_items - 1, |ui| {
            if ui
                .small_button("⬇")
                .on_hover_text(format!("Move {} Down", self.item_description))
                .clicked()
            {
                self.swap_indices = Some((self.current_index, self.current_index + 1));
            }
        });
        ui.add_enabled_ui(self.current_index != 0, |ui| {
            if ui
                .small_button("⬆")
                .on_hover_text(format!("Move {} Up", self.item_description))
                .clicked()
            {
                self.swap_indices = Some((self.current_index, self.current_index - 1));
            }
        });
        if !self.add_button_is_at_bottom
            && ui
                .small_button("➕")
                .on_hover_text(format!("Insert {} After This", self.item_description))
                .clicked()
        {
            self.new_index = Some(self.current_index + 1);
        }

        *ui.spacing_mut() = spacing;
    }
}

impl<'a, T> ArrangeableList<'a, T>
where
    T: Default,
{
    pub fn new(items: &'a mut Vec<T>, item_description: &str) -> Self {
        Self {
            inner: ArrangeableListInner {
                num_items: items.len(),
                item_description: item_description.to_string(),
                ..Default::default()
            },
            items,
        }
    }

    pub fn min_items(mut self, min_items: usize) -> Self {
        self.inner.min_items = min_items;
        self
    }

    pub fn item_spacing(mut self, item_spacing: Vec2) -> Self {
        self.inner.item_spacing = Some(item_spacing);
        self
    }

    pub fn add_button_is_at_bottom(mut self) -> Self {
        self.inner.add_button_is_at_bottom = true;
        self
    }

    pub fn show<F>(&mut self, ui: &mut Ui, mut add_contents: F)
    where
        F: FnMut(&mut ArrangeableListInner, &mut Ui, &mut T),
    {
        if self.inner.min_items == 0
            && self.inner.num_items == 0
            && ui
                .small_button(format!("Add {}", self.inner.item_description))
                .clicked()
        {
            self.inner.new_index = Some(0);
        }

        for (item_i, item) in self.items.iter_mut().enumerate() {
            self.inner.current_index = item_i;
            add_contents(&mut self.inner, ui, item);

            if self.inner.add_button_is_at_bottom
                && ui
                    .small_button(format!("Add {}", self.inner.item_description))
                    .clicked()
            {
                self.inner.new_index = Some(self.inner.current_index + 1);
            }
        }
        if let Some(index) = self.inner.delete_index {
            self.items.remove(index);
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if self.items.len() < self.inner.min_items {
            self.inner.new_index = Some(self.items.len());
        }
        if let Some(index) = self.inner.new_index {
            self.items.insert(index, T::default());
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if let Some((a, b)) = self.inner.swap_indices {
            self.items.swap(a, b);
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }
}
