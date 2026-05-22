use naite_core::CommitSummary;

#[derive(Debug, Clone)]
pub enum Message {
    Requested(CommitSummary),
    Done {
        commit: CommitSummary,
        result: Result<(), String>,
    },
}
