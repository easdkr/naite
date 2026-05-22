use std::path::PathBuf;

use naite_core::{CommitOptions, PushMode, Repository};

use crate::features::commit::CommitOutcome;

pub(crate) async fn run(
    path: PathBuf,
    options: CommitOptions,
    push_after: bool,
) -> Result<CommitOutcome, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.commit_with_options(&options)
            .map_err(|e| e.to_string())?;
        if push_after {
            repo.push_current_branch(PushMode::Normal)
                .map_err(|e| e.to_string())?;
        }
        Ok(CommitOutcome { pushed: push_after })
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
