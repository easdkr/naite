use std::collections::HashSet;

use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorktreeStatus {
    pub has_unstaged: bool,
    pub has_untracked: bool,
    pub has_staged: bool,
}

impl WorktreeStatus {
    pub fn is_dirty(self) -> bool {
        self.has_unstaged || self.has_untracked || self.has_staged
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorktreeStatusDetail {
    pub staged: Vec<StatusEntry>,
    pub unstaged: Vec<StatusEntry>,
    pub untracked: Vec<StatusEntry>,
    pub conflicted: Vec<StatusEntry>,
    pub ignored: Vec<StatusEntry>,
    pub submodules: Vec<StatusEntry>,
}

impl WorktreeStatusDetail {
    pub fn is_dirty(&self) -> bool {
        !self.staged.is_empty()
            || !self.unstaged.is_empty()
            || !self.untracked.is_empty()
            || !self.conflicted.is_empty()
            || !self.submodules.is_empty()
    }

    pub fn summary(&self) -> WorktreeStatus {
        WorktreeStatus {
            has_staged: !self.staged.is_empty() || !self.conflicted.is_empty(),
            has_unstaged: !self.unstaged.is_empty()
                || !self.conflicted.is_empty()
                || !self.submodules.is_empty(),
            has_untracked: !self.untracked.is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: StatusKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Ignored,
    Submodule,
    Unmerged { index: char, worktree: char },
}

impl Repository {
    pub fn status(&self) -> Result<WorktreeStatus, Error> {
        Ok(self.status_detail()?.summary())
    }

    pub fn status_detail(&self) -> Result<WorktreeStatusDetail, Error> {
        let output = self.git_without_optional_locks(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=matching",
        ])?;
        let mut detail = parse_worktree_status_detail(&output);
        let submodule_paths = self.submodule_paths()?;
        move_submodule_entries(&mut detail, &submodule_paths);
        Ok(detail)
    }

    fn submodule_paths(&self) -> Result<HashSet<String>, Error> {
        let output = self.git_allowing_exit_codes(
            ["config", "--file", ".gitmodules", "--get-regexp", "path"].as_slice(),
            &[1],
        )?;
        Ok(parse_submodule_paths(&output))
    }
}

fn parse_worktree_status_detail(output: &str) -> WorktreeStatusDetail {
    let mut detail = WorktreeStatusDetail::default();
    let mut records = output.split('\0');

    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }

        let Some((index, worktree, path)) = parse_status_record_header(record) else {
            continue;
        };
        let old_path = if is_rename_or_copy(index) || is_rename_or_copy(worktree) {
            records.next().filter(|path| !path.is_empty())
        } else {
            None
        };

        if index == '?' && worktree == '?' {
            detail
                .untracked
                .push(status_entry(path, None, StatusKind::Untracked));
            continue;
        }

        if index == '!' && worktree == '!' {
            detail
                .ignored
                .push(status_entry(path, None, StatusKind::Ignored));
            continue;
        }

        if is_unmerged_status(index, worktree) {
            detail.conflicted.push(StatusEntry {
                path: path.to_string(),
                old_path: old_path.map(str::to_string),
                status: StatusKind::Unmerged { index, worktree },
            });
            continue;
        }

        if let Some(status) = status_kind(index) {
            detail.staged.push(status_entry(path, old_path, status));
        }
        if let Some(status) = status_kind(worktree) {
            detail.unstaged.push(status_entry(path, old_path, status));
        }
    }

    detail
}

fn parse_submodule_paths(output: &str) -> HashSet<String> {
    output
        .lines()
        .filter_map(|line| line.split_once(' ').map(|(_, path)| path.trim()))
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn move_submodule_entries(detail: &mut WorktreeStatusDetail, submodule_paths: &HashSet<String>) {
    if submodule_paths.is_empty() {
        return;
    }

    let mut submodules = Vec::new();
    submodules.extend(take_submodule_entries(&mut detail.staged, submodule_paths));
    submodules.extend(take_submodule_entries(
        &mut detail.unstaged,
        submodule_paths,
    ));
    submodules.extend(take_submodule_entries(
        &mut detail.untracked,
        submodule_paths,
    ));
    submodules.extend(take_submodule_entries(
        &mut detail.conflicted,
        submodule_paths,
    ));

    for entry in submodules {
        if !detail
            .submodules
            .iter()
            .any(|existing| existing.path == entry.path)
        {
            detail.submodules.push(StatusEntry {
                path: entry.path,
                old_path: None,
                status: StatusKind::Submodule,
            });
        }
    }
}

fn take_submodule_entries(
    entries: &mut Vec<StatusEntry>,
    submodule_paths: &HashSet<String>,
) -> Vec<StatusEntry> {
    let mut extracted = Vec::new();
    let mut retained = Vec::with_capacity(entries.len());

    for entry in std::mem::take(entries) {
        if submodule_paths.contains(&entry.path) {
            extracted.push(entry);
        } else {
            retained.push(entry);
        }
    }

    *entries = retained;
    extracted
}

fn parse_status_record_header(record: &str) -> Option<(char, char, &str)> {
    let mut chars = record.chars();
    let index = chars.next()?;
    let worktree = chars.next()?;
    let path = chars.as_str().strip_prefix(' ')?;
    Some((index, worktree, path))
}

fn is_rename_or_copy(status: char) -> bool {
    matches!(status, 'R' | 'C')
}

fn is_unmerged_status(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

fn status_kind(status: char) -> Option<StatusKind> {
    match status {
        'A' => Some(StatusKind::Added),
        'M' => Some(StatusKind::Modified),
        'D' => Some(StatusKind::Deleted),
        'R' => Some(StatusKind::Renamed),
        'C' => Some(StatusKind::Copied),
        'T' => Some(StatusKind::TypeChanged),
        _ => None,
    }
}

fn status_entry(path: &str, old_path: Option<&str>, status: StatusKind) -> StatusEntry {
    let old_path = if matches!(status, StatusKind::Renamed | StatusKind::Copied) {
        old_path.map(str::to_string)
    } else {
        None
    };

    StatusEntry {
        path: path.to_string(),
        old_path,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command;
    use crate::test_helpers::*;

    #[test]
    fn parse_worktree_status_groups_porcelain_states() {
        let status =
            parse_worktree_status_detail(" M src/main.rs\0A  staged.txt\0?? new.txt\0").summary();

        assert!(status.has_unstaged);
        assert!(status.has_staged);
        assert!(status.has_untracked);
    }

    #[test]
    fn parse_worktree_status_treats_rename_as_staged() {
        let status = parse_worktree_status_detail("R  new.txt\0old.txt\0").summary();

        assert!(status.has_staged);
        assert!(!status.has_unstaged);
        assert!(!status.has_untracked);
    }

    #[test]
    fn parse_worktree_status_detail_groups_porcelain_z_states() {
        let detail = parse_worktree_status_detail(
            "M  staged.txt\0 M unstaged.txt\0?? new file.txt\0!! ignored.log\0",
        );

        assert_eq!(
            detail.staged,
            vec![StatusEntry {
                path: "staged.txt".into(),
                old_path: None,
                status: StatusKind::Modified,
            }]
        );
        assert_eq!(
            detail.unstaged,
            vec![StatusEntry {
                path: "unstaged.txt".into(),
                old_path: None,
                status: StatusKind::Modified,
            }]
        );
        assert_eq!(
            detail.untracked,
            vec![StatusEntry {
                path: "new file.txt".into(),
                old_path: None,
                status: StatusKind::Untracked,
            }]
        );
        assert!(detail.conflicted.is_empty());
        assert_eq!(
            detail.ignored,
            vec![StatusEntry {
                path: "ignored.log".into(),
                old_path: None,
                status: StatusKind::Ignored,
            }]
        );
        assert!(detail.submodules.is_empty());
        assert!(detail.is_dirty());
    }

    #[test]
    fn parse_submodule_paths_preserves_paths_with_spaces() {
        let output = "submodule.libs/core.path libs/core\nsubmodule.libs/ui.path libs/ui app\n";

        assert_eq!(
            parse_submodule_paths(output),
            HashSet::from(["libs/core".to_string(), "libs/ui app".to_string()])
        );
    }

    #[test]
    fn submodule_entries_move_to_submodule_group_without_duplicates() {
        let mut detail = parse_worktree_status_detail(" M deps/lib\0M  deps/lib\0 M src/main.rs\0");

        move_submodule_entries(&mut detail, &HashSet::from(["deps/lib".to_string()]));

        assert_eq!(
            detail.submodules,
            vec![StatusEntry {
                path: "deps/lib".into(),
                old_path: None,
                status: StatusKind::Submodule,
            }]
        );
        assert!(detail.staged.is_empty());
        assert_eq!(detail.unstaged.len(), 1);
        assert_eq!(detail.unstaged[0].path, "src/main.rs");
        assert!(detail.is_dirty());
        assert!(detail.summary().has_unstaged);
    }

    #[test]
    fn parse_worktree_status_detail_preserves_rename_and_copy_paths() {
        let detail = parse_worktree_status_detail(
            "R  renamed new.txt\0old name.txt\0C  copied new.txt\0source file.txt\0",
        );

        assert_eq!(
            detail.staged,
            vec![
                StatusEntry {
                    path: "renamed new.txt".into(),
                    old_path: Some("old name.txt".into()),
                    status: StatusKind::Renamed,
                },
                StatusEntry {
                    path: "copied new.txt".into(),
                    old_path: Some("source file.txt".into()),
                    status: StatusKind::Copied,
                },
            ]
        );
        assert!(detail.unstaged.is_empty());
        assert!(detail.untracked.is_empty());
        assert!(detail.conflicted.is_empty());
    }

    #[test]
    fn parse_worktree_status_detail_classifies_conflicts_once() {
        let detail = parse_worktree_status_detail(
            "UU both-modified.txt\0AA both-added.txt\0DD both-deleted.txt\0AU added-us.txt\0UA added-them.txt\0DU deleted-us.txt\0UD deleted-them.txt\0",
        );

        assert!(detail.staged.is_empty());
        assert!(detail.unstaged.is_empty());
        assert!(detail.untracked.is_empty());
        assert_eq!(detail.conflicted.len(), 7);
        assert_eq!(
            detail.conflicted[0],
            StatusEntry {
                path: "both-modified.txt".into(),
                old_path: None,
                status: StatusKind::Unmerged {
                    index: 'U',
                    worktree: 'U',
                },
            }
        );
        assert!(detail.summary().has_staged);
        assert!(detail.summary().has_unstaged);
    }

    #[test]
    fn parse_worktree_status_detail_keeps_paths_with_separator_text() {
        let detail = parse_worktree_status_detail(
            " M path with -> arrow.txt\0R  new -> name.txt\0old -> name.txt\0",
        );

        assert_eq!(detail.unstaged[0].path, "path with -> arrow.txt");
        assert_eq!(detail.staged[0].path, "new -> name.txt");
        assert_eq!(
            detail.staged[0].old_path.as_deref(),
            Some("old -> name.txt")
        );
    }

    #[test]
    fn parse_worktree_status_detail_tracks_deleted_and_type_changed_files() {
        let detail = parse_worktree_status_detail(
            "D  staged-deleted.txt\0 D unstaged-deleted.txt\0T  type-changed.txt\0",
        );

        assert_eq!(detail.staged[0].status, StatusKind::Deleted);
        assert_eq!(detail.unstaged[0].status, StatusKind::Deleted);
        assert_eq!(detail.staged[1].status, StatusKind::TypeChanged);
    }

    #[test]
    fn repository_status_detail_reports_file_groups() {
        let repo_dir = TempRepo::init_with_commit("status-detail");
        repo_dir.write("staged.txt", "staged\n");
        repo_dir.git(&["add", "staged.txt"]);
        repo_dir.write("file.txt", "dirty\n");
        repo_dir.write("untracked.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let detail = repo.status_detail().unwrap();

        assert_eq!(detail.staged.len(), 1);
        assert_eq!(detail.staged[0].path, "staged.txt");
        assert_eq!(detail.staged[0].status, StatusKind::Added);
        assert_eq!(detail.unstaged.len(), 1);
        assert_eq!(detail.unstaged[0].path, "file.txt");
        assert_eq!(detail.unstaged[0].status, StatusKind::Modified);
        assert_eq!(detail.untracked.len(), 1);
        assert_eq!(detail.untracked[0].path, "untracked.txt");
        assert!(detail.conflicted.is_empty());
        assert!(detail.ignored.is_empty());
        assert!(detail.submodules.is_empty());
        assert_eq!(
            detail.summary(),
            WorktreeStatus {
                has_staged: true,
                has_unstaged: true,
                has_untracked: true,
            }
        );
    }

    #[test]
    fn repository_status_detail_reports_ignored_files_without_dirty_summary() {
        let repo_dir = TempRepo::init_with_commit("status-ignored");
        repo_dir.write(".gitignore", "*.log\n");
        repo_dir.git(&["add", ".gitignore"]);
        repo_dir.git(&["commit", "-m", "ignore logs"]);
        repo_dir.write("debug.log", "ignored\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        let detail = repo.status_detail().unwrap();

        assert_eq!(detail.ignored.len(), 1);
        assert_eq!(detail.ignored[0].path, "debug.log");
        assert!(!detail.is_dirty());
        assert_eq!(detail.summary(), WorktreeStatus::default());
    }

    #[test]
    fn repository_status_detail_reports_submodule_changes() {
        let submodule = TempRepo::init_with_commit("status-submodule-child");
        let repo_dir = TempRepo::init_with_commit("status-submodule-parent");
        command::run_git(
            &repo_dir.path,
            [
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                submodule.path.to_str().unwrap(),
                "deps/lib",
            ],
        )
        .unwrap();
        repo_dir.git(&["commit", "-am", "add submodule"]);

        std::fs::write(repo_dir.path.join("deps/lib/file.txt"), "submodule dirty\n").unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        let detail = repo.status_detail().unwrap();

        assert_eq!(detail.submodules.len(), 1);
        assert_eq!(detail.submodules[0].path, "deps/lib");
        assert!(detail.unstaged.is_empty());
        assert!(detail.is_dirty());
        assert!(detail.summary().has_unstaged);
    }
}
