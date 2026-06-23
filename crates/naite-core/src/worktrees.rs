use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::command;
use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorktreeSummary {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head_short_id: String,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub locked: bool,
    pub lock_reason: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeAdd {
    pub path: PathBuf,
    pub start_point: String,
    pub new_branch: Option<String>,
}

impl Repository {
    pub fn list_worktrees(&self) -> Result<Vec<WorktreeSummary>, Error> {
        let output = self.git(&["worktree", "list", "--porcelain", "-z"])?;
        let current_path = canonicalize_best_effort(self.workdir().unwrap_or(self.path()));
        let mut worktrees = parse_worktree_list_porcelain_z(&output);

        for worktree in &mut worktrees {
            worktree.is_current = canonicalize_best_effort(&worktree.path) == current_path;
            if let Ok(repo) = Repository::open(&worktree.path) {
                worktree.dirty = repo
                    .status()
                    .map(|status| status.is_dirty())
                    .unwrap_or(false);
                if let Ok(sync_status) = repo.branch_sync_status() {
                    worktree.ahead = sync_status.ahead;
                    worktree.behind = sync_status.behind;
                }
            }
        }

        Ok(worktrees)
    }

    pub fn add_worktree(&self, add: &WorktreeAdd) -> Result<PathBuf, Error> {
        let path = validate_worktree_path(&add.path)?;
        let start_point = validate_start_point(&add.start_point)?;

        let cwd = self.workdir().unwrap_or(self.path());
        if let Some(branch) = add
            .new_branch
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            validate_branch_name(branch)?;
            let _ = self.git(&["check-ref-format", "--branch", branch])?;
            let _ = command::run_git(
                cwd,
                [
                    OsStr::new("worktree"),
                    OsStr::new("add"),
                    OsStr::new("-b"),
                    OsStr::new(branch),
                    path.as_os_str(),
                    OsStr::new(start_point),
                ],
            )?;
        } else {
            let _ = command::run_git(
                cwd,
                [
                    OsStr::new("worktree"),
                    OsStr::new("add"),
                    path.as_os_str(),
                    OsStr::new(start_point),
                ],
            )?;
        }

        Ok(path.to_path_buf())
    }

    pub fn remove_worktree(
        &self,
        path: impl AsRef<Path>,
        delete_branch: bool,
        force: bool,
    ) -> Result<(), Error> {
        let path = validate_worktree_path(path.as_ref())?;
        let branch = if delete_branch {
            Repository::open(path)
                .ok()
                .and_then(|repo| repo.head_branch())
        } else {
            None
        };

        let cwd = self.workdir().unwrap_or(self.path());
        if force {
            let _ = command::run_git(
                cwd,
                [
                    OsStr::new("worktree"),
                    OsStr::new("remove"),
                    OsStr::new("--force"),
                    path.as_os_str(),
                ],
            )?;
        } else {
            let _ = command::run_git(
                cwd,
                [
                    OsStr::new("worktree"),
                    OsStr::new("remove"),
                    path.as_os_str(),
                ],
            )?;
        }

        if let Some(branch) = branch {
            if self.head_branch().as_deref() != Some(branch.as_str()) {
                self.force_delete_local_branch(&branch)?;
            }
        }

        Ok(())
    }

    pub fn lock_worktree(&self, path: impl AsRef<Path>, reason: Option<&str>) -> Result<(), Error> {
        let path = validate_worktree_path(path.as_ref())?;
        let cwd = self.workdir().unwrap_or(self.path());
        if let Some(reason) = reason.map(str::trim).filter(|v| !v.is_empty()) {
            let _ = command::run_git(
                cwd,
                [
                    OsStr::new("worktree"),
                    OsStr::new("lock"),
                    OsStr::new("--reason"),
                    OsStr::new(reason),
                    path.as_os_str(),
                ],
            )?;
        } else {
            let _ = command::run_git(
                cwd,
                [OsStr::new("worktree"), OsStr::new("lock"), path.as_os_str()],
            )?;
        }
        Ok(())
    }

    pub fn unlock_worktree(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let path = validate_worktree_path(path.as_ref())?;
        let cwd = self.workdir().unwrap_or(self.path());
        let _ = command::run_git(
            cwd,
            [
                OsStr::new("worktree"),
                OsStr::new("unlock"),
                path.as_os_str(),
            ],
        )?;
        Ok(())
    }
}

fn parse_worktree_list_porcelain_z(output: &str) -> Vec<WorktreeSummary> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeSummary> = None;

    for record in output.split('\0') {
        if record.is_empty() {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            continue;
        }

        if let Some(path) = record.strip_prefix("worktree ") {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            current = Some(WorktreeSummary {
                path: PathBuf::from(path),
                ..WorktreeSummary::default()
            });
            continue;
        }

        let Some(worktree) = current.as_mut() else {
            continue;
        };

        if let Some(head) = record.strip_prefix("HEAD ") {
            worktree.head_short_id = head.chars().take(7).collect();
        } else if let Some(branch) = record.strip_prefix("branch ") {
            worktree.branch = Some(strip_ref_prefix(branch).to_string());
        } else if let Some(reason) = record.strip_prefix("locked") {
            worktree.locked = true;
            let reason = reason.trim();
            if !reason.is_empty() {
                worktree.lock_reason = Some(reason.to_string());
            }
        } else if record == "detached" {
            worktree.branch = None;
        }
    }

    if let Some(worktree) = current {
        worktrees.push(worktree);
    }

    worktrees
}

