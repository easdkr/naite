use crate::diff::parser::diff_from_outputs;
use crate::diff::CommitDiff;
use crate::repo::Repository;
use crate::worktree::validate_status_path;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeDiffKind {
    Staged,
    Unstaged,
    Untracked,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeDiffTarget {
    pub kind: WorktreeDiffKind,
    pub path: String,
}

impl Repository {
    pub fn worktree_diff(&self, target: &WorktreeDiffTarget) -> Result<CommitDiff, Error> {
        validate_status_path(&target.path)?;
        match target.kind {
            WorktreeDiffKind::Staged => self.tracked_worktree_diff(&target.path, true),
            WorktreeDiffKind::Unstaged => self.tracked_worktree_diff(&target.path, false),
            WorktreeDiffKind::Untracked => self.untracked_worktree_diff(&target.path),
            WorktreeDiffKind::Conflict => self.conflict_worktree_diff(&target.path),
        }
    }

    fn tracked_worktree_diff(&self, path: &str, cached: bool) -> Result<CommitDiff, Error> {
        let name_status = if cached {
            self.git_without_optional_locks(&[
                "diff",
                "--cached",
                "--name-status",
                "-M",
                "-C",
                "--",
                path,
            ])?
        } else {
            self.git_without_optional_locks(&["diff", "--name-status", "-M", "-C", "--", path])?
        };
        let patch = if cached {
            self.git_without_optional_locks(&[
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "-M",
                "-C",
                "--",
                path,
            ])?
        } else {
            self.git_without_optional_locks(&[
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "-M",
                "-C",
                "--",
                path,
            ])?
        };

        Ok(diff_from_outputs(&name_status, &patch))
    }

    fn untracked_worktree_diff(&self, path: &str) -> Result<CommitDiff, Error> {
        let patch = self.git_without_optional_locks_allowing_exit_codes(
            &[
                "diff",
                "--no-index",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--",
                "/dev/null",
                path,
            ],
            &[1],
        )?;
        let name_status = format!("A\t{path}\n");
        Ok(diff_from_outputs(&name_status, &patch))
    }

    fn conflict_worktree_diff(&self, path: &str) -> Result<CommitDiff, Error> {
        let patch = self.git_without_optional_locks_allowing_exit_codes(
            &[
                "diff",
                "--ours",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--",
                path,
            ],
            &[1],
        )?;
        let name_status = format!("M\t{path}\n");
        Ok(diff_from_outputs(&name_status, &patch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{ChangeStatus, DiffLine};
    use crate::test_helpers::*;

    #[test]
    fn worktree_diff_reports_unstaged_modified_file() {
        let repo_dir = TempRepo::init_with_commit("diff-unstaged");
        repo_dir.write("file.txt", "changed\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: "file.txt".into(),
            })
            .unwrap();

        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "file.txt");
        assert_eq!(diff.files[0].status, ChangeStatus::Modified);
        let lines = &diff.hunks_by_file["file.txt"][0].lines;
        assert!(lines
            .iter()
            .any(|line| line == &DiffLine::Del("initial".into())));
        assert!(lines
            .iter()
            .any(|line| line == &DiffLine::Add("changed".into())));
    }

    #[test]
    fn worktree_diff_reports_staged_file_separately_from_unstaged_file() {
        let repo_dir = TempRepo::init_with_commit("diff-staged");
        repo_dir.write("file.txt", "staged\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.write("file.txt", "unstaged\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
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

        assert!(staged.hunks_by_file["file.txt"][0]
            .lines
            .iter()
            .any(|line| line == &DiffLine::Add("staged".into())));
        assert!(unstaged.hunks_by_file["file.txt"][0]
            .lines
            .iter()
            .any(|line| line == &DiffLine::Add("unstaged".into())));
    }

    #[test]
    fn worktree_diff_reports_untracked_file_from_no_index_exit_one() {
        let repo_dir = TempRepo::init_with_commit("diff-untracked");
        repo_dir.write("new.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Untracked,
                path: "new.txt".into(),
            })
            .unwrap();

        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "new.txt");
        assert_eq!(diff.files[0].status, ChangeStatus::Added);
        assert!(diff.hunks_by_file["new.txt"][0]
            .lines
            .iter()
            .any(|line| line == &DiffLine::Add("new".into())));
    }

    #[test]
    fn worktree_diff_reports_conflicted_file_against_ours() {
        let repo_dir = TempRepo::init_with_commit("diff-conflict");
        repo_dir.git(&["switch", "-c", "feature"]);
        repo_dir.write("file.txt", "feature\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "feature"]);
        repo_dir.git(&["switch", "-"]);
        repo_dir.write("file.txt", "main\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "main"]);
        let _ = std::process::Command::new("git")
            .args(["merge", "feature"])
            .current_dir(&repo_dir.path)
            .output()
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        let diff = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Conflict,
                path: "file.txt".into(),
            })
            .unwrap();

        assert_eq!(diff.files[0].status, ChangeStatus::Modified);
        assert!(diff.hunks_by_file.contains_key("file.txt"));
    }

    #[test]
    fn worktree_diff_rejects_empty_path() {
        let repo_dir = TempRepo::init_with_commit("diff-empty");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo
            .worktree_diff(&WorktreeDiffTarget {
                kind: WorktreeDiffKind::Unstaged,
                path: String::new(),
            })
            .unwrap_err();

        assert!(matches!(err, Error::InvalidPath(path) if path.is_empty()));
    }
}
