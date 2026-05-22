use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Message {
    Requested(FetchScope),
    Done {
        scope: FetchScope,
        result: Result<(), String>,
    },
    AutoDone {
        path: PathBuf,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchScope {
    CurrentRemote,
    AllRemotes,
}
