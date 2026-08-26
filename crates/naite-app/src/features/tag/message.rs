use std::path::PathBuf;

use naite_core::{CommitSummary, RefSummary};

use crate::state::TagNameMode;

#[derive(Debug, Clone)]
pub enum Message {
    CreateRequested(Option<CommitSummary>),
    CreateAndPushRequested(Option<CommitSummary>),
    LocalUtcOffsetLoaded {
        repo_path: PathBuf,
        target_commit: Option<CommitSummary>,
        push_after_create: bool,
        result: Result<i32, String>,
    },
    CreateNameChanged(String),
    CreateNameModeChanged(TagNameMode),
    CreatePushAfterChanged(bool),
    CreateCancelled,
    CreateSubmitted,
    DeleteRequested(RefSummary),
    DeleteCancelled,
    DeleteConfirmed,
    Done {
        operation: Operation,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
pub enum Operation {
    Create {
        name: String,
        push_after_create: bool,
        target_commit: Option<CommitSummary>,
    },
    Delete(RefSummary),
}

impl Operation {
    pub(crate) fn success_message(&self) -> String {
        match self {
            Self::Create {
                name,
                push_after_create,
                ..
            } => {
                if *push_after_create {
                    format!("Created and pushed tag {}", name.trim())
                } else {
                    format!("Created tag {}", name.trim())
                }
            }
            Self::Delete(target) => format!("Deleted tag {}", target.short_name),
        }
    }
}
