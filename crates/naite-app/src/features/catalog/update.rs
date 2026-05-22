use iced::Task;

use crate::features::catalog::{self, Message as CatalogMessage};
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
                self.operation.error = Some(msg);
                Task::none()
            }
            CatalogMessage::Saved(Ok(())) => Task::none(),
            CatalogMessage::Saved(Err(msg)) => {
                self.operation.error = Some(msg);
                Task::none()
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
