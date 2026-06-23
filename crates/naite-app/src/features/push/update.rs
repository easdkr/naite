use iced::Task;

use crate::features::push::{self, Message as PushMessage, PushMode};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{features::repo_open, App, Message};

impl App {
    pub(crate) fn update_push(&mut self, message: PushMessage) -> Task<Message> {
        match message {
            PushMessage::Requested(mode) => {
                self.selection.context_menu = None;
                self.start_push(mode)
            }
            PushMessage::ForceWithLeaseConfirmationRequested => {
                self.selection.context_menu = None;
                match self.force_push_prompt_for_current_branch() {
                    Ok(prompt) => {
                        self.selection.force_push_confirmation = Some(prompt);
                        Task::none()
                    }
                    Err(message) => {
                        let id = self.operation_tracker.next_id();
                        self.operation.error = Some(message.clone());
                        let start = Task::done(Message::Operation(OperationEvent::Started {
                            id,
                            kind: OperationKind::ManualAction("push"),
                            label: "Preparing force push…".to_string(),
                        }));
                        let complete = Task::done(Message::Operation(OperationEvent::Completed {
                            id,
                            result: OpResult::Failed(message),
                            severity: OpSeverity::Recoverable,
                        }));
                        start.chain(complete)
                    }
                }
            }
            PushMessage::ForceWithLeaseConfirmed => {
                self.selection.force_push_confirmation = None;
                self.start_push(PushMode::ForceWithLease)
            }
            PushMessage::ForceWithLeaseCancelled => {
                self.selection.force_push_confirmation = None;
                Task::none()
            }
            PushMessage::Done { mode, result } => {
                let completion = match self
                    .operation_tracker
                    .current_id_for(&OperationKind::ManualAction("push"))
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
                self.selection.force_push_confirmation = None;
                match result {
                    Ok(()) => {
                        let status_message = self.push_success_message(mode);
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

    pub(crate) fn start_push(&mut self, mode: PushMode) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading || self.repo.head_branch.is_none() {
            return Task::none();
        }
        if matches!(mode, PushMode::ForceWithLease) && self.repo.sync_status.upstream.is_none() {
            return Task::none();
        }
        if matches!(mode, PushMode::ForceWithLease)
            && (self.repo.operation_state.is_busy() || self.repo.status_detail.is_dirty())
        {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.transient_status = None;
        self.operation.pending_transient_status_after_reload = None;
        self.operation.loading = true;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("push"),
            label: "Pushing current branch…".to_string(),
        }));
        start.chain(Task::perform(push::task::run(path, mode), move |result| {
            Message::from(PushMessage::Done { mode, result })
        }))
    }
}
