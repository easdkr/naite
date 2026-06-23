use iced::Task;

use crate::features::{repo_open, workspace};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{App, Message};

impl App {
    pub(crate) fn update_workspace(&mut self, message: workspace::Message) -> Task<Message> {
        match message {
            workspace::Message::DashboardToggled => {
                self.workspace.dashboard_open = !self.workspace.dashboard_open;
                self.manager.new_repo_menu_open = false;
                Task::none()
            }
            workspace::Message::RefreshRequested => self.refresh_workspace(),
            workspace::Message::Loaded(summaries) => {
                self.workspace.loading = false;
                self.workspace.error = None;
                self.workspace.summaries = summaries;
                Task::none()
            }
            workspace::Message::FetchAllRequested => self.start_workspace_fetch_all(),
            workspace::Message::FetchAllDone(summary) => self.finish_workspace_operation(
                "workspace_fetch_all",
                summary,
                "Fetched workspace repositories".into(),
            ),
            workspace::Message::PullAllRequested => self.start_workspace_pull_all(),
            workspace::Message::PullAllDone(summary) => self.finish_workspace_operation(
                "workspace_pull_all",
                summary,
                "Pulled workspace repositories".into(),
            ),
            workspace::Message::OpenRepo(path) => {
                self.workspace.dashboard_open = false;
                self.update(repo_open::Message::OpenRecent(path).into())
            }
            workspace::Message::LocateRepo(path) => {
                Task::perform(workspace::task::locate(path), |result| {
                    Message::from(workspace::Message::LocateDone(result))
                })
            }
            workspace::Message::LocateDone(Ok(())) => {
                self.set_transient_status("Revealed repository in Finder".into());
                Task::none()
            }
            workspace::Message::LocateDone(Err(msg)) => {
                let id = self.operation_tracker.next_id();
                self.operation.error = Some(msg.clone());
                let start = Task::done(Message::Operation(OperationEvent::Started {
                    id,
                    kind: OperationKind::Custom("workspace_locate".to_string()),
                    label: "Revealing repository in Finder…".to_string(),
                }));
                let complete = Task::done(Message::Operation(OperationEvent::Completed {
                    id,
                    result: OpResult::Failed(msg),
                    severity: OpSeverity::Recoverable,
                }));
                start.chain(complete)
            }
            workspace::Message::RemoveRepo(path) => {
                self.catalog.remove_entry(&path);
                self.workspace
                    .summaries
                    .retain(|summary| summary.path != path);
                Task::batch([self.save_catalog(), self.refresh_workspace()])
            }
        }
    }

    pub(crate) fn refresh_workspace(&mut self) -> Task<Message> {
        let paths = self
            .catalog
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        if paths.is_empty() {
            self.workspace.loading = false;
            self.workspace.summaries.clear();
            self.workspace.error = None;
            return Task::none();
        }

        self.workspace.loading = true;
        self.workspace.error = None;
        Task::perform(workspace::task::load(paths), |summaries| {
            Message::from(workspace::Message::Loaded(summaries))
        })
    }

    fn start_workspace_fetch_all(&mut self) -> Task<Message> {
        let paths = self.workspace_paths();
        if paths.is_empty() || self.operation.loading {
            return Task::none();
        }
        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("workspace_fetch_all"),
            label: format!("Fetching {} repositories…", paths.len()),
        }));
        start.chain(Task::perform(
            workspace::task::fetch_all(paths),
            |summary| Message::from(workspace::Message::FetchAllDone(summary)),
        ))
    }

    fn start_workspace_pull_all(&mut self) -> Task<Message> {
        let paths = self.workspace_paths();
        if paths.is_empty() || self.operation.loading {
            return Task::none();
        }
        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("workspace_pull_all"),
            label: format!("Pulling {} repositories…", paths.len()),
        }));
        start.chain(Task::perform(workspace::task::pull_all(paths), |summary| {
            Message::from(workspace::Message::PullAllDone(summary))
        }))
    }

    fn finish_workspace_operation(
        &mut self,
        kind_label: &'static str,
        summary: workspace::MultiRepoOperationSummary,
        success_message: String,
    ) -> Task<Message> {
        let kind = OperationKind::ManualAction(kind_label);
        self.operation.loading = false;
        let completion = match self.operation_tracker.current_id_for(&kind) {
            Some(id) => {
                let event = if summary.failures.is_empty() {
                    OperationEvent::Completed {
                        id,
                        result: OpResult::Success,
                        severity: OpSeverity::Recoverable,
                    }
                } else {
                    let failures = summary
                        .failures
                        .iter()
                        .map(|(path, err)| format!("{}: {err}", path.display()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    let message = format!(
                        "{success_message}: {} succeeded, {} failed. {failures}",
                        summary.succeeded,
                        summary.failures.len()
                    );
                    self.operation.error = Some(message.clone());
                    OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(message),
                        severity: OpSeverity::Recoverable,
                    }
                };
                Task::done(Message::Operation(event))
            }
            None => Task::none(),
        };
        if summary.failures.is_empty() {
            self.set_transient_status(format!("{success_message}: {}", summary.succeeded));
        }
        completion.chain(self.refresh_workspace())
    }

    fn workspace_paths(&self) -> Vec<std::path::PathBuf> {
        self.workspace
            .summaries
            .iter()
            .filter(|summary| summary.error.is_none())
            .map(|summary| summary.path.clone())
            .collect()
    }
}
