use iced::widget::text_input;
use iced::Task;

use crate::features::history::{self, Message as HistoryMessage, Operation};
use crate::features::repo_open;
use crate::state::{HistoryRewordState, OperationKind, UndoCheckpoint};
use crate::{App, HistoryPrompt, Message, UndoPrompt, UndoPromptAction};

impl App {
    pub(crate) fn update_history(&mut self, message: HistoryMessage) -> Task<Message> {
        match message {
            HistoryMessage::Requested(operation) => self.open_history_prompt(operation),
            HistoryMessage::Confirmed => {
                let Some(prompt) = self.selection.history_confirmation.take() else {
                    return Task::none();
                };
                self.start_history_operation(prompt.operation)
            }
            HistoryMessage::Cancelled => {
                self.selection.history_confirmation = None;
                Task::none()
            }
            HistoryMessage::RewordRequested(commit) => self.open_history_reword_form(commit),
            HistoryMessage::RewordTitleChanged(title) => {
                self.history_reword.title = title;
                Task::none()
            }
            HistoryMessage::RewordBodyAction(action) => {
                self.history_reword.body_content.perform(action);
                Task::none()
            }
            HistoryMessage::RewordFormLoaded(result) => {
                self.history_reword.loading = false;
                if let Ok(message) = result {
                    // Title was pre-filled from commit.summary for instant
                    // feedback; the loaded body is the one round-trip we
                    // actually need so the user does not lose existing
                    // body content on reword. Only seed if the user has
                    // not already typed during the load window.
                    if self.history_reword.body_content.text().trim().is_empty() {
                        self.history_reword.body_content =
                            iced::widget::text_editor::Content::with_text(&message.body);
                    }
                }
                Task::none()
            }
            HistoryMessage::RewordCancelled => {
                self.history_reword = HistoryRewordState::default();
                Task::none()
            }
            HistoryMessage::RewordSubmitted => self.submit_history_reword(),
            HistoryMessage::UndoRequested => self.open_undo_prompt(UndoPromptAction::Undo),
            HistoryMessage::RedoRequested => self.open_undo_prompt(UndoPromptAction::Redo),
            HistoryMessage::UndoConfirmed => {
                let Some(prompt) = self.selection.undo_confirmation.take() else {
                    return Task::none();
                };
                self.start_undo_operation(prompt)
            }
            HistoryMessage::UndoCancelled => {
                self.selection.undo_confirmation = None;
                Task::none()
            }
            HistoryMessage::Done {
                operation,
                checkpoint,
                head_before_reset,
                result,
            } => self.finish_history_operation(operation, checkpoint, head_before_reset, result),
        }
    }

