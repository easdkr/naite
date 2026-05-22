use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushMode {
    Normal,
    ForceWithLease,
}

impl Repository {
    pub fn push_current_branch(&self, mode: PushMode) -> Result<(), Error> {
        if self.current_upstream()?.is_some() {
            let args: &[&str] = match mode {
                PushMode::Normal => &["push"],
                PushMode::ForceWithLease => &["push", "--force-with-lease"],
            };
            let _ = self.git(args)?;
            return Ok(());
        }

        // No upstream: force-with-lease has no remote-tracking ref to compare
        // against, so refuse rather than degrade to a plain push.
        if matches!(mode, PushMode::ForceWithLease) {
            return Err(Error::NoUpstream);
        }

        let branch_name = self.head_branch().ok_or(Error::NoCurrentBranch)?;
        if branch_name.trim().is_empty() || branch_name.starts_with('-') {
            return Err(Error::InvalidRefName(branch_name));
        }

        let _ = self.git(&["check-ref-format", "--branch", &branch_name])?;
        let _ = self.git(&["push", "-u", "origin", &branch_name])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::BranchSyncStatus;
    use crate::test_helpers::*;

    #[test]
    fn push_current_branch_requires_attached_branch() {
        let repo_dir = TempRepo::init_with_commit("push-detached");
        repo_dir.git(&["checkout", "--detach", "HEAD"]);
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.push_current_branch(PushMode::Normal).unwrap_err();

        assert!(matches!(err, Error::NoCurrentBranch));
    }

    #[test]
    fn push_current_branch_sets_upstream_when_missing() {
        let remote = TempRepo::new("push-set-upstream-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("push-set-upstream-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);

        let repo = Repository::open(&source.path).unwrap();
        assert_eq!(
            repo.branch_sync_status().unwrap(),
            BranchSyncStatus::default()
        );

        repo.push_current_branch(PushMode::Normal).unwrap();

        let remote_head = remote.git_output(&["rev-parse", "refs/heads/main"]);
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(remote_head.trim(), source_head.trim());
        assert_eq!(
            Repository::open(&source.path)
                .unwrap()
                .branch_sync_status()
                .unwrap(),
            BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            }
        );
    }

    #[test]
    fn push_current_branch_updates_existing_upstream() {
        let remote = TempRepo::new("push-existing-upstream-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("push-existing-upstream-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);

        source.write("file.txt", "local change\n");
        source.git(&["add", "file.txt"]);
        source.git(&["commit", "-m", "local change"]);

        let repo = Repository::open(&source.path).unwrap();
        assert_eq!(
            repo.branch_sync_status().unwrap(),
            BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 0,
            }
        );

        repo.push_current_branch(PushMode::Normal).unwrap();

        let remote_head = remote.git_output(&["rev-parse", "refs/heads/main"]);
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(remote_head.trim(), source_head.trim());
        assert_eq!(
            Repository::open(&source.path)
                .unwrap()
                .branch_sync_status()
                .unwrap(),
            BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            }
        );
    }

    #[test]
    fn force_with_lease_requires_upstream() {
        let repo_dir = TempRepo::init_with_commit("push-force-no-upstream");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo
            .push_current_branch(PushMode::ForceWithLease)
            .unwrap_err();

        assert!(matches!(err, Error::NoUpstream));
    }

    #[test]
    fn force_with_lease_overwrites_diverged_remote() {
        let remote = TempRepo::new("push-force-lease-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("push-force-lease-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);

        // Diverge: rewrite local history so the remote tip is no longer an
        // ancestor.
        source.write("a.txt", "v1\n");
        source.git(&["add", "a.txt"]);
        source.git(&["commit", "--amend", "--no-edit"]);

        let repo = Repository::open(&source.path).unwrap();
        let plain_push_err = repo.push_current_branch(PushMode::Normal).unwrap_err();
        assert!(matches!(plain_push_err, Error::GitCommand { .. }));

        repo.push_current_branch(PushMode::ForceWithLease).unwrap();

        let remote_head = remote.git_output(&["rev-parse", "refs/heads/main"]);
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(remote_head.trim(), source_head.trim());
    }

    #[test]
    fn force_with_lease_refuses_when_remote_moved_unseen() {
        // Set up a remote with an additional commit that the local has never
        // fetched. --force-with-lease should refuse to overwrite it because the
        // local remote-tracking ref disagrees with the actual remote tip.
        let remote = TempRepo::new("push-force-lease-stale-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("push-force-lease-stale-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);

        // Another worker pushes a commit to the remote that `source` never sees.
        let other_parent = TempRepo::new("push-force-lease-stale-other");
        let other_path = clone_main(&remote, &other_parent);
        std::fs::write(other_path.join("remote.txt"), "remote change\n").unwrap();
        crate::command::run_git(&other_path, ["add", "remote.txt"]).unwrap();
        crate::command::run_git(&other_path, ["commit", "-m", "remote change"]).unwrap();
        crate::command::run_git(&other_path, ["push", "origin", "main"]).unwrap();

        // Local diverges from its (stale) view of the remote.
        source.write("a.txt", "v1\n");
        source.git(&["add", "a.txt"]);
        source.git(&["commit", "--amend", "--no-edit"]);

        let repo = Repository::open(&source.path).unwrap();
        let err = repo
            .push_current_branch(PushMode::ForceWithLease)
            .unwrap_err();

        assert!(matches!(err, Error::GitCommand { .. }));

        // Remote tip still matches the other worker's commit, not the local
        // amended one.
        let other_head = crate::command::run_git(&other_path, ["rev-parse", "HEAD"]).unwrap();
        let remote_head = remote.git_output(&["rev-parse", "refs/heads/main"]);
        assert_eq!(other_head.trim(), remote_head.trim());
    }
}
