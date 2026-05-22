use crate::refs::RefKind;
use crate::repo::Repository;
use crate::Error;

impl Repository {
    pub fn checkout_ref(
        &self,
        full_ref_name: &str,
        kind: RefKind,
        force: bool,
    ) -> Result<(), Error> {
        match kind {
            RefKind::LocalBranch => {
                let branch_name = full_ref_name
                    .strip_prefix("refs/heads/")
                    .ok_or_else(|| Error::InvalidRefName(full_ref_name.to_string()))?;

                self.checkout_local_branch(branch_name, force)
            }
            RefKind::RemoteBranch => {
                let remote_branch = remote_tracking_branch(full_ref_name)?;
                self.checkout_remote_tracking_branch(&remote_branch, force)
            }
            RefKind::Tag => Err(Error::UnsupportedCheckoutTarget(full_ref_name.to_string())),
        }
    }

    pub fn force_sync_remote_branch(&self, full_ref_name: &str) -> Result<(), Error> {
        let remote_branch = remote_tracking_branch(full_ref_name)?;
        let _ = self.git(&["check-ref-format", "--branch", remote_branch.local_name])?;
        self.ensure_remote_branch(remote_branch.remote_name)?;

        if self.local_branch_exists(remote_branch.local_name)? {
            self.checkout_local_branch(remote_branch.local_name, true)?;
            let _ = self.git(&["reset", "--hard", remote_branch.remote_name])?;
            let _ = self.git(&[
                "branch",
                "--set-upstream-to",
                remote_branch.remote_name,
                remote_branch.local_name,
            ])?;
            return Ok(());
        }

        self.checkout_remote_tracking_branch(&remote_branch, true)
    }

    fn checkout_local_branch(&self, ref_name: &str, force: bool) -> Result<(), Error> {
        if ref_name.trim().is_empty() || ref_name.starts_with('-') {
            return Err(Error::InvalidRefName(ref_name.to_string()));
        }

        if !force && self.status()?.is_dirty() {
            return Err(Error::DirtyWorkdir);
        }

        let args = if force {
            vec!["checkout", "--force", ref_name]
        } else {
            vec!["checkout", ref_name]
        };
        let _ = self.git(&args)?;
        Ok(())
    }

    fn checkout_remote_tracking_branch(
        &self,
        remote_branch: &RemoteTrackingBranch<'_>,
        force: bool,
    ) -> Result<(), Error> {
        let _ = self.git(&["check-ref-format", "--branch", remote_branch.local_name])?;
        self.ensure_remote_branch(remote_branch.remote_name)?;

        if self.local_branch_exists(remote_branch.local_name)? {
            return self.checkout_local_branch(remote_branch.local_name, force);
        }

        if !force && self.status()?.is_dirty() {
            return Err(Error::DirtyWorkdir);
        }

        let args = if force {
            vec!["checkout", "--force", "--track", remote_branch.remote_name]
        } else {
            vec!["checkout", "--track", remote_branch.remote_name]
        };
        let _ = self.git(&args)?;
        Ok(())
    }

    fn ensure_remote_branch(&self, remote_branch_name: &str) -> Result<(), Error> {
        let full_ref = format!("refs/remotes/{remote_branch_name}");
        let _ = self.git(&["show-ref", "--verify", &full_ref])?;
        Ok(())
    }

    fn local_branch_exists(&self, local_branch_name: &str) -> Result<bool, Error> {
        let full_ref = format!("refs/heads/{local_branch_name}");
        let output =
            self.git_allowing_exit_codes(&["show-ref", "--verify", &full_ref], &[1, 128])?;
        Ok(!output.trim().is_empty())
    }
}

#[derive(Debug)]
struct RemoteTrackingBranch<'a> {
    remote_name: &'a str,
    local_name: &'a str,
}

