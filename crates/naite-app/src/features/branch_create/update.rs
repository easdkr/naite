use iced::widget::text_input;
use iced::Task;
use naite_core::CommitSummary;

use crate::features::branch_create::{self, Message as BranchCreateMessage};
use crate::state::{BranchCreateBase, BranchCreateState, OperationKind};
use crate::{features::repo_open, App, Message};

impl App {
    pub(crate) fn update_branch_create(&mut self, message: BranchCreateMessage) -> Task<Message> {
        match message {
            BranchCreateMessage::Requested => self.open_branch_create_form(),
            BranchCreateMessage::RequestedFromCommit(commit) => {
                self.open_branch_create_form_from_commit(commit)
            }
            BranchCreateMessage::NameChanged(name) => {
                self.branch_create.name = name;
                Task::none()
            }
            BranchCreateMessage::Cancelled => {
                self.branch_create.open = false;
                Task::none()
            }
            BranchCreateMessage::Submitted => self.start_branch_create(),
            BranchCreateMessage::Done(result) => {
                let completion = self.complete_manual_op(
                    &OperationKind::ManualAction("branch_create"),
                    result.as_ref().map(|_| ()).map_err(|e| e.clone()),
                );
                self.operation.loading = false;
                match result {
                    Ok(()) => {
                        self.branch_create = BranchCreateState::default();
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.loading = true;
                            let reload_start = self.start_manual_op(
                                OperationKind::Custom("repo_open".to_string()),
                                "Reloading repository…".to_string(),
                            );
                            completion.chain(
                                reload_start.chain(Task::perform(
                                    repo_open::task::load(path),
                                    |result| {
                                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                                    },
                                )),
                            )
                        } else {
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

    pub(crate) fn open_branch_create_form(&mut self) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }

        self.operation.error = None;
        self.branch_create.open = true;
        self.branch_create.name.clear();
        self.branch_create.base = self.current_branch_create_base();
        text_input::focus(self.branch_create_input_id.clone())
    }

    pub(crate) fn open_branch_create_form_from_commit(
        &mut self,
        commit: CommitSummary,
    ) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }

        self.operation.error = None;
        self.branch_create.open = true;
        self.branch_create.name.clear();
        self.branch_create.base = BranchCreateBase::Commit {
            id: commit.id,
            short_id: commit.short_id,
            summary: commit.summary,
        };
        text_input::focus(self.branch_create_input_id.clone())
    }

    pub(crate) fn start_branch_create(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading || self.branch_create.name.trim().is_empty() {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.loading = true;
        let label = format!("Creating branch {}…", self.branch_create.name);
        let start = self.start_manual_op(OperationKind::ManualAction("branch_create"), label);
        start.chain(Task::perform(
            branch_create::task::run(
                path,
                self.branch_create.name.clone(),
                self.branch_create.base.start_point().map(str::to_string),
            ),
            |result| Message::from(BranchCreateMessage::Done(result)),
        ))
    }
}