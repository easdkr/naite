use naite_core::{CommitSummary, ResetMode};

#[derive(Debug, Clone)]
pub enum Message {
    Requested(CommitSummary),
    Cancelled,
    Confirmed(ResetMode),
    Done {
        commit: CommitSummary,
        mode: ResetMode,
        result: Result<(), String>,
    },
}
