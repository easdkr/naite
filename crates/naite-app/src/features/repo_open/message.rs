use std::path::PathBuf;

use naite_core::{
    BranchSyncStatus, CommitAuthorAvatar, CommitPage, CommitPageCursor, CommitSummary,
    GitOperationState, Refs, StashSummary, WorktreeStatusDetail, WorktreeSummary,
};

pub(crate) type LoadedRepo = (
    PathBuf,
    Vec<CommitSummary>,
    Option<CommitPageCursor>,
    Refs,
    Vec<StashSummary>,
    Vec<WorktreeSummary>,
    Option<String>,
    WorktreeStatusDetail,
    BranchSyncStatus,
    GitOperationState,
);

#[derive(Debug, Clone)]
pub enum Message {
    OpenClicked,
    OpenRecent(PathBuf),
    PathPicked(Option<PathBuf>),
    Loaded(Box<Result<LoadedRepo, String>>),
    LoadMoreCommitsRequested,
    MoreCommitsLoaded {
        path: PathBuf,
        result: Result<CommitPage, String>,
    },
    CommitAuthorAvatarsLoaded {
        path: PathBuf,
        result: Result<Vec<CommitAuthorAvatar>, String>,
    },
    ToggleFavorite(PathBuf),
    RemoveFavorite(PathBuf),
    RemoveRecent(PathBuf),
    CloneFormToggled,
    CloneUrlChanged(String),
    CloneClicked,
    NewRepoMenuToggled,
    NewRepoMenuClosed,
    CloneParentPicked(Option<PathBuf>),
    CloneDone(Result<PathBuf, String>),
    InitClicked,
    InitPathPicked(Option<PathBuf>),
    InitDone(Result<PathBuf, String>),
}
