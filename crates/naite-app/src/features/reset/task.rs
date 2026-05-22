use std::path::PathBuf;

use naite_core::{Repository, ResetMode};

pub(crate) async fn run(path: PathBuf, commit_id: String, mode: ResetMode) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.reset_to(&commit_id, mode).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
