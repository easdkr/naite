use iced::widget::text_input;
use iced::Task;

use crate::features::stash::{self, Message as StashMessage, Operation};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind, StashBranchState, StashCreateState};
use crate::{features::repo_open, App, Message, StashPrompt, StashPromptAction};

impl App {
    pub(crate) fn update_stash(&mut self, message: StashMessage) -> Task<Message> {
        match message {
            StashMessage::CreateRequested => self.open_stash_create_form(),
            StashMessage::DescriptionChanged(message) => {
                self.stash_create.message = message;
                Task::none()
            }
            StashMessage::IncludeUntrackedChanged(include_untracked) => {
                self.stash_create.include_untracked = include_untracked;
                Task::none()
            }
            StashMessage::Cancelled => {
                self.stash_create.open = false;
                Task::none()
            }
            StashMessage::Submitted => self.start_stash_create(),
            StashMessage::ApplyRequested(stash) => {
                self.start_stash_operation(Operation::Apply(stash))
            }
            StashMessage::PopRequested(stash) => {
                self.selection.stash_confirmation = Some(StashPrompt {
                    action: StashPromptAction::Pop,
                    stash,
                });
                Task::none()
            }
            StashMessage::DropRequested(stash) => {
                self.selection.stash_confirmation = Some(StashPrompt {
                    action: StashPromptAction::Drop,
                    stash,
                });
                Task::none()
            }
            StashMessage::BranchRequested(stash) => self.open_stash_branch_form(stash),
            StashMessage::BranchNameChanged(name) => {
                self.stash_branch.name = name;
                Task::none()
            }
            StashMessage::BranchCancelled => {
                self.stash_branch.open = false;
                Task::none()
            }
            StashMessage::BranchSubmitted => self.start_stash_branch(),
            StashMessage::ConfirmationCancelled => {
                self.selection.stash_confirmation = None;
                Task::none()
            }
            StashMessage::Confirmed => {
                let Some(prompt) = self.selection.stash_confirmation.take() else {
                    return Task::none();
                };
                let operation = match prompt.action {
                    StashPromptAction::Pop => Operation::Pop(prompt.stash),
                    StashPromptAction::Drop => Operation::Drop(prompt.stash),
                };
                self.start_stash_operation(operation)
            }
            StashMessage::Done { operation, result } => {
                let completion = match self
                    .operation_tracker
                    .current_id_for(&OperationKind::ManualAction("stash"))
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
                        self.stash_create = StashCreateState::default();
                        self.stash_branch = StashBranchState::default();
                        self.selection.stash_confirmation = None;
                        let status_message = operation.success_message();
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

    pub(crate) fn open_stash_create_form(&mut self) -> Task<Message> {
        if self.repo.path.is_none()
            || self.operation.loading
            || !self.repo.status_detail.is_dirty()
            || !self.repo.status_detail.conflicted.is_empty()
        {
            return Task::none();
        }

        self.operation.error = None;
        self.stash_create.open = true;
        self.stash_create.message.clear();
        self.stash_create.include_untracked = false;
        text_input::focus(self.stash_create_input_id.clone())
    }

    pub(crate) fn start_stash_create(&mut self) -> Task<Message> {
        if !self.can_submit_stash_create() {
            return Task::none();
        }

        self.start_stash_operation(Operation::Create {
            message: self.stash_create.message.clone(),
            include_untracked: self.stash_create.include_untracked,
        })
    }

    pub(crate) fn open_stash_branch_form(
        &mut self,
        stash: naite_core::StashSummary,
    ) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }

        self.operation.error = None;
        self.selection.stash_confirmation = None;
        self.stash_create.open = false;
        self.stash_branch.open = true;
        self.stash_branch.name = default_stash_branch_name(&stash.selector);
        self.stash_branch.stash = Some(stash);
        text_input::focus(self.stash_branch_input_id.clone())
    }

    pub(crate) fn start_stash_branch(&mut self) -> Task<Message> {
        let branch_name = self.stash_branch.name.trim();
        if branch_name.is_empty() {
            return Task::none();
        }
        let Some(stash) = self.stash_branch.stash.clone() else {
            return Task::none();
        };

        self.start_stash_operation(Operation::Branch {
            stash,
            branch_name: branch_name.to_string(),
        })
    }

    pub(crate) fn can_submit_stash_create(&self) -> bool {
        if self.repo.path.is_none()
            || self.operation.loading
            || !self.repo.status_detail.conflicted.is_empty()
        {
            return false;
        }

        let has_tracked_changes = !self.repo.status_detail.staged.is_empty()
            || !self.repo.status_detail.unstaged.is_empty();
        let has_untracked = !self.repo.status_detail.untracked.is_empty();
        has_tracked_changes || (has_untracked && self.stash_create.include_untracked)
    }

    pub(crate) fn start_stash_operation(&mut self, operation: Operation) -> Task<Message> {
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
        let label = match &operation {
            Operation::Create { .. } => "Stashing changes…".to_string(),
            Operation::Apply(_) => "Applying stash…".to_string(),
            Operation::Pop(_) => "Popping stash…".to_string(),
            Operation::Drop(_) => "Dropping stash…".to_string(),
            Operation::Branch { branch_name, .. } => {
                format!("Creating branch {branch_name} from stash…")
            }
        };
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("stash"),
            label,
        }));
        start.chain(Task::perform(
            stash::task::run(path, operation),
            move |result| {
                Message::from(StashMessage::Done {
                    operation: operation_for_message.clone(),
                    result,
                })
            },
        ))
    }
}

fn default_stash_branch_name(selector: &str) -> String {
    let index = selector
        .strip_prefix("stash@{")
        .and_then(|value| value.strip_suffix('}'))
        .filter(|value| !value.is_empty())
        .unwrap_or("0");
    format!("stash/stash-{index}")
}

#[cfg(test)]
mod tests {
    use super::default_stash_branch_name;

    #[test]
    fn default_stash_branch_name_uses_selector_index() {
        assert_eq!(default_stash_branch_name("stash@{2}"), "stash/stash-2");
        assert_eq!(default_stash_branch_name("custom"), "stash/stash-0");
    }
}
