use iced::Task;

use crate::features::checkout::{self, Message as CheckoutMessage};
use crate::{App, CheckoutPrompt, Message};

impl App {
    pub(crate) fn update_checkout(&mut self, message: CheckoutMessage) -> Task<Message> {
        match message {
            CheckoutMessage::Requested(target) => {
                self.operation.error = None;
                if let Some(path) = self.repo.path.clone() {
                    Task::perform(checkout::task::load_status(path), move |result| {
                        Message::from(CheckoutMessage::WorktreeStatusLoaded {
                            target: target.clone(),
                            result,
                        })
                    })
                } else {
                    Task::none()
                }
            }
            CheckoutMessage::ForceSyncRequested(target) => {
                self.operation.error = None;
                self.selection.context_menu = None;
                if let Some(path) = self.repo.path.clone() {
                    Task::perform(checkout::task::load_status(path), move |result| {
                        Message::from(CheckoutMessage::ForceSyncStatusLoaded {
                            target: target.clone(),
                            result,
                        })
                    })
                } else {
                    Task::none()
                }
            }
            CheckoutMessage::Cancelled => {
                self.selection.checkout_confirmation = None;
                self.selection.force_sync_confirmation = None;
                Task::none()
            }
            CheckoutMessage::WorktreeStatusLoaded { target, result } => match result {
                Ok(status) if status.is_dirty() => {
                    self.selection.checkout_confirmation = Some(CheckoutPrompt { target, status });
                    Task::none()
                }
                Ok(_) => self.start_checkout(target, false),
                Err(msg) => {
                    self.operation.error = Some(msg);
                    Task::none()
                }
            },
            CheckoutMessage::ForceSyncStatusLoaded { target, result } => match result {
                Ok(status) => {
                    if let Some(prompt) = self.force_sync_prompt_for_remote_ref(target, status) {
                        self.selection.force_sync_confirmation = Some(prompt);
                    } else {
                        self.operation.error =
                            Some("No matching local branch for remote reset".into());
                    }
                    Task::none()
                }
                Err(msg) => {
                    self.operation.error = Some(msg);
                    Task::none()
                }
            },
            CheckoutMessage::Confirmed { target, force } => self.start_checkout(target, force),
            CheckoutMessage::ForceSyncConfirmed { target } => self.start_force_sync(target),
            CheckoutMessage::Done(result) => {
                self.selection.checkout_confirmation = None;
                self.selection.force_sync_confirmation = None;
                self.selection.context_menu = None;

                self.finish_checkout_operation(result)
            }
            CheckoutMessage::ForceSyncDone(result) => {
                self.selection.checkout_confirmation = None;
                self.selection.force_sync_confirmation = None;
                self.selection.context_menu = None;

                self.finish_checkout_operation(result)
            }
        }
    }

    pub(crate) fn start_checkout(
        &mut self,
        target: naite_core::RefSummary,
        force: bool,
    ) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        self.operation.loading = true;
        Task::perform(checkout::task::run(path, target, force), |result| {
            Message::from(CheckoutMessage::Done(result))
        })
    }

    pub(crate) fn start_force_sync(&mut self, target: naite_core::RefSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        self.operation.loading = true;
        Task::perform(checkout::task::force_sync(path, target), |result| {
            Message::from(CheckoutMessage::ForceSyncDone(result))
        })
    }

    fn finish_checkout_operation(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(()) => {
                if let Some(path) = self.repo.path.clone() {
                    self.operation.loading = true;
                    Task::perform(crate::features::repo_open::task::load(path), |result| {
                        Message::from(crate::features::repo_open::Message::Loaded(Box::new(
                            result,
                        )))
                    })
                } else {
                    Task::none()
                }
            }
            Err(msg) => {
                self.operation.loading = false;
                self.operation.error = Some(msg);
                Task::none()
            }
        }
    }
}
