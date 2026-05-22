use naite_core::PullMode;

#[derive(Debug, Clone)]
pub enum Message {
    Requested(PullMode),
    Done {
        mode: PullMode,
        result: Result<(), String>,
    },
}
