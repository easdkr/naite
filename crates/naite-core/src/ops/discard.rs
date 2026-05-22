use crate::command;
use crate::diff::{hunk_patch, Hunk, HunkPatchMode};
use crate::repo::Repository;
use crate::worktree::{validate_status_path, WorktreeDiffKind};
use crate::Error;

impl Repository {
    pub fn discard_worktree_path(&self, kind: WorktreeDiffKind, path: &str) -> Result<(), Error> {
        validate_status_path(path)?;
        match kind {
            WorktreeDiffKind::Unstaged => {
                let _ = self.git(&["restore", "--worktree", "--", path])?;
                Ok(())
            }
            WorktreeDiffKind::Untracked => {
                let _ = self.git(&["clean", "-f", "--", path])?;
                Ok(())
            }
            WorktreeDiffKind::Staged | WorktreeDiffKind::Conflict => {
                Err(Error::UnsupportedDiscardTarget)
            }
        }
    }

    pub fn discard_worktree_hunk(&self, path: &str, hunk: &Hunk) -> Result<(), Error> {
        validate_status_path(path)?;
        let patch = hunk_patch(path, hunk, HunkPatchMode::Normal);
        let _ = command::run_git_with_stdin(
            self.workdir().unwrap_or(self.path()),
            ["apply", "--reverse", "--recount"],
            &patch,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use crate::worktree::WorktreeDiffTarget;
    use std::fs;

    #[test]
    fn discard_worktree_path_restores_modified_tracked_file() {
        let repo_dir = TempRepo::init_with_commit("discard-modified");
        repo_dir.write("file.txt", "dirty\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.discard_worktree_path(WorktreeDiffKind::Unstaged, "file.txt")
            .unwrap();

        assert_eq!(
            fs::read_to_string(repo_dir.path.join("file.txt")).unwrap(),
            "initial\n"
        );
        assert!(repo.status_detail().unwrap().unstaged.is_empty());
    }

    #[test]
    fn discard_worktree_path_restores_deleted_tracked_file() {
        let repo_dir = TempRepo::init_with_commit("discard-deleted");
        fs::remove_file(repo_dir.path.join("file.txt")).unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.discard_worktree_path(WorktreeDiffKind::Unstaged, "file.txt")
            .unwrap();

        assert_eq!(
            fs::read_to_string(repo_dir.path.join("file.txt")).unwrap(),
            "initial\n"
        );
        assert!(repo.status_detail().unwrap().unstaged.is_empty());
    }

    #[test]
    fn discard_worktree_path_deletes_untracked_file() {
        let repo_dir = TempRepo::init_with_commit("discard-untracked");
        repo_dir.write("new.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.discard_worktree_path(WorktreeDiffKind::Untracked, "new.txt")
            .unwrap();

        assert!(!repo_dir.path.join("new.txt").exists());
        assert!(repo.status_detail().unwrap().untracked.is_empty());
    }

    #[test]
    fn discard_worktree_path_rejects_staged_file() {
        let repo_dir = TempRepo::init_with_commit("discard-staged");
        repo_dir.write("file.txt", "dirty\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo
            .discard_worktree_path(WorktreeDiffKind::Staged, "file.txt")
            .unwrap_err();

        assert!(matches!(err, Error::UnsupportedDiscardTarget));
    }

    #[test]
    fn discard_worktree_path_rejects_unsafe_paths() {
        let repo_dir = TempRepo::init_with_commit("discard-unsafe-paths");
        let repo = Repository::open(&repo_dir.path).unwrap();

        for path in [
            "",
            "/tmp/file.txt",
            "../file.txt",
            "dir/../file.txt",
            "bad\0path",
            ".",
        ] {
            let err = repo
                .discard_worktree_path(WorktreeDiffKind::Untracked, path)
                .unwrap_err();
            assert!(matches!(err, Error::InvalidPath(_)), "{path:?}");
        }
    }

    #[test]
    fn discard_worktree_hunk_reverses_only_selected_hunk() {
        let repo_dir = TempRepo::init_with_commit("discard-hunk");
        let initial = (1..=20).map(|n| format!("line {n}\n")).collect::<String>();
        repo_dir.write("file.txt", &initial);
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "expand file"]);
        let changed = initial
            .replace("line 2\n", "line 2 changed\n")
            .replace("line 18\n", "line 18 changed\n");
        repo_dir.write("file.txt", &changed);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: "file.txt".into(),
            })
            .unwrap();
        let first_hunk = diff.hunks_by_file["file.txt"][0].clone();

        repo.discard_worktree_hunk("file.txt", &first_hunk).unwrap();

        let contents = fs::read_to_string(repo_dir.path.join("file.txt")).unwrap();
        assert!(contents.contains("line 2\n"));
        assert!(!contents.contains("line 2 changed\n"));
        assert!(contents.contains("line 18 changed\n"));
        assert_eq!(repo.status_detail().unwrap().unstaged.len(), 1);
    }

    #[test]
    fn discard_worktree_hunk_failure_preserves_worktree() {
        let repo_dir = TempRepo::init_with_commit("discard-hunk-stale");
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
            .discard_worktree_hunk("file.txt", &stale_hunk)
            .unwrap_err();

        assert!(matches!(err, Error::GitCommand { .. }));
        assert_eq!(
            fs::read_to_string(repo_dir.path.join("file.txt")).unwrap(),
            "unrelated\n"
        );
    }
}
