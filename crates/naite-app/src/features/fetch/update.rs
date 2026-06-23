use std::time::{Duration, Instant};

use iced::Task;

use crate::features::fetch::{self, FetchScope, Message as FetchMessage};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind, ReleasePrepPhase};
use crate::{features::repo_open, App, Message};

const AUTO_FETCH_MIN_INTERVAL: Duration = Duration::from_secs(60);

impl App {
    pub(crate) fn update_fetch(&mut self, message: FetchMessage) -> Task<Message> {
        match message {
            FetchMessage::Requested(scope) => self.start_fetch(scope),
            FetchMessage::AutoDone { path, result } => {
                if self.operation.auto_fetch_path.as_ref() != Some(&path) {
                    return Task::none();
                }
                self.operation.auto_fetch_path = None;
                let completion = self.complete_auto_fetch(&result);
                if self.release_prep.auto_running {
                    return completion.chain(self.continue_release_prep_auto());
                }
                let next_task = if result.is_ok() && self.repo.path.as_ref() == Some(&path) {
                    if self.operation.loading {
                        Task::none()
                    } else {
                        self.spawn_tab_refresh(path)
                    }
                } else if self.repo.path.as_ref() != Some(&path) {
                    self.start_auto_fetch()
                } else {
                    self.continue_release_prep_auto()
                };
                completion.chain(next_task)
            }
            FetchMessage::Done { scope, result } => {
                self.operation.loading = false;
                match result {
                    Ok(()) => {
                        let status_message = self.fetch_success_message(scope);
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.pending_transient_status_after_reload =
                                Some(status_message);
                            self.operation.loading = true;
                            Task::perform(repo_open::task::load(path), |result| {
                                Message::from(repo_open::Message::Loaded(Box::new(result)))
                            })
                        } else {
                            self.set_transient_status(status_message);
                            Task::none()
                        }
                    }
                    Err(msg) => {
                        self.operation.pending_transient_status_after_reload = None;
                        self.operation.error = Some(msg);
                        Task::none()
                    }
                }
            }
        }
    }

    pub(crate) fn start_fetch(&mut self, scope: FetchScope) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading
            || self.operation.auto_fetch_path.is_some()
            || (scope == FetchScope::CurrentRemote && self.repo.sync_status.upstream.is_none())
        {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.transient_status = None;
        self.operation.pending_transient_status_after_reload = None;
        self.operation.loading = true;
        Task::perform(fetch::task::run(path, scope), move |result| {
            Message::from(FetchMessage::Done { scope, result })
        })
    }

    pub(crate) fn start_auto_fetch(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading
            || self.operation.auto_fetch_path.is_some()
            || self.release_prep.phase != ReleasePrepPhase::Idle
            || self.release_prep.auto_running
            || self.repo.sync_status.upstream.is_none()
            || self.operation.auto_fetch_last_started.as_ref().is_some_and(
                |(last_path, started_at)| {
                    last_path == &path && started_at.elapsed() < AUTO_FETCH_MIN_INTERVAL
                },
            )
        {
            return Task::none();
        }

        let path_for_message = path.clone();
        self.operation.auto_fetch_path = Some(path_for_message.clone());
        self.operation.auto_fetch_last_started = Some((path_for_message.clone(), Instant::now()));
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::AutoFetch,
            label: "Fetching (auto)...".to_string(),
        }));
        start.chain(Task::perform(
            fetch::task::run(path, FetchScope::CurrentRemote),
            move |result| {
                Message::from(FetchMessage::AutoDone {
                    path: path_for_message.clone(),
                    result,
                })
            },
        ))
    }

    fn complete_auto_fetch(&mut self, result: &Result<(), String>) -> Task<Message> {
        let Some(id) = self
            .operation_tracker
            .current_id_for(&OperationKind::AutoFetch)
        else {
            return Task::none();
        };
        let event = match result {
            Ok(()) => OperationEvent::Completed {
                id,
                result: OpResult::Success,
                severity: OpSeverity::Recoverable,
            },
            Err(msg) => OperationEvent::Completed {
                id,
                result: OpResult::Failed(msg.clone()),
                severity: OpSeverity::Recoverable,
            },
        };
        Task::done(Message::Operation(event))
    }
}
