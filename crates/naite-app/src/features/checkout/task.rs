use std::path::PathBuf;

use naite_core::{RefSummary, Repository, WorktreeStatus};

pub(crate) async fn load_status(path: PathBuf) -> Result<WorktreeStatus, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.status().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn run(path: PathBuf, target: RefSummary, force: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.checkout_ref(&target.full_name, target.kind, force)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn force_sync(path: PathBuf, target: RefSummary) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.force_sync_remote_branch(&target.full_name)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
