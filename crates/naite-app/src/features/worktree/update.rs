use std::path::{Path, PathBuf};

use iced::widget::text_input;
use iced::Task;
use naite_core::{WorktreeAdd, WorktreeSummary};

use crate::features::{repo_open, worktree};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{App, Message, WorktreeRemovePrompt};

impl App {
    pub(crate) fn update_worktree(&mut self, message: worktree::Message) -> Task<Message> {
        match message {
            worktree::Message::Selected(summary) => {
                self.selection.selected_worktree = Some(summary.clone());
                self.selection.selected = None;
                self.selection.selected_commit_id = None;
                self.selection.selected_wip = false;
                self.selection.selected_stash = None;
                self.terminal
                    .ensure_session(summary.path.clone(), worktree_label(&summary));
                Task::none()
            }
            worktree::Message::OpenRequested(summary) => {
                self.update(repo_open::Message::OpenRecent(summary.path).into())
            }
            worktree::Message::CreateRequested => self.open_worktree_create(),
            worktree::Message::CreatePathChanged(path) => {
                self.worktree_create.path = path;
                Task::none()
            }
            worktree::Message::CreateStartPointChanged(start_point) => {
                self.worktree_create.start_point = start_point;
                Task::none()
            }
            worktree::Message::CreateBranchChanged(branch) => {
                self.worktree_create.new_branch = branch;
                Task::none()
            }
            worktree::Message::CreateCancelled => {
                self.worktree_create.open = false;
                Task::none()
            }
            worktree::Message::CreateConfirmed => self.start_worktree_create(),
            worktree::Message::CreateDone(result) => self.finish_worktree_create(result),
            worktree::Message::RemoveRequested(target) => {
                if target.is_current {
                    return self.worktree_validation_failure(
                        OperationKind::ManualAction("worktree_remove"),
                        "Cannot remove the current worktree.".to_string(),
                    );
                }
                if target.locked {
                    return self.worktree_validation_failure(
                        OperationKind::ManualAction("worktree_remove"),
                        "Unlock the worktree before removing it.".to_string(),
                    );
                }
                self.selection.worktree_remove_confirmation = Some(WorktreeRemovePrompt {
                    target,
                    delete_branch: false,
                    force: false,
                });
                Task::none()
            }
            worktree::Message::RemoveDeleteBranchToggled(delete_branch) => {
                if let Some(prompt) = &mut self.selection.worktree_remove_confirmation {
                    prompt.delete_branch = delete_branch;
                }
                Task::none()
            }
            worktree::Message::RemoveForceToggled(force) => {
                if let Some(prompt) = &mut self.selection.worktree_remove_confirmation {
                    prompt.force = force;
                }
                Task::none()
            }
            worktree::Message::RemoveCancelled => {
                self.selection.worktree_remove_confirmation = None;
                Task::none()
            }
            worktree::Message::RemoveConfirmed => self.start_worktree_remove(),
            worktree::Message::RemoveDone(result) => self.finish_worktree_remove(result),
            worktree::Message::LockRequested(target) => self.start_worktree_lock(target),
            worktree::Message::LockDone(result) => self.finish_worktree_mutation(
                &OperationKind::ManualAction("worktree_lock"),
                result,
                "Locked worktree".into(),
            ),
            worktree::Message::UnlockRequested(target) => self.start_worktree_unlock(target),
            worktree::Message::UnlockDone(result) => self.finish_worktree_mutation(
                &OperationKind::ManualAction("worktree_unlock"),
                result,
                "Unlocked worktree".into(),
            ),
        }
    }

