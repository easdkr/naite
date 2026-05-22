use std::path::PathBuf;

use naite_core::{Repository, WorktreeStatusDetail};

use crate::features::stage::Operation;

pub(crate) async fn run(
    path: PathBuf,
    operation: Operation,
) -> Result<WorktreeStatusDetail, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match operation {
            Operation::StagePath(path) => repo.stage_path(&path),
            Operation::UnstagePath(path) => repo.unstage_path(&path),
            Operation::StageHunk { path, hunk } => repo.stage_worktree_hunk(&path, &hunk),
            Operation::UnstageHunk { path, hunk } => repo.unstage_worktree_hunk(&path, &hunk),
            Operation::StageAll => repo.stage_all(),
            Operation::UnstageAll => repo.unstage_all(),
        }
        .map_err(|e| e.to_string())?;
        repo.status_detail().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
