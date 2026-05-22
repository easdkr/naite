use naite_core::{PullRequestFilter, PullRequestSummary};

#[derive(Debug, Clone)]
pub enum Message {
    RefreshRequested,
    FilterChanged(PullRequestFilter),
    SearchQueryChanged(String),
    SearchSubmitted,
    Loaded {
        filter: PullRequestFilter,
        search_query: Option<String>,
        result: Result<Vec<PullRequestSummary>, String>,
    },
    Selected(PullRequestSummary),
    CreateRequested,
    CreateBaseChanged(String),
    CreateDraftChanged(bool),
    CreateCancelled,
    CreateSubmitted,
    CreateDone(Result<String, String>),
    CheckoutRequested(PullRequestSummary),
    CheckoutWorktreeRequested(PullRequestSummary),
    CheckoutWorktreePathChanged(String),
    CheckoutWorktreeBranchChanged(String),
    CheckoutWorktreeCancelled,
    CheckoutWorktreeSubmitted,
    CheckoutDone {
        number: u32,
        worktree_path: Option<String>,
        result: Result<(), String>,
    },
    OpenInBrowserRequested(PullRequestSummary),
    OpenInBrowserDone {
        number: u32,
        result: Result<(), String>,
    },
}
