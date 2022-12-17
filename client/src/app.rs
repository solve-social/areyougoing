use crate::misc::{
    console_log, get_window, listen_in_window, log, AtomicBoolExt, Submitter, UrlExt,
};
use crate::new_poll::NewPoll;
use crate::participation::ParticipationState;
use crate::poll::PollState;
use crate::retrieve::RetrievingState;
use crate::time::Instant;
use areyougoing_shared::{
    ConditionDescription, ConditionState, Form, Poll, PollResult, ProgressReportResult, Question,
};
use derivative::Derivative;
use eframe::epaint::RectShape;
use egui::{panel::TopBottomSide, Align, CentralPanel, Layout, RichText, TopBottomPanel};
use egui::{Color32, Direction, Grid, Pos2, Rect, Shape, Stroke, Vec2};
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
                console_log!("URL: {:?}", app.original_url);
                for (query_key, query_value) in url.query_pairs() {
                    console_log!("query: {:?}", (&query_key, &query_value));
                    if query_key == "poll_key" {
                        if let Ok(key) = query_value.parse::<u64>() {
                            url_key = Some(key);
                            console_log!("url_key: {:?}", url_key);
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
                        show_conditions: false,
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

        // TopBottomPanel::bottom("bottom").show(ctx, |ui| ui.label(SERVER_URL));

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
                                show_conditions: false,
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

        CentralPanel::default().show(ctx, |ui| {});
    }
}
