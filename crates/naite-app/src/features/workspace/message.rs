use std::path::PathBuf;

use naite_core::WorkspaceRepoSummary;

#[derive(Debug, Clone)]
pub enum Message {
    DashboardToggled,
    RefreshRequested,
    Loaded(Vec<WorkspaceRepoSummary>),
    FetchAllRequested,
    FetchAllDone(MultiRepoOperationSummary),
    PullAllRequested,
    PullAllDone(MultiRepoOperationSummary),
    OpenRepo(PathBuf),
    LocateRepo(PathBuf),
    LocateDone(Result<(), String>),
    RemoveRepo(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiRepoOperationSummary {
    pub attempted: usize,
    pub succeeded: usize,
    pub failures: Vec<(PathBuf, String)>,
}
