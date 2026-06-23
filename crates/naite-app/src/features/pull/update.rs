use iced::Task;

use crate::features::pull::{self, Message as PullMessage, PullMode};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{features::repo_open, App, Message};

impl App {
    pub(crate) fn update_pull(&mut self, message: PullMessage) -> Task<Message> {
        match message {
            PullMessage::Requested(mode) => self.start_pull(mode),
            PullMessage::Done { mode, result } => {
                let completion = match self
                    .operation_tracker
                    .current_id_for(&OperationKind::ManualAction("pull"))
                {
                    Some(id) => {
                        let event = match &result {
                            Ok(()) => OperationEvent::Completed {
                                id,
                                result: OpResult::Success,
                                severity: OpSeverity::Recoverable,
                            },
                            Err(message) => OperationEvent::Completed {
                                id,
                                result: OpResult::Failed(message.clone()),
                                severity: OpSeverity::Recoverable,
                            },
                        };
                        Task::done(Message::Operation(event))
                    }
                    None => Task::none(),
                };
                self.operation.loading = false;
                match result {
                    Ok(()) => {
                        let status_message = self.pull_success_message(mode);
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.pending_transient_status_after_reload =
                                Some(status_message);
                            self.operation.loading = true;
                            let reload_start =
                                Task::done(Message::Operation(OperationEvent::Started {
                                    id: self.operation_tracker.next_id(),
                                    kind: OperationKind::ManualAction("repo_open"),
                                    label: "Reloading repository…".to_string(),
                                }));
                            completion.chain(reload_start.chain(Task::perform(
                                repo_open::task::load(path),
                                |result| {
                                    Message::from(repo_open::Message::Loaded(Box::new(result)))
                                },
                            )))
                        } else {
                            self.set_transient_status(status_message);
                            completion
                        }
                    }
                    Err(msg) => {
                        self.operation.pending_transient_status_after_reload = None;
                        self.operation.error = Some(msg);
                        completion
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
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("pull"),
            label: "Pulling current branch…".to_string(),
        }));
        start.chain(Task::perform(pull::task::run(path, mode), move |result| {
            Message::from(PullMessage::Done { mode, result })
        }))
    }
}
