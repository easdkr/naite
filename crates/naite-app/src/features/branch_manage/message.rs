use naite_core::RefSummary;

use crate::BranchDeleteTarget;

#[derive(Debug, Clone)]
pub enum Message {
    RenameRequested(RefSummary),
    RenameNameChanged(String),
    RenameCancelled,
    RenameSubmitted,
    DeleteRequested(BranchDeleteTarget),
    DeleteMatchingLocalBranchesToggled(bool),
    DeleteLinkedWorktreesToggled(bool),
    DeleteForceLinkedWorktreesToggled(bool),
    DeleteCancelled,
    DeleteConfirmed,
    Done {
        operation: Operation,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
pub enum Operation {
    Rename {
        target: RefSummary,
        new_name: String,
    },
    Delete {
        target: BranchDeleteTarget,
        delete_matching_local_branches: bool,
        delete_linked_worktrees: bool,
        force_linked_worktrees: bool,
        linked_worktrees: Vec<crate::LinkedWorktreeDeleteTarget>,
    },
}

impl Operation {
    pub(crate) fn success_message(&self) -> String {
        match self {
            Self::Rename { target, new_name } => {
                format!("Renamed {} to {}", target.short_name, new_name.trim())
            }
            Self::Delete {
                target,
                delete_matching_local_branches,
                delete_linked_worktrees,
                force_linked_worktrees: _,
                linked_worktrees,
            } => {
                let mut suffix = String::new();
                if *delete_matching_local_branches {
                    suffix.push_str(" and matching local branches");
                }
                if *delete_linked_worktrees && !linked_worktrees.is_empty() {
                    suffix.push_str(" and linked worktrees");
                }
                let local_suffix = if suffix.is_empty() {
                    ""
                } else {
                    suffix.as_str()
                };
                format!("Deleted {}{}", target.label(), local_suffix)
            }
        }
    }
}
