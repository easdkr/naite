use std::path::PathBuf;

use naite_core::WorktreeSummary;

#[derive(Debug, Clone)]
pub enum Message {
    Selected(WorktreeSummary),
    OpenRequested(WorktreeSummary),
    CreateRequested,
    CreatePathChanged(String),
    CreateStartPointChanged(String),
    CreateBranchChanged(String),
    CreateCancelled,
    CreateConfirmed,
    CreateDone(Result<PathBuf, String>),
    RemoveRequested(WorktreeSummary),
    RemoveDeleteBranchToggled(bool),
    RemoveForceToggled(bool),
    RemoveCancelled,
    RemoveConfirmed,
    RemoveDone(Result<(), String>),
    LockRequested(WorktreeSummary),
    LockDone(Result<(), String>),
    UnlockRequested(WorktreeSummary),
    UnlockDone(Result<(), String>),
}
