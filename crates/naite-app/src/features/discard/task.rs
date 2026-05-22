use std::path::PathBuf;

use naite_core::{Repository, WorktreeStatusDetail};

use crate::DiscardTarget;

pub(crate) async fn run(
    path: PathBuf,
    target: DiscardTarget,
) -> Result<WorktreeStatusDetail, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match target {
            DiscardTarget::File(target) => repo.discard_worktree_path(target.kind, &target.path),
            DiscardTarget::Hunk { path, hunk } => repo.discard_worktree_hunk(&path, &hunk),
        }
        .map_err(|e| e.to_string())?;
        repo.status_detail().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
