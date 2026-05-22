use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullMode {
    FastForwardOnly,
    FastForward,
    Rebase,
}

impl Repository {
    pub fn pull_fast_forward_only(&self) -> Result<(), Error> {
        self.pull(PullMode::FastForwardOnly)
    }

    pub fn pull(&self, mode: PullMode) -> Result<(), Error> {
        let _ = self.current_upstream()?.ok_or(Error::NoUpstream)?;
        let args = match mode {
            PullMode::FastForwardOnly => ["pull", "--ff-only"],
            PullMode::FastForward => ["pull", "--ff"],
            PullMode::Rebase => ["pull", "--rebase"],
        };
        let _ = self.git(&args)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command;
    use crate::refs::BranchSyncStatus;
    use crate::test_helpers::*;
    use std::fs;

    #[test]
    fn pull_fast_forward_only_requires_upstream() {
        let repo_dir = TempRepo::init_with_commit("pull-no-upstream");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.pull_fast_forward_only().unwrap_err();

        assert!(matches!(err, Error::NoUpstream));
    }

    #[test]
    fn pull_fast_forward_only_advances_current_branch() {
        let remote = TempRepo::new("pull-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("pull-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("pull-local-parent");
        let local_path = clone_main(&remote, &local_parent);

        source.write("file.txt", "remote change\n");
        source.git(&["add", "file.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);

        let repo = Repository::open(&local_path).unwrap();
        repo.pull_fast_forward_only().unwrap();

        let local_head = command::run_git(&local_path, ["rev-parse", "HEAD"]).unwrap();
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(local_head.trim(), source_head.trim());
        assert_eq!(
            Repository::open(&local_path)
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
    fn pull_fast_forward_advances_current_branch() {
        let remote = TempRepo::new("pull-ff-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("pull-ff-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("pull-ff-local-parent");
        let local_path = clone_main(&remote, &local_parent);

        source.write("file.txt", "remote change\n");
        source.git(&["add", "file.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);

        let repo = Repository::open(&local_path).unwrap();
        repo.pull(PullMode::FastForward).unwrap();

        let local_head = command::run_git(&local_path, ["rev-parse", "HEAD"]).unwrap();
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(local_head.trim(), source_head.trim());
    }

    #[test]
    fn pull_rebase_replays_local_commit_on_remote_head() {
        let remote = TempRepo::new("pull-rebase-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("pull-rebase-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("pull-rebase-local-parent");
        let local_path = clone_main(&remote, &local_parent);
        command::run_git(&local_path, ["config", "user.name", "naite test"]).unwrap();
        command::run_git(&local_path, ["config", "user.email", "naite@example.com"]).unwrap();

        source.write("remote.txt", "remote change\n");
        source.git(&["add", "remote.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);

        fs::write(local_path.join("local.txt"), "local change\n").unwrap();
        command::run_git(&local_path, ["add", "local.txt"]).unwrap();
        command::run_git(&local_path, ["commit", "-m", "local change"]).unwrap();

        let repo = Repository::open(&local_path).unwrap();
        repo.pull(PullMode::Rebase).unwrap();

        let messages = command::run_git(&local_path, ["log", "--format=%s", "-2"]).unwrap();
        assert_eq!(messages.trim(), "local change\nremote change");
        assert_eq!(
            Repository::open(&local_path)
                .unwrap()
                .branch_sync_status()
                .unwrap(),
            BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 0,
            }
        );
    }

    #[test]
    fn pull_fast_forward_only_rejects_diverged_branch_without_moving_head() {
        let remote = TempRepo::new("pull-diverged-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("pull-diverged-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("pull-diverged-local-parent");
        let local_path = clone_main(&remote, &local_parent);
        command::run_git(&local_path, ["config", "user.name", "naite test"]).unwrap();
        command::run_git(&local_path, ["config", "user.email", "naite@example.com"]).unwrap();

        source.write("file.txt", "remote change\n");
        source.git(&["add", "file.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);

        fs::write(local_path.join("local.txt"), "local change\n").unwrap();
        command::run_git(&local_path, ["add", "local.txt"]).unwrap();
        command::run_git(&local_path, ["commit", "-m", "local change"]).unwrap();
        let before_pull = command::run_git(&local_path, ["rev-parse", "HEAD"]).unwrap();

        let repo = Repository::open(&local_path).unwrap();
        let err = repo.pull_fast_forward_only().unwrap_err();

        assert!(matches!(err, Error::GitCommand { .. }));
        let after_pull = command::run_git(&local_path, ["rev-parse", "HEAD"]).unwrap();
        assert_eq!(after_pull.trim(), before_pull.trim());
    }
}
