use std::path::PathBuf;

use naite_core::{Repository, WorktreeAdd};

pub(crate) async fn add(repo_path: PathBuf, add: WorktreeAdd) -> Result<PathBuf, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&repo_path).map_err(|e| e.to_string())?;
        repo.add_worktree(&add).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn remove(
    repo_path: PathBuf,
    target_path: PathBuf,
    delete_branch: bool,
    force: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&repo_path).map_err(|e| e.to_string())?;
        repo.remove_worktree(target_path, delete_branch, force)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn lock(
    repo_path: PathBuf,
    target_path: PathBuf,
    reason: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&repo_path).map_err(|e| e.to_string())?;
        repo.lock_worktree(target_path, Some(&reason))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn unlock(repo_path: PathBuf, target_path: PathBuf) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&repo_path).map_err(|e| e.to_string())?;
        repo.unlock_worktree(target_path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
