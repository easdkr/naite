use naite_core::CommitSummary;

#[derive(Debug, Clone)]
pub enum Message {
    Requested,
    RequestedFromCommit(CommitSummary),
    NameChanged(String),
    Cancelled,
    Submitted,
    Done(Result<(), String>),
}
