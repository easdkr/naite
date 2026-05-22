use naite_core::{RefSummary, WorktreeStatus};

#[derive(Debug, Clone)]
pub enum Message {
    Requested(RefSummary),
    ForceSyncRequested(RefSummary),
    Cancelled,
    WorktreeStatusLoaded {
        target: RefSummary,
        result: Result<WorktreeStatus, String>,
    },
    ForceSyncStatusLoaded {
        target: RefSummary,
        result: Result<WorktreeStatus, String>,
    },
    Confirmed {
        target: RefSummary,
        force: bool,
    },
    ForceSyncConfirmed {
        target: RefSummary,
    },
    Done(Result<(), String>),
    ForceSyncDone(Result<(), String>),
}
