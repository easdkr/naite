use std::path::PathBuf;

use naite_core::{PullMode, Repository};

pub(crate) async fn run(path: PathBuf, mode: PullMode) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.pull(mode).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
