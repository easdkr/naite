use crate::repo::Repository;
use crate::Error;

impl Repository {
    /// Apply the changes from `commit_id` onto the current branch as a new
    /// commit. Conflicts are surfaced by the worktree status; exit code 1
    /// from git cherry-pick is accepted so the conflict state is visible.
    pub fn cherry_pick(&self, commit_id: &str) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let _ = self.git(&["rev-parse", "--verify", commit_id])?;
        let _ = self.git_allowing_exit_codes(&["cherry-pick", commit_id], &[1])?;
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

    #[test]
    fn cherry_pick_applies_commit_from_other_branch_onto_current_branch() {
        let repo_dir = TempRepo::init_with_commit("cherry-pick-other-branch");
        repo_dir.git(&["branch", "-M", "main"]);
        repo_dir.git(&["switch", "-c", "feature"]);
        repo_dir.write("feature.txt", "feature\n");
        repo_dir.git(&["add", "feature.txt"]);
        repo_dir.git(&["commit", "-m", "feature commit"]);
        let feature_commit = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(1).unwrap()[0].id.clone()
        };
        repo_dir.git(&["switch", "main"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.cherry_pick(&feature_commit).unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        let subjects = repo_dir.git_output(&["log", "--format=%s"]);
        assert!(subjects.contains("feature commit"));
        assert!(repo_dir.path.join("feature.txt").exists());
        assert_eq!(repo.head_branch().as_deref(), Some("main"));
    }

    #[test]
    fn cherry_pick_rejects_invalid_commit_id() {
        let repo_dir = TempRepo::init_with_commit("cherry-pick-invalid");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.cherry_pick("  ").unwrap_err();
        assert!(matches!(err, Error::InvalidCommit(_)));

        let err = repo.cherry_pick("-bad").unwrap_err();
        assert!(matches!(err, Error::InvalidCommit(_)));
    }
}