    pub(crate) fn open_history_prompt(&mut self, operation: Operation) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }
        self.operation.error = None;
        self.selection.context_menu = None;
        self.selection.history_confirmation = Some(HistoryPrompt { operation });
        Task::none()
    }

    pub(crate) fn open_history_reword_form(
        &mut self,
        commit: naite_core::CommitSummary,
    ) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }
        let commit_id = commit.id.clone();
        self.selection.context_menu = None;
        self.history_reword.open = true;
        self.history_reword.loading = true;
        self.history_reword.title = commit.summary.clone();
        self.history_reword.body_content = iced::widget::text_editor::Content::new();
        self.history_reword.commit = Some(commit);
        Task::batch([
            text_input::focus(self.history_reword_input_id.clone()),
            Task::perform(
                super::task::load_commit_message(path, commit_id),
                |result| Message::from(HistoryMessage::RewordFormLoaded(result)),
            ),
        ])
    }

    pub(crate) fn submit_history_reword(&mut self) -> Task<Message> {
        if self.operation.loading || self.history_reword.title.trim().is_empty() {
            return Task::none();
        }
        let Some(commit) = self.history_reword.commit.clone() else {
            return Task::none();
        };
        let message = compose_reword_message(
            &self.history_reword.title,
            &self.history_reword.body_content.text(),
        );
        self.open_history_prompt(Operation::Reword { commit, message })
    }

    pub(crate) fn open_undo_prompt(&mut self, action: UndoPromptAction) -> Task<Message> {
        let checkpoint = match action {
            UndoPromptAction::Undo => self.undo_stack.last().cloned(),
            UndoPromptAction::Redo => self.redo_stack.last().cloned(),
        };
        let Some(checkpoint) = checkpoint else {
            return Task::none();
        };
        self.selection.undo_confirmation = Some(UndoPrompt { action, checkpoint });
        Task::none()
    }

    pub(crate) fn start_undo_operation(&mut self, prompt: UndoPrompt) -> Task<Message> {
        let head_before_reset = self.current_undo_checkpoint("redo reset");
        match prompt.action {
            UndoPromptAction::Undo => {
                self.undo_stack.pop();
                self.start_history_operation_with_checkpoint(
                    Operation::Undo(prompt.checkpoint),
                    None,
                    head_before_reset,
                )
            }
            UndoPromptAction::Redo => {
                self.redo_stack.pop();
                self.start_history_operation_with_checkpoint(
                    Operation::Redo(prompt.checkpoint),
                    None,
                    head_before_reset,
                )
            }
        }
    }

    pub(crate) fn start_history_operation(&mut self, operation: Operation) -> Task<Message> {
        let checkpoint = operation
            .undo_label()
            .and_then(|label| self.current_undo_checkpoint(&label));
        self.start_history_operation_with_checkpoint(operation, checkpoint, None)
    }

    fn start_history_operation_with_checkpoint(
        &mut self,
        operation: Operation,
        checkpoint: Option<UndoCheckpoint>,
        head_before_reset: Option<UndoCheckpoint>,
    ) -> Task<Message> {
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
        let operation_for_message = operation.clone();
        let label = operation.title().to_string();
        let start = self.start_manual_op(OperationKind::ManualAction("history"), label);
        start.chain(Task::perform(history::task::run(path, operation), move |result| {
            Message::from(HistoryMessage::Done {
                operation: operation_for_message.clone(),
                checkpoint: checkpoint.clone(),
                head_before_reset: head_before_reset.clone(),
                result,
            })
        }))
    }

    fn finish_history_operation(
        &mut self,
        operation: Operation,
        checkpoint: Option<UndoCheckpoint>,
        head_before_reset: Option<UndoCheckpoint>,
        result: Result<(), String>,
    ) -> Task<Message> {
        let completion = self.complete_manual_op(
            &OperationKind::ManualAction("history"),
            result.as_ref().map(|_| ()).map_err(|e| e.clone()),
        );
        self.operation.loading = false;
        match result {
            Ok(()) => {
                self.history_reword = HistoryRewordState::default();
                self.selection.history_confirmation = None;
                self.selection.undo_confirmation = None;
                match &operation {
                    Operation::Undo(done) => {
                        if let Some(mut redo) = head_before_reset {
                            redo.label = done.label.clone();
                            self.redo_stack.push(redo);
                        }
                    }
                    Operation::Redo(done) => {
                        if let Some(mut undo) = head_before_reset {
                            undo.label = done.label.clone();
                            self.undo_stack.push(undo);
                        }
                    }
                    _ => {
                        if let Some(checkpoint) = checkpoint {
                            self.undo_stack.push(checkpoint);
                            self.redo_stack.clear();
                        }
                    }
                }

                let status_message = operation.success_message();
                if let Some(path) = self.repo.path.clone() {
                    self.operation.pending_transient_status_after_reload = Some(status_message);
                    self.operation.loading = true;
                    let reload_start = self.start_manual_op(
                        OperationKind::Custom("repo_open".to_string()),
                        "Reloading repository…".to_string(),
                    );
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

    fn current_undo_checkpoint(&self, label: &str) -> Option<UndoCheckpoint> {
        self.repo.commits.first().map(|head| UndoCheckpoint {
            label: label.to_string(),
            head_id: head.id.clone(),
        })
    }
}

/// Compose a title and optional body into a single commit message string.
/// Trims whitespace on both sides; if the body is empty, returns just the
/// title so `validate_reword_message` does not see trailing blank lines.
fn compose_reword_message(title: &str, body: &str) -> String {
    let title = title.trim();
    let body = body.trim();
    if body.is_empty() {
        title.to_string()
    } else {
        format!("{title}\n\n{body}")
    }
}
