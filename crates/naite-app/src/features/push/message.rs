use naite_core::PushMode;

#[derive(Debug, Clone)]
pub enum Message {
    Requested(PushMode),
    ForceWithLeaseConfirmationRequested,
    ForceWithLeaseConfirmed,
    ForceWithLeaseCancelled,
    Done {
        mode: PushMode,
        result: Result<(), String>,
    },
}
