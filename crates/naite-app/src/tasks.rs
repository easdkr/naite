use std::path::PathBuf;

use naite_core::{
    CommitDiff, HighlightedDiff, Repository, WorktreeDiffTarget, WorktreeStatusDetail,
};

pub(crate) async fn load_diff(
    path: PathBuf,
    commit_id: String,
) -> (String, Result<(CommitDiff, HighlightedDiff), String>) {
    let task_commit_id = commit_id.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        let diff = repo
            .commit_diff(&task_commit_id)
            .map_err(|e| e.to_string())?;
        let hl = naite_core::highlight_diff(&diff);
        Ok((diff, hl))
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))
    .and_then(|result| result);

    (commit_id, result)
}

pub(crate) async fn load_wip_diff(
    path: PathBuf,
    target: WorktreeDiffTarget,
) -> (
    WorktreeDiffTarget,
    Result<(CommitDiff, HighlightedDiff), String>,
) {
    let task_target = target.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        let diff = repo
            .worktree_diff(&task_target)
            .map_err(|e| e.to_string())?;
        let hl = naite_core::highlight_diff(&diff);
        Ok((diff, hl))
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))
    .and_then(|result| result);

    (target, result)
}

pub(crate) async fn load_status_detail(path: PathBuf) -> Result<WorktreeStatusDetail, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.status_detail().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
