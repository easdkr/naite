use std::collections::{BTreeMap, BTreeSet};

use crate::repo::Repository;
use crate::Error;

impl Repository {
    pub fn create_branch_and_checkout(
        &self,
        branch_name: &str,
        start_point: Option<&str>,
    ) -> Result<(), Error> {
        let branch_name = branch_name.trim();
        if branch_name.is_empty() || branch_name.starts_with('-') {
            return Err(Error::InvalidRefName(branch_name.to_string()));
        }

        let _ = self.git(&["check-ref-format", "--branch", branch_name])?;

        let mut args = vec!["switch", "-c", branch_name];
        if let Some(start_point) = start_point {
            args.push(start_point);
        }
        let _ = self.git(&args)?;
        Ok(())
    }

    pub fn rename_local_branch(&self, old_name: &str, new_name: &str) -> Result<(), Error> {
        let old_name = validate_branch_name(old_name)?;
        let new_name = validate_branch_name(new_name)?;

        let _ = self.git(&["check-ref-format", "--branch", old_name])?;
        self.ensure_local_branch(old_name)?;
        let _ = self.git(&["check-ref-format", "--branch", new_name])?;
        let _ = self.git(&["branch", "-m", old_name, new_name])?;
        Ok(())
    }

    pub fn delete_local_branch(&self, branch_name: &str) -> Result<(), Error> {
        self.delete_local_branches(&[branch_name.to_string()])
    }

    pub fn force_delete_local_branch(&self, branch_name: &str) -> Result<(), Error> {
        let branch_name = validate_branch_name(branch_name)?;
        let _ = self.git(&["check-ref-format", "--branch", branch_name])?;
        self.ensure_local_branch(branch_name)?;

        if self.head_branch().as_deref() == Some(branch_name) {
            return Err(Error::CannotDeleteCurrentBranch(branch_name.to_string()));
        }

        let _ = self.git(&["branch", "--delete", "--force", branch_name])?;
        Ok(())
    }

    pub fn delete_local_branches(&self, branch_names: &[String]) -> Result<(), Error> {
        let branch_names = branch_names
            .iter()
            .map(|branch_name| validate_branch_name(branch_name).map(str::to_string))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if branch_names.is_empty() {
            return Ok(());
        }

        for branch_name in &branch_names {
            let _ = self.git(&["check-ref-format", "--branch", branch_name])?;
            self.ensure_local_branch(branch_name)?;
        }

        if let Some(current) = self.head_branch() {
            if branch_names.contains(&current) {
                return Err(Error::CannotDeleteCurrentBranch(current));
            }
        }

        let mut args = vec!["branch", "--delete"];
        args.extend(branch_names.iter().map(String::as_str));
        let _ = self.git(&args)?;
        Ok(())
    }

    pub fn delete_remote_branches(
        &self,
        full_ref_names: &[String],
        delete_matching_local_branches: bool,
    ) -> Result<(), Error> {
        let mut branches_by_remote: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut matching_local_branches = BTreeSet::new();

        for full_ref_name in full_ref_names {
            let remote_branch = remote_branch_delete_target(full_ref_name)?;
            branches_by_remote
                .entry(remote_branch.remote.to_string())
                .or_default()
                .insert(remote_branch.branch.to_string());
            if delete_matching_local_branches
                && self.branch_local_branch_exists(remote_branch.branch)?
            {
                matching_local_branches.insert(remote_branch.branch.to_string());
            }
        }

        if delete_matching_local_branches {
            if let Some(current) = self.head_branch() {
                if matching_local_branches.contains(&current) {
                    return Err(Error::CannotDeleteCurrentBranch(current));
                }
            }
        }

        for (remote, branches) in &branches_by_remote {
            let mut args = vec!["push", remote.as_str(), "--delete"];
            args.extend(branches.iter().map(String::as_str));
            let _ = self.git(&args)?;

            for branch in branches {
                let tracking_ref = format!("{remote}/{branch}");
                if self.remote_tracking_branch_exists(&tracking_ref)? {
                    let full_tracking_ref = format!("refs/remotes/{tracking_ref}");
                    let _ = self.git(&["update-ref", "-d", &full_tracking_ref])?;
                }
            }
        }

        if delete_matching_local_branches {
            for branch in matching_local_branches {
                self.force_delete_local_branch(&branch)?;
            }
        }

        Ok(())
    }

