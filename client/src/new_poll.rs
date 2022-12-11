use areyougoing_shared::{Form, Poll, Question};
use derivative::Derivative;
use egui::{Align, Layout, Pos2, Rect, ScrollArea, TextEdit, Ui, Vec2};
use serde::{Deserialize, Serialize};
use url::Url;
use wasm_bindgen_futures::JsFuture;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub enum NewPoll {
    Creating {
        ui_data: CreatingUiData,
    },
    Submitting {
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        state: Option<SubmittingState>,
    },
    Submitted {
        key: u64,
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
            NewPoll::Creating { ref mut ui_data } => {
                if let Some(rect) = ui_data.available_rect {
                    if rect != ui.available_rect_before_wrap() {
                        // Somehow the size of the window has changed, so reset/recalculate everything
                        *ui_data = Default::default();
                    }
                }
                ui_data.available_rect = Some(ui.available_rect_before_wrap());

                ui.heading("Create a new poll!");
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add(TextEdit::singleline(&mut poll.title).hint_text("Title"));
                    ui.add(
                        TextEdit::multiline(&mut poll.description)
                            .hint_text("Description")
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
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.spacing_mut().button_padding =
                                                        Vec2 { x: 0., y: 0.0 };
                                                    ui.spacing_mut().item_spacing =
                                                        Vec2 { x: 3., y: 1.0 };

                                                    ui.add_enabled_ui(num_options > 1, |ui| {
                                                        if ui
                                                            .small_button("🗑")
                                                            .on_hover_text("Delete option")
                                                            .clicked()
                                                        {
                                                            delete_i = Some(option_i);
                                                        }
                                                    });

                                                    ui.add_enabled_ui(
                                                        option_i < num_options - 1,
                                                        |ui| {
                                                            if ui
                                                                .small_button("⬇")
                                                                .on_hover_text("Move option down")
                                                                .clicked()
                                                            {
                                                                swap_indices =
                                                                    Some((option_i, option_i + 1));
                                                            }
                                                        },
                                                    );
                                                    ui.add_enabled_ui(option_i != 0, |ui| {
                                                        if ui
                                                            .small_button("⬆")
                                                            .on_hover_text("Move option up")
                                                            .clicked()
                                                        {
                                                            swap_indices =
                                                                Some((option_i, option_i - 1));
                                                        }
                                                    });
                                                    if ui
                                                        .small_button("➕")
                                                        .on_hover_text(
                                                            "Insert option after this one",
                                                        )
                                                        .clicked()
                                                    {
                                                        new_option_index = Some(option_i + 1);
                                                    }
                                                    ui.add(TextEdit::singleline(option).hint_text(
                                                        format!("Option {}", option_i + 1),
                                                    ));
                                                },
                                            );
                                        });
                                    }
                                    if let Some(index) = delete_i {
                                        options.remove(index);
                                    }
                                    if options.len() == 0 {
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
                    if poll.questions.len() == 0 {
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
                    ui.separator();
                    if ui.button("SUBMIT").clicked() {}
                });
            }
            NewPoll::Submitting { .. } => {
                next_new_poll_state = Some(NewPoll::Submitted { key: 0 });
            }
            NewPoll::Submitted { key } => {
                ui.label("Your new poll has been created!");
                let mut link = original_url.as_ref().unwrap().clone();
                link.set_path(&format!("{key}"));
                ui.label(format!("Share it with this link: {link}"));
            }
        }
        if let Some(next_state) = next_new_poll_state {
            *self = next_state;
        }
    }
}
