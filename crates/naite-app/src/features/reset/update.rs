use iced::Task;
use naite_core::{CommitSummary, ResetMode};

use crate::features::repo_open;
use crate::features::reset::{self, Message as ResetMessage};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{App, Message, ResetPrompt};

impl App {
    pub(crate) fn update_reset(&mut self, message: ResetMessage) -> Task<Message> {
        match message {
            ResetMessage::Requested(commit) => self.open_reset_prompt(commit),
            ResetMessage::Cancelled => {
                self.selection.reset_confirmation = None;
                Task::none()
            }
            ResetMessage::Confirmed(mode) => self.start_reset(mode),
            ResetMessage::Done {
                commit,
                mode,
                result,
            } => self.finish_reset(commit, mode, result),
        }
    }

    fn open_reset_prompt(&mut self, commit: CommitSummary) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }
        self.operation.error = None;
        self.selection.context_menu = None;
        self.selection.reset_confirmation = Some(ResetPrompt { target: commit });
        Task::none()
    }

    fn start_reset(&mut self, mode: ResetMode) -> Task<Message> {
        let Some(prompt) = self.selection.reset_confirmation.take() else {
            return Task::none();
        };
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
        let commit = prompt.target;
        let commit_for_message = commit.clone();
        let label = format!("Resetting to {}…", commit.short_id);
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("reset"),
            label,
        }));
        start.chain(Task::perform(
            reset::task::run(path, commit.id.clone(), mode),
            move |result| {
                Message::from(ResetMessage::Done {
                    commit: commit_for_message.clone(),
                    mode,
                    result,
                })
            },
        ))
    }

    fn finish_reset(
        &mut self,
        commit: CommitSummary,
        mode: ResetMode,
        result: Result<(), String>,
    ) -> Task<Message> {
        let completion = match self
            .operation_tracker
            .current_id_for(&OperationKind::ManualAction("reset"))
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
                let label = match mode {
                    ResetMode::Soft => "soft",
                    ResetMode::Mixed => "mixed",
                    ResetMode::Hard => "hard",
                };
                let status_message = format!("Reset --{label} to {}", commit.short_id);
                if let Some(path) = self.repo.path.clone() {
                    self.operation.pending_transient_status_after_reload = Some(status_message);
                    self.operation.loading = true;
                    let reload_start = Task::done(Message::Operation(OperationEvent::Started {
                        id: self.operation_tracker.next_id(),
                        kind: OperationKind::ManualAction("repo_open"),
                        label: "Reloading repository…".to_string(),
                    }));
                    completion.chain(
                        reload_start.chain(Task::perform(repo_open::task::load(path), |result| {
                            Message::from(repo_open::Message::Loaded(Box::new(result)))
                        })),
                    )
                } else {
                    self.set_transient_status(status_message);
                    completion
                }
            }
            Err(msg) => {
                self.operation.error = Some(msg);
                completion
            }
        }
    }
}
