use iced::Task;
use naite_core::CommitSummary;

use crate::features::cherry_pick::{self, Message as CherryPickMessage};
use crate::features::repo_open;
use crate::{App, Message};

impl App {
    pub(crate) fn update_cherry_pick(&mut self, message: CherryPickMessage) -> Task<Message> {
        match message {
            CherryPickMessage::Requested(commit) => self.start_cherry_pick(commit),
            CherryPickMessage::Done { commit, result } => self.finish_cherry_pick(commit, result),
        }
    }

    fn start_cherry_pick(&mut self, commit: CommitSummary) -> Task<Message> {
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
        Task::perform(
            cherry_pick::task::run(path, commit.id.clone()),
            move |result| {
                Message::from(CherryPickMessage::Done {
                    commit: commit_for_message.clone(),
                    result,
                })
            },
        )
    }

    fn finish_cherry_pick(
        &mut self,
        commit: CommitSummary,
        result: Result<(), String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        match result {
            Ok(()) => {
                let status_message = format!("Cherry-picked {}", commit.short_id);
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
