use iced::Task;

use crate::features::pull::{self, Message as PullMessage, PullMode};
use crate::{features::repo_open, App, Message};

impl App {
    pub(crate) fn update_pull(&mut self, message: PullMessage) -> Task<Message> {
        match message {
            PullMessage::Requested(mode) => self.start_pull(mode),
            PullMessage::Done { mode, result } => {
                self.operation.loading = false;
                match result {
                    Ok(()) => {
                        let status_message = self.pull_success_message(mode);
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.pending_transient_status_after_reload =
                                Some(status_message);
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
                        self.operation.pending_transient_status_after_reload = None;
                        self.operation.error = Some(msg);
                        Task::none()
                    }
                }
            }
        }
    }

    pub(crate) fn start_pull(&mut self, mode: PullMode) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading || self.repo.sync_status.upstream.is_none() {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.transient_status = None;
        self.operation.pending_transient_status_after_reload = None;
        self.operation.loading = true;
        Task::perform(pull::task::run(path, mode), move |result| {
            Message::from(PullMessage::Done { mode, result })
        })
    }
}
