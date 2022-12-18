use std::time::Duration;

use crate::{app::SignInData, misc::Submitter};
use areyougoing_shared::{Form, FormResponse, Poll, PollResponse, PollSubmissionResult};
use derivative::Derivative;
use egui::{Button, ScrollArea, TextEdit, Ui};
use serde::{Deserialize, Serialize};

const SIGN_IN_TEXT: &str = "SIGN IN";

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub enum ParticipationState {
    SignedIn {
        user: String,
        question_responses: Vec<FormResponse>,
    },
    SignIn,
    Submitting {
        response: PollResponse,
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        state: Option<Submitter<PollResponse, PollSubmissionResult>>,
    },
    SubmitConfirmation,
}

impl ParticipationState {
    pub fn process(
        &mut self,
        ui: &mut Ui,
        sign_in_data: &mut SignInData,
        key: u64,
        poll: &Poll,
        stale: &mut bool,
    ) {
        let mut next_participation_state = None;
        match self {
            ParticipationState::SignIn => {
                const SIGN_IN_HINT: &str = "Type a name";
                ui.label("Participate in this poll?");
                ui.add(TextEdit::singleline(&mut sign_in_data.user_entry).hint_text(SIGN_IN_HINT));
                if ui.button(SIGN_IN_TEXT).clicked() {
                    next_participation_state = Some(ParticipationState::SignedIn {
                        user: sign_in_data.user_entry.clone(),
                        question_responses: Vec::new(),
                    });
                    if !sign_in_data.old_names.contains(&sign_in_data.user_entry) {
                        sign_in_data.old_names.push(sign_in_data.user_entry.clone());
                    }
                    sign_in_data.user_entry = "".to_string();
                }
                if !sign_in_data.old_names.is_empty() {
                    ui.separator();
                    ui.label("Autofill a previous name?");
                    ScrollArea::vertical()
                        .id_source("name_scroll")
                        .show(ui, |ui| {
                            for name in sign_in_data.old_names.iter().rev() {
                                if ui.button(name).clicked() {
                                    sign_in_data.user_entry = name.to_string();
                                }
                            }
                        });
                }
            }
            ParticipationState::SignedIn {
                user,
                ref mut question_responses,
            } => {
                if question_responses.is_empty() {
                    *question_responses = poll.init_responses();
                }
                ScrollArea::vertical()
                    .id_source("participation_scroll")
                    .show(ui, |ui| {
                        for (question, mut question_response) in
                            poll.questions.iter().zip(question_responses.iter_mut())
                        {
                            ui.group(|ui| {
                                ui.label(&question.prompt);
                                match (&question.form, &mut question_response) {
                                    (
                                        Form::ChooseOneorNone { options },
                                        FormResponse::ChooseOneOrNone(choice),
                                    ) => {
                                        for (i, option) in options.iter().enumerate() {
                                            let selected =
                                                choice.is_some() && choice.unwrap() == i as u8;
                                            let mut button = Button::new(option);
                                            if selected {
                                                button = button.fill(
                                                    ui.ctx().style().visuals.selection.bg_fill,
                                                );
                                            }
                                            let response = ui.add(button);
                                            if !selected && response.clicked() {
                                                *choice = Some(i as u8);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                        if ui.button("SUBMIT").clicked() {
                            next_participation_state = Some(ParticipationState::Submitting {
                                response: PollResponse {
                                    poll_id: key,
                                    user: user.to_string(),
                                    responses: question_responses.clone(),
                                },
                                state: None,
                            });
                        }
                    });
            }
            ParticipationState::Submitting {
                response,
                ref mut state,
            } => {
                ui.label("Your response is being submitted...");
                if let Some(submitter) = state {
                    if let Some(response) = submitter.poll() {
                        *stale = true;
                        match response {
                            PollSubmissionResult::Success => {
                                next_participation_state =
                                    Some(ParticipationState::SubmitConfirmation);
                            }
                            PollSubmissionResult::Error => {}
                        }
                    }
                } else {
                    *state = Some(Submitter::new("submit", response.clone()));
                }
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            ParticipationState::SubmitConfirmation => {
                ui.label("Your response has been submitted! Thanks!");
                ui.label("To change your response, sign in with the exact same name again.");
                if ui.button(SIGN_IN_TEXT).clicked() {
                    next_participation_state = Some(ParticipationState::SignIn);
                }
            }
        }
        if let Some(state) = next_participation_state {
            *self = state;
        }
    }
}
