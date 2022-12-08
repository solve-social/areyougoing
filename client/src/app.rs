use std::time::Duration;

use areyougoing_shared::{Form, FormResponse, Poll, PollQueryResult, Question};
use derivative::Derivative;
use egui::TextEdit;
use egui::{Align, Button, CentralPanel, Layout, ScrollArea};
use futures_lite::future;
use futures_lite::Future;
use serde::{Deserialize, Serialize};
use url::Url;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::time::Instant;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    state: AppState,
    poll_state: PollState,
    user_entry: String,
    old_names: Vec<String>,
    // this how you opt-out of serialization of a member
    // #[serde(skip)]
}

enum RetrievingState {
    None,
    Fetching(JsFuture),
    Converting(JsFuture),
}

impl Default for RetrievingState {
    fn default() -> Self {
        Self::None
    }
}

enum SubmittingState {
    None,
    Fetching(JsFuture),
    Converting(JsFuture),
}

impl Default for SubmittingState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize)]
enum PollState {
    None,
    Creating {
        new_poll: Poll,
    },
    SubmittingPoll {
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        state: SubmittingState,
    },
    SubmittedPoll {
        key: u64,
    },
    Retrieving {
        key: u64,
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        state: RetrievingState,
    },
    Found {
        key: u64,
        poll: Poll,
    },
    NotFound {
        key: u64,
    },
}

