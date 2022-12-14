use std::time::Duration;

use areyougoing_shared::{
    ConditionDescription, ConditionState, CreatePollResult, Form, Poll, PollResult, Question,
};
use derivative::Derivative;
use egui::{Align, ComboBox, Layout, Pos2, Rect, ScrollArea, TextEdit, Ui, Vec2};
use serde::{Deserialize, Serialize};
use url::Url;
use wasm_bindgen_futures::JsFuture;

use crate::misc::Submitter;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub enum NewPoll {
    Creating {
        ui_data: CreatingUiData,
        show_conditions: bool,
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
}

#[derive(Debug)]
pub enum SubmittingState {
    Fetching(JsFuture),
    Converting(JsFuture),
}

impl NewPoll {
    pub fn process(&mut self, ui: &mut Ui, poll: &mut Poll, original_url: &Option<Url>) {
        let mut next_new_poll_state = None;
        match self {
            NewPoll::Creating {
                ref mut ui_data,
                ref mut show_conditions,
            } => {
                if let Some(rect) = ui_data.available_rect {
                    if rect != ui.available_rect_before_wrap() {
                        // Somehow the size of the window has changed, so reset/recalculate everything
                        *ui_data = Default::default();
                    }
                }
                ui_data.available_rect = Some(ui.available_rect_before_wrap());

                ui.heading("Create a new poll!");

                let switcher_button_text = if *show_conditions {
                    "<- Back to Questions"
                } else {
                    "Show Result Trackers"
                };
                if ui.small_button(switcher_button_text).clicked() {
                    *show_conditions = !*show_conditions;
                }
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    if *show_conditions {
                        Self::show_results_form(ui, poll, ui_data);
                    } else {
                        Self::show_main_form(ui, poll, ui_data);
                        ui.separator();
                        if ui.button("SUBMIT").clicked() {
                            next_new_poll_state = Some(NewPoll::Submitting {
                                poll: poll.clone(),
                                state: None,
                            });
                        }
                    }
                });
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

        let mut new_question_index = None;
        let mut delete_i = None;
        let mut swap_indices = None;

        let num_questions = poll.questions.len();
        for (question_i, question) in poll.questions.iter_mut().enumerate() {
            let response = ui.group(|ui| {
                let label_response = ui.label(format!("Question {}", question_i + 1));

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
                            ui.spacing_mut().button_padding = Vec2 { x: 0., y: 0.0 };
                            ui.spacing_mut().item_spacing = Vec2 { x: 3., y: 0.0 };

                            ui.add_enabled_ui(num_questions > 1, |ui| {
                                if ui
                                    .small_button("🗑")
                                    .on_hover_text("Delete question")
                                    .clicked()
                                {
                                    delete_i = Some(question_i);
                                }
                            });
                            ui.add_enabled_ui(question_i < num_questions - 1, |ui| {
                                if ui
                                    .small_button("⬇")
                                    .on_hover_text("Move question down")
                                    .clicked()
                                {
                                    swap_indices = Some((question_i, question_i + 1));
                                }
                            });
                            ui.add_enabled_ui(question_i != 0, |ui| {
                                if ui
                                    .small_button("⬆")
                                    .on_hover_text("Move question up")
                                    .clicked()
                                {
                                    swap_indices = Some((question_i, question_i - 1));
                                }
                            });
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
                        let mut new_option_index = None;
                        let mut delete_i = None;
                        let mut swap_indices = None;
                        let num_options = options.len();
                        for (option_i, option) in options.iter_mut().enumerate() {
                            ui.allocate_ui(ui_data.fields_rect.unwrap().size(), |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.spacing_mut().button_padding = Vec2 { x: 0., y: 0.0 };
                                    ui.spacing_mut().item_spacing = Vec2 { x: 3., y: 1.0 };

                                    ui.add_enabled_ui(num_options > 1, |ui| {
                                        if ui
                                            .small_button("🗑")
                                            .on_hover_text("Delete option")
                                            .clicked()
                                        {
                                            delete_i = Some(option_i);
                                        }
                                    });

                                    ui.add_enabled_ui(option_i < num_options - 1, |ui| {
                                        if ui
                                            .small_button("⬇")
                                            .on_hover_text("Move option down")
                                            .clicked()
                                        {
                                            swap_indices = Some((option_i, option_i + 1));
                                        }
                                    });
                                    ui.add_enabled_ui(option_i != 0, |ui| {
                                        if ui
                                            .small_button("⬆")
                                            .on_hover_text("Move option up")
                                            .clicked()
                                        {
                                            swap_indices = Some((option_i, option_i - 1));
                                        }
                                    });
                                    if ui
                                        .small_button("➕")
                                        .on_hover_text("Insert option after this one")
                                        .clicked()
                                    {
                                        new_option_index = Some(option_i + 1);
                                    }
                                    ui.add(
                                        TextEdit::singleline(option)
                                            .hint_text(format!("Option {}", option_i + 1)),
                                    );
                                });
                            });
                        }
                        if let Some(index) = delete_i {
                            options.remove(index);
                        }
                        if options.is_empty() {
                            new_option_index = Some(0);
                        }
                        if let Some(index) = new_option_index {
                            options.insert(index, "".to_string())
                        }
                        if let Some((a, b)) = swap_indices {
                            options.swap(a, b);
                        }
                    }
                }
            });
            if question_i == 0 {
                ui_data.question_group_rect = Some(response.response.rect);
            }
            if ui.small_button("Add Question").clicked() {
                new_question_index = Some(question_i + 1);
            }
        }
        if let Some(index) = delete_i {
            poll.questions.remove(index);
        }
        if let Some((a, b)) = swap_indices {
            poll.questions.swap(a, b);
        }
        if poll.questions.is_empty() {
            new_question_index = Some(0);
        }
        if let Some(index) = new_question_index {
            poll.questions.insert(
                index,
                Question {
                    prompt: "".to_string(),
                    form: Form::ChooseOneorNone {
                        options: Vec::new(),
                    },
                },
            );
        }
    }

    fn show_results_form(ui: &mut Ui, poll: &mut Poll, ui_data: &mut CreatingUiData) {
        let mut new_index = None;
        let mut delete_i = None;
        let mut swap_indices = None;

        if ui.small_button("Add Result Tracker").clicked() {
            new_index = Some(0);
        }

        let num_results = poll.results.len();
        for (result_i, result) in poll.results.iter_mut().enumerate() {
            let response = ui.group(|ui| {
                let label_response = ui.label(format!("Result Tracker {}", result_i + 1));
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
                                .on_hover_text("Delete result tracker")
                                .clicked()
                            {
                                delete_i = Some(result_i);
                            }
                            ui.add_enabled_ui(result_i < num_results - 1, |ui| {
                                if ui
                                    .small_button("⬇")
                                    .on_hover_text("Move result tracker down")
                                    .clicked()
                                {
                                    swap_indices = Some((result_i, result_i + 1));
                                }
                            });
                            ui.add_enabled_ui(result_i != 0, |ui| {
                                if ui
                                    .small_button("⬆")
                                    .on_hover_text("Move result tracker up")
                                    .clicked()
                                {
                                    swap_indices = Some((result_i, result_i - 1));
                                }
                            });
                        });
                    });
                }
                let response = ui.add(
                    TextEdit::multiline(&mut result.result)
                        .desired_rows(1)
                        .hint_text("What happens if condition is met? (Optional)"),
                );
                ui_data.fields_rect = Some(response.rect);
                let field_shape = Vec2::new(response.rect.width(), 0.);

                let mut selected = match &result.description {
                    ConditionDescription::AtLeast { .. } => 0,
                };
                let selected_before = selected;
                const TYPES: &[&str] = &["At Least X Specific Responses"];
                ui.allocate_ui(field_shape, |ui| {
                    ComboBox::new(format!("condition_type_{result_i}"), "Condition Type")
                        .show_index(ui, &mut selected, 1, |i| TYPES[i].to_string());
                });
                if selected != selected_before {
                    result.description = match selected {
                        0 => ConditionDescription::AtLeast {
                            minimum: 1,
                            question_index: 0,
                            choice_index: 0,
                        },
                        _ => unreachable!(),
                    };
                }

                const MAX_FIELD_LEN: usize = 20;
                match &mut result.description {
                    ConditionDescription::AtLeast {
                        minimum,
                        question_index,
                        choice_index,
                    } => {
                        ui.allocate_ui(field_shape, |ui| {
                            ComboBox::new(format!("selected_question_{result_i}"), "Question")
                                .show_index(ui, question_index, poll.questions.len(), |i| {
                                    format!("{i}: {}", limit(&poll.questions[i].prompt))
                                });
                        });
                        match &poll.questions[*question_index].form {
                            Form::ChooseOneorNone { options } => {
                                let mut selected = *choice_index as usize;
                                ui.allocate_ui(field_shape, |ui| {
                                    ComboBox::new(format!("selected_answer_{result_i}"), "Answer")
                                        .show_index(ui, &mut selected, options.len(), |i| {
                                            format!("{i}: {}", limit(&options[i]))
                                        });
                                });

                                *choice_index = selected as u8;
                            }
                        }

                        let mut minimum_usize = *minimum as usize - 1;
                        ui.allocate_ui(field_shape, |ui| {
                            ComboBox::new(format!("minimum_{result_i}"), "Minimum").show_index(
                                ui,
                                &mut minimum_usize,
                                30,
                                |i| (i + 1).to_string(),
                            );
                        });
                        *minimum = minimum_usize as u16 + 1;
                    }
                }
                ui.label("How should progress be publicly displayed?");

                let mut selected_index = match &result.progress {
                    ConditionState::MetOrNotMet(..) => 0,
                    ConditionState::Progress(..) => 1,
                };
                const METHODS: &[&str] = &[
                    "Only show whether condition is met",
                    "Show progress toward condition",
                ];
                ui.allocate_ui(field_shape, |ui| {
                    ComboBox::new(format!("condition_state_{result_i}"), "").show_index(
                        ui,
                        &mut selected_index,
                        2,
                        |i| METHODS[i].to_string(),
                    );
                });
                result.progress = match selected_index {
                    0 => ConditionState::MetOrNotMet(false),
                    1 => ConditionState::Progress(0),
                    _ => unreachable!(),
                }
            });
            if result_i == 0 {
                ui_data.question_group_rect = Some(response.response.rect);
            }
            if ui.small_button("Add Result Tracker").clicked() {
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
                PollResult {
                    description: ConditionDescription::AtLeast {
                        minimum: 1,
                        question_index: 0,
                        choice_index: 0,
                    },
                    progress: ConditionState::MetOrNotMet(false),
                    result: "".to_string(),
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
