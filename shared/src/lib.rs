use chrono::{DateTime, Utc};
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct Question {
    pub prompt: String,
    pub form: Form,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub enum FormResponse {
    ChooseOneOrNone(Option<Choice>),
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum Form {
    ChooseOneorNone { options: Vec<String> },
    YesOrNo,
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

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, EnumAsInner)]
pub enum Choice {
    Index(u8),
    YesOrNo(bool),
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum Metric {
    SpecificResponses {
        question_index: usize,
        choice: Choice,
    },
}

impl Metric {
    pub fn render(&self, questions: &[Question]) -> String {
        match self {
            Metric::SpecificResponses {
                question_index,
                choice,
            } => {
                let Question { prompt, form } = &questions[*question_index];
                use Form::*;
                let choice = match form {
                    ChooseOneorNone { options } => &options[*choice.as_index().unwrap() as usize],
                    YesOrNo => {
                        if *choice.as_yes_or_no().unwrap() {
                            "Yes"
                        } else {
                            "No"
                        }
                    }
                };
                format!("{choice} to {prompt}")
            }
        }
    }
}

// impl Default for Metric {
//     fn default() -> Self {
//         Self::SpecificResponses {
//             question_index: 0,
//             choice: 0,
//         }
//     }
// }

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct MetricTracker {
    pub metric: Metric,
    pub publicly_visible: bool,
}

impl MetricTracker {
    pub fn init_from_questions(questions: &[Question]) -> Option<Self> {
        questions.get(0).map(|question| MetricTracker {
            publicly_visible: false,
            metric: Metric::SpecificResponses {
                question_index: 0,
                choice: match question.form {
                    Form::ChooseOneorNone { .. } => Choice::Index(0),
                    Form::YesOrNo => Choice::YesOrNo(true),
                },
            },
        })
    }
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
                choice: metric_choice,
            } => {
                let mut count = 0;
                for poll_response in responses.values() {
                    match poll_response.get(*question_index).unwrap() {
                        FormResponse::ChooseOneOrNone(choice) => {
                            if let Some(chosen_index) = choice {
                                if chosen_index == metric_choice {
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
pub struct PollResult {
    pub desc: String,
    pub requirements: Vec<Requirement>,
}

impl Default for PollResult {
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
    pub fn from_result(result: &PollResult) -> Self {
        Self {
            requirements_met: vec![false; result.requirements.len()],
            overall_met: false,
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
    pub results: Vec<PollResult>,
    pub status: PollStatus,
    pub questions: Vec<Question>,
}

impl Poll {
    pub fn init_responses(&self) -> Vec<FormResponse> {
        self.questions
            .iter()
            .map(|q| match q.form {
                Form::ChooseOneorNone { .. } | Form::YesOrNo => FormResponse::ChooseOneOrNone(None),
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
