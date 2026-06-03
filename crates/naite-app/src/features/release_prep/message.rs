use naite_core::{ReleaseProfileSuggestion, ReleaseSyncCheck};

use super::task::PrepareOutcome;

#[derive(Debug, Clone)]
pub enum Message {
    Requested,
    ConfigureRequested,
    SuggestionLoaded(Result<ReleaseProfileSuggestion, String>),
    RemoteChanged(String),
    SourceBranchChanged(String),
    TargetBranchChanged(String),
    ValidationScriptChanged(String),
    BackupToggled(bool),
    Cancelled,
    ProfileSubmitted,
    Prepared(Box<Result<PrepareOutcome, String>>),
    AutoRequested,
    ActionRequested(ReleasePrepAction),
    ActionDone {
        action: ReleasePrepAction,
        result: Box<Result<ReleaseSyncCheck, String>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleasePrepAction {
    UpdateTargetFromSource,
    ValidateTarget,
    PushTarget,
    SyncSourceFromTarget,
}

impl ReleasePrepAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::UpdateTargetFromSource => "Update target from source",
            Self::ValidateTarget => "Run validation script",
            Self::PushTarget => "Push target",
            Self::SyncSourceFromTarget => "Rebase source onto target",
        }
    }
}
