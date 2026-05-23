//! naite core: Git domain logic, decoupled from the UI.
//!
//! This crate owns repository access via [`gix`] and exposes plain-data
//! structures the UI layer can consume without depending on `gix` directly.

mod command;
mod commits;
mod diff;
mod error;
mod file_inspect;
mod graph;
mod highlight;
mod ops;
mod providers;
mod refs;
mod repo;
mod text;
mod workspace;
mod worktree;
mod worktrees;

#[cfg(test)]
mod test_helpers;

pub use commits::{CommitMessage, CommitPage, CommitPageCursor, CommitSummary};
pub use diff::{ChangeStatus, CommitDiff, DiffLine, FileChange, Hunk};
pub use error::Error;
pub use file_inspect::{BlameLine, FileHistoryEntry};
pub use graph::{
    build_rebase_gutter, compute_graph_layout, pick_inherits_reword, GraphLayout, GraphRow,
};
pub use highlight::{
    detect_language, highlight_diff, HighlightedDiff, HighlightedHunk, HighlightedLine, Language,
    LineState, TokenKind, TokenSpan, MAX_LINE_BYTES, MAX_SPANS_PER_LINE,
};
pub use ops::commit::CommitOptions;
pub use ops::history::{
    ConflictSide, GitOperationState, HistoryCommit, RebaseAction, RebasePlanEntry,
    ReorderDirection, SquashMode,
};
pub use ops::pull::PullMode;
pub use ops::push::PushMode;
pub use ops::release::{
    ReleaseBranchSync, ReleaseProfile, ReleaseProfileSuggestion, ReleaseSyncCheck,
};
pub use ops::reset::ResetMode;
pub use ops::stash::StashSummary;
pub use providers::{
    CheckoutPullRequestOptions, CommitAuthorAvatar, CreatePullRequestOptions, GitHubIssueFilter,
    GitHubIssueSummary, HostingProvider, IssueLink, ListGitHubIssuesOptions,
    ListPullRequestsOptions, PullRequestCiStatus, PullRequestFilter, PullRequestReviewStatus,
    PullRequestSummary,
};
pub use refs::{BranchSyncStatus, RefKind, RefSummary, Refs};
pub use repo::Repository;
pub use text::{compose_hangul, is_hangul_compatibility_jamo};
pub use workspace::WorkspaceRepoSummary;
pub use worktree::{
    StatusEntry, StatusKind, WorktreeDiffKind, WorktreeDiffTarget, WorktreeStatus,
    WorktreeStatusDetail,
};
pub use worktrees::{WorktreeAdd, WorktreeSummary};
