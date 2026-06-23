use iced::Task;
use naite_core::{GitHubIssueFilter, GitHubIssueSummary, ListGitHubIssuesOptions};

use crate::features::github_issue;
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind};
use crate::{App, Message};

impl App {
    pub(crate) fn update_github_issue(&mut self, message: github_issue::Message) -> Task<Message> {
        match message {
            github_issue::Message::RefreshRequested => self.refresh_github_issues(),
            github_issue::Message::FilterChanged(filter) => {
                if filter == GitHubIssueFilter::Search {
                    return self.update_github_issue(github_issue::Message::SearchSubmitted);
                }
                self.github_issues.filter = filter;
                self.github_issues.last_non_search_filter = filter;
                self.refresh_github_issues()
            }
            github_issue::Message::SearchQueryChanged(query) => {
                let cleared = query.trim().is_empty();
                self.github_issues.search_query = query;
                if cleared && self.github_issues.filter == GitHubIssueFilter::Search {
                    self.github_issues.filter = self.github_issues.last_non_search_filter;
                    self.github_issues.error = None;
                    return self.refresh_github_issues();
                }
                Task::none()
            }
            github_issue::Message::SearchSubmitted => {
                if self.github_issues.search_query.trim().is_empty() {
                    self.github_issues.filter = self.github_issues.last_non_search_filter;
                    self.github_issues.error = None;
                    return self.refresh_github_issues();
                }
                self.github_issues.filter = GitHubIssueFilter::Search;
                self.refresh_github_issues()
            }
            github_issue::Message::Loaded {
                filter,
                search_query,
                result,
            } => {
                if self.github_issues.filter != filter
                    || (filter == GitHubIssueFilter::Search
                        && Some(self.github_issues.search_query.trim()) != search_query.as_deref())
                {
                    return Task::none();
                }
                self.github_issues.loading = false;
                match result {
                    Ok(items) => {
                        self.repo.github_issues = items;
                        self.github_issues.error = None;
                        self.sync_selected_github_issue();
                    }
                    Err(msg) => {
                        self.repo.github_issues.clear();
                        self.selection.selected_github_issue = None;
                        self.github_issues.error = Some(msg);
                    }
                }
                Task::none()
            }
            github_issue::Message::Selected(issue) => {
                self.select_github_issue(issue);
                Task::none()
            }
            github_issue::Message::OpenInBrowserRequested(issue) => {
                self.start_github_issue_open(issue)
            }
            github_issue::Message::OpenInBrowserDone { number, result } => {
                let kind = OperationKind::ManualAction("github_issue_open");
                match result {
                    Ok(()) => {
                        let id = self.operation_tracker.next_id();
                        self.set_transient_status(format!(
                            "Opened GitHub issue #{number} in browser"
                        ));
                        let start = Task::done(Message::Operation(OperationEvent::Started {
                            id,
                            kind,
                            label: format!("Opening GitHub issue #{number}…"),
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
                            label: format!("Opening GitHub issue #{number}…"),
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

    pub(crate) fn refresh_github_issues(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            self.github_issues.loading = false;
            self.github_issues.error = None;
            self.repo.github_issues.clear();
            self.selection.selected_github_issue = None;
            return Task::none();
        };

        let filter = self.github_issues.filter;
        let search_query = (filter == GitHubIssueFilter::Search)
            .then(|| self.github_issues.search_query.trim().to_string());
        let options = ListGitHubIssuesOptions {
            filter,
            search_query: search_query.clone(),
        };
        self.github_issues.loading = true;
        self.github_issues.error = None;
        Task::perform(github_issue::task::list(path, options), move |result| {
            Message::from(github_issue::Message::Loaded {
                filter,
                search_query: search_query.clone(),
                result,
            })
        })
    }

    fn select_github_issue(&mut self, issue: GitHubIssueSummary) {
        self.selection.selected = None;
        self.selection.selected_commit_id = None;
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_stash = None;
        self.selection.selected_worktree = None;
        self.selection.selected_pull_request = None;
        self.selection.selected_github_issue = Some(issue);
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

    fn start_github_issue_open(&mut self, issue: GitHubIssueSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let number = issue.number;
        Task::perform(
            github_issue::task::open_in_browser(path, number),
            move |result| {
                Message::from(github_issue::Message::OpenInBrowserDone { number, result })
            },
        )
    }

    fn sync_selected_github_issue(&mut self) {
        let Some(selected) = self.selection.selected_github_issue.as_ref() else {
            return;
        };
        self.selection.selected_github_issue = self
            .repo
            .github_issues
            .iter()
            .find(|issue| issue.number == selected.number)
            .cloned();
    }
}
