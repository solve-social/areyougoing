use std::time::Duration;

use crate::{
    app::SignInData,
    misc::{console_log, get_window, log, Pollable},
    SERVER_URL,
};
use areyougoing_shared::{Form, FormResponse, Poll, PollResponse, PollSubmissionResult};
use derivative::Derivative;
use egui::{Button, ScrollArea, Ui};
use gloo::{console::__macro::JsValue, net::http::RequestMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

const SIGN_IN_TEXT: &str = "SIGN_IN";

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize)]
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
        state: Option<SubmittingState>,
    },
    SubmitConfirmation,
}

#[derive(Debug)]
pub enum SubmittingState {
    Sending(JsFuture),
    Converting(JsFuture),
}

impl ParticipationState {
    pub fn process(&mut self, ui: &mut Ui, sign_in_data: &mut SignInData, key: u64, poll: &Poll) {
        ui.heading(format!("{} (#{key})", poll.title));

        ui.label(&poll.description);
        ui.separator();
        let mut next_participation_state = None;
        match self {
            ParticipationState::SignIn => {
                ui.label(format!(
                    "Type your name or choose a previous name \
                                    from below and select \"{SIGN_IN_TEXT}\""
                ));
                ui.text_edit_singleline(&mut sign_in_data.user_entry);
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
                user,
                ref mut question_responses,
            } => {
                if question_responses.len() == 0 {
                    *question_responses = poll.init_responses();
                }
                ScrollArea::vertical().show(ui, |ui| {
                    for (question, mut question_response) in
                        poll.questions.iter().zip(question_responses.iter_mut())
                    {
                        ui.group(|ui| {
                            ui.label(&question.prompt);
                            match (&question.form, &mut question_response) {
                                (
                                    Form::ChooseOne { options },
                                    FormResponse::ChooseOne { choice },
                                ) => {
                                    for (i, option) in options.iter().enumerate() {
                                        let selected =
                                            choice.is_some() && choice.unwrap() == i as u8;
                                        let mut button = Button::new(option);
                                        if selected {
                                            button = button
                                                .fill(ui.ctx().style().visuals.selection.bg_fill);
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
                let mut next_submitting_state = None;
                if let Some(state) = state {
                    match state {
                        SubmittingState::Sending(future) => {
                            if let Some(result) = future.poll() {
                                next_submitting_state = Some(None);
                                if let Ok(response) = result {
                                    assert!(response.is_instance_of::<Response>());
                                    let resp: Response = response.dyn_into().unwrap();
                                    if let Ok(json) = resp.json() {
                                        next_submitting_state = Some(Some(
                                            SubmittingState::Converting(JsFuture::from(json)),
                                        ));
                                    }
                                }
                            }
                        }
                        SubmittingState::Converting(future) => {
                            if let Some(result) = future.poll() {
                                next_submitting_state = Some(None);
                                if let Ok(json) = result {
                                    if let Ok(submission_result) =
                                        serde_wasm_bindgen::from_value(json)
                                    {
                                        console_log!("Received from server: {submission_result:?}");
                                        match submission_result {
                                            PollSubmissionResult::Success => {
                                                next_participation_state =
                                                    Some(ParticipationState::SubmitConfirmation);
                                            }
                                            PollSubmissionResult::Error => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    let mut opts = RequestInit::new();
                    opts.method("POST");
                    opts.body(Some(&JsValue::from(
                        serde_json::to_string(response).unwrap(),
                    )));
                    opts.credentials(web_sys::RequestCredentials::Include);
                    opts.mode(RequestMode::Cors);
                    let url = format!("{SERVER_URL}/submit");
                    let request = Request::new_with_str_and_init(&url, &opts).unwrap();
                    request
                        .headers()
                        .set("Content-Type", "application/json")
                        .unwrap();
                    *state = Some(SubmittingState::Sending(JsFuture::from(
                        get_window().fetch_with_request(&request),
                    )));
                }
                if let Some(next_state) = next_submitting_state {
                    *state = next_state;
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