impl Default for PollState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Deserialize, Serialize, PartialEq)]
enum AppState {
    SignedIn {
        user: String,
        responses: Vec<FormResponse>,
    },
    SignIn,
    Submitting {
        #[serde(skip)]
        progress: Option<Instant>,
    },
    SubmitConfirmation,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: AppState::SignIn,
            poll_state: PollState::None,
            // poll_state: PollState::Found {
            //     poll: Poll {
            //         title: "Party!".to_string(),
            //         description: "Saturday, 3pm, Mike's House".to_string(),
            //         announcement: None,
            //         expiration: None,
            //         results: vec![],
            //         status: PollStatus::SeekingResponses,
            //         questions: vec![Question {
            //             prompt: "Would you go?".to_string(),
            //             form: Form::ChooseOne {
            //                 options: vec!["YES".to_string(), "NO".to_string()],
            //             },
            //         }],
            //     },
            // },
            user_entry: "".to_string(),
            old_names: vec!["Sandra", "Peter", "Bob"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        }
    }
}
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[allow(unused)]
macro_rules! console_log {
    // Note that this is using the `log` function imported above during
    // `bare_bones`
    ($($t:tt)*) => (
        #[allow(unused_unsafe)]
        unsafe{log(&format_args!($($t)*).to_string())}
    )
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app: App = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        let window = web_sys::window().expect("no global `window` exists");
        let url_string = window.location().href().unwrap();
        if let Ok(url) = Url::parse(&url_string) {
            if let Some(segments) = &mut url.path_segments() {
                if let Some(first) = &segments.next() {
                    if let Ok(key) = first.parse::<u64>() {
                        let mut new_key = Some(key);
                        if let PollState::Found {
                            poll: _,
                            key: prexisting_key,
                        } = app.poll_state
                        {
                            if prexisting_key == key {
                                // If the key is the same as last time, cancel the reload.
                                new_key = None;
                            }
                        }
                        if let Some(key) = new_key {
                            app.poll_state = PollState::Retrieving {
                                key,
                                state: RetrievingState::None,
                            };
                        }
                    }
                }
            }
        }

        app
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            let mut next_poll_state = None;
            ui.vertical_centered(|ui| match &mut self.poll_state {
                PollState::None => {
                    next_poll_state = Some(PollState::Creating {
                        new_poll: Poll::default(),
                    });
                }
                PollState::Creating { new_poll } => {
                    ui.heading("Create a new poll!");
                    ui.separator();
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.label("Title:");
                        ui.text_edit_singleline(&mut new_poll.title);

                        ui.label("Description:");
                        ui.add(TextEdit::multiline(&mut new_poll.description).desired_rows(1));

                        let mut new_question_index = None;
                        if ui.button("Add Question").clicked() {
                            new_question_index = Some(0);
                        }
                        ui.separator();
                        for (question_i, question) in new_poll.questions.iter_mut().enumerate() {
                            ui.group(|ui| {
                                ui.label("Prompt:");
                                ui.add(TextEdit::multiline(&mut question.prompt).desired_rows(1));

                                match &mut question.form {
                                    Form::ChooseOne { ref mut options } => {
                                        let mut new_option_index = None;
                                        if ui.button("Add Option").clicked() {
                                            new_option_index = Some(0);
                                        }
                                        for (option_i, option) in options.iter_mut().enumerate() {
                                            ui.text_edit_singleline(option);
                                            if ui.small_button("Add Option").clicked() {
                                                new_option_index = Some(option_i + 1);
                                            }
                                        }
                                        if let Some(index) = new_option_index {
                                            options.insert(index, "".to_string())
                                        }
                                    }
                                }
                            });
                            if ui.button("Add Question").clicked() {
                                new_question_index = Some(question_i + 1);
                            }
                        }
                        if let Some(index) = new_question_index {
                            new_poll.questions.insert(
                                index,
                                Question {
                                    prompt: "".to_string(),
                                    form: Form::ChooseOne {
                                        options: Vec::new(),
                                    },
                                },
                            );
                        }
                        ui.separator();
                        if ui.button("SUBMIT").clicked() {}
                    });
                }
                PollState::SubmittingPoll { state: _ } => {
                    next_poll_state = Some(PollState::SubmittedPoll { key: 0 });
                }
                PollState::SubmittedPoll { key } => {
                    ui.label("Your new poll has been created!");
                    ui.label(format!(
                        "Share it with this link: http://127.0.0.1:5001/{key}"
                    ));
                }
                PollState::Retrieving { key, ref mut state } => {
                    ui.label(format!("Retreiving Poll #{key}"));
                    let mut next_retreiving_state = None;
                    match state {
                        RetrievingState::None => {
                            let mut opts = RequestInit::new();
                            opts.method("GET");
                            opts.mode(RequestMode::Cors);

                            let url = format!("http://127.0.0.1:3000/{key}");

                            let request = Request::new_with_str_and_init(&url, &opts).unwrap();

                            let window = web_sys::window().unwrap();
                            next_retreiving_state = Some(RetrievingState::Fetching(
                                JsFuture::from(window.fetch_with_request(&request)),
                            ));
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
                                    if let Ok(poll_query_result) =
                                        serde_wasm_bindgen::from_value(json)
                                    {
                                        match poll_query_result {
                                            PollQueryResult::Found(poll) => {
                                                next_poll_state = Some(PollState::Found {
                                                    poll,
                                                    key: key.clone(),
                                                });
                                            }
                                            PollQueryResult::NotFound => {
                                                next_poll_state =
                                                    Some(PollState::NotFound { key: key.clone() });
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
                        *state = next_state;
                    }
                }
                PollState::Found { key, poll } => {
                    ui.heading(format!("{} (#{key})", poll.title));

                    ui.label(&poll.description);
                    ui.separator();
                    let mut next_app_state = None;
                    match &mut self.state {
                        AppState::SignIn => {
                            ui.label(
                                "Type your name or choose a previous name \
                                    from below and select \"SIGN IN\"",
                            );
                            ui.text_edit_singleline(&mut self.user_entry);
                            if ui.button("SIGN IN").clicked() {
                                next_app_state = Some(AppState::SignedIn {
                                    user: self.user_entry.clone(),
                                    responses: poll.init_responses(),
                                });
                                if !self.old_names.contains(&self.user_entry) {
                                    self.old_names.push(self.user_entry.clone());
                                }
                                self.user_entry = "".to_string();
                            }
                            ui.separator();
                            ScrollArea::vertical().show(ui, |ui| {
                                for name in self.old_names.iter().rev() {
                                    if ui.button(name).clicked() {
                                        self.user_entry = name.to_string();
                                    }
                                }
                            });
                        }
                        AppState::SignedIn {
                            ref user,
                            ref mut responses,
                        } => {
                            ui.horizontal(|ui| {
                                ui.label(format!("Welcome, {user}!"));
                                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                    if ui.add(Button::new("Sign Out").small()).clicked() {
                                        next_app_state = Some(AppState::SignIn);
                                    }
                                });
                            });
                            for (question, mut response) in
                                poll.questions.iter().zip(responses.iter_mut())
                            {
                                ui.group(|ui| {
                                    ui.label(&question.prompt);
                                    match (&question.form, &mut response) {
                                        (
                                            Form::ChooseOne { options },
                                            FormResponse::ChooseOne { choice },
                                        ) => {
                                            for (i, option) in options.iter().enumerate() {
                                                let selected =
                                                    choice.is_some() && choice.unwrap() == i as u8;
                                                let mut button = Button::new(option);
                                                if selected {
                                                    button = button.fill(
                                                        ui.ctx().style().visuals.selection.bg_fill,
                                                    );
                                                }
                                                let response = ui.add(button);
                                                if !selected {
                                                    if response.clicked() {
                                                        *choice = Some(i as u8);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }

                            if ui.button("SUBMIT").clicked() {
                                next_app_state = Some(AppState::Submitting { progress: None });
                            }
                        }
                        AppState::Submitting { ref mut progress } => {
                            ui.label("Your response is being submitted...");
                            if let Some(start_time) = progress {
                                if start_time.elapsed() > Duration::from_secs_f64(1.0) {
                                    next_app_state = Some(AppState::SubmitConfirmation);
                                }
                            } else {
                                *progress = Some(Instant::now());
                            }
                        }
                        AppState::SubmitConfirmation => {
                            ui.label("Your response has been submitted!. Thanks!");
                            ui.label(
                                "To change your response, sign in with the exact same name again.",
                            );
                            if ui.button("SIGN IN").clicked() {
                                next_app_state = Some(AppState::SignIn);
                            }
                        }
                    }
                    if let Some(state) = next_app_state {
                        self.state = state;
                    }
                }
                PollState::NotFound { key } => {
                    ui.label(format!("No poll with ID #{key} was found 😥"));
                }
            });
            if let Some(state) = next_poll_state {
                self.poll_state = state;
            }
        });
    }
}

pub trait Pollable
where
    Self: Future + Sized + Unpin,
{
    fn poll(&mut self) -> Option<<Self as Future>::Output> {
        future::block_on(future::poll_once(&mut *self))
    }
}

impl<T> Pollable for T where T: Future + Sized + Unpin {}
