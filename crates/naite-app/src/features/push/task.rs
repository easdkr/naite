use std::path::PathBuf;

use naite_core::{PushMode, Repository};

pub(crate) async fn run(path: PathBuf, mode: PushMode) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.push_current_branch(mode).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