fn remote_tracking_branch(full_ref_name: &str) -> Result<RemoteTrackingBranch<'_>, Error> {
    let branch_name = full_ref_name
        .strip_prefix("refs/remotes/")
        .ok_or_else(|| Error::InvalidRefName(full_ref_name.to_string()))?;
    let Some((remote, branch)) = branch_name.split_once('/') else {
        return Err(Error::InvalidRefName(full_ref_name.to_string()));
    };

    if remote.trim().is_empty()
        || branch.trim().is_empty()
        || branch == "HEAD"
        || remote.starts_with('-')
        || branch.starts_with('-')
    {
        return Err(Error::InvalidRefName(full_ref_name.to_string()));
    }

    Ok(RemoteTrackingBranch {
        remote_name: branch_name,
        local_name: branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn checkout_local_branch_switches_clean_worktree() {
        let repo_dir = TempRepo::init_with_commit("checkout-clean");
        repo_dir.git(&["branch", "feature"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.checkout_ref("refs/heads/feature", RefKind::LocalBranch, false)
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature"));
    }

    #[test]
    fn checkout_local_branch_rejects_dirty_worktree_without_force() {
        let repo_dir = TempRepo::init_with_commit("checkout-dirty");
        repo_dir.git(&["branch", "feature"]);
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo
            .checkout_ref("refs/heads/feature", RefKind::LocalBranch, false)
            .unwrap_err();

        assert!(matches!(err, Error::DirtyWorkdir));
    }

    #[test]
    fn checkout_remote_branch_creates_local_tracking_branch() {
        let remote = TempRepo::init_with_commit("checkout-remote-source");
        remote.git(&["switch", "-c", "feature/demo"]);
        remote.write("file.txt", "feature\n");
        remote.git(&["add", "file.txt"]);
        remote.git(&["commit", "-m", "feature"]);

        let parent = TempRepo::new("checkout-remote-parent");
        let local_path = clone_main(&remote, &parent);
        let repo = Repository::open(&local_path).unwrap();

        repo.checkout_ref(
            "refs/remotes/origin/feature/demo",
            RefKind::RemoteBranch,
            false,
        )
        .unwrap();

        let repo = Repository::open(&local_path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/demo"));
        assert_eq!(
            repo.current_upstream().unwrap().as_deref(),
            Some("origin/feature/demo")
        );
    }

    #[test]
    fn checkout_remote_branch_rejects_dirty_worktree_without_force() {
        let remote = TempRepo::init_with_commit("checkout-remote-dirty-source");
        remote.git(&["branch", "feature/demo"]);

        let parent = TempRepo::new("checkout-remote-dirty-parent");
        let local_path = clone_main(&remote, &parent);
        let repo_dir = TempRepo { path: local_path };
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo
            .checkout_ref(
                "refs/remotes/origin/feature/demo",
                RefKind::RemoteBranch,
                false,
            )
            .unwrap_err();

        assert!(matches!(err, Error::DirtyWorkdir));
    }

    #[test]
    fn checkout_remote_branch_force_allows_dirty_worktree() {
        let remote = TempRepo::init_with_commit("checkout-remote-force-source");
        remote.git(&["branch", "feature/demo"]);

        let parent = TempRepo::new("checkout-remote-force-parent");
        let local_path = clone_main(&remote, &parent);
        let repo_dir = TempRepo { path: local_path };
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.checkout_ref(
            "refs/remotes/origin/feature/demo",
            RefKind::RemoteBranch,
            true,
        )
        .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/demo"));
    }

    #[test]
    fn checkout_remote_branch_rejects_origin_head_and_malformed_refs() {
        for full_ref_name in [
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin",
            "refs/remotes//feature/demo",
            "refs/remotes/origin/",
            "refs/remotes/-origin/feature/demo",
            "refs/remotes/origin/-bad",
        ] {
            let err = remote_tracking_branch(full_ref_name).unwrap_err();
            assert!(matches!(err, Error::InvalidRefName(_)), "{full_ref_name}");
        }
    }

    #[test]
    fn checkout_remote_branch_uses_existing_local_branch_when_name_exists() {
        let remote = TempRepo::init_with_commit("checkout-remote-existing-source");
        remote.git(&["branch", "feature/demo"]);

        let parent = TempRepo::new("checkout-remote-existing-parent");
        let local_path = clone_main(&remote, &parent);
        let repo_dir = TempRepo { path: local_path };
        repo_dir.git(&["branch", "feature/demo"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.checkout_ref(
            "refs/remotes/origin/feature/demo",
            RefKind::RemoteBranch,
            false,
        )
        .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/demo"));
    }

    #[test]
    fn force_sync_remote_branch_resets_existing_local_branch_to_remote() {
        let remote = TempRepo::init_with_commit("force-sync-remote-source");
        remote.git(&["switch", "-c", "feature/demo"]);
        remote.write("file.txt", "remote\n");
        remote.git(&["add", "file.txt"]);
        remote.git(&["commit", "-m", "remote"]);
        let remote_head = remote.git_output(&["rev-parse", "feature/demo"]);

        let parent = TempRepo::new("force-sync-remote-parent");
        let local_path = clone_main(&remote, &parent);
        let repo_dir = TempRepo { path: local_path };
        repo_dir.git(&["switch", "-c", "feature/demo"]);
        repo_dir.write("file.txt", "local\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "local"]);
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.force_sync_remote_branch("refs/remotes/origin/feature/demo")
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/demo"));
        assert_eq!(
            repo.git(&["rev-parse", "HEAD"]).unwrap().trim(),
            remote_head.trim()
        );
        assert_eq!(
            repo.current_upstream().unwrap().as_deref(),
            Some("origin/feature/demo")
        );
        assert!(!repo.status().unwrap().has_unstaged);
    }
}
