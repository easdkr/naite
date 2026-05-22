use naite_core::StashSummary;

#[derive(Debug, Clone)]
pub enum Message {
    CreateRequested,
    DescriptionChanged(String),
    IncludeUntrackedChanged(bool),
    Cancelled,
    Submitted,
    ApplyRequested(StashSummary),
    PopRequested(StashSummary),
    DropRequested(StashSummary),
    BranchRequested(StashSummary),
    BranchNameChanged(String),
    BranchCancelled,
    BranchSubmitted,
    ConfirmationCancelled,
    Confirmed,
    Done {
        operation: Operation,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
pub enum Operation {
    Create {
        message: String,
        include_untracked: bool,
    },
    Apply(StashSummary),
    Pop(StashSummary),
    Drop(StashSummary),
    Branch {
        stash: StashSummary,
        branch_name: String,
    },
}

impl Operation {
    pub(crate) fn success_message(&self) -> String {
        match self {
            Self::Create { .. } => "Stashed working tree changes".into(),
            Self::Apply(stash) => format!("Applied {}", stash.selector),
            Self::Pop(stash) => format!("Popped {}", stash.selector),
            Self::Drop(stash) => format!("Dropped {}", stash.selector),
            Self::Branch { stash, branch_name } => {
                format!("Created branch {branch_name} from {}", stash.selector)
            }
        }
    }
}
