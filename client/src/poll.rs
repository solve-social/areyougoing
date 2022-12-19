use crate::{
    app::SignInData, misc::UrlExt, new_poll::NewPoll, participation::ParticipationState,
    results_ui::ResultsUi, retrieve::RetrievingState,
};
use areyougoing_shared::Poll;
use derivative::Derivative;
use egui::Ui;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Deserialize, Serialize, Debug)]
pub enum PollState {
    None,
    NewPoll {
        state: NewPoll,
        poll: Poll,
    },
    Retrieving {
        key: u64,
        #[serde(skip)]
        #[derivative(PartialEq = "ignore")]
        state: RetrievingState,
    },
    Found {
        key: u64,
        poll: Poll,
        participation_state: ParticipationState,
        results_ui: ResultsUi,
    },
    NotFound {
        key: u64,
    },
}

impl Default for PollState {
    fn default() -> Self {
        Self::None
    }
}

impl PollState {
    pub fn process(
        &mut self,
        ui: &mut Ui,
        next_poll_state: &mut Option<PollState>,
        original_url: &Option<Url>,
        sign_in_data: &mut SignInData,
    ) {
        ui.vertical_centered(|ui| match self {
            PollState::None => {
                *next_poll_state = Some(PollState::NewPoll {
                    state: NewPoll::Creating {
                        ui_data: Default::default(),
                        show_conditions: false,
                    },
                    poll: Default::default(),
                });
            }
            PollState::NewPoll { poll, state } => {
                state.process(ui, poll, original_url);
            }
            PollState::Retrieving { key, ref mut state } => {
                ui.label(format!("Retreiving Poll #{key}"));
                state.process(next_poll_state, *key);
                // Make sure the UI keeps updating in order to keep polling the fetch process
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            PollState::Found {
                key,
                poll,
                ref mut participation_state,
                ref mut results_ui,
            } => {
                ui.heading(format!("{} (#{key})", poll.title));
                ui.label(&poll.description);
                results_ui.process(ui, poll, *key);
                ui.separator();
                participation_state.process(ui, sign_in_data, *key, poll, &mut results_ui.stale);
            }
            PollState::NotFound { key } => {
                ui.label(format!("No poll with ID #{key} was found 😥"));
            }
        });
        if let Some(mut state) = next_poll_state.take() {
            {
                use PollState::*;
                match &mut state {
                    NewPoll { .. } => {
                        original_url.with_query(Option::None).push_to_window();
                    }
                    Found {
                        participation_state:
                            ParticipationState::SignedIn {
                                ref mut question_responses,
                                ..
                            },
                        ..
                    } => {
                        // Temporary for debugging, with changing polls as we go
                        *question_responses = Default::default();
                    }
                    _ => {}
                }
            }
            *self = state;
        }
    }
}
