use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct Question {
    pub prompt: String,
    pub form: Form,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub enum FormResponse {
    ChooseOneOrNone(Option<u8>),
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum Form {
    ChooseOneorNone { options: Vec<String> },
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum PollStatus {
    SeekingResponses,
    Closed,
}

impl Default for PollStatus {
    fn default() -> Self {
        Self::SeekingResponses
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum ConditionDescription {
    AtLeast {
        minimum: u16,
        question_index: usize,
        choice_index: u8,
    },
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum ConditionState {
    MetOrNotMet(bool),
    Progress(u16),
}

impl Default for ConditionState {
    fn default() -> Self {
        ConditionState::MetOrNotMet(false)
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct PollResult {
    pub description: ConditionDescription,
    pub result: Option<String>,
    pub progress: ConditionState,
}

impl PollResult {
    pub fn update(&mut self, responses: &HashMap<String, Vec<FormResponse>>) {
        let PollResult {
            description,
            result: _,
            ref mut progress,
        } = self;

        match description {
            ConditionDescription::AtLeast {
                minimum,
                question_index,
                choice_index,
            } => {
                let mut count = 0;
                for poll_response in responses.values() {
                    match poll_response.get(*question_index).unwrap() {
                        FormResponse::ChooseOneOrNone(choice) => {
                            if let Some(chosen_index) = choice {
                                if chosen_index == choice_index {
                                    count += 1;
                                }
                            }
                        }
                    }
                }
                let condition_met = count >= *minimum;
                if condition_met {
                    println!("Condition met: {description:?}");
                }
                match progress {
                    ConditionState::MetOrNotMet(met) => {
                        *met = condition_met;
                    }
                    ConditionState::Progress(progress) => {
                        *progress = count;
                    }
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct PollProgress {
    pub condition_states: Vec<ConditionState>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct Poll {
    pub title: String,
    pub description: String,
    pub expiration: Option<DateTime<Utc>>,
    pub announcement: Option<String>,
    pub results: Vec<PollResult>,
    pub status: PollStatus,
    pub questions: Vec<Question>,
}

impl Poll {
    pub fn init_responses(&self) -> Vec<FormResponse> {
        self.questions
            .iter()
            .map(|q| match q.form {
                Form::ChooseOneorNone { options: _ } => FormResponse::ChooseOneOrNone(None),
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Deserialize, Serialize)]
pub struct PollQuery {
    pub id: u64,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum PollQueryResult {
    Found(Poll),
    NotFound,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Default, Clone)]
pub struct PollResponse {
    pub poll_id: u64,
    pub user: String,
    pub responses: Vec<FormResponse>,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum PollSubmissionResult {
    Success,
    Error,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum CreatePollResult {
    Success { key: u64 },
    Error,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ProgressReportResult {
    Success { progress: PollProgress },
    Error,
}
