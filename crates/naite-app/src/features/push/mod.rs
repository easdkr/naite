pub(crate) mod message;
pub(crate) mod task;
pub(crate) mod update;

pub(crate) use message::Message;
pub(crate) use naite_core::PushMode;

/// Returns `true` when a push error matches Git's non-fast-forward rejection,
/// which is the recovery path where `--force-with-lease` is useful.
pub(crate) fn is_non_fast_forward_rejection(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("non-fast-forward")
        || lower.contains("(fetch first)")
        || (lower.contains("rejected") && lower.contains("fast-forward"))
}

#[cfg(test)]
mod tests {
    use super::is_non_fast_forward_rejection;

    #[test]
    fn detects_typical_github_rejection() {
        let msg = "To github.com:WISELYTECH/commerce-web-api.git\n \
                   ! [rejected]        staging -> staging (non-fast-forward)\n\
                   error: failed to push some refs";
        assert!(is_non_fast_forward_rejection(msg));
    }

    #[test]
    fn detects_fetch_first_rejection() {
        assert!(is_non_fast_forward_rejection(
            "! [rejected] main -> main (fetch first)"
        ));
    }

    #[test]
    fn ignores_unrelated_errors() {
        assert!(!is_non_fast_forward_rejection("authentication failed"));
        assert!(!is_non_fast_forward_rejection(
            "could not read from remote repository"
        ));
    }
}
