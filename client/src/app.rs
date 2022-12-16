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
        results_ui_state: ResultsUiState,
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
#[derive(Deserialize, Serialize, Debug, Default, PartialEq)]
pub struct ResultsUiState {
    metric_rects: Vec<Rect>,
    result_rects: Vec<Rect>,
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

        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| match &mut self.poll_state {
                PollState::None => {
                    next_poll_state = Some(PollState::NewPoll {
                        state: NewPoll::Creating {
                            ui_data: Default::default(),
                            show_conditions: false,
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
                    ref mut results_ui_state,
                } => {
                    ui.heading(format!("{} (#{key})", poll.title));

                    ui.label(&poll.description);

                    let processed_results = poll
                        .results
                        .iter()
                        .map(
                            |PollResult {
                                 description,
                                 result,
                                 progress,
                             }| {
                                match description {
                                    ConditionDescription::AtLeast {
                                        minimum,
                                        question_index,
                                        choice_index,
                                    } => {
                                        let Question { prompt, form } =
                                            &poll.questions[*question_index];
                                        let choice = match form {
                                            Form::ChooseOneorNone { options } => {
                                                &options[*choice_index as usize]
                                            }
                                        };
                                        let (progress, color) = match progress {
                                            ConditionState::MetOrNotMet(met) => (
                                                None,
                                                if *met {
                                                    Color32::DARK_GREEN
                                                } else {
                                                    Color32::DARK_RED
                                                },
                                            ),
                                            ConditionState::Progress(progress) => (
                                                Some(progress.to_string()),
                                                if progress >= minimum {
                                                    Color32::DARK_GREEN
                                                } else {
                                                    Color32::DARK_RED
                                                },
                                            ),
                                        };
                                        let desc = format!("≥{minimum}");
                                        (
                                            desc,
                                            progress,
                                            color,
                                            format!("\"{choice}\" to \"{prompt}\""),
                                            result,
                                        )
                                    }
                                }
                            },
                        )
                        .collect::<Vec<_>>();

                    let ui_width = ui.available_width();
                    const MIDDLE_CHANNEL_WIDTH: f32 = 35.0;
                    const SIDE_MARGIN: f32 = 1.0;
                    let available_width_each_side = (ui_width - MIDDLE_CHANNEL_WIDTH) / 2.0;
                    let heading_top = ui.cursor().top();

                    let left_rect = Rect {
                        min: Pos2 {
                            x: SIDE_MARGIN,
                            y: heading_top,
                        },
                        max: Pos2 {
                            x: available_width_each_side,
                            y: f32::MAX,
                        },
                    };
                    let heading_rect =
                        if let Some(top_metric_rect) = results_ui_state.metric_rects.first() {
                            Rect {
                                min: Pos2 {
                                    x: top_metric_rect.left(),
                                    y: left_rect.top(),
                                },
                                max: Pos2 {
                                    x: top_metric_rect.right(),
                                    y: f32::INFINITY,
                                },
                            }
                        } else {
                            left_rect
                        };
                    ui.allocate_ui_at_rect(heading_rect, |ui| {
                        ui.with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.label(RichText::new("Metrics").underline().strong());
                        });
                    });
                    let left_rect = Rect {
                        min: Pos2 {
                            x: SIDE_MARGIN,
                            y: ui.cursor().top(),
                        },
                        max: Pos2 {
                            x: available_width_each_side,
                            y: f32::MAX,
                        },
                    };
                    results_ui_state.metric_rects.clear();
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        for (desc, state_text, color, metric, result) in processed_results.iter() {
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                let mut style = (*ui.ctx().style()).clone();
                                style.spacing.item_spacing.x = 3.0;
                                ui.ctx().set_style(style);

                                let progress_rect = if let Some(progress) = state_text {
                                    let where_to_put_background = ui.painter().add(Shape::Noop);
                                    let response = ui.label(progress); // Change this to collapsing
                                    let progress_rect = response.rect.expand2(Vec2::new(1.5, 1.));
                                    ui.painter().set(
                                        where_to_put_background,
                                        RectShape {
                                            rounding: ui.style().visuals.widgets.hovered.rounding,
                                            fill: ui.style().visuals.widgets.active.bg_fill,
                                            stroke: ui.style().visuals.widgets.hovered.bg_stroke,
                                            rect: progress_rect,
                                        },
                                    );
                                    Some(progress_rect)
                                } else {
                                    None
                                };

                                ui.label(":");

                                let where_to_put_background = ui.painter().add(Shape::Noop);
                                let response = ui.label(metric); // Change this to collapsing
                                let rect = response.rect.expand2(Vec2::new(1.5, 1.));
                                ui.painter().set(
                                    where_to_put_background,
                                    RectShape {
                                        rounding: ui.style().visuals.widgets.hovered.rounding,
                                        fill: ui.style().visuals.widgets.active.bg_fill,
                                        stroke: ui.style().visuals.widgets.hovered.bg_stroke,
                                        rect,
                                    },
                                );
                                let mut total_rect = rect;
                                if let Some(progress_rect) = progress_rect {
                                    total_rect = total_rect.union(progress_rect);
                                }
                                results_ui_state.metric_rects.push(total_rect);
                            });
                        }
                    });

                    ///////////////////////////

                    let right_rect = Rect {
                        min: Pos2 {
                            x: ui_width - available_width_each_side,
                            y: heading_top,
                        },
                        max: Pos2 {
                            x: ui_width - SIDE_MARGIN,
                            y: f32::MAX,
                        },
                    };
                    let heading_rect =
                        if let Some(top_result_rect) = results_ui_state.result_rects.first() {
                            Rect {
                                min: Pos2 {
                                    x: top_result_rect.left(),
                                    y: right_rect.top(),
                                },
                                max: Pos2 {
                                    x: top_result_rect.right(),
                                    y: f32::INFINITY,
                                },
                            }
                        } else {
                            right_rect
                        };
                    ui.allocate_ui_at_rect(heading_rect, |ui| {
                        ui.with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.label(RichText::new("Results").underline().strong());
                        });
                    });
                    let right_rect = Rect {
                        min: Pos2 {
                            x: ui_width - available_width_each_side,
                            y: ui.cursor().top(),
                        },
                        max: Pos2 {
                            x: ui_width - SIDE_MARGIN,
                            y: f32::MAX,
                        },
                    };

                    results_ui_state.result_rects.clear();
                    ui.allocate_ui_at_rect(right_rect, |ui| {
                        for (desc, state_text, color, metric, result) in processed_results.iter() {
                            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                                let mut style = (*ui.ctx().style()).clone();
                                style.spacing.item_spacing.x = 3.0;
                                ui.ctx().set_style(style);

                                let where_to_put_background = ui.painter().add(Shape::Noop);
                                let response = ui.label(RichText::new(desc).strong()); // Change this to collapsing
                                let progress_rect = response.rect.expand2(Vec2::new(1.5, 1.));
                                ui.painter().set(
                                    where_to_put_background,
                                    RectShape {
                                        rounding: ui.style().visuals.widgets.hovered.rounding,
                                        fill: *color,
                                        stroke: ui.style().visuals.widgets.hovered.bg_stroke,
                                        rect: progress_rect,
                                    },
                                );

                                ui.label(":");

                                let where_to_put_background = ui.painter().add(Shape::Noop);
                                let response = ui.label(RichText::new(*result).strong()); // Change this to collapsing
                                let rect = response.rect.expand2(Vec2::new(1.5, 1.));
                                ui.painter().set(
                                    where_to_put_background,
                                    RectShape {
                                        rounding: ui.style().visuals.widgets.hovered.rounding,
                                        fill: *color,
                                        stroke: ui.style().visuals.widgets.hovered.bg_stroke,
                                        rect,
                                    },
                                );
                                let total_rect = rect.union(progress_rect);
                                results_ui_state.result_rects.push(total_rect);
                            });
                        }
                    });

                    /////////////////////////////////////////////////////////////

                    for (left_rect, (right_rect, (desc, state_text, color, metric, result))) in
                        results_ui_state.metric_rects.iter().zip(
                            results_ui_state
                                .result_rects
                                .iter()
                                .zip(processed_results.iter()),
                        )
                    {
                        const MARGIN: f32 = 3.0;
                        let mut left = left_rect.right_center();
                        let mut right = right_rect.left_center();
                        left.x += MARGIN;
                        right.x -= MARGIN;
                        let vector = right - left;
                        ui.painter().arrow(left, vector, Stroke::new(1.5, *color));
                    }

                    ////////////////////////////////////////////////////////////

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
                    } else if *stale
                        || last_fetch.is_none()
                        || last_fetch.unwrap().elapsed() > Duration::from_secs_f32(1.0)
                    {
                        *poll_progress_fetch = Some(Submitter::new("progress", *key));
                        *last_fetch = Some(Instant::now());
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
                            self.original_url.with_query(Option::None).push_to_window();
                        }
                        Found { .. } => {
                            if let ParticipationState::SignedIn {
                                question_responses: ref mut responses,
                                ..
                            } = &mut self.participation_state
                            {
                                // Temporary for debugging, with changing polls as we go
                                *responses = Default::default();
                            }
                        }
                        _ => {}
                    }
                }
                self.poll_state = state;
            }
        });
    }
}
