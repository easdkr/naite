use std::path::PathBuf;

use naite_core::Repository;

pub(crate) async fn run(path: PathBuf, commit_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.cherry_pick(&commit_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
