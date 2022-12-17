use std::time::Duration;

use areyougoing_shared::{
    ConditionDescription, ConditionState, Form, Poll, PollResult, ProgressReportResult, Question,
};
use derivative::Derivative;

use egui::{
    vec2, Align, Color32, Frame, Label, Layout, Rect, RichText, Stroke, TextStyle, Ui, Vec2,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    app::SignInData,
    misc::{Submitter, UiExt, UrlExt},
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
    progress_rects: Vec<Rect>,
    condition_rects: Vec<Rect>,
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
                let left_right_col_width = available_width_each_side - ui.spacing().item_spacing.x;

                let results_frame = Frame::none()
                    .inner_margin(Vec2::new(2.0, 1.))
                    .stroke(ui.style().visuals.widgets.hovered.bg_stroke)
                    .rounding(3.)
                    .fill(ui.style().visuals.widgets.active.bg_fill);

                ui.separator();

                ui.unequal_columns(
                    &[
                        left_right_col_width,
                        MIDDLE_CHANNEL_WIDTH,
                        left_right_col_width,
                    ],
                    |columns| {
                        columns[0].with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.label(RichText::new("Metrics").underline().strong());
                        });
                        results_ui_state.progress_rects.clear();
                        for (i, (_desc, state_text, _color, metric, _result)) in
                            processed_results.iter().enumerate()
                        {
                            let mut size = columns[0].available_size();
                            size.y = 0.;
                            columns[0].allocate_ui_with_layout(
                                size,
                                Layout::right_to_left(Align::Center),
                                |ui| {
                                    let mut progress_rect = None;
                                    if let Some(progress) = state_text {
                                        if let Some(metric_rect) =
                                            results_ui_state.metric_rects.get(i)
                                        {
                                            let rect = ui
                                                .available_rect_before_wrap()
                                                .translate(vec2(
                                                    0.,
                                                    metric_rect.height() / 2.
                                                        - ui.text_style_height(&TextStyle::Body)
                                                            / 2.,
                                                ))
                                                .expand(results_frame.stroke.width);
                                            ui.allocate_ui_at_rect(rect, |ui| {
                                                let response = results_frame.show(ui, |ui| {
                                                    ui.label(progress);
                                                });
                                                progress_rect = Some(response.response.rect);
                                            });
                                        }
                                    }
                                    let metric_rect = results_frame
                                        .show(ui, |ui| ui.add(Label::new(metric).wrap(true)))
                                        .response
                                        .rect;
                                    results_ui_state.progress_rects.push(
                                        if let Some(progress_rect) = progress_rect {
                                            progress_rect
                                        } else {
                                            metric_rect
                                        },
                                    );
                                    if let Some(old_rect) = results_ui_state.metric_rects.get_mut(i)
                                    {
                                        *old_rect = metric_rect
                                    } else {
                                        results_ui_state.metric_rects.push(metric_rect);
                                    }
                                },
                            );
                        }

                        columns[2].with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.label(RichText::new("Results").underline().strong());
                        });
                        results_ui_state.condition_rects.clear();
                        for (i, (desc, _state_text, _color, _metric, _result)) in
                            processed_results.iter().enumerate()
                        {
                            let results_frame = results_frame.fill(*_color);
                            let mut size = columns[2].available_size();
                            size.y = 0.;
                            columns[2].allocate_ui_with_layout(
                                size,
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    if let Some(result) = results_ui_state.result_rects.get(i) {
                                        let rect = ui
                                            .available_rect_before_wrap()
                                            .translate(vec2(
                                                0.,
                                                result.height() / 2.
                                                    - ui.text_style_height(&TextStyle::Body) / 2.,
                                            ))
                                            .expand(results_frame.stroke.width);
                                        ui.allocate_ui_at_rect(rect, |ui| {
                                            let response = results_frame.show(ui, |ui| {
                                                ui.label(desc);
                                            });
                                            results_ui_state
                                                .condition_rects
                                                .push(response.response.rect);
                                        });
                                    }

                                    let rect = results_frame
                                        .show(ui, |ui| ui.add(Label::new(*_result).wrap(true)))
                                        .response
                                        .rect;
                                    if let Some(old_rect) = results_ui_state.result_rects.get_mut(i)
                                    {
                                        *old_rect = rect
                                    } else {
                                        results_ui_state.result_rects.push(rect);
                                    }
                                },
                            );
                        }
                    },
                );

                for (left_rect, (right_rect, (_desc, _state_text, color, _metric, _result))) in
                    results_ui_state.progress_rects.iter().zip(
                        results_ui_state
                            .condition_rects
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

                ui.separator();

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
