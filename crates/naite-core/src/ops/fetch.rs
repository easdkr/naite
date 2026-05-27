use crate::repo::Repository;
use crate::Error;

impl Repository {
    pub fn fetch_current_remote(&self) -> Result<(), Error> {
        let upstream = self.current_upstream()?.ok_or(Error::NoUpstream)?;
        let remote = upstream
            .split_once('/')
            .map(|(remote, _)| remote)
            .filter(|remote| !remote.is_empty())
            .ok_or(Error::NoUpstream)?;

        let _ = self.git(&["fetch", "--tags", remote])?;
        Ok(())
    }

    pub fn fetch_all_remotes(&self) -> Result<(), Error> {
        let _ = self.git(&["fetch", "--all", "--tags"])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command;
    use crate::refs::BranchSyncStatus;
    use crate::test_helpers::*;
    use std::process::Command;

    #[test]
    fn fetch_current_remote_requires_upstream() {
        let repo_dir = TempRepo::init_with_commit("fetch-no-upstream");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.fetch_current_remote().unwrap_err();

        assert!(matches!(err, Error::NoUpstream));
    }

    #[test]
    fn fetch_current_remote_updates_current_upstream_tracking_ref() {
        let remote = TempRepo::new("fetch-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("fetch-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("fetch-local-parent");
        let local_path = local_parent.path.join("local");
        let output = Command::new("git")
            .args([
                "clone",
                "--branch",
                "main",
                remote.path.to_str().unwrap(),
                local_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let repo = Repository::open(&local_path).unwrap();
        assert_eq!(
            repo.branch_sync_status().unwrap(),
            BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            }
        );

        source.write("file.txt", "remote change\n");
        source.git(&["add", "file.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);

        let before_fetch = command::run_git(&local_path, ["rev-parse", "origin/main"]).unwrap();
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_ne!(before_fetch.trim(), source_head.trim());

        repo.fetch_current_remote().unwrap();

        let after_fetch = command::run_git(&local_path, ["rev-parse", "origin/main"]).unwrap();
        assert_eq!(after_fetch.trim(), source_head.trim());
        assert_eq!(
            Repository::open(&local_path)
                .unwrap()
                .branch_sync_status()
                .unwrap(),
            BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 1,
            }
        );
    }

    #[test]
    fn fetch_current_remote_fetches_tags_added_to_existing_commits() {
        let remote = TempRepo::new("fetch-tag-existing-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("fetch-tag-existing-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("fetch-tag-existing-local-parent");
        let local_path = clone_main(&remote, &local_parent);

        source.git(&["tag", "v1.0.0"]);
        source.git(&["push", "origin", "refs/tags/v1.0.0"]);

        assert!(
            command::run_git(&local_path, ["rev-parse", "--verify", "refs/tags/v1.0.0"]).is_err()
        );

        let repo = Repository::open(&local_path).unwrap();
        repo.fetch_current_remote().unwrap();

        let local_tag =
            command::run_git(&local_path, ["rev-parse", "--verify", "refs/tags/v1.0.0"]).unwrap();
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(local_tag.trim(), source_head.trim());
    }

    #[test]
    fn fetch_all_remotes_updates_each_remote_tracking_ref() {
        let origin = TempRepo::new("fetch-all-origin");
        origin.git(&["init", "--bare"]);
        let backup = TempRepo::new("fetch-all-backup");
        backup.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("fetch-all-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", origin.path.to_str().unwrap()]);
        source.git(&["remote", "add", "backup", backup.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        source.git(&["push", "backup", "main"]);
        origin.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
        backup.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("fetch-all-local-parent");
        let local_path = local_parent.path.join("local");
        let output = Command::new("git")
            .args([
                "clone",
                "--branch",
                "main",
                origin.path.to_str().unwrap(),
                local_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        command::run_git(
            &local_path,
            ["remote", "add", "backup", backup.path.to_str().unwrap()],
        )
        .unwrap();
        command::run_git(&local_path, ["fetch", "backup"]).unwrap();

        source.write("file.txt", "remote change\n");
        source.git(&["add", "file.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);
        source.git(&["push", "backup", "main"]);

        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        let before_origin = command::run_git(&local_path, ["rev-parse", "origin/main"]).unwrap();
        let before_backup = command::run_git(&local_path, ["rev-parse", "backup/main"]).unwrap();
        assert_ne!(before_origin.trim(), source_head.trim());
        assert_ne!(before_backup.trim(), source_head.trim());

        let repo = Repository::open(&local_path).unwrap();
        repo.fetch_all_remotes().unwrap();

        let after_origin = command::run_git(&local_path, ["rev-parse", "origin/main"]).unwrap();
        let after_backup = command::run_git(&local_path, ["rev-parse", "backup/main"]).unwrap();
        assert_eq!(after_origin.trim(), source_head.trim());
        assert_eq!(after_backup.trim(), source_head.trim());
    }

    #[test]
    fn fetch_all_remotes_fetches_tags_added_to_existing_commits() {
        let origin = TempRepo::new("fetch-all-tag-origin");
        origin.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("fetch-all-tag-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", origin.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        origin.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let local_parent = TempRepo::new("fetch-all-tag-local-parent");
        let local_path = clone_main(&origin, &local_parent);

        source.git(&["tag", "v1.0.0"]);
        source.git(&["push", "origin", "refs/tags/v1.0.0"]);

        assert!(
            command::run_git(&local_path, ["rev-parse", "--verify", "refs/tags/v1.0.0"]).is_err()
        );

        let repo = Repository::open(&local_path).unwrap();
        repo.fetch_all_remotes().unwrap();

        let local_tag =
            command::run_git(&local_path, ["rev-parse", "--verify", "refs/tags/v1.0.0"]).unwrap();
        let source_head = source.git_output(&["rev-parse", "HEAD"]);
        assert_eq!(local_tag.trim(), source_head.trim());
    }
}
