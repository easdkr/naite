use naite_core::Hunk;

#[derive(Debug, Clone)]
pub enum Message {
    StatusPath(String),
    UnstageStatusPath(String),
    HunkRequested { path: String, hunk: Hunk },
    UnstageHunkRequested { path: String, hunk: Hunk },
    All,
    UnstageAll,
    Done(Result<naite_core::WorktreeStatusDetail, String>),
}

#[derive(Debug, Clone)]
pub(crate) enum Operation {
    StagePath(String),
    UnstagePath(String),
    StageHunk { path: String, hunk: Hunk },
    UnstageHunk { path: String, hunk: Hunk },
    StageAll,
    UnstageAll,
}
