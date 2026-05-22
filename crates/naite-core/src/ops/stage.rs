use crate::command;
use crate::diff::{hunk_patch, Hunk, HunkPatchMode};
use crate::repo::Repository;
use crate::worktree::{validate_status_path, WorktreeDiffKind, WorktreeDiffTarget};
use crate::Error;

impl Repository {
    pub fn stage_path(&self, path: &str) -> Result<(), Error> {
        validate_status_path(path)?;
        let _ = self.git(&["add", "--", path])?;
        Ok(())
    }

    pub fn unstage_path(&self, path: &str) -> Result<(), Error> {
        validate_status_path(path)?;
        let _ = self.git(&["reset", "--", path])?;
        Ok(())
    }

    pub fn stage_all(&self) -> Result<(), Error> {
        let _ = self.git(&["add", "-A"])?;
        Ok(())
    }

    pub fn unstage_all(&self) -> Result<(), Error> {
        let _ = self.git(&["reset"])?;
        Ok(())
    }

    pub fn stage_worktree_hunk(&self, path: &str, hunk: &Hunk) -> Result<(), Error> {
        validate_status_path(path)?;
        let tracked_in_index = self.path_tracked_in_index(path)?;
        let kind = if tracked_in_index {
            WorktreeDiffKind::Unstaged
        } else {
            WorktreeDiffKind::Untracked
        };
        self.ensure_current_hunk(path, kind, hunk, "git apply --cached --recount")?;
        let mode = if tracked_in_index {
            HunkPatchMode::Normal
        } else {
            HunkPatchMode::NewFile
        };
        let patch = hunk_patch(path, hunk, mode);
        let _ = command::run_git_with_stdin(
            self.workdir().unwrap_or(self.path()),
            ["apply", "--cached", "--recount"],
            &patch,
        )?;
        Ok(())
    }

    pub fn unstage_worktree_hunk(&self, path: &str, hunk: &Hunk) -> Result<(), Error> {
        validate_status_path(path)?;
        self.ensure_current_hunk(
            path,
            WorktreeDiffKind::Staged,
            hunk,
            "git apply --cached --reverse --recount",
        )?;
        let mode = if self.path_tracked_in_head(path)? {
            HunkPatchMode::Normal
        } else {
            HunkPatchMode::NewFile
        };
        let patch = hunk_patch(path, hunk, mode);
        let _ = command::run_git_with_stdin(
            self.workdir().unwrap_or(self.path()),
            ["apply", "--cached", "--reverse", "--recount"],
            &patch,
        )?;
        Ok(())
    }

    fn path_tracked_in_index(&self, path: &str) -> Result<bool, Error> {
        let output =
            self.git_allowing_exit_codes(&["ls-files", "--error-unmatch", "--", path], &[1])?;
        Ok(!output.trim().is_empty())
    }

    fn path_tracked_in_head(&self, path: &str) -> Result<bool, Error> {
        let output =
            self.git_allowing_exit_codes(&["ls-tree", "--name-only", "HEAD", "--", path], &[128])?;
        Ok(!output.trim().is_empty())
    }

