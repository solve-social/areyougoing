use std::time::Duration;

use areyougoing_shared::{
    ConditionDescription, ConditionState, Form, Poll, PollResult, ProgressReportResult, Question,
};
use derivative::Derivative;
use eframe::epaint::RectShape;
use egui::{Align, Color32, Layout, Pos2, Rect, RichText, Shape, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    app::SignInData,
    misc::{Submitter, UrlExt},
    new_poll::NewPoll,
    participation::ParticipationState,
    retrieve::RetrievingState,
    time::Instant,
};

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

#[derive(Deserialize, Serialize, Debug, Default, PartialEq)]
pub struct ResultsUiState {
    metric_rects: Vec<Rect>,
    result_rects: Vec<Rect>,
}

impl PollState {
    pub fn process(
        &mut self,
        ui: &mut Ui,
        next_poll_state: &mut Option<PollState>,
        original_url: &Option<Url>,
        participation_state: &mut ParticipationState,
        sign_in_data: &mut SignInData,
    ) {
        ui.vertical_centered(|ui| match self {
            PollState::None => {
                *next_poll_state = Some(PollState::NewPoll {
                    state: NewPoll::Creating {
                        ui_data: Default::default(),
                        show_conditions: false,
                    },
                    poll: Default::default(),
                });
            }
            PollState::NewPoll { poll, state } => {
                state.process(ui, poll, original_url);
            }
            PollState::Retrieving { key, ref mut state } => {
                ui.label(format!("Retreiving Poll #{key}"));
                state.process(next_poll_state, *key);
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
                const MIDDLE_CHANNEL_WIDTH: f32 = 65.0;
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
                    for (_desc, state_text, _color, metric, _result) in processed_results.iter() {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            let mut style = (*ui.ctx().style()).clone();
                            style.spacing.item_spacing.x = 3.5;
                            ui.ctx().set_style(style);

                            let progress_rect = if let Some(progress) = state_text {
                                let where_to_put_background = ui.painter().add(Shape::Noop);
                                let response = ui.label(progress); // Change this to collapsing
                                let progress_rect = response.rect.expand2(Vec2::new(2.0, 1.));
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
                    for (desc, _state_text, color, _metric, result) in processed_results.iter() {
                        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                            let mut style = (*ui.ctx().style()).clone();
                            style.spacing.item_spacing.x = 3.5;
                            ui.ctx().set_style(style);

                            let where_to_put_background = ui.painter().add(Shape::Noop);
                            let response = ui.label(RichText::new(desc).strong()); // Change this to collapsing
                            let progress_rect = response.rect.expand2(Vec2::new(3.0, 1.));
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
                            let rect = response.rect.expand2(Vec2::new(3.0, 1.));
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

                for (left_rect, (right_rect, (_desc, _state_text, color, _metric, _result))) in
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
                    ui.painter().arrow(left, vector, Stroke::new(3.0, *color));
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
                participation_state.process(ui, sign_in_data, *key, poll, stale);
            }
            PollState::NotFound { key } => {
                ui.label(format!("No poll with ID #{key} was found 😥"));
            }
        });
        if let Some(state) = next_poll_state.take() {
            {
                use PollState::*;
                match &state {
                    NewPoll { .. } => {
                        original_url.with_query(Option::None).push_to_window();
                    }
                    Found { .. } => {
                        if let ParticipationState::SignedIn {
                            question_responses: ref mut responses,
                            ..
                        } = participation_state
                        {
                            // Temporary for debugging, with changing polls as we go
                            *responses = Default::default();
                        }
                    }
                    _ => {}
                }
            }
            *self = state;
        }
    }
}
