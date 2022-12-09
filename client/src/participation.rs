use std::time::Duration;

use crate::{app::SignInData, time::Instant};
use areyougoing_shared::{Form, FormResponse, Poll};
use egui::{Button, ScrollArea, Ui};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq)]
pub enum ParticipationState {
    SignedIn {
        user: String,
        responses: Vec<FormResponse>,
    },
    SignIn,
    Submitting {
        #[serde(skip)]
        progress: Option<Instant>,
    },
    SubmitConfirmation,
}

impl ParticipationState {
    pub fn process(&mut self, ui: &mut Ui, sign_in_data: &mut SignInData, key: u64, poll: &Poll) {
        ui.heading(format!("{} (#{key})", poll.title));

        ui.label(&poll.description);
        ui.separator();
        let mut next_participation_state = None;
        match self {
            ParticipationState::SignIn => {
                ui.label(
                    "Type your name or choose a previous name \
                                    from below and select \"SIGN IN\"",
                );
                ui.text_edit_singleline(&mut sign_in_data.user_entry);
                if ui.button("SIGN IN").clicked() {
                    next_participation_state = Some(ParticipationState::SignedIn {
                        user: sign_in_data.user_entry.clone(),
                        responses: poll.init_responses(),
                    });
                    if !sign_in_data.old_names.contains(&sign_in_data.user_entry) {
                        sign_in_data.old_names.push(sign_in_data.user_entry.clone());
                    }
                    sign_in_data.user_entry = "".to_string();
                }
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    for name in sign_in_data.old_names.iter().rev() {
                        if ui.button(name).clicked() {
                            sign_in_data.user_entry = name.to_string();
                        }
                    }
                });
            }
            ParticipationState::SignedIn {
                user: _,
                ref mut responses,
            } => {
                for (question, mut response) in poll.questions.iter().zip(responses.iter_mut()) {
                    ui.group(|ui| {
                        ui.label(&question.prompt);
                        match (&question.form, &mut response) {
                            (Form::ChooseOne { options }, FormResponse::ChooseOne { choice }) => {
                                for (i, option) in options.iter().enumerate() {
                                    let selected = choice.is_some() && choice.unwrap() == i as u8;
                                    let mut button = Button::new(option);
                                    if selected {
                                        button =
                                            button.fill(ui.ctx().style().visuals.selection.bg_fill);
                                    }
                                    let response = ui.add(button);
                                    if !selected {
                                        if response.clicked() {
                                            *choice = Some(i as u8);
                                        }
                                    }
                                }
                            }
                        }
                    });
                }

                if ui.button("SUBMIT").clicked() {
                    next_participation_state =
                        Some(ParticipationState::Submitting { progress: None });
                }
            }
            ParticipationState::Submitting { ref mut progress } => {
                ui.label("Your response is being submitted...");
                if let Some(start_time) = progress {
                    if start_time.elapsed() > Duration::from_secs_f64(1.0) {
                        next_participation_state = Some(ParticipationState::SubmitConfirmation);
                    }
                } else {
                    *progress = Some(Instant::now());
                }
            }
            ParticipationState::SubmitConfirmation => {
                ui.label("Your response has been submitted!. Thanks!");
                ui.label("To change your response, sign in with the exact same name again.");
                if ui.button("SIGN IN").clicked() {
                    next_participation_state = Some(ParticipationState::SignIn);
                }
            }
        }
        if let Some(state) = next_participation_state {
            *self = state;
        }
    }
}
