use iced::widget::text_input;
use iced::Task;
use naite_core::{RefKind, RefSummary};

use crate::app::remote_ref_local_branch;
use crate::features::branch_manage::{self, Message as BranchManageMessage, Operation};
use crate::message::OperationEvent;
use crate::state::{BranchManageRenameState, OpResult, OpSeverity, OperationKind};
use crate::{features::repo_open, App, BranchDeletePrompt, BranchDeleteTarget, Message};

impl App {
    pub(crate) fn update_branch_manage(&mut self, message: BranchManageMessage) -> Task<Message> {
        match message {
            BranchManageMessage::RenameRequested(target) => self.open_branch_rename_form(target),
            BranchManageMessage::RenameNameChanged(name) => {
                self.branch_manage_rename.name = name;
                Task::none()
            }
            BranchManageMessage::RenameCancelled => {
                self.branch_manage_rename = BranchManageRenameState::default();
                Task::none()
            }
            BranchManageMessage::RenameSubmitted => self.start_branch_rename(),
            BranchManageMessage::DeleteRequested(target) => self.open_branch_delete_prompt(target),
            BranchManageMessage::DeleteMatchingLocalBranchesToggled(checked) => {
                if let Some(prompt) = &mut self.selection.branch_delete_confirmation {
                    prompt.delete_matching_local_branches = checked;
                }
                Task::none()
            }
            BranchManageMessage::DeleteLinkedWorktreesToggled(checked) => {
                if let Some(prompt) = &mut self.selection.branch_delete_confirmation {
                    prompt.delete_linked_worktrees = checked;
                }
                Task::none()
            }
            BranchManageMessage::DeleteForceLinkedWorktreesToggled(checked) => {
                if let Some(prompt) = &mut self.selection.branch_delete_confirmation {
                    prompt.force_linked_worktrees = checked;
                }
                Task::none()
            }
            BranchManageMessage::DeleteCancelled => {
                self.selection.branch_delete_confirmation = None;
                Task::none()
            }
            BranchManageMessage::DeleteConfirmed => {
                let Some(prompt) = self.selection.branch_delete_confirmation.take() else {
                    return Task::none();
                };
                if !prompt.linked_worktrees.is_empty() && !prompt.delete_linked_worktrees {
                    let message = "Enable linked worktree removal before deleting this branch.";
                    let id = self.operation_tracker.next_id();
                    self.operation.error = Some(message.to_string());
                    let start = Task::done(Message::Operation(OperationEvent::Started {
                        id,
                        kind: OperationKind::ManualAction("branch_manage"),
                        label: "Validating delete…".to_string(),
                    }));
                    let complete = Task::done(Message::Operation(OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(message.to_string()),
                        severity: OpSeverity::Recoverable,
                    }));
                    self.selection.branch_delete_confirmation = Some(prompt);
                    return start.chain(complete);
                }
                self.start_branch_manage_operation(Operation::Delete {
                    target: prompt.target,
                    delete_matching_local_branches: prompt.delete_matching_local_branches,
                    delete_linked_worktrees: prompt.delete_linked_worktrees,
                    force_linked_worktrees: prompt.force_linked_worktrees,
                    linked_worktrees: prompt.linked_worktrees,
                })
            }
            BranchManageMessage::Done { operation, result } => {
                let completion = match self
                    .operation_tracker
                    .current_id_for(&OperationKind::ManualAction("branch_manage"))
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
                        self.branch_manage_rename = BranchManageRenameState::default();
                        self.selection.branch_delete_confirmation = None;
                        self.selection.context_menu = None;
                        let status_message = operation.success_message();
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.pending_transient_status_after_reload =
                                Some(status_message);
                            self.operation.loading = true;
                            let reload_start =
                                Task::done(Message::Operation(OperationEvent::Started {
                                    id: self.operation_tracker.next_id(),
                                    kind: OperationKind::Custom("repo_open".to_string()),
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
                        self.operation.error = Some(msg);
                        completion
                    }
                }
            }
        }
    }

    pub(crate) fn open_branch_rename_form(&mut self, target: RefSummary) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading || target.kind != RefKind::LocalBranch
        {
            return Task::none();
        }