fn validate_worktree_path(path: &Path) -> Result<&Path, Error> {
    if path.as_os_str().is_empty() {
        return Err(Error::InvalidPath(path.display().to_string()));
    }
    Ok(path)
}

fn validate_start_point(start_point: &str) -> Result<&str, Error> {
    let start_point = start_point.trim();
    if start_point.is_empty() || start_point.starts_with('-') {
        return Err(Error::InvalidRefName(start_point.to_string()));
    }
    Ok(start_point)
}

fn validate_branch_name(branch: &str) -> Result<&str, Error> {
    let branch = branch.trim();
    if branch.is_empty() || branch.starts_with('-') {
        return Err(Error::InvalidRefName(branch.to_string()));
    }
    Ok(branch)
}

fn strip_ref_prefix(full: &str) -> &str {
    full.strip_prefix("refs/heads/")
        .or_else(|| full.strip_prefix("refs/remotes/"))
        .or_else(|| full.strip_prefix("refs/tags/"))
        .unwrap_or(full)
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn parses_worktree_list_with_lock_reason_and_branch() {
        let raw = concat!(
            "worktree /tmp/main\0",
            "HEAD 1234567890abcdef\0",
            "branch refs/heads/main\0",
            "\0",
            "worktree /tmp/feature\0",
            "HEAD abcdef1234567890\0",
            "branch refs/heads/feature/demo\0",
            "locked do not remove\0",
            "\0",
        );

        let parsed = parse_worktree_list_porcelain_z(raw);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path, PathBuf::from("/tmp/main"));
        assert_eq!(parsed[0].branch.as_deref(), Some("main"));
        assert_eq!(parsed[0].head_short_id, "1234567");
        assert!(!parsed[0].locked);
        assert_eq!(parsed[1].branch.as_deref(), Some("feature/demo"));
        assert!(parsed[1].locked);
        assert_eq!(parsed[1].lock_reason.as_deref(), Some("do not remove"));
    }

    #[test]
    fn list_worktrees_marks_dirty_and_current_worktrees() {
        let repo_dir = TempRepo::init_with_commit("worktree-list");
        repo_dir.git(&["branch", "-M", "main"]);
        repo_dir.git(&["branch", "feature/worktree"]);
        let sibling = repo_dir.path.with_file_name(format!(
            "{}-linked",
            repo_dir.path.file_name().unwrap().to_string_lossy()
        ));
        repo_dir.git(&[
            "worktree",
            "add",
            sibling.to_str().unwrap(),
            "feature/worktree",
        ]);
        std::fs::write(sibling.join("dirty.txt"), "dirty\n").unwrap();
        let sibling = sibling.canonicalize().unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        let worktrees = repo.list_worktrees().unwrap();

        assert!(worktrees
            .iter()
            .any(|worktree| worktree.is_current && worktree.branch.as_deref() == Some("main")));
        assert!(worktrees.iter().any(|worktree| {
            worktree.path == sibling
                && worktree.branch.as_deref() == Some("feature/worktree")
                && worktree.dirty
        }));

        let _ = std::fs::remove_dir_all(sibling);
    }

    #[test]
    fn add_lock_unlock_and_remove_worktree() {
        let repo_dir = TempRepo::init_with_commit("worktree-ops");
        repo_dir.git(&["branch", "-M", "main"]);
        let sibling = repo_dir.path.with_file_name(format!(
            "{}-linked",
            repo_dir.path.file_name().unwrap().to_string_lossy()
        ));

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.add_worktree(&WorktreeAdd {
            path: sibling.clone(),
            start_point: "main".into(),
            new_branch: Some("feature/linked".into()),
        })
        .unwrap();

        let linked = Repository::open(&sibling).unwrap();
        assert_eq!(linked.head_branch().as_deref(), Some("feature/linked"));
        let sibling = sibling.canonicalize().unwrap();

        repo.lock_worktree(&sibling, Some("phase3")).unwrap();
        let worktrees = repo.list_worktrees().unwrap();
        assert!(worktrees.iter().any(|worktree| {
            worktree.path == sibling
                && worktree.locked
                && worktree.lock_reason.as_deref() == Some("phase3")
        }));

        repo.unlock_worktree(&sibling).unwrap();
        repo.remove_worktree(&sibling, true, false).unwrap();

        let refs = Repository::open(&repo_dir.path).unwrap().refs().unwrap();
        assert!(!refs
            .local
            .iter()
            .any(|branch| branch.short_name == "feature/linked"));
        assert!(!sibling.exists());
    }

    #[test]
    fn remove_worktree_force_clears_dirty_changes() {
        let repo_dir = TempRepo::init_with_commit("worktree-force");
        repo_dir.git(&["branch", "-M", "main"]);
        let sibling = repo_dir.path.with_file_name(format!(
            "{}-linked",
            repo_dir.path.file_name().unwrap().to_string_lossy()
        ));

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.add_worktree(&WorktreeAdd {
            path: sibling.clone(),
            start_point: "main".into(),
            new_branch: Some("feature/dirty".into()),
        })
        .unwrap();

        // Make the linked worktree dirty so plain `git worktree remove` refuses.
        std::fs::write(sibling.join("dirty.txt"), "unsaved\n").unwrap();
        let sibling = sibling.canonicalize().unwrap();

        // Plain remove must fail — this is the bug the force fallback addresses.
        let repo = Repository::open(&repo_dir.path).unwrap();
        assert!(repo.remove_worktree(&sibling, false, false).is_err());

        // Force remove succeeds even with uncommitted changes.
        repo.remove_worktree(&sibling, false, true).unwrap();
        assert!(!sibling.exists());
    }
}
