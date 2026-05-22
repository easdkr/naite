use std::path::PathBuf;

use naite_core::{CommitMessage, ConflictSide, ReorderDirection, Repository, SquashMode};

use crate::features::history::Operation;

pub(crate) async fn load_commit_message(
    path: PathBuf,
    commit_id: String,
) -> Result<CommitMessage, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.commit_message(&commit_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn run(path: PathBuf, operation: Operation) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match operation {
            Operation::Merge(target) => repo.merge_ref(&target.full_name),
            Operation::Rebase(target) => repo.rebase_onto(&target.full_name),
            Operation::AbortMerge => repo.abort_merge(),
            Operation::AbortRebase => repo.abort_rebase(),
            Operation::ContinueRebase => repo.continue_rebase(),
            Operation::ResolveWithSide { path, side } => repo.resolve_conflict_with_side(
                &path,
                match side {
                    ConflictSide::Ours => ConflictSide::Ours,
                    ConflictSide::Theirs => ConflictSide::Theirs,
                },
            ),
            Operation::MarkResolved(path) => repo.mark_conflict_resolved(&path),
            Operation::Reword { commit, message } => repo.reword_commit(&commit.id, &message),
            Operation::Drop(commit) => repo.drop_commit(&commit.id),
            Operation::Squash(commit) => {
                repo.squash_commit_into_parent(&commit.id, SquashMode::Squash)
            }
            Operation::Fixup(commit) => {
                repo.squash_commit_into_parent(&commit.id, SquashMode::Fixup)
            }
            Operation::Edit(commit) => repo.edit_commit(&commit.id),
            Operation::Move { commit, direction } => repo.reorder_commit(
                &commit.id,
                match direction {
                    ReorderDirection::Earlier => ReorderDirection::Earlier,
                    ReorderDirection::Later => ReorderDirection::Later,
                },
            ),
            Operation::Undo(checkpoint) | Operation::Redo(checkpoint) => {
                repo.reset_hard_to(&checkpoint.head_id)
            }
        }
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
