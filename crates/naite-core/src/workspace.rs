use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceRepoSummary {
    pub path: PathBuf,
    pub name: String,
    pub remote: Option<String>,
    pub current_branch: Option<String>,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub worktree_count: usize,
    pub dirty_worktree_count: usize,
    pub last_fetch_seconds: Option<i64>,
    pub error: Option<String>,
}

impl Repository {
    pub fn workspace_summary(path: impl AsRef<Path>) -> WorkspaceRepoSummary {
        let path = path.as_ref().to_path_buf();
        match Self::workspace_summary_result(&path) {
            Ok(summary) => summary,
            Err(err) => WorkspaceRepoSummary {
                name: repo_name(&path),
                path,
                error: Some(err.to_string()),
                ..WorkspaceRepoSummary::default()
            },
        }
    }

    fn workspace_summary_result(path: &Path) -> Result<WorkspaceRepoSummary, Error> {
        let repo = Repository::open(path)?;
        let workdir = repo.workdir().unwrap_or(repo.path());
        let canonical_path = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());
        let status = repo.status()?;
        let sync = repo.branch_sync_status().unwrap_or_default();
        let worktrees = repo.list_worktrees().unwrap_or_default();
        let remote = repo.origin_remote_url().ok().flatten();
        let last_fetch_seconds = repo.last_fetch_seconds();

        Ok(WorkspaceRepoSummary {
            name: repo_name(&canonical_path),
            path: canonical_path,
            remote,
            current_branch: repo.head_branch(),
            dirty: status.is_dirty(),
            ahead: sync.ahead,
            behind: sync.behind,
            worktree_count: worktrees.len(),
            dirty_worktree_count: worktrees.iter().filter(|worktree| worktree.dirty).count(),
            last_fetch_seconds,
            error: None,
        })
    }

    pub fn origin_remote_url(&self) -> Result<Option<String>, Error> {
        let output = self.git_allowing_exit_codes(&["remote", "get-url", "origin"], &[2])?;
        let remote = output.trim();
        Ok((!remote.is_empty()).then(|| remote.to_string()))
    }

    pub fn last_fetch_seconds(&self) -> Option<i64> {
        let path = self
            .git_allowing_exit_codes(&["rev-parse", "--git-path", "FETCH_HEAD"], &[128])
            .ok()?;
        let path = path.trim();
        if path.is_empty() {
            return None;
        }

        let git_path = PathBuf::from(path);
        let fetch_head = if git_path.is_absolute() {
            git_path
        } else {
            self.workdir().unwrap_or(self.path()).join(git_path)
        };
        let modified = fetch_head.metadata().ok()?.modified().ok()?;
        modified
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs() as i64)
    }
}

fn repo_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn workspace_summary_reports_branch_dirty_remote_and_worktrees() {
        let remote = TempRepo::new("workspace-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("workspace-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let parent = TempRepo::new("workspace-parent");
        let local_path = clone_main(&remote, &parent);
        let linked_path = parent.path.join("linked");
        let local = TempRepo {
            path: local_path.clone(),
        };
        local.git(&["branch", "feature/worktree"]);
        local.git(&[
            "worktree",
            "add",
            linked_path.to_str().unwrap(),
            "feature/worktree",
        ]);
        std::fs::write(local_path.join("dirty.txt"), "dirty\n").unwrap();

        let summary = Repository::workspace_summary(&local_path);

        assert_eq!(summary.path, local_path.canonicalize().unwrap());
        assert_eq!(summary.current_branch.as_deref(), Some("main"));
        assert_eq!(
            summary.remote.as_deref(),
            Some(remote.path.to_str().unwrap())
        );
        assert!(summary.dirty);
        assert_eq!(summary.worktree_count, 2);
        assert_eq!(summary.dirty_worktree_count, 1);
        assert_eq!(summary.error, None);

        let _ = std::fs::remove_dir_all(linked_path);
    }

    #[test]
    fn workspace_summary_keeps_invalid_repo_as_error_row() {
        let missing = PathBuf::from("/tmp/naite-missing-workspace-summary");

        let summary = Repository::workspace_summary(&missing);

        assert_eq!(summary.path, missing);
        assert!(summary.error.is_some());
    }
}
