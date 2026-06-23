use std::path::{Path, PathBuf};

use iced::widget::text_input;
use iced::Task;
use naite_core::{
    CheckoutPullRequestOptions, CreatePullRequestOptions, ListPullRequestsOptions,
    PullRequestFilter, PullRequestSummary,
};

use crate::features::pull_request;
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{App, Message};

impl App {
    pub(crate) fn update_pull_request(&mut self, message: pull_request::Message) -> Task<Message> {
        match message {
            pull_request::Message::RefreshRequested => self.refresh_pull_requests(),
            pull_request::Message::FilterChanged(filter) => {
                if filter == PullRequestFilter::Search {
                    return self.update_pull_request(pull_request::Message::SearchSubmitted);
                }
                self.pull_requests.filter = filter;
                self.pull_requests.last_non_search_filter = filter;
                self.refresh_pull_requests()
            }
            pull_request::Message::SearchQueryChanged(query) => {
                let cleared = query.trim().is_empty();
                self.pull_requests.search_query = query;
                if cleared && self.pull_requests.filter == PullRequestFilter::Search {
                    self.pull_requests.filter = self.pull_requests.last_non_search_filter;
                    self.pull_requests.error = None;
                    return self.refresh_pull_requests();
                }
                Task::none()
            }
            pull_request::Message::SearchSubmitted => {
                if self.pull_requests.search_query.trim().is_empty() {
                    self.pull_requests.filter = self.pull_requests.last_non_search_filter;
                    self.pull_requests.error = None;
                    return self.refresh_pull_requests();
                }
                self.pull_requests.filter = PullRequestFilter::Search;
                self.refresh_pull_requests()
            }
            pull_request::Message::Loaded {
                filter,
                search_query,
                result,
            } => {
                if self.pull_requests.filter != filter
                    || (filter == PullRequestFilter::Search
                        && Some(self.pull_requests.search_query.trim()) != search_query.as_deref())
                {
                    return Task::none();
                }
                self.pull_requests.loading = false;
                match result {
                    Ok(items) => {
                        self.repo.pull_requests = items;
                        self.pull_requests.error = None;
                        self.sync_selected_pull_request();
                    }
                    Err(msg) => {
                        self.repo.pull_requests.clear();
                        self.selection.selected_pull_request = None;
                        self.selection.selected_github_issue = None;
                        self.pull_requests.error = Some(msg);
                    }
                }
                Task::none()
            }
            pull_request::Message::Selected(pull_request) => {
                let avatar_task =
                    self.maybe_fetch_avatar(pull_request.author_avatar_url.as_deref());
                self.select_pull_request(pull_request);
                avatar_task
            }
            pull_request::Message::CreateRequested => self.open_pull_request_create(),
            pull_request::Message::CreateBaseChanged(base) => {
                self.pull_requests.create.base_branch = base;
                Task::none()
            }
            pull_request::Message::CreateDraftChanged(draft) => {
                self.pull_requests.create.draft = draft;
                Task::none()
            }
            pull_request::Message::CreateCancelled => {
                self.pull_requests.create.open = false;
                Task::none()
            }
            pull_request::Message::CreateSubmitted => self.start_pull_request_create(),
            pull_request::Message::CreateDone(result) => {
                let kind = OperationKind::ManualAction("pull_request_create");
                let completion = match self.operation_tracker.current_id_for(&kind) {
                    Some(id) => {
                        let event = match &result {
                            Ok(url) => {
                                self.pull_requests.create.open = false;
                                self.set_transient_status(format!("Created pull request: {url}"));
                                OperationEvent::Completed {
                                    id,
                                    result: OpResult::Success,
                                    severity: OpSeverity::Recoverable,
                                }
                            }
                            Err(message) => {
                                self.operation.error = Some(message.clone());
                                OperationEvent::Completed {
                                    id,
                                    result: OpResult::Failed(message.clone()),
                                    severity: OpSeverity::Recoverable,
                                }
                            }
                        };
                        Task::done(Message::Operation(event))
                    }
                    None => Task::none(),
                };
                self.operation.loading = false;
                if result.is_ok() {
                    completion.chain(self.refresh_pull_requests())
                } else {
                    completion
                }
            }
            pull_request::Message::CheckoutRequested(pull_request) => {
                self.start_pull_request_checkout(pull_request)
            }
            pull_request::Message::CheckoutWorktreeRequested(pull_request) => {
                self.open_pull_request_worktree_checkout(pull_request)
            }
            pull_request::Message::CheckoutWorktreePathChanged(path) => {
                self.pull_requests.checkout_worktree.path = path;
                Task::none()
            }
            pull_request::Message::CheckoutWorktreeBranchChanged(branch) => {
                self.pull_requests.checkout_worktree.branch_name = branch;
                Task::none()
            }
            pull_request::Message::CheckoutWorktreeCancelled => {
                self.pull_requests.checkout_worktree.open = false;
                Task::none()
            }
            pull_request::Message::CheckoutWorktreeSubmitted => {
                self.start_pull_request_worktree_checkout()
            }
            pull_request::Message::CheckoutDone {
                number,
                worktree_path,
                result,
            } => {
                let kind = OperationKind::ManualAction(if worktree_path.is_some() {
                    "pull_request_worktree_checkout"
                } else {
                    "pull_request_checkout"
                });
                let completion = match self.operation_tracker.current_id_for(&kind) {
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
                        self.pull_requests.checkout_worktree.open = false;
                        self.operation.pending_transient_status_after_reload =
                            Some(match worktree_path {
                                Some(path) => {
                                    format!("Checked out pull request #{number} into {path}")
                                }
                                None => format!("Checked out pull request #{number}"),
                            });
                        completion.chain(self.reload_current_repo())
                    }
                    Err(msg) => {
                        self.operation.error = Some(msg);
                        completion
                    }
                }
            }
            pull_request::Message::OpenInBrowserRequested(pull_request) => {
                self.start_pull_request_open(pull_request)
            }
            pull_request::Message::OpenInBrowserDone { number, result } => {
                let kind = OperationKind::ManualAction("pull_request_open_browser");
                match result {
                    Ok(()) => {
                        let id = self.operation_tracker.next_id();
                        self.set_transient_status(format!(
                            "Opened pull request #{number} in browser"
                        ));
                        let start = Task::done(Message::Operation(OperationEvent::Started {
                            id,
                            kind: kind.clone(),
                            label: format!("Opening pull request #{number} in browser"),
                        }));
                        let complete = Task::done(Message::Operation(OperationEvent::Completed {
                            id,
                            result: OpResult::Success,
                            severity: OpSeverity::Recoverable,
                        }));
                        start.chain(complete)
                    }
                    Err(msg) => {
                        let id = self.operation_tracker.next_id();
                        self.operation.error = Some(msg.clone());
                        let start = Task::done(Message::Operation(OperationEvent::Started {
                            id,
                            kind,
                            label: format!("Opening pull request #{number} in browser"),
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
        }
    }

    pub(crate) fn refresh_pull_requests(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            self.pull_requests.loading = false;
            self.pull_requests.error = None;
            self.repo.pull_requests.clear();
            self.selection.selected_pull_request = None;
            return Task::none();
        };

        let filter = self.pull_requests.filter;
        let search_query = (filter == PullRequestFilter::Search)
            .then(|| self.pull_requests.search_query.trim().to_string());
        let options = ListPullRequestsOptions {
            filter,
            search_query: search_query.clone(),
        };
        self.pull_requests.loading = true;
        self.pull_requests.error = None;
        Task::perform(pull_request::task::list(path, options), move |result| {
            Message::from(pull_request::Message::Loaded {
                filter,
                search_query: search_query.clone(),
                result,
            })
        })
    }

    fn select_pull_request(&mut self, pull_request: PullRequestSummary) {
        self.selection.selected = None;
        self.selection.selected_commit_id = None;
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_stash = None;
        self.selection.selected_worktree = None;
        self.selection.selected_pull_request = Some(pull_request);
        self.selection.selected_github_issue = None;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_loading = false;
        self.operation.diff_error = None;
        self.operation.pending_diff_commit_id = None;
        self.operation.pending_wip_diff_target = None;
        self.operation.pending_stash_diff_selector = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;
        self.file_insight = Default::default();
    }

    fn open_pull_request_create(&mut self) -> Task<Message> {
        if self.repo.path.is_none() || self.repo.head_branch.is_none() {
            return Task::none();
        }
        self.pull_requests.create.open = true;
        self.operation.error = None;
        text_input::focus(self.pull_request_create_base_input_id.clone())
    }

    fn start_pull_request_create(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        let options = CreatePullRequestOptions {
            base_branch: (!self.pull_requests.create.base_branch.trim().is_empty())
                .then(|| self.pull_requests.create.base_branch.trim().to_string()),
            draft: self.pull_requests.create.draft,
        };
        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("pull_request_create"),
            label: "Creating pull request…".to_string(),
        }));
        start.chain(Task::perform(
            pull_request::task::create(path, options),
            |result| Message::from(pull_request::Message::CreateDone(result)),
        ))
    }

    fn start_pull_request_checkout(&mut self, pull_request: PullRequestSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        let number = pull_request.number;
        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("pull_request_checkout"),
            label: format!("Checking out pull request #{number}…"),
        }));
        start.chain(Task::perform(
            pull_request::task::checkout(path, number, CheckoutPullRequestOptions::default()),
            move |result| {
                Message::from(pull_request::Message::CheckoutDone {
                    number,
                    worktree_path: None,
                    result,
                })
            },
        ))
    }

    fn open_pull_request_worktree_checkout(
        &mut self,
        pull_request: PullRequestSummary,
    ) -> Task<Message> {
        if self.repo.path.is_none() {
            return Task::none();
        }
        if self.pull_requests.checkout_worktree.path.trim().is_empty() {
            self.pull_requests.checkout_worktree.path =
                default_pull_request_worktree_path(self.repo.path.as_deref(), &pull_request);
        }
        if self
            .pull_requests
            .checkout_worktree
            .branch_name
            .trim()
            .is_empty()
        {
            self.pull_requests.checkout_worktree.branch_name =
                default_pull_request_branch_name(&pull_request);
        }
        self.pull_requests.checkout_worktree.pull_request = Some(pull_request);
        self.pull_requests.checkout_worktree.open = true;
        self.operation.error = None;
        text_input::focus(self.pull_request_worktree_path_input_id.clone())
    }

    fn start_pull_request_worktree_checkout(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(pull_request) = self.pull_requests.checkout_worktree.pull_request.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        let worktree_path = self.pull_requests.checkout_worktree.path.trim();
        if worktree_path.is_empty() {
            let msg = "Enter a worktree path.".to_string();
            let id = self.operation_tracker.next_id();
            self.operation.error = Some(msg.clone());
            let start = Task::done(Message::Operation(OperationEvent::Started {
                id,
                kind: OperationKind::ManualAction("pull_request_worktree_checkout"),
                label: "Validating worktree path…".to_string(),
            }));
            let complete = Task::done(Message::Operation(OperationEvent::Completed {
                id,
                result: OpResult::Failed(msg),
                severity: OpSeverity::Recoverable,
            }));
            return start.chain(complete);
        }

        let branch_name = self.pull_requests.checkout_worktree.branch_name.trim();
        let options = CheckoutPullRequestOptions {
            worktree_path: Some(PathBuf::from(worktree_path)),
            branch_name: (!branch_name.is_empty()).then(|| branch_name.to_string()),
        };
        let number = pull_request.number;
        let worktree_path = worktree_path.to_string();
        self.operation.loading = true;
        self.operation.error = None;
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("pull_request_worktree_checkout"),
            label: format!("Checking out pull request #{number} into worktree…"),
        }));
        start.chain(Task::perform(
            pull_request::task::checkout(path, number, options),
            move |result| {
                Message::from(pull_request::Message::CheckoutDone {
                    number,
                    worktree_path: Some(worktree_path.clone()),
                    result,
                })
            },
        ))
    }

    fn start_pull_request_open(&mut self, pull_request: PullRequestSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        let number = pull_request.number;
        self.operation.error = None;
        Task::perform(
            pull_request::task::open_in_browser(path, number),
            move |result| {
                Message::from(pull_request::Message::OpenInBrowserDone { number, result })
            },
        )
    }

    fn sync_selected_pull_request(&mut self) {
        let Some(selected_number) = self
            .selection
            .selected_pull_request
            .as_ref()
            .map(|pull_request| pull_request.number)
        else {
            return;
        };

        self.selection.selected_pull_request = self
            .repo
            .pull_requests
            .iter()
            .find(|pull_request| pull_request.number == selected_number)
            .cloned();
    }
}

fn default_pull_request_worktree_path(
    repo_path: Option<&Path>,
    pull_request: &PullRequestSummary,
) -> String {
    let Some(repo_path) = repo_path else {
        return String::new();
    };
    let name = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let suffix = format!("pr-{}", pull_request.number);
    repo_path
        .parent()
        .map(|parent| parent.join(format!("{name}-{suffix}")))
        .unwrap_or_else(|| repo_path.with_file_name(format!("{name}-{suffix}")))
        .display()
        .to_string()
}

fn default_pull_request_branch_name(pull_request: &PullRequestSummary) -> String {
    format!("pr-{}", pull_request.number)
}
