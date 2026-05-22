use naite_core::{GitHubIssueFilter, GitHubIssueSummary};

#[derive(Debug, Clone)]
pub enum Message {
    RefreshRequested,
    FilterChanged(GitHubIssueFilter),
    SearchQueryChanged(String),
    SearchSubmitted,
    Loaded {
        filter: GitHubIssueFilter,
        search_query: Option<String>,
        result: Result<Vec<GitHubIssueSummary>, String>,
    },
    Selected(GitHubIssueSummary),
    OpenInBrowserRequested(GitHubIssueSummary),
    OpenInBrowserDone {
        number: u32,
        result: Result<(), String>,
    },
}
