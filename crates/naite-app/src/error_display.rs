use std::borrow::Cow;

pub(crate) const GIT_INDEX_LOCK_MESSAGE: &str = "Another Git operation is using this repository. Wait and retry. Only remove .git/index.lock after confirming no Git process is running.";

pub(crate) fn format_git_error_for_display(error: &str) -> Cow<'_, str> {
    if is_git_index_lock_error(error) {
        Cow::Borrowed(GIT_INDEX_LOCK_MESSAGE)
    } else {
        Cow::Borrowed(error)
    }
}

pub(crate) fn is_git_index_lock_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains(".git/index.lock")
        || (lower.contains("unable to create") && lower.contains("index.lock"))
        || lower.contains("another git process seems to be running")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_index_lock_errors_for_display() {
        let raw = "git command failed: git status --porcelain=v1: fatal: Unable to create '/repo/.git/index.lock': File exists.\nAnother git process seems to be running in this repository";

        assert_eq!(
            format_git_error_for_display(raw).as_ref(),
            GIT_INDEX_LOCK_MESSAGE
        );
    }

    #[test]
    fn leaves_non_fast_forward_errors_raw_for_recovery_matching() {
        let raw = "! [rejected] staging -> staging (non-fast-forward)";

        assert_eq!(format_git_error_for_display(raw).as_ref(), raw);
    }
}