    fn ensure_local_branch(&self, branch_name: &str) -> Result<(), Error> {
        let full_ref = format!("refs/heads/{branch_name}");
        let output =
            self.git_allowing_exit_codes(&["show-ref", "--verify", &full_ref], &[1, 128])?;
        if output.trim().is_empty() {
            return Err(Error::UnsupportedBranchTarget(branch_name.to_string()));
        }
        Ok(())
    }

    fn branch_local_branch_exists(&self, branch_name: &str) -> Result<bool, Error> {
        let branch_name = validate_branch_name(branch_name)?;
        let _ = self.git(&["check-ref-format", "--branch", branch_name])?;
        let full_ref = format!("refs/heads/{branch_name}");
        let output =
            self.git_allowing_exit_codes(&["show-ref", "--verify", &full_ref], &[1, 128])?;
        Ok(!output.trim().is_empty())
    }

    fn remote_tracking_branch_exists(&self, remote_branch_name: &str) -> Result<bool, Error> {
        let remote_branch_name = validate_branch_name(remote_branch_name)?;
        let _ = self.git(&["check-ref-format", "--branch", remote_branch_name])?;
        let full_ref = format!("refs/remotes/{remote_branch_name}");
        let output =
            self.git_allowing_exit_codes(&["show-ref", "--verify", &full_ref], &[1, 128])?;
        Ok(!output.trim().is_empty())
    }
}

fn validate_branch_name(branch_name: &str) -> Result<&str, Error> {
    let branch_name = branch_name.trim();
    if branch_name.is_empty() || branch_name.starts_with('-') {
        return Err(Error::InvalidRefName(branch_name.to_string()));
    }
    Ok(branch_name)
}

struct RemoteBranchDeleteTarget<'a> {
    remote: &'a str,
    branch: &'a str,
}

