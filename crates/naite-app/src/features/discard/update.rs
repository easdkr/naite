use iced::Task;
use naite_core::WorktreeDiffKind;

use crate::features::discard::{self, Message as DiscardMessage};
use crate::{App, DiscardPrompt, DiscardTarget, Message};

impl App {
    pub(crate) fn update_discard(&mut self, message: DiscardMessage) -> Task<Message> {
        match message {
            DiscardMessage::FileRequested(target) => {
                if self.operation.loading || target.kind == WorktreeDiffKind::Staged {
                    return Task::none();
                }
                self.selection.discard_confirmation = Some(DiscardPrompt {
                    target: DiscardTarget::File(target),
                });
                Task::none()
            }
            DiscardMessage::HunkRequested { path, hunk } => {
                if self.operation.loading {
                    return Task::none();
                }
                self.selection.discard_confirmation = Some(DiscardPrompt {
                    target: DiscardTarget::Hunk { path, hunk },
                });
                Task::none()
            }
            DiscardMessage::Cancelled => {
                self.selection.discard_confirmation = None;
                Task::none()
            }
            DiscardMessage::Confirmed => {
                let Some(prompt) = self.selection.discard_confirmation.clone() else {
                    return Task::none();
                };
                self.start_discard_operation(prompt.target)
            }
            DiscardMessage::Done(result) => {
                self.operation.loading = false;
                self.selection.discard_confirmation = None;
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

    pub(crate) fn start_discard_operation(&mut self, target: DiscardTarget) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        self.operation.error = None;
        self.operation.loading = true;
        Task::perform(discard::task::run(path, target), |result| {
            Message::from(DiscardMessage::Done(result))
        })
    }
}
