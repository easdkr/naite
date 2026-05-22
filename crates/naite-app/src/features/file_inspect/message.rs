use naite_core::{BlameLine, FileHistoryEntry};

use crate::state::FileInsightMode;

#[derive(Debug, Clone)]
pub enum Message {
    HistoryRequested(String),
    BlameRequested(String),
    Done {
        path: String,
        mode: FileInsightMode,
        result: Result<FileInsightResult, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileInsightResult {
    History(Vec<FileHistoryEntry>),
    Blame(Vec<BlameLine>),
}
