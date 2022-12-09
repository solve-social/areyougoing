use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::misc::{self, get_window, listen_in_window, AtomicBoolExt, UrlExt};
use crate::participation::ParticipationState;
use crate::retrieve::RetrievingState;
use areyougoing_shared::{Form, Poll, Question};
use derivative::Derivative;
use egui::panel::TopBottomSide;
use egui::{Align, CentralPanel, Layout, ScrollArea};
use egui::{TextEdit, TopBottomPanel};
use misc::{console_log, log};
use serde::{Deserialize, Serialize};
use url::Url;
use wasm_bindgen_futures::JsFuture;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    participation_state: ParticipationState,
    poll_state: PollState,
    sign_in_data: SignInData,
    top_panel_inner_height: Option<f32>,
    #[serde(skip)]
    original_url: Option<Url>,
    #[serde(skip)]
    need_reload: Arc<AtomicBool>,
}

#[derive(Deserialize, Serialize)]
pub struct SignInData {
    pub user_entry: String,
    pub old_names: Vec<String>,
}

#[derive(Debug)]
pub enum SubmittingState {
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
#[derive(Deserialize, Serialize, Debug)]
pub enum PollState {
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

impl Default for App {
    fn default() -> Self {
        Self {
            participation_state: ParticipationState::SignIn,
            poll_state: PollState::None,
            sign_in_data: SignInData {
                user_entry: "".to_string(),
                old_names: vec!["Sandra", "Peter", "Bob"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            },
            top_panel_inner_height: None,
            original_url: None,
            need_reload: Default::default(),
        }
    }
}

impl App {
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

        {
            let clone = app.need_reload.clone();
            listen_in_window("popstate", move |_event| {
                clone.set(true);
            });
        }

        let url_key = {
            let mut url_key = None;
            let window = web_sys::window().expect("no global `window` exists");
            let url_string = window.location().href().unwrap();
            if let Ok(url) = Url::parse(&url_string) {
                app.original_url = Some(url.clone());
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

        match (&mut app.poll_state, url_key) {
            (PollState::Found { key, poll: _ }, Some(url_key)) if *key != url_key => {
                app.poll_state = PollState::Retrieving {
                    key: url_key,
                    state: RetrievingState::None,
                };
            }
            (PollState::Found { key: _, poll: _ }, None) => {
                app.poll_state = PollState::Creating {
                    new_poll: Poll::default(),
                };
            }
            (_, Some(url_key)) => {
                app.poll_state = PollState::Retrieving {
                    key: url_key,
                    state: RetrievingState::None,
                };
            }
            _ => {}
        }
        console_log!("Initial PollState: {:?}", app.poll_state);

        app
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.need_reload.get() {
            get_window().location().reload().expect("Failed to reload");
        }
        let mut next_poll_state = None;

        let mut top_panel = TopBottomPanel::new(TopBottomSide::Top, "top_panel");
        if let Some(height) = self.top_panel_inner_height {
            top_panel = top_panel.height_range((height - 1.0)..=(height + 1.0));
        }
        top_panel.show(ctx, |ui| {
            ui.columns(3, |columns| {
                let response = columns[0].with_layout(Layout::left_to_right(Align::Min), |ui| {
                    let create_poll_text =
                        if let PollState::Creating { new_poll: _ } = &self.poll_state {
                            "Clear Poll"
                        } else {
                            "Create Poll"
                        };
                    if ui.small_button(create_poll_text).clicked() {
                        next_poll_state = Some(PollState::Creating {
                            new_poll: Poll::default(),
                        })
                    }
                });
                self.top_panel_inner_height = Some(response.response.rect.height());
                if let ParticipationState::SignedIn { user, responses: _ } =
                    &self.participation_state
                {
                    columns[1].with_layout(
                        Layout::top_down(Align::Min).with_cross_align(Align::Center),
                        |ui| {
                            ui.label(format!("Welcome, {user}!"));
                        },
                    );
                    columns[2].with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if ui.small_button("Sign Out").clicked() {
                            self.participation_state = ParticipationState::SignIn;
                        }
                    });
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
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
                        ui.add(TextEdit::singleline(&mut new_poll.title).hint_text("Title"));
                        ui.add(
                            TextEdit::multiline(&mut new_poll.description)
                                .hint_text("Description")
                                .desired_rows(1),
                        );

                        let mut new_question_index = None;
                        if ui.small_button("Add Question").clicked() {
                            new_question_index = Some(0);
                        }
                        for (question_i, question) in new_poll.questions.iter_mut().enumerate() {
                            ui.group(|ui| {
                                ui.label(format!("Question {}", question_i + 1));
                                ui.add(
                                    TextEdit::multiline(&mut question.prompt)
                                        .desired_rows(1)
                                        .hint_text("Prompt"),
                                );

                                match &mut question.form {
                                    Form::ChooseOne { ref mut options } => {
                                        let mut new_option_index = None;
                                        if ui.small_button("Add Option").clicked() {
                                            new_option_index = Some(0);
                                        }
                                        for (option_i, option) in options.iter_mut().enumerate() {
                                            ui.add(
                                                TextEdit::singleline(option)
                                                    .hint_text(format!("Option {}", option_i + 1)),
                                            );
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
                            if ui.small_button("Add Question").clicked() {
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
                    let mut link = self.original_url.as_ref().unwrap().clone();
                    link.set_path(&format!("{key}"));
                    ui.label(format!("Share it with this link: {link}"));
                }
                PollState::Retrieving { key, ref mut state } => {
                    ui.label(format!("Retreiving Poll #{key}"));
                    state.process(&mut next_poll_state, *key);
                }
                PollState::Found { key, poll } => {
                    self.participation_state
                        .process(ui, &mut self.sign_in_data, *key, poll);
                }
                PollState::NotFound { key } => {
                    ui.label(format!("No poll with ID #{key} was found 😥"));
                }
            });
            if let Some(state) = next_poll_state {
                {
                    use PollState::*;
                    match &state {
                        Creating { new_poll: _ } => {
                            self.original_url.with_path("").push_to_window();
                        }
                        _ => {}
                    }
                }
                self.poll_state = state;
            }
        });
    }
}
