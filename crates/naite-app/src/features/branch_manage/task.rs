use std::collections::BTreeSet;
use std::path::PathBuf;

use naite_core::Repository;

use crate::features::branch_manage::Operation;
use crate::BranchDeleteTarget;

pub(crate) async fn run(path: PathBuf, operation: Operation) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match operation {
            Operation::Rename { target, new_name } => repo
                .rename_local_branch(&target.short_name, &new_name)
                .map_err(|e| e.to_string()),
            Operation::Delete {
                target,
                delete_matching_local_branches,
                delete_linked_worktrees,
                linked_worktrees,
            } => match target {
                BranchDeleteTarget::LocalBranch(target) => {
                    let removed_branches =
                        remove_linked_worktrees(&repo, delete_linked_worktrees, linked_worktrees)?;
                    if removed_branches.contains(&target.short_name) {
                        Ok(())
                    } else {
                        repo.force_delete_local_branch(&target.short_name)
                            .map_err(|e| e.to_string())
                    }
                }
                BranchDeleteTarget::LocalBranches { branches, .. } => {
                    let removed_branches =
                        remove_linked_worktrees(&repo, delete_linked_worktrees, linked_worktrees)?;
                    let branch_names = branches
                        .into_iter()
                        .map(|branch| branch.short_name)
                        .filter(|branch| !removed_branches.contains(branch))
                        .collect::<Vec<_>>();
                    for branch_name in branch_names {
                        repo.force_delete_local_branch(&branch_name)
                            .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                }
                BranchDeleteTarget::RemoteBranches { branches, .. } => {
                    let full_ref_names = branches
                        .into_iter()
                        .map(|branch| branch.full_name)
                        .collect::<Vec<_>>();
                    repo.delete_remote_branches(&full_ref_names, delete_matching_local_branches)
                        .map_err(|e| e.to_string())
                }
            },
        }
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

fn remove_linked_worktrees(
    repo: &Repository,
    delete_linked_worktrees: bool,
    linked_worktrees: Vec<crate::LinkedWorktreeDeleteTarget>,
) -> Result<BTreeSet<String>, String> {
    let mut removed_branches = BTreeSet::new();
    if !delete_linked_worktrees {
        return Ok(removed_branches);
    }

    for worktree in linked_worktrees {
        repo.remove_worktree(&worktree.path, true)
            .map_err(|e| e.to_string())?;
        removed_branches.insert(worktree.branch);
    }
    Ok(removed_branches)
}
