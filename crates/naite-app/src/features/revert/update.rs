use iced::Task;
use naite_core::CommitSummary;

use crate::features::repo_open;
use crate::features::revert::{self, Message as RevertMessage};
use crate::{App, Message};

impl App {
    pub(crate) fn update_revert(&mut self, message: RevertMessage) -> Task<Message> {
        match message {
            RevertMessage::Requested(commit) => self.start_revert(commit),
            RevertMessage::Done { commit, result } => self.finish_revert(commit, result),
        }
    }

    fn start_revert(&mut self, commit: CommitSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.transient_status = None;
        self.operation.pending_transient_status_after_reload = None;
        self.operation.loading = true;
        let commit_for_message = commit.clone();
        Task::perform(revert::task::run(path, commit.id.clone()), move |result| {
            Message::from(RevertMessage::Done {
                commit: commit_for_message.clone(),
                result,
            })
        })
    }

    fn finish_revert(
        &mut self,
        commit: CommitSummary,
        result: Result<(), String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        match result {
            Ok(()) => {
                let status_message = format!("Reverted {}", commit.short_id);
                if let Some(path) = self.repo.path.clone() {
                    self.operation.pending_transient_status_after_reload = Some(status_message);
                    self.operation.loading = true;
                    Task::perform(repo_open::task::load(path), |result| {
                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                    })
                } else {
                    self.set_transient_status(status_message);
                    Task::none()
                }
            }
            Err(msg) => {
                self.operation.error = Some(msg);
                Task::none()
            }
        }
    }
}