fn remote_branch_delete_target(full_ref_name: &str) -> Result<RemoteBranchDeleteTarget<'_>, Error> {
    let name = full_ref_name
        .strip_prefix("refs/remotes/")
        .ok_or_else(|| Error::UnsupportedBranchTarget(full_ref_name.to_string()))?;
    let (remote, branch) = name
        .split_once('/')
        .ok_or_else(|| Error::InvalidRefName(full_ref_name.to_string()))?;
    validate_branch_name(remote)?;
    validate_branch_name(branch)?;
    if branch == "HEAD" {
        return Err(Error::InvalidRefName(full_ref_name.to_string()));
    }
    Ok(RemoteBranchDeleteTarget { remote, branch })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn create_branch_and_checkout_from_head_switches_to_new_branch() {
        let repo_dir = TempRepo::init_with_commit("branch-create-head");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_branch_and_checkout("feature/head", None)
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/head"));
    }

    #[test]
    fn create_branch_and_checkout_from_selected_commit() {
        let repo_dir = TempRepo::init_with_commit("branch-create-commit");
        repo_dir.write("file.txt", "second\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "second"]);
        let first_commit = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(2).unwrap()[1].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_branch_and_checkout("feature/from-commit", Some(&first_commit))
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/from-commit"));
        assert_eq!(repo.list_commits(1).unwrap()[0].id, first_commit);
    }

    #[test]
    fn create_branch_and_checkout_preserves_dirty_worktree_from_head() {
        let repo_dir = TempRepo::init_with_commit("branch-create-dirty");
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_branch_and_checkout("feature/dirty", None)
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/dirty"));
        assert!(repo.status().unwrap().has_unstaged);
    }

    #[test]
    fn create_branch_and_checkout_rejects_invalid_branch_names() {
        let repo_dir = TempRepo::init_with_commit("branch-create-invalid");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let empty = repo.create_branch_and_checkout("  ", None).unwrap_err();
        assert!(matches!(empty, Error::InvalidRefName(_)));

        let option_like = repo.create_branch_and_checkout("-bad", None).unwrap_err();
        assert!(matches!(option_like, Error::InvalidRefName(_)));

        let invalid = repo
            .create_branch_and_checkout("feature..bad", None)
            .unwrap_err();
        assert!(matches!(invalid, Error::GitCommand { .. }));
    }

    #[test]
    fn rename_local_branch_renames_existing_branch() {
        let repo_dir = TempRepo::init_with_commit("branch-rename-existing");
        repo_dir.git(&["branch", "feature/old"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.rename_local_branch("feature/old", "feature/new")
            .unwrap();

        let refs = repo.refs().unwrap();
        assert!(refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/new"));
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/old"));
    }

    #[test]
    fn rename_local_branch_can_rename_current_branch() {
        let repo_dir = TempRepo::init_with_commit("branch-rename-current");
        let old_branch = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.head_branch().unwrap()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.rename_local_branch(&old_branch, "feature/current")
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/current"));
    }

    #[test]
    fn rename_local_branch_rejects_invalid_new_name() {
        let repo_dir = TempRepo::init_with_commit("branch-rename-invalid");
        repo_dir.git(&["branch", "feature/old"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo
            .rename_local_branch("feature/old", "feature..bad")
            .unwrap_err();

        assert!(matches!(err, Error::GitCommand { .. }));
    }

    #[test]
    fn rename_local_branch_rejects_non_local_target() {
        let repo_dir = TempRepo::init_with_commit("branch-rename-non-local");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo
            .rename_local_branch("origin/main", "feature/new")
            .unwrap_err();

        assert!(matches!(err, Error::UnsupportedBranchTarget(_)));
    }

    #[test]
    fn delete_local_branch_rejects_current_branch() {
        let repo_dir = TempRepo::init_with_commit("branch-delete-current");
        let current_branch = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.head_branch().unwrap()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo.delete_local_branch(&current_branch).unwrap_err();

        assert!(matches!(err, Error::CannotDeleteCurrentBranch(_)));
    }

    #[test]
    fn delete_local_branch_rejects_unmerged_branch_without_removing_it() {
        let repo_dir = TempRepo::init_with_commit("branch-delete-unmerged");
        repo_dir.git(&["switch", "-c", "feature/unmerged"]);
        repo_dir.write("file.txt", "feature\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "feature"]);
        repo_dir.git(&["switch", "-"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo.delete_local_branch("feature/unmerged").unwrap_err();

        assert!(matches!(err, Error::GitCommand { .. }));
        let refs = repo.refs().unwrap();
        assert!(refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/unmerged"));
    }

    #[test]
    fn delete_local_branch_deletes_merged_non_current_branch() {
        let repo_dir = TempRepo::init_with_commit("branch-delete-merged");
        repo_dir.git(&["branch", "feature/merged"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.delete_local_branch("feature/merged").unwrap();

        let refs = repo.refs().unwrap();
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/merged"));
    }

    #[test]
    fn force_delete_local_branch_deletes_unmerged_non_current_branch() {
        let repo_dir = TempRepo::init_with_commit("branch-force-delete-unmerged");
        repo_dir.git(&["switch", "-c", "feature/unmerged"]);
        repo_dir.write("file.txt", "feature\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "feature"]);
        repo_dir.git(&["switch", "-"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.force_delete_local_branch("feature/unmerged").unwrap();

        let refs = repo.refs().unwrap();
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/unmerged"));
    }

    #[test]
    fn delete_remote_branches_force_deletes_unmerged_matching_local_branch() {
        let (_remote, local_path) =
            remote_delete_fixture("remote-delete-unmerged-matching-local", &["claude/a"]);
        let local = TempRepo { path: local_path };
        local.git(&["switch", "-c", "claude/a", "--track", "origin/claude/a"]);
        local.write("file.txt", "local feature\n");
        local.git(&["add", "file.txt"]);
        local.git(&["commit", "-m", "local feature"]);
        local.git(&["switch", "main"]);
        let repo = Repository::open(&local.path).unwrap();

        repo.delete_remote_branches(&[String::from("refs/remotes/origin/claude/a")], true)
            .unwrap();

        let refs = repo.refs().unwrap();
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "claude/a"));
    }

    #[test]
    fn delete_local_branches_deletes_merged_folder_batch() {
        let repo_dir = TempRepo::init_with_commit("branch-delete-folder");
        repo_dir.git(&["branch", "feature/a"]);
        repo_dir.git(&["branch", "feature/b"]);
        repo_dir.git(&["branch", "other/a"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.delete_local_branches(&["feature/a".into(), "feature/b".into()])
            .unwrap();

        let refs = repo.refs().unwrap();
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/a"));
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/b"));
        assert!(refs
            .local
            .iter()
            .any(|branch| branch.short_name == "other/a"));
    }

    #[test]
    fn delete_remote_branches_deletes_remote_only_branch_and_tracking_ref() {
        let (_remote, local_path) = remote_delete_fixture("remote-delete-single", &["claude/a"]);
        let repo = Repository::open(&local_path).unwrap();

        repo.delete_remote_branches(&[String::from("refs/remotes/origin/claude/a")], false)
            .unwrap();

        assert!(!git_success(
            &local_path,
            &["show-ref", "--verify", "refs/remotes/origin/claude/a"]
        ));
        assert!(!git_success(
            repo.path(),
            &["ls-remote", "--exit-code", "origin", "refs/heads/claude/a"]
        ));
        assert!(!repo
            .refs()
            .unwrap()
            .local
            .iter()
            .any(|branch| branch.short_name == "claude/a"));
    }

    #[test]
    fn delete_remote_branches_deletes_folder_batch_without_sibling_prefix() {
        let (_remote, local_path) =
            remote_delete_fixture("remote-delete-folder", &["claude/a", "claude/b", "other/a"]);
        let repo = Repository::open(&local_path).unwrap();

        repo.delete_remote_branches(
            &[
                String::from("refs/remotes/origin/claude/a"),
                String::from("refs/remotes/origin/claude/b"),
            ],
            false,
        )
        .unwrap();

        assert!(!git_success(
            repo.path(),
            &["ls-remote", "--exit-code", "origin", "refs/heads/claude/a"]
        ));
        assert!(!git_success(
            repo.path(),
            &["ls-remote", "--exit-code", "origin", "refs/heads/claude/b"]
        ));
        assert!(git_success(
            repo.path(),
            &["ls-remote", "--exit-code", "origin", "refs/heads/other/a"]
        ));
    }

    #[test]
    fn delete_remote_branches_deletes_matching_local_by_name_only() {
        let (_remote, local_path) =
            remote_delete_fixture("remote-delete-matching-local", &["claude/a", "claude/b"]);
        let local = TempRepo { path: local_path };
        local.git(&["branch", "claude/a", "origin/claude/a"]);
        local.git(&["branch", "local-only", "origin/claude/a"]);
        local.git(&["branch", "claude/other", "origin/claude/a"]);
        let repo = Repository::open(&local.path).unwrap();

        repo.delete_remote_branches(&[String::from("refs/remotes/origin/claude/a")], true)
            .unwrap();

        let refs = repo.refs().unwrap();
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "claude/a"));
        assert!(refs
            .local
            .iter()
            .any(|branch| branch.short_name == "local-only"));
        assert!(refs
            .local
            .iter()
            .any(|branch| branch.short_name == "claude/other"));
    }

    #[test]
    fn delete_remote_branches_rejects_current_matching_local_before_remote_delete() {
        let (_remote, local_path) =
            remote_delete_fixture("remote-delete-current-local", &["claude/a"]);
        let local = TempRepo { path: local_path };
        local.git(&["switch", "-c", "claude/a", "--track", "origin/claude/a"]);
        let repo = Repository::open(&local.path).unwrap();

        let err = repo
            .delete_remote_branches(&[String::from("refs/remotes/origin/claude/a")], true)
            .unwrap_err();

        assert!(matches!(err, Error::CannotDeleteCurrentBranch(_)));
        assert!(git_success(
            repo.path(),
            &["ls-remote", "--exit-code", "origin", "refs/heads/claude/a"]
        ));
    }

    fn remote_delete_fixture(name: &str, branches: &[&str]) -> (TempRepo, PathBuf) {
        let remote = TempRepo::new(&format!("{name}-remote"));
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit(&format!("{name}-source"));
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        for branch in branches {
            source.git(&["branch", branch, "main"]);
            source.git(&["push", "origin", branch]);
        }

        let parent = TempRepo::new(&format!("{name}-parent"));
        let local_path = clone_main(&remote, &parent);
        std::mem::forget(parent);
        (remote, local_path)
    }

    fn git_success(path: &std::path::Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .unwrap()
            .status
            .success()
    }
}
