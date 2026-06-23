use iced::Task;

use crate::features::catalog::{self, Message as CatalogMessage};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{App, Message};

impl App {
    pub(crate) fn update_catalog(&mut self, message: CatalogMessage) -> Task<Message> {
        match message {
            CatalogMessage::Loaded(Ok(mut catalog)) => {
                let mut should_save = false;
                if let Some(path) = self.repo.path.clone() {
                    catalog.remember(path);
                    should_save = true;
                }
                self.catalog = catalog;
                let workspace_task = self.refresh_workspace();
                if should_save {
                    Task::batch([self.save_catalog(), workspace_task])
                } else {
                    workspace_task
                }
            }
            CatalogMessage::Loaded(Err(msg)) => {
                let id = self.operation_tracker.next_id();
                self.operation.error = Some(msg.clone());
                let start = Task::done(Message::Operation(OperationEvent::Started {
                    id,
                    kind: OperationKind::ManualAction("catalog_load"),
                    label: "Loading catalog…".to_string(),
                }));
                let complete = Task::done(Message::Operation(OperationEvent::Completed {
                    id,
                    result: OpResult::Failed(msg),
                    severity: OpSeverity::Recoverable,
                }));
                start.chain(complete)
            }
            CatalogMessage::Saved(Ok(())) => Task::none(),
            CatalogMessage::Saved(Err(msg)) => {
                let id = self.operation_tracker.next_id();
                self.operation.error = Some(msg.clone());
                let start = Task::done(Message::Operation(OperationEvent::Started {
                    id,
                    kind: OperationKind::ManualAction("catalog_save"),
                    label: "Saving catalog…".to_string(),
                }));
                let complete = Task::done(Message::Operation(OperationEvent::Completed {
                    id,
                    result: OpResult::Failed(msg),
                    severity: OpSeverity::Recoverable,
                }));
                start.chain(complete)
            }
        }
    }

    pub(crate) fn save_catalog(&self) -> Task<Message> {
        let catalog = self.catalog.clone();
        Task::perform(catalog::task::save(catalog), |result| {
            Message::from(CatalogMessage::Saved(result))
        })
    }
}
