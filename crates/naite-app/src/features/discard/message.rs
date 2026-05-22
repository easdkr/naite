use naite_core::{Hunk, WorktreeDiffTarget, WorktreeStatusDetail};

#[derive(Debug, Clone)]
pub enum Message {
    FileRequested(WorktreeDiffTarget),
    HunkRequested { path: String, hunk: Hunk },
    Cancelled,
    Confirmed,
    Done(Result<WorktreeStatusDetail, String>),
}