    fn ensure_current_hunk(
        &self,
        path: &str,
        kind: WorktreeDiffKind,
        hunk: &Hunk,
        command: &str,
    ) -> Result<(), Error> {
        let diff = self.worktree_diff(&WorktreeDiffTarget {
            kind,
            path: path.to_string(),
        })?;
        if diff
            .hunks_by_file
            .get(path)
            .is_some_and(|hunks| hunks.iter().any(|current| current == hunk))
        {
            return Ok(());
        }

        Err(Error::GitCommand {
            command: command.to_string(),
            stderr: "selected hunk no longer matches the current diff".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffLine;
    use crate::test_helpers::*;
    use crate::worktree::StatusKind;
    use std::fs;

    #[test]
    fn stage_path_moves_modified_file_to_staged_group() {
        let repo_dir = TempRepo::init_with_commit("stage-modified");
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.stage_path("file.txt").unwrap();
        let detail = repo.status_detail().unwrap();

        assert!(detail.unstaged.is_empty());
        assert_eq!(detail.staged.len(), 1);
        assert_eq!(detail.staged[0].path, "file.txt");
        assert_eq!(detail.staged[0].status, StatusKind::Modified);
    }

    #[test]
    fn stage_path_moves_untracked_file_to_staged_added_group() {
        let repo_dir = TempRepo::init_with_commit("stage-untracked");
        repo_dir.write("new.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.stage_path("new.txt").unwrap();
        let detail = repo.status_detail().unwrap();

        assert!(detail.untracked.is_empty());
        assert_eq!(detail.staged.len(), 1);
        assert_eq!(detail.staged[0].path, "new.txt");
        assert_eq!(detail.staged[0].status, StatusKind::Added);
    }

    #[test]
    fn unstage_path_keeps_worktree_change_and_moves_file_to_unstaged_group() {
        let repo_dir = TempRepo::init_with_commit("unstage-modified");
        repo_dir.write("file.txt", "dirty\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.unstage_path("file.txt").unwrap();
        let detail = repo.status_detail().unwrap();

        assert!(detail.staged.is_empty());
        assert_eq!(detail.unstaged.len(), 1);
        assert_eq!(detail.unstaged[0].path, "file.txt");
        assert_eq!(detail.unstaged[0].status, StatusKind::Modified);
    }

    #[test]
    fn unstage_path_succeeds_in_repository_without_head() {
        let repo_dir = TempRepo::new("unstage-no-head");
        Repository::init(&repo_dir.path).unwrap();
        repo_dir.write("new.txt", "new\n");
        repo_dir.git(&["add", "new.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.unstage_path("new.txt").unwrap();
        let detail = repo.status_detail().unwrap();

        assert!(detail.staged.is_empty());
        assert_eq!(detail.untracked.len(), 1);
        assert_eq!(detail.untracked[0].path, "new.txt");
    }

    #[test]
    fn stage_all_and_unstage_all_move_multiple_files_between_groups() {
        let repo_dir = TempRepo::init_with_commit("stage-all");
        repo_dir.write("file.txt", "dirty\n");
        repo_dir.write("new.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.stage_all().unwrap();
        let detail = repo.status_detail().unwrap();

        assert!(detail.unstaged.is_empty());
        assert!(detail.untracked.is_empty());
        assert_eq!(detail.staged.len(), 2);

        repo.unstage_all().unwrap();
        let detail = repo.status_detail().unwrap();

        assert!(detail.staged.is_empty());
        assert_eq!(detail.unstaged.len(), 1);
        assert_eq!(detail.untracked.len(), 1);
    }

    #[test]
    fn stage_worktree_hunk_stages_only_selected_unstaged_hunk() {
        let repo_dir = TempRepo::init_with_commit("stage-hunk");
        let initial = expanded_file();
        repo_dir.write("file.txt", &initial);
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "expand file"]);
        let changed = initial
            .replace("line 2\n", "line 2 changed\n")
            .replace("line 25\n", "line 25 changed\n");
        repo_dir.write("file.txt", &changed);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: "file.txt".into(),
            })
            .unwrap();
        let first_hunk = diff.hunks_by_file["file.txt"][0].clone();

        repo.stage_worktree_hunk("file.txt", &first_hunk).unwrap();

        let staged = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Staged,
                path: "file.txt".into(),
            })
            .unwrap();
        let unstaged = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: "file.txt".into(),
            })
            .unwrap();
        assert!(diff_contains_added(&staged, "file.txt", "line 2 changed"));
        assert!(!diff_contains_added(&staged, "file.txt", "line 25 changed"));
        assert!(diff_contains_added(
            &unstaged,
            "file.txt",
            "line 25 changed"
        ));
        assert!(!diff_contains_added(
            &unstaged,
            "file.txt",
            "line 2 changed"
        ));
    }

