use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
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

impl Default for Form {
    fn default() -> Self {
        Self::ChooseOneorNone {
            options: Default::default(),
        }
    }
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
pub enum Metric {
    SpecificResponses {
        question_index: usize,
        choice_index: u8,
    },
}

impl Metric {
    pub fn render(&self, questions: &[Question]) -> String {
        match self {
            Metric::SpecificResponses {
                question_index,
                choice_index,
            } => {
                let Question { prompt, form } = &questions[*question_index];
                let choice = match form {
                    Form::ChooseOneorNone { options } => &options[*choice_index as usize],
                };
                format!("\"{choice}\" to \"{prompt}\"")
            }
        }
    }
}

impl Default for Metric {
    fn default() -> Self {
        Self::SpecificResponses {
            question_index: 0,
            choice_index: 0,
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct MetricTracker {
    pub metric: Metric,
    pub publicly_visible: bool,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum Progress {
    Count(u64),
}

impl Metric {
    pub fn calculate_progress(&self, responses: &HashMap<String, Vec<FormResponse>>) -> Progress {
        match self {
            Metric::SpecificResponses {
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
                Progress::Count(count)
            }
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum Requirement {
    AtLeast { metric_index: u16, minimum: u64 },
}

impl Requirement {
    pub fn evaluate(&self, progresses: &[Progress]) -> bool {
        match self {
            Requirement::AtLeast {
                minimum,
                metric_index,
            } => {
                let Progress::Count(count) = progresses.get(*metric_index as usize).unwrap();
                count >= minimum
            }
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct PollResult2 {
    pub desc: String,
    pub requirements: Vec<Requirement>,
}

impl Default for PollResult2 {
    fn default() -> Self {
        Self {
            desc: "".to_string(),
            requirements: vec![Requirement::AtLeast {
                metric_index: 0,
                minimum: 1,
            }],
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct ResultState {
    pub requirements_met: Vec<bool>,
    pub overall_met: bool,
}

impl ResultState {
    pub fn from_result(result: &PollResult2) -> Self {
        Self {
            requirements_met: vec![false; result.requirements.len()],
            overall_met: false,
        }
    }
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
    pub result: String,
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
    pub metric_progresses: Vec<Option<Progress>>,
    pub result_states: Vec<ResultState>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct Poll {
    pub title: String,
    pub description: String,
    pub expiration: Option<DateTime<Utc>>,
    pub announcement: Option<String>,
    pub metric_trackers: Vec<MetricTracker>,
    pub results: Vec<PollResult2>,
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
