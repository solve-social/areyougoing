use crate::misc::{ArrangeableList, Submitter, UiExt};
use areyougoing_shared::{
    CreatePollResult, Form, Metric, MetricTracker, Poll, PollResult2, Requirement,
};
use derivative::Derivative;
use egui::{
    pos2, vec2, Align, Button, ComboBox, FontId, Layout, Pos2, Rect, RichText, ScrollArea,
    TextEdit, Ui, Vec2,
};
use enum_iterator::{all, Sequence};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub enum NewPoll {
    Creating {
        ui_data: CreatingUiData,
        ui_tab: UiTab,
    },
    Submitting {
        poll: Poll,
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        state: Option<Submitter<Poll, CreatePollResult>>,
    },
    Submitted {
        key: u64,
        copied: bool,
    },
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq)]
pub struct CreatingUiData {
    fields_rect: Option<Rect>,
    question_group_rect: Option<Rect>,
    available_rect: Option<Rect>,
    group_border_thickness: Option<f32>,
    tabs_rect: Option<Rect>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Sequence)]
pub enum UiTab {
    Questions,
    Metrics,
    Results,
}

impl Default for UiTab {
    fn default() -> Self {
        Self::Questions
    }
}

impl NewPoll {
    pub fn process(&mut self, ui: &mut Ui, poll: &mut Poll, original_url: &Option<Url>) {
        let mut next_new_poll_state = None;
        match self {
            NewPoll::Creating {
                ref mut ui_data,
                ref mut ui_tab,
            } => {
                if let Some(rect) = ui_data.available_rect {
                    if rect != ui.available_rect_before_wrap() {
                        // Somehow the size of the window has changed, so reset/recalculate everything
                        *ui_data = Default::default();
                    }
                }
                ui_data.available_rect = Some(ui.available_rect_before_wrap());

                ui.heading("Create a new poll!");

                let tabs_rect = if let Some(rect) = ui_data.tabs_rect {
                    let left_margin = (ui.available_width() - rect.width()).max(0.) / 2.0;
                    Rect {
                        min: pos2(left_margin, ui.cursor().top()),
                        max: pos2(ui.available_width(), f32::INFINITY),
                    }
                } else {
                    ui.max_rect()
                };

                ui.allocate_ui_at_rect(tabs_rect, |ui| {
                    let response = ui.with_layout(
                        Layout::left_to_right(Align::Min).with_main_wrap(true),
                        |ui| {
                            for tab in all::<UiTab>() {
                                ui.add_enabled_ui(*ui_tab != tab, |ui| {
                                    let mut button = Button::new(
                                        RichText::new(
                                            format!("{tab:?}").split(':').last().unwrap(),
                                        )
                                        .font(FontId::proportional(17.)),
                                    );
                                    if *ui_tab == tab {
                                        button = button.fill(ui.style().visuals.selection.bg_fill);
                                    }
                                    if ui.add(button).clicked() {
                                        *ui_tab = tab;
                                    }
                                });
                            }
                        },
                    );
                    ui_data.tabs_rect =
                        Some(response.response.rect.shrink2(ui.spacing().item_spacing));
                });

                ui.separator();

                ScrollArea::vertical()
                    .id_source("create_poll_scroll")
                    .show(ui, |ui| {
                        match ui_tab {
                            UiTab::Questions => {
                                Self::show_main_form(ui, poll, ui_data);
                            }
                            UiTab::Metrics => {
                                Self::show_metrics_form(ui, poll, ui_data);
                            }
                            UiTab::Results => {
                                Self::show_results_form(ui, poll, ui_data);
                            }
                        }
                        ui.separator();
                        if ui.button("SUBMIT").clicked() {
                            next_new_poll_state = Some(NewPoll::Submitting {
                                poll: poll.clone(),
                                state: None,
                            });
                        }
                    });
                ui.ctx().request_repaint_after(Duration::from_millis(300));
            }
            NewPoll::Submitting {
                poll,
                ref mut state,
            } => {
                if let Some(submitter) = state {
                    if let Some(response) = submitter.poll() {
                        match response {
                            CreatePollResult::Success { key } => {
                                next_new_poll_state =
                                    Some(NewPoll::Submitted { key, copied: false });
                            }
                            CreatePollResult::Error => {}
                        }
                    }
                } else {
                    *state = Some(Submitter::new("new_poll", poll.clone()));
                }
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            NewPoll::Submitted { key, .. } => {
                ui.label("Your new poll has been created at:");
                let mut link = original_url.as_ref().unwrap().clone();

                link.set_path("");
                link.set_query(Some(&format!("poll_key={key}")));
                let link = format!("{link}");
                ui.hyperlink(&link);

                // Need to enable that one feature for clipboard access I think???
                // but its conflicting with the per crate compile targets I think
                // if ui.button("Copy Link to Clipboard").clicked() {
                //     ui.output().copied_text = link;
                //     *copied = true;
                // }
                // if *copied {
                //     ui.label("Copied!");
                // }
            }
        }
        if let Some(next_state) = next_new_poll_state {
            *self = next_state;
        }
    }

    fn show_main_form(ui: &mut Ui, poll: &mut Poll, ui_data: &mut CreatingUiData) {
        ui.add(TextEdit::singleline(&mut poll.title).hint_text("Title"));
        ui.add(
            TextEdit::multiline(&mut poll.description)
                .hint_text("Description (Optional)")
                .desired_rows(1),
        );

        ArrangeableList::new(&mut poll.questions, "Question")
            .min_items(1)
            .item_spacing(vec2(3., 1.))
            .add_button_is_at_bottom()
            .show(ui, |list_state, ui, question| {
                let response = ui.group(|ui| {
                    let label_response =
                        ui.label(format!("Question {}", list_state.current_index + 1));

                    if let Some(fields_rect) = ui_data.fields_rect {
                        let question_controls_rect = Rect {
                            min: Pos2 {
                                x: label_response.rect.right(),
                                y: label_response.rect.top(),
                            },
                            max: Pos2 {
                                x: fields_rect.right(),
                                y: label_response.rect.bottom(),
                            },
                        };
                        ui.allocate_ui_at_rect(question_controls_rect, |ui| {
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                list_state.show_controls(ui);
                            });
                        });
                    }
                    let response = ui.add(
                        TextEdit::multiline(&mut question.prompt)
                            .desired_rows(1)
                            .hint_text("Prompt"),
                    );
                    ui_data.fields_rect = Some(response.rect);
                    ui.separator();

                    match &mut question.form {
                        Form::ChooseOneorNone { ref mut options } => {
                            ArrangeableList::new(options, "Option")
                                .min_items(1)
                                .item_spacing(vec2(3., 1.))
                                .show(ui, |list_state, ui, option| {
                                    ui.allocate_ui(ui_data.fields_rect.unwrap().size(), |ui| {
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                list_state.show_controls(ui);
                                                ui.add(TextEdit::singleline(option).hint_text(
                                                    format!(
                                                        "Option {}",
                                                        list_state.current_index + 1
                                                    ),
                                                ));
                                            },
                                        );
                                    });
                                });
                        }
                    }
                });
                if list_state.current_index == 0 {
                    ui_data.question_group_rect = Some(response.response.rect);
                }
            });
    }

    fn show_metrics_form(ui: &mut Ui, poll: &mut Poll, ui_data: &mut CreatingUiData) {
        let mut new_index = None;
        let mut delete_i = None;
        let mut swap_indices = None;

        if ui.small_button("Add Metric").clicked() {
            new_index = Some(0);
        }

        let num_metrics = poll.metric_trackers.len();
        for (metric_i, metric_tracker) in poll.metric_trackers.iter_mut().enumerate() {
            let response = ui.group(|ui| {
                let label_response = ui.label(format!("Metric {}", metric_i + 1));
                if let Some(fields_rect) = ui_data.fields_rect {
                    let result_controls_rect = Rect {
                        min: Pos2 {
                            x: label_response.rect.right(),
                            y: label_response.rect.top(),
                        },
                        max: Pos2 {
                            x: fields_rect.right(),
                            y: label_response.rect.bottom(),
                        },
                    };
                    ui.allocate_ui_at_rect(result_controls_rect, |ui| {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            ui.spacing_mut().button_padding = Vec2 { x: 0., y: 0.0 };
                            ui.spacing_mut().item_spacing = Vec2 { x: 3., y: 0.0 };

                            if ui
                                .small_button("🗑")
                                .on_hover_text("Delete metric")
                                .clicked()
                            {
                                delete_i = Some(metric_i);
                            }
                            ui.add_enabled_ui(metric_i < num_metrics - 1, |ui| {
                                if ui
                                    .small_button("⬇")
                                    .on_hover_text("Move metric down")
                                    .clicked()
                                {
                                    swap_indices = Some((metric_i, metric_i + 1));
                                }
                            });
                            ui.add_enabled_ui(metric_i != 0, |ui| {
                                if ui
                                    .small_button("⬆")
                                    .on_hover_text("Move metric up")
                                    .clicked()
                                {
                                    swap_indices = Some((metric_i, metric_i - 1));
                                }
                            });
                        });
                    });
                }

                let desired_width = ui.standard_width();
                let field_shape = Vec2::new(desired_width, 0.);

                const MAX_FIELD_LEN: usize = 20;
                match &mut metric_tracker.metric {
                    Metric::SpecificResponses {
                        question_index,
                        choice_index,
                    } => {
                        ui.label("Question");
                        ui.allocate_ui(field_shape, |ui| {
                            ComboBox::from_id_source(format!("selected_question_{metric_i}"))
                                .width(desired_width)
                                .show_index(ui, question_index, poll.questions.len(), |i| {
                                    format!("{i}: {}", limit(&poll.questions[i].prompt))
                                });
                        });
                        match &poll.questions[*question_index].form {
                            Form::ChooseOneorNone { options } => {
                                let mut selected = *choice_index as usize;
                                ui.label("Answer");
                                ui.allocate_ui(field_shape, |ui| {
                                    ComboBox::from_id_source(format!("selected_answer_{metric_i}"))
                                        .show_index(ui, &mut selected, options.len(), |i| {
                                            format!("{i}: {}", limit(&options[i]))
                                        });
                                });

                                *choice_index = selected as u8;
                            }
                        }
                    }
                }
                ui.checkbox(
                    &mut metric_tracker.publicly_visible,
                    "Show progress publicly",
                );
            });
            if metric_i == 0 {
                ui_data.question_group_rect = Some(response.response.rect);
            }
            if ui.small_button("Add Metric").clicked() {
                new_index = Some(metric_i + 1);
            }
        }
        if let Some(index) = delete_i {
            poll.results.remove(index);
        }
        if let Some((a, b)) = swap_indices {
            poll.results.swap(a, b);
        }
        if let Some(index) = new_index {
            poll.metric_trackers.insert(
                index,
                MetricTracker {
                    publicly_visible: false,
                    metric: Metric::SpecificResponses {
                        question_index: 0,
                        choice_index: 0,
                    },
                },
            );
        }
    }

    fn show_results_form(ui: &mut Ui, poll: &mut Poll, ui_data: &mut CreatingUiData) {
        let mut new_index = None;
        let mut delete_i = None;
        let mut swap_indices = None;

        if poll.metric_trackers.is_empty() {
            ui.label("Before you can add a result, you need to add at least one metric.");
            return;
        }
        if ui.small_button("Add Result").clicked() {
            new_index = Some(0);
        }

        let num_results = poll.results.len();
        for (result_i, result) in poll.results.iter_mut().enumerate() {
            let response = ui.group(|ui| {
                let label_response = ui.label(format!("Result {}", result_i + 1));
                if let Some(fields_rect) = ui_data.fields_rect {
                    let result_controls_rect = Rect {
                        min: Pos2 {
                            x: label_response.rect.right(),
                            y: label_response.rect.top(),
                        },
                        max: Pos2 {
                            x: fields_rect.right(),
                            y: label_response.rect.bottom(),
                        },
                    };
                    ui.allocate_ui_at_rect(result_controls_rect, |ui| {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            ui.spacing_mut().button_padding = Vec2 { x: 0., y: 0.0 };
                            ui.spacing_mut().item_spacing = Vec2 { x: 3., y: 0.0 };

                            if ui
                                .small_button("🗑")
                                .on_hover_text("Delete result")
                                .clicked()
                            {
                                delete_i = Some(result_i);
                            }
                            ui.add_enabled_ui(result_i < num_results - 1, |ui| {
                                if ui
                                    .small_button("⬇")
                                    .on_hover_text("Move result down")
                                    .clicked()
                                {
                                    swap_indices = Some((result_i, result_i + 1));
                                }
                            });
                            ui.add_enabled_ui(result_i != 0, |ui| {
                                if ui
                                    .small_button("⬆")
                                    .on_hover_text("Move result up")
                                    .clicked()
                                {
                                    swap_indices = Some((result_i, result_i - 1));
                                }
                            });
                        });
                    });
                }
                let response = ui.add(
                    TextEdit::multiline(&mut result.desc)
                        .desired_rows(1)
                        .hint_text("What will happen?"),
                );
                ui_data.fields_rect = Some(response.rect);
                let field_shape = Vec2::new(response.rect.width(), 0.);

                let mut selected = match &result.requirements[0] {
                    Requirement::AtLeast { .. } => 0,
                };
                let selected_before = selected;
                const TYPES: &[&str] = &["At Least X"];
                ui.label("Requirements Type");
                ui.allocate_ui(field_shape, |ui| {
                    ComboBox::from_id_source(format!("requirement_type_{result_i}"))
                        .width(ui.standard_width())
                        .show_index(ui, &mut selected, 1, |i| TYPES[i].to_string());
                });
                if selected != selected_before {
                    result.requirements[0] = match selected {
                        0 => Requirement::AtLeast {
                            minimum: 1,
                            metric_index: 0,
                        },
                        _ => unreachable!(),
                    };
                }

                const MAX_FIELD_LEN: usize = 20;
                match &mut result.requirements[0] {
                    Requirement::AtLeast {
                        minimum,
                        metric_index,
                    } => {
                        *metric_index = {
                            let compatible_metrics = poll
                                .metric_trackers
                                .iter()
                                .enumerate()
                                .filter(|(_, metric_tracker)| match metric_tracker.metric {
                                    Metric::SpecificResponses { .. } => true,
                                })
                                .collect::<Vec<_>>();
                            let mut sub_index = compatible_metrics
                                .iter()
                                .map(|(i, _)| *i)
                                .find(|i| *i == *metric_index as usize)
                                .unwrap_or(0);
                            ui.label("Metric");
                            ui.allocate_ui(field_shape, |ui| {
                                ComboBox::from_id_source(format!("selected_metric_{result_i}"))
                                    .show_index(
                                        ui,
                                        &mut sub_index,
                                        compatible_metrics.len(),
                                        |i| {
                                            format!(
                                                "{}: {}",
                                                &compatible_metrics[i].0,
                                                limit(
                                                    &compatible_metrics[i]
                                                        .1
                                                        .metric
                                                        .render(&poll.questions)
                                                )
                                            )
                                        },
                                    );
                            });
                            compatible_metrics[sub_index].0 as u16
                        };

                        ui.label("Minimum");
                        let mut minimum_usize = *minimum as usize - 1;
                        ui.allocate_ui(field_shape, |ui| {
                            ComboBox::from_id_source(format!("minimum_{result_i}")).show_index(
                                ui,
                                &mut minimum_usize,
                                30,
                                |i| (i + 1).to_string(),
                            );
                        });
                        *minimum = minimum_usize as u64 + 1;
                    }
                }
            });
            if result_i == 0 {
                ui_data.question_group_rect = Some(response.response.rect);
            }
            if ui.small_button("Add Result").clicked() {
                new_index = Some(result_i + 1);
            }
        }
        if let Some(index) = delete_i {
            poll.results.remove(index);
        }
        if let Some((a, b)) = swap_indices {
            poll.results.swap(a, b);
        }
        if let Some(index) = new_index {
            poll.results.insert(
                index,
                PollResult2 {
                    desc: "".to_string(),
                    requirements: vec![Requirement::AtLeast {
                        metric_index: 0,
                        minimum: 1,
                    }],
                },
            );
        }
    }
}

const MAX_FIELD_LEN: usize = 30;

fn limit(s: &str) -> String {
    if s.len() > MAX_FIELD_LEN {
        format!("{}...", s.get(..(MAX_FIELD_LEN - 3)).unwrap())
    } else {
        s.to_string()
    }
}
