use chrono::{DateTime, Utc};
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use strum::EnumIter;

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct Question {
    pub prompt: String,
    pub form: Form,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub enum FormResponse {
    ChooseOneOrNone(Option<Choice>),
    ChooseOne(Choice),
    ChooseMultiple(Vec<Choice>),
    RankedChoice(Vec<Choice>),
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, EnumIter)]
pub enum Form {
    OneOrNone { options: Vec<String> },
    One { options: Vec<String> },
    Multiple { options: Vec<String> },
    RankedChoice { options: Vec<String> },
    YesNoNone,
    YesNo,
}

impl Display for Form {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Form::OneOrNone { .. } => {
                    "Pick 1/None"
                }
                Form::One { .. } => {
                    "Pick 1"
                }
                Form::Multiple { .. } => {
                    "Pick Multiple"
                }
                Form::RankedChoice { .. } => {
                    "Ranked Choice"
                }
                Form::YesNoNone => {
                    "Yes/No/None"
                }
                Form::YesNo => {
                    "Yes/No"
                }
            }
        )
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::OneOrNone {
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
                    OneOrNone { options }
                    | One { options }
                    | Multiple { options }
                    | RankedChoice { options } => &options[*choice.as_index().unwrap() as usize],
                    YesNoNone | YesNo => {
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

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct MetricTracker {
    pub metric: Metric,
    pub publicly_visible: bool,
}

impl MetricTracker {
    pub fn init_from_questions(questions: &[Question]) -> Option<Self> {
        use Form::*;
        questions.get(0).map(|question| MetricTracker {
            publicly_visible: false,
            metric: Metric::SpecificResponses {
                question_index: 0,
                choice: match question.form {
                    OneOrNone { .. } | One { .. } | Multiple { .. } | RankedChoice { .. } => {
                        Choice::Index(0)
                    }
                    YesNoNone | YesNo => Choice::YesOrNo(true),
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
                    use FormResponse::*;
                    match poll_response.get(*question_index).unwrap() {
                        ChooseOneOrNone(response_choice) => {
                            if let Some(response_choice) = response_choice {
                                if response_choice == metric_choice {
                                    count += 1;
                                }
                            }
                        }
                        ChooseOne(response_choice) => {
                            if response_choice == metric_choice {
                                count += 1;
                            }
                        }
                        ChooseMultiple(response_choices) => {
                            for response_choice in response_choices {
                                if response_choice == metric_choice {
                                    count += 1;
                                }
                            }
                        }
                        RankedChoice(response_ordered_choices) => {
                            if response_ordered_choices.get(0).unwrap() == metric_choice {
                                count += 1;
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
            .map(|q| match &q.form {
                Form::OneOrNone { .. } | Form::YesNoNone => FormResponse::ChooseOneOrNone(None),
                Form::One { .. } => FormResponse::ChooseOne(Choice::Index(0)),
                Form::YesNo => FormResponse::ChooseOne(Choice::YesOrNo(false)),
                Form::Multiple { .. } => FormResponse::ChooseMultiple(Vec::new()),
                Form::RankedChoice { options } => FormResponse::RankedChoice(
                    (0..options.len()).map(|i| Choice::Index(i as u8)).collect(),
                ),
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
