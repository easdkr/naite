#[derive(Debug, Clone)]
pub enum Message {
    TitleChanged(String),
    BodyChanged(String),
    CoAuthorsChanged(String),
    AmendChanged(bool),
    SkipHooksChanged(bool),
    PushAfterChanged(bool),
    Requested,
    Done(Result<CommitOutcome, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitOutcome {
    pub pushed: bool,
}
