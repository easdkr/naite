use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not open repository at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("repository has no HEAD commit yet")]
    NoHead,

    #[error("failed to walk commit graph: {0}")]
    Walk(Box<dyn std::error::Error + Send + Sync>),

    #[error("failed to read commit: {0}")]
    ReadCommit(Box<dyn std::error::Error + Send + Sync>),

    #[error(
        "could not find `git` on PATH; install Git or add it to PATH before using write operations"
    )]
    GitNotFound,

    #[error("git command failed: {command}: {stderr}")]
    GitCommand { command: String, stderr: String },

    #[error(
        "could not find `{program}` on PATH; install the required provider CLI and authenticate before using this integration"
    )]
    ProviderCliNotFound { program: String },

    #[error("provider command failed: {command}: {stderr}")]
    ProviderCommand { command: String, stderr: String },

    #[error("worktree has local changes")]
    DirtyWorkdir,

    #[error("invalid ref name: {0}")]
    InvalidRefName(String),

    #[error("invalid tag name: {0}")]
    InvalidTagName(String),

    #[error("tag already exists: {0}")]
    TagAlreadyExists(String),

    #[error("invalid commit: {0}")]
    InvalidCommit(String),

    #[error("history operation is not supported for this commit: {0}")]
    UnsupportedHistoryOperation(String),

    #[error("branch operation is only supported for local branches: {0}")]
    UnsupportedBranchTarget(String),

    #[error("cannot delete the current branch: {0}")]
    CannotDeleteCurrentBranch(String),

    #[error("invalid stash selector: {0}")]
    InvalidStashSelector(String),

    #[error("invalid clone URL: {0}")]
    InvalidCloneUrl(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("invalid commit message: {0}")]
    InvalidCommitMessage(String),

    #[error("checkout is only supported for local branches: {0}")]
    UnsupportedCheckoutTarget(String),

    #[error("discard is only supported for unstaged tracked or untracked files")]
    UnsupportedDiscardTarget,

    #[error("current branch does not have an upstream remote")]
    NoUpstream,

    #[error("current HEAD is detached; push requires a local branch")]
    NoCurrentBranch,
}
