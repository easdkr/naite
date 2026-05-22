use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

impl Repository {
    /// Move HEAD (and optionally the index/worktree) to the given commit.
    /// `ResetMode::Hard` is destructive: it overwrites uncommitted changes.
    /// Callers must confirm explicitly before invoking `Hard`.
    pub fn reset_to(&self, commit_id: &str, mode: ResetMode) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let _ = self.git(&["rev-parse", "--verify", commit_id])?;
        let _ = self.git(&["reset", mode.flag(), commit_id])?;
        Ok(())
    }
}

fn validate_commit_id(commit_id: &str) -> Result<&str, Error> {
    let commit_id = commit_id.trim();
    if commit_id.is_empty() || commit_id.starts_with('-') {
        return Err(Error::InvalidCommit(commit_id.to_string()));
    }
    Ok(commit_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    fn three_commit_history(name: &str) -> TempRepo {
        let repo = TempRepo::init_with_commit(name);
        repo.write("file.txt", "one\n");
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", "one"]);
        repo.write("file.txt", "two\n");
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", "two"]);
        repo
    }

    #[test]
    fn reset_to_soft_moves_head_and_keeps_changes_staged() {
        let repo_dir = three_commit_history("reset-soft");
        let initial = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(3).unwrap()[2].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.reset_to(&initial, ResetMode::Soft).unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.list_commits(1).unwrap()[0].id, initial);
        let status = repo.status_detail().unwrap();
        assert!(!status.staged.is_empty());
    }

    #[test]
    fn reset_to_mixed_moves_head_and_leaves_changes_unstaged() {
        let repo_dir = three_commit_history("reset-mixed");
        let initial = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(3).unwrap()[2].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.reset_to(&initial, ResetMode::Mixed).unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.list_commits(1).unwrap()[0].id, initial);
        let status = repo.status_detail().unwrap();
        assert!(status.staged.is_empty());
        assert!(!status.unstaged.is_empty());
    }

    #[test]
    fn reset_to_hard_moves_head_and_discards_worktree_changes() {
        let repo_dir = three_commit_history("reset-hard");
        let initial = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(3).unwrap()[2].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.reset_to(&initial, ResetMode::Hard).unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.list_commits(1).unwrap()[0].id, initial);
        let status = repo.status_detail().unwrap();
        assert!(status.staged.is_empty());
        assert!(status.unstaged.is_empty());
        let contents = std::fs::read_to_string(repo_dir.path.join("file.txt")).unwrap();
        assert_eq!(contents, "initial\n");
    }

    #[test]
    fn reset_to_rejects_invalid_commit_id() {
        let repo_dir = TempRepo::init_with_commit("reset-invalid");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.reset_to("  ", ResetMode::Soft).unwrap_err();
        assert!(matches!(err, Error::InvalidCommit(_)));

        let err = repo.reset_to("-bad", ResetMode::Mixed).unwrap_err();
        assert!(matches!(err, Error::InvalidCommit(_)));
    }
}
