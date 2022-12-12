use crate::misc::{
    console_log, get_window, listen_in_window, log, AtomicBoolExt, Submitter, UrlExt,
};
use crate::new_poll::NewPoll;
use crate::participation::ParticipationState;
use crate::retrieve::RetrievingState;
use crate::time::Instant;
use areyougoing_shared::{
    ConditionDescription, ConditionState, Form, Poll, PollResult, ProgressReportResult, Question,
};
use derivative::Derivative;
use egui::Color32;
use egui::{panel::TopBottomSide, Align, CentralPanel, Layout, RichText, TopBottomPanel};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

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

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub enum PollState {
    None,
    NewPoll {
        state: NewPoll,
        poll: Poll,
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
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        last_fetch: Option<Instant>,
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        poll_progress_fetch: Option<Submitter<u64, ProgressReportResult>>,
        stale: bool,
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
            (PollState::Found { key, .. }, Some(url_key)) if *key != url_key => {
                app.poll_state = PollState::Retrieving {
                    key: url_key,
                    state: RetrievingState::None,
                };
            }
            (PollState::Found { .. }, None) => {
                app.poll_state = PollState::NewPoll {
                    state: NewPoll::Creating {
                        ui_data: Default::default(),
                    },
                    poll: Default::default(),
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
        {
            let mut new_state = None;

            if let ParticipationState::Submitting { ref response, .. } = app.participation_state {
                new_state = Some(ParticipationState::SignedIn {
                    user: response.user.clone(),
                    question_responses: response.responses.clone(),
                });
            }
            if let Some(state) = new_state {
                app.participation_state = state;
            }
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

        let mut top_panel =
            TopBottomPanel::new(TopBottomSide::Top, "top_panel").show_separator_line(false);
        if let Some(height) = self.top_panel_inner_height {
            top_panel = top_panel.exact_height(height);
        }
        top_panel.show(ctx, |ui| {
            ui.columns(3, |columns| {
                let response = columns[0].with_layout(Layout::left_to_right(Align::Min), |ui| {
                    let create_poll_text = if let PollState::NewPoll { .. } = &self.poll_state {
                        "Clear Poll"
                    } else {
                        "Create Poll"
                    };
                    if ui.small_button(create_poll_text).clicked() {
                        next_poll_state = Some(PollState::NewPoll {
                            state: NewPoll::Creating {
                                ui_data: Default::default(),
                            },
                            poll: Default::default(),
                        });
                    }
                });
                self.top_panel_inner_height = Some(response.response.rect.height());
                if let ParticipationState::SignedIn {
                    user,
                    question_responses: _,
                } = &self.participation_state
                {
                    columns[1].with_layout(
                        Layout::top_down(Align::Min).with_cross_align(Align::Center),
                        |ui| {
                            ui.label(RichText::new(format!("Welcome, {user}!")).strong());
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
                    next_poll_state = Some(PollState::NewPoll {
                        state: NewPoll::Creating {
                            ui_data: Default::default(),
                        },
                        poll: Default::default(),
                    });
                }
                PollState::NewPoll { poll, state } => {
                    state.process(ui, poll, &self.original_url);
                }
                PollState::Retrieving { key, ref mut state } => {
                    ui.label(format!("Retreiving Poll #{key}"));
                    state.process(&mut next_poll_state, *key);
                    // Make sure the UI keeps updating in order to keep polling the fetch process
                    ui.ctx().request_repaint_after(Duration::from_millis(100));
                }
                PollState::Found {
                    key,
                    poll,
                    poll_progress_fetch,
                    last_fetch,
                    ref mut stale,
                } => {
                    ui.heading(format!("{} (#{key})", poll.title));

                    ui.label(&poll.description);

                    for PollResult {
                        description,
                        result,
                        progress,
                    } in poll.results.iter()
                    {
                        // let defaults = ("", ui.ctx().style().visuals.text_color(), "");
                        let (description_text, state_result, color) = match description {
                            ConditionDescription::AtLeast {
                                minimum,
                                question_index,
                                choice_index,
                            } => {
                                let Question { prompt, form } =
                                    &poll.questions[*question_index as usize];
                                let choice = match form {
                                    Form::ChooseOneorNone { options } => {
                                        &options[*choice_index as usize]
                                    }
                                };
                                let (state_text, color) = match progress {
                                    ConditionState::MetOrNotMet(met) => (
                                        (if *met { "☑" } else { "☐" }).to_string(),
                                        if *met { Color32::GREEN } else { Color32::RED },
                                    ),
                                    ConditionState::Progress(progress) => (
                                        format!("{progress}/{minimum}"),
                                        if progress >= minimum {
                                            Color32::GREEN
                                        } else {
                                            Color32::RED
                                        },
                                    ),
                                };
                                let desc = format!("≥{minimum} of \"{choice}\" to \"{prompt}\"",);
                                (desc, state_text, color)
                            }
                        };
                        let mut output = format!("{state_result}: {description_text}");
                        if let Some(result) = result {
                            output = format!("{output} ➡ \"{result}\"");
                        }
                        ui.add_enabled_ui(!*stale, |ui| {
                            ui.colored_label(color, output);
                        });
                    }

                    let mut fetch_complete = false;
                    if let Some(fetch) = poll_progress_fetch {
                        if let Some(progress) = fetch.poll() {
                            match progress {
                                ProgressReportResult::Success { progress } => {
                                    for (
                                        PollResult {
                                            progress: condition_state,
                                            ..
                                        },
                                        new_condition_state,
                                    ) in poll.results.iter_mut().zip(progress.condition_states)
                                    {
                                        *condition_state = new_condition_state;
                                    }
                                    *stale = false;
                                }
                                ProgressReportResult::Error => {}
                            }
                            fetch_complete = true;
                        }
                    } else {
                        if *stale
                            || last_fetch.is_none()
                            || last_fetch.unwrap().elapsed() > Duration::from_secs_f32(1.0)
                        {
                            *poll_progress_fetch = Some(Submitter::new("progress", *key));
                            *last_fetch = Some(Instant::now());
                        }
                    }
                    if fetch_complete {
                        *poll_progress_fetch = None;
                    }
                    ui.ctx().request_repaint_after(Duration::from_millis(200));
                    self.participation_state
                        .process(ui, &mut self.sign_in_data, *key, poll, stale);
                }
                PollState::NotFound { key } => {
                    ui.label(format!("No poll with ID #{key} was found 😥"));
                }
            });
            if let Some(state) = next_poll_state {
                {
                    use PollState::*;
                    match &state {
                        NewPoll { .. } => {
                            self.original_url.with_path("").push_to_window();
                        }
                        Found { .. } => match &mut self.participation_state {
                            ParticipationState::SignedIn {
                                question_responses: ref mut responses,
                                ..
                            } => {
                                // Temporary for debugging, with changing polls as we go
                                *responses = Default::default();
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
                self.poll_state = state;
            }
        });
    }
}
