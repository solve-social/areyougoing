use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub struct Question {
    pub prompt: String,
    pub form: Form,
}

#[derive(Deserialize, Serialize, PartialEq)]
pub enum FormResponse {
    ChooseOne { choice: Option<u8> },
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum Form {
    ChooseOne { options: Vec<String> },
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum PollStatus {
    SeekingResponses,
    Closed,
}

impl Default for PollStatus {
    fn default() -> Self {
        Self::SeekingResponses
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum ConditionDescription {
    AtLeast {
        minimum: u16,
        question_index: usize,
        choice_index: u8,
    },
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum ConditionState {
    Met,
    NotMet,
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub struct PollResult {
    pub description: ConditionDescription,
    pub state: ConditionState,
    pub result: Option<String>,
}

#[derive(Deserialize, Serialize, Default, PartialEq, Clone)]
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
                Form::ChooseOne { options: _ } => FormResponse::ChooseOne { choice: None },
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Deserialize, Serialize)]
pub struct PollQuery {
    pub id: u64,
}

#[derive(Deserialize, Serialize)]
pub enum PollQueryResult {
    Found(Poll),
    NotFound,
}