    fn worktree_validation_failure(
        &mut self,
        kind: OperationKind,
        message: String,
    ) -> Task<Message> {
        let id = self.operation_tracker.next_id();
        self.operation.error = Some(message.clone());
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id,
            kind,
            label: "Validating worktree operation…".to_string(),
        }));
        let complete = Task::done(Message::Operation(OperationEvent::Completed {
            id,
            result: OpResult::Failed(message),
            severity: OpSeverity::Recoverable,
        }));
        start.chain(complete)
    }

    pub(crate) fn open_worktree_create(&mut self) -> Task<Message> {
        let start_point = self
            .selected_commit()
            .map(|commit| commit.id)
            .or_else(|| self.repo.head_branch.clone())
            .unwrap_or_else(|| "HEAD".into());
        self.worktree_create.open = true;
        self.worktree_create.start_point = start_point;
        if self.worktree_create.path.trim().is_empty() {
            self.worktree_create.path = default_worktree_path(self.repo.path.as_deref());
        }
        text_input::focus(self.worktree_path_input_id.clone())
    }

    fn start_worktree_create(&mut self) -> Task<Message> {
        let Some(repo_path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        let path = self.worktree_create.path.trim();
        let start_point = self.worktree_create.start_point.trim();
        if path.is_empty() || start_point.is_empty() {
            return self.worktree_validation_failure(
                OperationKind::ManualAction("worktree_create"),
                "Enter a worktree path and start point.".to_string(),
            );
        }

        let add = WorktreeAdd {
            path: PathBuf::from(path),
            start_point: start_point.to_string(),
            new_branch: (!self.worktree_create.new_branch.trim().is_empty())
                .then(|| self.worktree_create.new_branch.trim().to_string()),
        };
        self.operation.loading = true;
        self.operation.error = None;
        let label = format!("Creating worktree at {}…", path);
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("worktree_create"),
            label,
        }));
        start.chain(Task::perform(
            worktree::task::add(repo_path, add),
            |result| Message::from(worktree::Message::CreateDone(result)),
        ))
    }

    fn finish_worktree_create(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        let kind = OperationKind::ManualAction("worktree_create");
        let completion = match self.operation_tracker.current_id_for(&kind) {
            Some(id) => {
                let event = match &result {
                    Ok(_) => OperationEvent::Completed {
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
            Ok(path) => {
                self.worktree_create.open = false;
                self.set_transient_status(format!("Created worktree at {}", path.display()));
                completion.chain(self.reload_current_repo())
            }
            Err(msg) => {
                self.operation.error = Some(msg);
                completion
            }
        }
    }

    fn start_worktree_remove(&mut self) -> Task<Message> {
        let Some(repo_path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(prompt) = self.selection.worktree_remove_confirmation.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("worktree_remove"),
            label: "Removing worktree…".to_string(),
        }));
        start.chain(Task::perform(
            worktree::task::remove(
                repo_path,
                prompt.target.path,
                prompt.delete_branch,
                prompt.force,
            ),
            |result| Message::from(worktree::Message::RemoveDone(result)),
        ))
    }

    fn finish_worktree_remove(&mut self, result: Result<(), String>) -> Task<Message> {
        let kind = OperationKind::ManualAction("worktree_remove");
        let needs_force_confirmation = result.as_ref().is_err_and(|message| {
            worktree_remove_requires_force(message)
                && self
                    .selection
                    .worktree_remove_confirmation
                    .as_ref()
                    .is_some_and(|prompt| !prompt.force)
        });

        if needs_force_confirmation {
            self.operation.loading = false;
            self.operation.error = None;
            if let Some(prompt) = &mut self.selection.worktree_remove_confirmation {
                prompt.force = true;
            }
            self.set_transient_status(
                "Worktree has modified or untracked files. Confirm force removal to delete them."
                    .into(),
            );

            return self
                .operation_tracker
                .current_id_for(&kind)
                .map(|id| Task::done(Message::Operation(OperationEvent::Cancelled { id })))
                .unwrap_or_else(Task::none);
        }

        self.finish_worktree_mutation(&kind, result, "Removed worktree".into())
    }

    fn start_worktree_lock(&mut self, target: WorktreeSummary) -> Task<Message> {
        let Some(repo_path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("worktree_lock"),
            label: "Locking worktree…".to_string(),
        }));
        start.chain(Task::perform(
            worktree::task::lock(repo_path, target.path, "Locked by naite".into()),
            |result| Message::from(worktree::Message::LockDone(result)),
        ))
    }

    fn start_worktree_unlock(&mut self, target: WorktreeSummary) -> Task<Message> {
        let Some(repo_path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("worktree_unlock"),
            label: "Unlocking worktree…".to_string(),
        }));
        start.chain(Task::perform(
            worktree::task::unlock(repo_path, target.path),
            |result| Message::from(worktree::Message::UnlockDone(result)),
        ))
    }

    fn finish_worktree_mutation(
        &mut self,
        kind: &OperationKind,
        result: Result<(), String>,
        success_message: String,
    ) -> Task<Message> {
        let completion = match self.operation_tracker.current_id_for(kind) {
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
        self.selection.worktree_remove_confirmation = None;
        match result {
            Ok(()) => {
                self.set_transient_status(success_message);
                completion.chain(self.reload_current_repo())
            }
            Err(msg) => {
                self.operation.error = Some(msg);
                completion
            }
        }
    }

    pub(crate) fn reload_current_repo(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        self.operation.loading = true;
        let reload_start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::RepositoryLoad,
            label: "Reloading repository…".to_string(),
        }));
        reload_start.chain(Task::perform(repo_open::task::load(path), |result| {
            Message::from(repo_open::Message::Loaded(Box::new(result)))
        }))
    }
}

fn default_worktree_path(repo_path: Option<&Path>) -> String {
    let Some(repo_path) = repo_path else {
        return String::new();
    };
    let name = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("worktree");
    repo_path
        .parent()
        .map(|parent| parent.join(format!("{name}-worktree")))
        .unwrap_or_else(|| repo_path.with_file_name(format!("{name}-worktree")))
        .display()
        .to_string()
}

fn worktree_remove_requires_force(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("contains modified or untracked files")
}

fn worktree_label(summary: &WorktreeSummary) -> String {
    summary
        .branch
        .clone()
        .or_else(|| {
            (!summary.head_short_id.is_empty()).then(|| format!("HEAD {}", summary.head_short_id))
        })
        .unwrap_or_else(|| "Worktree".into())
}