        self.operation.error = None;
        self.branch_manage_rename.open = true;
        self.branch_manage_rename.name = target.short_name.clone();
        self.branch_manage_rename.target = Some(target);
        text_input::focus(self.branch_manage_input_id.clone())
    }

    pub(crate) fn open_branch_delete_prompt(
        &mut self,
        target: BranchDeleteTarget,
    ) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }

        let target = match target {
            BranchDeleteTarget::LocalBranch(target) => {
                if target.kind != RefKind::LocalBranch || target.is_head {
                    return Task::none();
                }
                BranchDeleteTarget::LocalBranch(target)
            }
            BranchDeleteTarget::LocalBranches { label, branches } => {
                let branches = branches
                    .into_iter()
                    .filter(|branch| branch.kind == RefKind::LocalBranch && !branch.is_head)
                    .collect::<Vec<_>>();
                if branches.is_empty() {
                    return Task::none();
                }
                BranchDeleteTarget::LocalBranches { label, branches }
            }
            BranchDeleteTarget::RemoteBranches { label, branches } => {
                let branches = branches
                    .into_iter()
                    .filter(|branch| {
                        branch.kind == RefKind::RemoteBranch
                            && remote_ref_local_branch(branch).is_some()
                    })
                    .collect::<Vec<_>>();
                if branches.is_empty() {
                    return Task::none();
                }
                BranchDeleteTarget::RemoteBranches { label, branches }
            }
        };
        let matching_local_branches = self.matching_local_branches_for_delete(&target);
        let linked_worktrees = self.linked_worktrees_for_delete(&target);
        self.operation.error = None;
        self.selection.branch_delete_confirmation = Some(BranchDeletePrompt {
            target,
            delete_matching_local_branches: false,
            matching_local_branches,
            delete_linked_worktrees: false,
            force_linked_worktrees: false,
            linked_worktrees,
        });
        Task::none()
    }

    pub(crate) fn start_branch_rename(&mut self) -> Task<Message> {
        if self.operation.loading || self.branch_manage_rename.name.trim().is_empty() {
            return Task::none();
        }
        let Some(target) = self.branch_manage_rename.target.clone() else {
            return Task::none();
        };

        self.start_branch_manage_operation(Operation::Rename {
            target,
            new_name: self.branch_manage_rename.name.clone(),
        })
    }

    pub(crate) fn start_branch_manage_operation(&mut self, operation: Operation) -> Task<Message> {
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
            Operation::Rename { target, .. } => format!("Renaming {}…", target.short_name),
            Operation::Delete { .. } => "Deleting branch…".to_string(),
        };
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("branch_manage"),
            label,
        }));
        start.chain(Task::perform(
            branch_manage::task::run(path, operation),
            move |result| {
                Message::from(BranchManageMessage::Done {
                    operation: operation_for_message.clone(),
                    result,
                })
            },
        ))
    }

    fn matching_local_branches_for_delete(&self, target: &BranchDeleteTarget) -> Vec<String> {
        let Some(remote_branches) = target.remote_branches() else {
            return Vec::new();
        };
        let mut names = remote_branches
            .iter()
            .filter_map(remote_ref_local_branch)
            .filter(|remote_local_name| {
                self.repo
                    .refs
                    .local
                    .iter()
                    .any(|local| local.short_name == *remote_local_name)
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    fn linked_worktrees_for_delete(
        &self,
        target: &BranchDeleteTarget,
    ) -> Vec<crate::LinkedWorktreeDeleteTarget> {
        let branch_names = match target {
            BranchDeleteTarget::LocalBranch(target) => vec![target.short_name.clone()],
            BranchDeleteTarget::LocalBranches { branches, .. } => branches
                .iter()
                .map(|branch| branch.short_name.clone())
                .collect::<Vec<_>>(),
            BranchDeleteTarget::RemoteBranches { .. } => Vec::new(),
        };
        if branch_names.is_empty() {
            return Vec::new();
        }

        self.repo
            .worktrees
            .iter()
            .filter_map(|worktree| {
                let branch = worktree.branch.as_ref()?;
                branch_names.iter().any(|name| name == branch).then(|| {
                    crate::LinkedWorktreeDeleteTarget {
                        branch: branch.clone(),
                        path: worktree.path.clone(),
                        dirty: worktree.dirty,
                    }
                })
            })
            .collect()
    }
}
