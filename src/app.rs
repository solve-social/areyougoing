use std::time::Duration;

use egui::{Align, Button, CentralPanel, ComboBox, Grid, Layout, ScrollArea};
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use url::Url;

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

#[derive(Deserialize, Serialize, PartialEq)]
enum PollState {
    None,
    Creating { new_poll: Poll },
    Retrieving { key: u64 },
    Found { poll: Poll },
    NotFound,
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
        responses: Vec<Response>,
    },
    SignIn,
    Submitting {
        #[serde(skip)]
        progress: Option<Instant>,
    },
    SubmitConfirmation,
}

#[derive(Deserialize, Serialize, PartialEq)]
pub struct Question {
    prompt: String,
    form: Form,
}

#[derive(Deserialize, Serialize, Sequence, PartialEq)]
enum Response {
    ChooseOne { choice: Option<u8> },
}

#[derive(Deserialize, Serialize, PartialEq)]
enum Form {
    ChooseOne { options: Vec<String> },
}

#[derive(Deserialize, Serialize, Default, PartialEq)]
struct Poll {
    title: String,
    description: String,
    questions: Vec<Question>,
}

impl Poll {
    fn init_responses(&self) -> Vec<Response> {
        self.questions
            .iter()
            .map(|q| match q.form {
                Form::ChooseOne { options: _ } => Response::ChooseOne { choice: None },
            })
            .collect::<Vec<_>>()
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: AppState::SignIn,
            poll_state: PollState::Found {
                poll: Poll {
                    title: "Party!".to_string(),
                    description: "Saturday, 3pm, Mike's House".to_string(),
                    questions: vec![Question {
                        prompt: "Would you go?".to_string(),
                        form: Form::ChooseOne {
                            options: vec!["YES".to_string(), "NO".to_string()],
                        },
                    }],
                },
            },
            user_entry: "".to_string(),
            old_names: vec!["Sandra", "Peter", "Bob"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let poll_key = {
            let window = web_sys::window().expect("no global `window` exists");
            let url_string = window.location().href().unwrap();
            let mut url_key = None;
            if let Ok(url) = Url::parse(&url_string) {
                if let Some(segments) = &mut url.path_segments() {
                    if let Some(first) = &segments.next() {
                        if let Ok(key) = first.parse::<u64>() {
                            url_key = Some(key);
                        }
                    }
                }
            }
            url_key
        };

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app: App = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };
        if PollState::None == app.poll_state {
            if let Some(key) = poll_key {
                app.poll_state = PollState::Retrieving { key };
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
                    Grid::new("new_poll_form").show(ui, |ui| {
                        ui.label("Title:");
                        ui.text_edit_singleline(&mut new_poll.title);
                        ui.end_row();

                        ui.label("Description:");
                        ui.text_edit_multiline(&mut new_poll.description);
                        ui.end_row();
                    });
                    let mut new_question_index = None;
                    if ui.button("Add Question").clicked() {
                        new_question_index = Some(0);
                    }
                    for (question_i, question) in new_poll.questions.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.label("Prompt:");
                            ui.text_edit_multiline(&mut question.prompt);

                            match &mut question.form {
                                Form::ChooseOne { ref mut options } => {
                                    let mut new_option_index = None;
                                    if ui.button("Add Option").clicked() {
                                        new_option_index = Some(0);
                                    }
                                    for (option_i, option) in options.iter_mut().enumerate() {
                                        ui.text_edit_singleline(option);
                                        if ui.small_button("Add Option").clicked() {
                                            new_option_index = Some(option_i);
                                        }
                                    }
                                    if let Some(index) = new_option_index {
                                        options.insert(index, "".to_string())
                                    }
                                }
                            }
                        });
                        if ui.button("Add Question").clicked() {
                            new_question_index = Some(question_i);
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
                }
                PollState::Retrieving { key } => {
                    ui.label(format!("Retreiving Poll #{key}"));
                }
                PollState::Found { poll } => {
                    ui.heading(&poll.title);

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
                                            Response::ChooseOne { choice },
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
                PollState::NotFound => {}
            });
            if let Some(state) = next_poll_state {
                self.poll_state = state;
            }
        });
    }
}