    #[test]
    fn unstage_worktree_hunk_unstages_only_selected_staged_hunk() {
        let repo_dir = TempRepo::init_with_commit("unstage-hunk");
        let initial = expanded_file();
        repo_dir.write("file.txt", &initial);
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "expand file"]);
        let changed = initial
            .replace("line 2\n", "line 2 changed\n")
            .replace("line 25\n", "line 25 changed\n");
        repo_dir.write("file.txt", &changed);
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Staged,
                path: "file.txt".into(),
            })
            .unwrap();
        let first_hunk = diff.hunks_by_file["file.txt"][0].clone();

        repo.unstage_worktree_hunk("file.txt", &first_hunk).unwrap();

        let staged = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Staged,
                path: "file.txt".into(),
            })
            .unwrap();
        let unstaged = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: "file.txt".into(),
            })
            .unwrap();
        assert!(!diff_contains_added(&staged, "file.txt", "line 2 changed"));
        assert!(diff_contains_added(&staged, "file.txt", "line 25 changed"));
        assert!(diff_contains_added(&unstaged, "file.txt", "line 2 changed"));
        assert!(!diff_contains_added(
            &unstaged,
            "file.txt",
            "line 25 changed"
        ));
    }

    #[test]
    fn stage_worktree_hunk_failure_preserves_index_and_worktree() {
        let repo_dir = TempRepo::init_with_commit("stage-hunk-stale");
        repo_dir.write("file.txt", "changed\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: "file.txt".into(),
            })
            .unwrap();
        let stale_hunk = diff.hunks_by_file["file.txt"][0].clone();
        repo_dir.write("file.txt", "unrelated\n");

        let err = repo
            .stage_worktree_hunk("file.txt", &stale_hunk)
            .unwrap_err();

        assert!(matches!(err, Error::GitCommand { .. }));
        assert!(repo.status_detail().unwrap().staged.is_empty());
        assert_eq!(
            fs::read_to_string(repo_dir.path.join("file.txt")).unwrap(),
            "unrelated\n"
        );
    }

    #[test]
    fn hunk_stage_operations_reject_unsafe_paths() {
        let repo_dir = TempRepo::init_with_commit("hunk-stage-unsafe-paths");
        let repo = Repository::open(&repo_dir.path).unwrap();
        let hunk = Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            header: "@@ -1 +1 @@".into(),
            lines: vec![
                DiffLine::Del("initial".into()),
                DiffLine::Add("changed".into()),
            ],
        };

        for path in ["", "/tmp/file.txt", "../file.txt", "dir/../file.txt", "."] {
            let err = repo.stage_worktree_hunk(path, &hunk).unwrap_err();
            assert!(matches!(err, Error::InvalidPath(_)), "{path:?}");
            let err = repo.unstage_worktree_hunk(path, &hunk).unwrap_err();
            assert!(matches!(err, Error::InvalidPath(_)), "{path:?}");
        }
    }

    #[test]
    fn stage_worktree_hunk_stages_untracked_text_file_as_added() {
        let repo_dir = TempRepo::init_with_commit("stage-untracked-hunk");
        repo_dir.write("new.txt", "new\nfile\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Untracked,
                path: "new.txt".into(),
            })
            .unwrap();
        let hunk = diff.hunks_by_file["new.txt"][0].clone();

        repo.stage_worktree_hunk("new.txt", &hunk).unwrap();

        let detail = repo.status_detail().unwrap();
        assert!(detail.untracked.is_empty());
        assert_eq!(detail.staged.len(), 1);
        assert_eq!(detail.staged[0].path, "new.txt");
        assert_eq!(detail.staged[0].status, StatusKind::Added);
    }

    #[test]
    fn stage_path_rejects_empty_path() {
        let repo_dir = TempRepo::init_with_commit("stage-empty");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.stage_path("").unwrap_err();

        assert!(matches!(err, Error::InvalidPath(path) if path.is_empty()));
    }
}
