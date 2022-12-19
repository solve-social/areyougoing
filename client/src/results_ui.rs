use crate::{
    misc::{Submitter, UiExt},
    time::Instant,
};
use areyougoing_shared::{
    ConditionDescription, ConditionState, Form, Poll, PollResult, ProgressReportResult, Question,
};
use derivative::Derivative;
use egui::{
    pos2, vec2, Align, Color32, Frame, Label, Layout, Rect, RichText, Stroke, TextStyle, Ui,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub struct ResultsUi {
    #[serde(skip)]
    #[derivative(PartialEq = "ignore")]
    pub last_fetch: Option<Instant>,
    #[serde(skip)]
    #[derivative(PartialEq = "ignore")]
    pub poll_progress_fetch: Option<Submitter<u64, ProgressReportResult>>,
    pub stale: bool,
    pub ui_state: ResultsUiState,
}

impl Default for ResultsUi {
    fn default() -> Self {
        Self {
            poll_progress_fetch: None,
            last_fetch: None,
            stale: true,
            ui_state: Default::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq)]
pub struct ResultsUiState {
    metric_rects: Vec<Rect>,
    result_rects: Vec<Rect>,
    progress_rects: Vec<Rect>,
    condition_rects: Vec<Rect>,
}

impl ResultsUi {
    pub fn process(&mut self, ui: &mut Ui, poll: &mut Poll, key: u64) {
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
                            let Question { prompt, form } = &poll.questions[*question_index];
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
        const MIDDLE_CHANNEL_WIDTH: f32 = 45.0;
        let available_width_each_side = (ui_width - MIDDLE_CHANNEL_WIDTH) / 2.0;
        let left_right_col_width = available_width_each_side - ui.spacing().item_spacing.x;

        let results_frame = Frame::none()
            .inner_margin(vec2(2.0, 1.))
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
                let heading_rect = if let Some(rect) = self.ui_state.metric_rects.first() {
                    Rect {
                        min: pos2(rect.left(), columns[0].available_rect_before_wrap().top()),
                        max: pos2(rect.right(), f32::INFINITY),
                    }
                } else {
                    columns[0].available_rect_before_wrap()
                };
                columns[0].allocate_ui_at_rect(heading_rect, |ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label(RichText::new("Metrics").underline().strong());
                    });
                });

                self.ui_state.progress_rects.clear();
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
                                if let Some(metric_rect) = self.ui_state.metric_rects.get(i) {
                                    let rect = ui
                                        .available_rect_before_wrap()
                                        .translate(vec2(
                                            0.,
                                            metric_rect.height() / 2.
                                                - ui.text_style_height(&TextStyle::Body) / 2.,
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
                            self.ui_state.progress_rects.push(
                                if let Some(progress_rect) = progress_rect {
                                    progress_rect
                                } else {
                                    metric_rect
                                },
                            );
                            if let Some(old_rect) = self.ui_state.metric_rects.get_mut(i) {
                                *old_rect = metric_rect
                            } else {
                                self.ui_state.metric_rects.push(metric_rect);
                            }
                        },
                    );
                }

                let heading_rect = if let Some(rect) = self.ui_state.result_rects.first() {
                    Rect {
                        min: pos2(rect.left(), columns[2].available_rect_before_wrap().top()),
                        max: pos2(rect.right(), f32::INFINITY),
                    }
                } else {
                    columns[2].available_rect_before_wrap()
                };
                columns[2].allocate_ui_at_rect(heading_rect, |ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.label(RichText::new("Results").underline().strong());
                    });
                });

                self.ui_state.condition_rects.clear();
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
                            if let Some(result) = self.ui_state.result_rects.get(i) {
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
                                    self.ui_state.condition_rects.push(response.response.rect);
                                });
                            }

                            let rect = results_frame
                                .show(ui, |ui| ui.add(Label::new(*_result).wrap(true)))
                                .response
                                .rect;
                            if let Some(old_rect) = self.ui_state.result_rects.get_mut(i) {
                                *old_rect = rect
                            } else {
                                self.ui_state.result_rects.push(rect);
                            }
                        },
                    );
                }
            },
        );

        for (left_rect, (right_rect, (_desc, _state_text, color, _metric, _result))) in
            self.ui_state.progress_rects.iter().zip(
                self.ui_state
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

        self.fetch(poll, key);
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }

    fn fetch(&mut self, poll: &mut Poll, key: u64) {
        let mut fetch_complete = false;
        if let Some(ref mut fetch) = self.poll_progress_fetch {
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
                        self.stale = false;
                    }
                    ProgressReportResult::Error => {}
                }
                fetch_complete = true;
            }
        } else if self.stale
            || self.last_fetch.is_none()
            || self.last_fetch.unwrap().elapsed() > Duration::from_secs_f32(1.0)
        {
            self.poll_progress_fetch = Some(Submitter::new("progress", key));
            self.last_fetch = Some(Instant::now());
        }
        if fetch_complete {
            self.poll_progress_fetch = None;
        }
    }
}
