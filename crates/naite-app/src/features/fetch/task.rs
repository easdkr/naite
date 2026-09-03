use std::path::PathBuf;

use naite_core::Repository;

use crate::features::fetch::FetchScope;

// No timeout here: `naite-core` bounds the fetch subprocess itself
// (`run_command_with_timeout` kills the child), so this future always
// completes and the worker thread is never leaked.
pub(crate) async fn run(path: PathBuf, scope: FetchScope) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match scope {
            FetchScope::CurrentRemote => repo.fetch_current_remote(),
            FetchScope::AllRemotes => repo.fetch_all_remotes(),
        }
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
