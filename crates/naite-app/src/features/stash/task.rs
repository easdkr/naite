use std::path::PathBuf;

use naite_core::{CommitDiff, HighlightedDiff, Repository};

use crate::features::stash::Operation;

pub(crate) async fn run(path: PathBuf, operation: Operation) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match operation {
            Operation::Create {
                message,
                include_untracked,
            } => repo.create_stash(&message, include_untracked),
            Operation::Apply(stash) => repo.apply_stash(&stash.selector),
            Operation::Pop(stash) => repo.pop_stash(&stash.selector),
            Operation::Drop(stash) => repo.drop_stash(&stash.selector),
            Operation::Branch { stash, branch_name } => {
                repo.create_branch_from_stash(&branch_name, &stash.selector)
            }
        }
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn load_diff(
    path: PathBuf,
    selector: String,
) -> (String, Result<(CommitDiff, HighlightedDiff), String>) {
    let selector_for_task = selector.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        let diff = repo
            .stash_diff(&selector_for_task)
            .map_err(|e| e.to_string())?;
        let hl = naite_core::highlight_diff(&diff);
        Ok((diff, hl))
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))
    .and_then(|result| result);
    (selector, result)
}
