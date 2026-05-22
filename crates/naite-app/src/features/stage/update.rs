use iced::Task;

use crate::features::stage::{self, Message as StageMessage, Operation};
use crate::{App, Message};

impl App {
    pub(crate) fn update_stage(&mut self, message: StageMessage) -> Task<Message> {
        match message {
            StageMessage::StatusPath(path) => {
                self.start_stage_operation(Operation::StagePath(path))
            }
            StageMessage::UnstageStatusPath(path) => {
                self.start_stage_operation(Operation::UnstagePath(path))
            }
            StageMessage::HunkRequested { path, hunk } => {
                self.start_stage_operation(Operation::StageHunk { path, hunk })
            }
            StageMessage::UnstageHunkRequested { path, hunk } => {
                self.start_stage_operation(Operation::UnstageHunk { path, hunk })
            }
            StageMessage::All => self.start_stage_operation(Operation::StageAll),
            StageMessage::UnstageAll => self.start_stage_operation(Operation::UnstageAll),
            StageMessage::Done(result) => {
                self.operation.loading = false;
                match result {
                    Ok(status_detail) => {
                        let previous_target = self.selection.selected_wip_file.clone();
                        self.repo.status_detail = status_detail;
                        if !self.repo.status_detail.is_dirty() {
                            self.clear_dirty_selection();
                        }
                        self.operation.error = None;
                        if self.selection.selected_wip {
                            return self.select_wip_after_status_update(previous_target.as_ref());
                        }
                    }
                    Err(msg) => {
                        self.operation.error = Some(msg);
                    }
                }
                Task::none()
            }
        }
    }

    pub(crate) fn start_stage_operation(&mut self, operation: Operation) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        self.operation.error = None;
        self.operation.loading = true;
        Task::perform(stage::task::run(path, operation), |result| {
            Message::from(StageMessage::Done(result))
        })
    }
}
