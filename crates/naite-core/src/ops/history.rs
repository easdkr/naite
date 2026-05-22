use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::repo::Repository;
use crate::worktree::validate_status_path;
use crate::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GitOperationState {
    pub merge_in_progress: bool,
    pub rebase_in_progress: bool,
}

impl GitOperationState {
    pub fn is_busy(self) -> bool {
        self.merge_in_progress || self.rebase_in_progress
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictSide {
    Ours,
    Theirs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquashMode {
    Squash,
    Fixup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderDirection {
    Earlier,
    Later,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseAction {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePlanEntry {
    pub action: RebaseAction,
    pub commit_id: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCommit {
    pub id: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
}

impl Repository {
    pub fn operation_state(&self) -> GitOperationState {
        GitOperationState {
            merge_in_progress: self
                .git_path("MERGE_HEAD")
                .is_some_and(|path| path.exists()),
            rebase_in_progress: self
                .git_path("rebase-merge")
                .is_some_and(|path| path.exists())
                || self
                    .git_path("rebase-apply")
                    .is_some_and(|path| path.exists()),
        }
    }

    pub fn merge_ref(&self, ref_name: &str) -> Result<(), Error> {
        let ref_name = validate_refish(ref_name)?;
        let _ = self.git(&["rev-parse", "--verify", ref_name])?;
        let _ = self.git_allowing_exit_codes(&["merge", "--no-edit", ref_name], &[1])?;
        Ok(())
    }

    pub fn rebase_onto(&self, ref_name: &str) -> Result<(), Error> {
        let ref_name = validate_refish(ref_name)?;
        let _ = self.git(&["rev-parse", "--verify", ref_name])?;
        let _ = self.git_allowing_exit_codes(&["rebase", ref_name], &[1])?;
        Ok(())
    }

    pub fn abort_merge(&self) -> Result<(), Error> {
        let _ = self.git(&["merge", "--abort"])?;
        Ok(())
    }

    pub fn abort_rebase(&self) -> Result<(), Error> {
        let _ = self.git(&["rebase", "--abort"])?;
        Ok(())
    }

    pub fn continue_rebase(&self) -> Result<(), Error> {
        let _ = self.git_with_env(
            &["rebase", "--continue"],
            &[(OsString::from("GIT_EDITOR"), OsString::from(":"))],
        )?;
        Ok(())
    }

    pub fn resolve_conflict_with_side(&self, path: &str, side: ConflictSide) -> Result<(), Error> {
        validate_status_path(path)?;
        let side_arg = match side {
            ConflictSide::Ours => "--ours",
            ConflictSide::Theirs => "--theirs",
        };
        let _ = self.git(&["checkout", side_arg, "--", path])?;
        self.mark_conflict_resolved(path)
    }

    pub fn mark_conflict_resolved(&self, path: &str) -> Result<(), Error> {
        validate_status_path(path)?;
        let _ = self.git(&["add", "--", path])?;
        Ok(())
    }

    pub fn reword_commit(&self, commit_id: &str, message: &str) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let message = validate_reword_message(message)?;
        let base = self.single_parent(commit_id)?;
        let mut entries = self.todo_entries_after_base(&base)?;
        let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.commit_id == commit_id)
        else {
            return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
        };
        entry.action = RebaseAction::Reword;
        self.run_interactive_rebase(
            &base,
            &entries,
            &[(commit_id.to_string(), message.to_string())],
        )
    }

    pub fn drop_commit(&self, commit_id: &str) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let base = self.single_parent(commit_id)?;
        let mut entries = self.todo_entries_after_base(&base)?;
        let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.commit_id == commit_id)
        else {
            return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
        };
        entry.action = RebaseAction::Drop;
        self.run_interactive_rebase(&base, &entries, &[])
    }

    pub fn squash_commit_into_parent(
        &self,
        commit_id: &str,
        mode: SquashMode,
    ) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let parent = self.single_parent(commit_id)?;
        let base = self.single_parent(&parent)?;
        let mut entries = self.todo_entries_after_base(&base)?;
        let Some(position) = entries
            .iter()
            .position(|entry| entry.commit_id == commit_id)
        else {
            return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
        };
        if position == 0 || entries[position - 1].commit_id != parent {
            return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
        }
        entries[position].action = match mode {
            SquashMode::Squash => RebaseAction::Squash,
            SquashMode::Fixup => RebaseAction::Fixup,
        };
        self.run_interactive_rebase(&base, &entries, &[])
    }

    pub fn edit_commit(&self, commit_id: &str) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let base = self.single_parent(commit_id)?;
        let mut entries = self.todo_entries_after_base(&base)?;
        let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.commit_id == commit_id)
        else {
            return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
        };
        entry.action = RebaseAction::Edit;
        self.run_interactive_rebase(&base, &entries, &[])
    }

    pub fn reorder_commit(
        &self,
        commit_id: &str,
        direction: ReorderDirection,
    ) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let head_entries = self.history_commits_from_head()?;
        let Some(index) = head_entries.iter().position(|entry| entry.id == commit_id) else {
            return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
        };

        match direction {
            ReorderDirection::Earlier => {
                if index == 0 {
                    return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
                }
                let previous = &head_entries[index - 1].id;
                let base = self.single_parent(previous)?;
                let mut entries = self.todo_entries_after_base(&base)?;
                let Some(position) = entries
                    .iter()
                    .position(|entry| entry.commit_id == commit_id)
                else {
                    return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
                };
                if position == 0 {
                    return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
                }
                entries.swap(position - 1, position);
                self.run_interactive_rebase(&base, &entries, &[])
            }
            ReorderDirection::Later => {
                if index + 1 >= head_entries.len() {
                    return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
                }
                let base = self.single_parent(commit_id)?;
                let mut entries = self.todo_entries_after_base(&base)?;
                let Some(position) = entries
                    .iter()
                    .position(|entry| entry.commit_id == commit_id)
                else {
                    return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
                };
                if position + 1 >= entries.len() {
                    return Err(Error::UnsupportedHistoryOperation(commit_id.to_string()));
                }
                entries.swap(position, position + 1);
                self.run_interactive_rebase(&base, &entries, &[])
            }
        }
    }

    pub fn reset_hard_to(&self, commit_id: &str) -> Result<(), Error> {
        let commit_id = validate_commit_id(commit_id)?;
        let _ = self.git(&["rev-parse", "--verify", commit_id])?;
        let _ = self.git(&["reset", "--hard", commit_id])?;
        Ok(())
    }

    fn single_parent(&self, commit_id: &str) -> Result<String, Error> {
        let parents = self.commit_parent_ids(commit_id)?;
        match parents.as_slice() {
            [parent] => Ok(parent.clone()),
            _ => Err(Error::UnsupportedHistoryOperation(commit_id.to_string())),
        }
    }

    pub fn history_commits_after(&self, base: &str) -> Result<Vec<HistoryCommit>, Error> {
        let range = format!("{base}..HEAD");
        let output = self.git(&[
            "log",
            "--reverse",
            "--first-parent",
            "--format=%H%x1f%an%x1f%ae%x1f%s%x1e",
            &range,
        ])?;
        parse_history_commits(&output)
    }

    pub fn rebase_plan_entries_onto(
        &self,
        target_ref: &str,
    ) -> Result<Vec<RebasePlanEntry>, Error> {
        let target_ref = validate_refish(target_ref)?;
        let _ = self.git(&["rev-parse", "--verify", target_ref])?;
        let commits = self.history_commits_after(target_ref)?;
        if commits.is_empty() {
            return Err(Error::UnsupportedHistoryOperation(target_ref.to_string()));
        }
        for commit in &commits {
            if self.commit_parent_ids(&commit.id)?.len() != 1 {
                return Err(Error::UnsupportedHistoryOperation(commit.id.clone()));
            }
        }
        Ok(commits
            .into_iter()
            .map(|commit| RebasePlanEntry {
                action: RebaseAction::Pick,
                commit_id: commit.id,
                summary: commit.summary,
                author_name: commit.author_name,
                author_email: commit.author_email,
            })
            .collect())
    }

    pub fn apply_rebase_plan_onto(
        &self,
        target_ref: &str,
        entries: &[RebasePlanEntry],
        reword_messages: &[(String, String)],
    ) -> Result<(), Error> {
        let target_ref = validate_refish(target_ref)?;
        let _ = self.git(&["rev-parse", "--verify", target_ref])?;

        let canonical = self.rebase_plan_entries_onto(target_ref)?;
        let canonical = canonical
            .into_iter()
            .map(|entry| HistoryCommit {
                id: entry.commit_id,
                summary: entry.summary,
                author_name: entry.author_name,
                author_email: entry.author_email,
            })
            .collect::<Vec<_>>();
        validate_rebase_plan(target_ref, &canonical, entries, reword_messages)?;

        self.run_interactive_rebase(target_ref, entries, reword_messages)
    }

    fn todo_entries_after_base(&self, base: &str) -> Result<Vec<RebasePlanEntry>, Error> {
        let commits = self.history_commits_after(base)?;
        if commits.is_empty() {
            return Err(Error::UnsupportedHistoryOperation(base.to_string()));
        }
        Ok(commits
            .into_iter()
            .map(|commit| RebasePlanEntry {
                action: RebaseAction::Pick,
                commit_id: commit.id,
                summary: commit.summary,
                author_name: commit.author_name,
                author_email: commit.author_email,
            })
            .collect())
    }

    fn history_commits_from_head(&self) -> Result<Vec<HistoryCommit>, Error> {
        let output = self.git(&[
            "log",
            "--reverse",
            "--first-parent",
            "--format=%H%x1f%an%x1f%ae%x1f%s%x1e",
        ])?;
        parse_history_commits(&output)
    }

    fn run_interactive_rebase(
        &self,
        base: &str,
        entries: &[RebasePlanEntry],
        reword_messages: &[(String, String)],
    ) -> Result<(), Error> {
        let temp_dir = self.rebase_temp_dir()?;
        fs::create_dir_all(&temp_dir).map_err(|source| Error::GitCommand {
            command: "prepare naite rebase editor".into(),
            stderr: source.to_string(),
        })?;

        let result = (|| {
            let todo_path = temp_dir.join("todo");
            let sequence_editor_path = temp_dir.join("sequence-editor.sh");
            fs::write(&todo_path, format_rebase_todo(entries)).map_err(io_error)?;
            write_script(
                &sequence_editor_path,
                &format!("#!/bin/sh\ncat {} > \"$1\"\n", shell_quote(&todo_path)),
            )?;

            let editor_path = temp_dir.join("editor.sh");
            let ordered_reword_messages = ordered_reword_messages(entries, reword_messages)?;
            if ordered_reword_messages.is_empty() {
                write_script(&editor_path, "#!/bin/sh\n:\n")?;
            } else {
                let messages_dir = temp_dir.join("messages");
                fs::create_dir_all(&messages_dir).map_err(io_error)?;
                for (index, message) in ordered_reword_messages.iter().enumerate() {
                    fs::write(messages_dir.join(index.to_string()), message).map_err(io_error)?;
                }
                let counter_path = temp_dir.join("reword-counter");
                write_script(
                    &editor_path,
                    &format!(
                        "#!/bin/sh\ncounter={counter}\nindex=0\nif [ -f \"$counter\" ]; then index=$(cat \"$counter\"); fi\nmessage={messages_dir}/$index\nif [ -f \"$message\" ]; then cat \"$message\" > \"$1\"; fi\nnext=$((index + 1))\nprintf '%s' \"$next\" > \"$counter\"\n",
                        counter = shell_quote(&counter_path),
                        messages_dir = shell_quote(&messages_dir),
                    ),
                )?;
            }

            let envs = [
                (
                    OsString::from("GIT_SEQUENCE_EDITOR"),
                    sequence_editor_path.as_os_str().to_os_string(),
                ),
                (
                    OsString::from("GIT_EDITOR"),
                    editor_path.as_os_str().to_os_string(),
                ),
            ];
            self.append_history_log(&format!(
                "interactive-rebase base={base} todo={}",
                compact_todo_for_log(entries)
            ));
            let _ = self.git_with_env(&["rebase", "-i", base], &envs)?;
            Ok(())
        })();

        let _ = fs::remove_dir_all(temp_dir);
        result
    }

    fn rebase_temp_dir(&self) -> Result<PathBuf, Error> {
        let git_path = self
            .git_path("naite-rebase")
            .ok_or_else(|| Error::UnsupportedHistoryOperation("git-path".into()))?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Ok(git_path.with_file_name(format!("naite-rebase-{nanos}")))
    }

    fn git_path(&self, name: &str) -> Option<PathBuf> {
        let output = self.git(&["rev-parse", "--git-path", name]).ok()?;
        let path = PathBuf::from(output.trim());
        Some(if path.is_absolute() {
            path
        } else {
            self.workdir().unwrap_or(self.path()).join(path)
        })
    }

    fn append_history_log(&self, event: &str) {
        let Some(path) = self.git_path("naite-history.log") else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{timestamp}\t{event}");
        }
    }
}

fn parse_history_commits(output: &str) -> Result<Vec<HistoryCommit>, Error> {
    output
        .split('\x1e')
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            (!record.is_empty()).then_some(record)
        })
        .map(|record| {
            let mut fields = record.splitn(4, '\x1f');
            let id = fields.next().unwrap_or_default().trim();
            let author_name = fields.next().unwrap_or_default().trim();
            let author_email = fields.next().unwrap_or_default().trim();
            let summary = fields.next().unwrap_or_default().trim();
            if id.is_empty() {
                return Err(Error::GitCommand {
                    command: "git log --reverse --first-parent --format=%H%x1f%an%x1f%ae%x1f%s%x1e"
                        .into(),
                    stderr: format!("unexpected commit record: {record}"),
                });
            }
            Ok(HistoryCommit {
                id: id.to_string(),
                summary: summary.to_string(),
                author_name: author_name.to_string(),
                author_email: author_email.to_string(),
            })
        })
        .collect()
}

fn validate_rebase_plan(
    target_ref: &str,
    canonical: &[HistoryCommit],
    entries: &[RebasePlanEntry],
    reword_messages: &[(String, String)],
) -> Result<(), Error> {
    if entries.is_empty() {
        return Err(Error::UnsupportedHistoryOperation(target_ref.to_string()));
    }

    let canonical_ids = canonical
        .iter()
        .map(|commit| commit.id.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        if !canonical_ids.contains(entry.commit_id.as_str()) {
            return Err(Error::UnsupportedHistoryOperation(entry.commit_id.clone()));
        }
        if !seen.insert(entry.commit_id.as_str()) {
            return Err(Error::UnsupportedHistoryOperation(entry.commit_id.clone()));
        }
        if index == 0 && matches!(entry.action, RebaseAction::Squash | RebaseAction::Fixup) {
            return Err(Error::UnsupportedHistoryOperation(entry.commit_id.clone()));
        }
    }

    for commit in canonical {
        if !seen.contains(commit.id.as_str()) {
            return Err(Error::UnsupportedHistoryOperation(commit.id.clone()));
        }
    }

    for (commit_id, message) in reword_messages {
        validate_reword_message(message)?;
        let Some(entry) = entries.iter().find(|entry| entry.commit_id == *commit_id) else {
            return Err(Error::UnsupportedHistoryOperation(commit_id.clone()));
        };
        if entry.action != RebaseAction::Reword {
            return Err(Error::UnsupportedHistoryOperation(commit_id.clone()));
        }
    }

    for entry in entries
        .iter()
        .filter(|entry| entry.action == RebaseAction::Reword)
    {
        if !reword_messages
            .iter()
            .any(|(commit_id, _)| commit_id == &entry.commit_id)
        {
            return Err(Error::UnsupportedHistoryOperation(entry.commit_id.clone()));
        }
    }

    Ok(())
}

fn ordered_reword_messages(
    entries: &[RebasePlanEntry],
    reword_messages: &[(String, String)],
) -> Result<Vec<String>, Error> {
    entries
        .iter()
        .filter(|entry| entry.action == RebaseAction::Reword)
        .map(|entry| {
            reword_messages
                .iter()
                .find(|(commit_id, _)| commit_id == &entry.commit_id)
                .map(|(_, message)| validate_reword_message(message).map(str::to_string))
                .unwrap_or_else(|| Err(Error::UnsupportedHistoryOperation(entry.commit_id.clone())))
        })
        .collect()
}

fn format_rebase_todo(entries: &[RebasePlanEntry]) -> String {
    entries
        .iter()
        .map(|entry| {
            format!(
                "{} {} {}\n",
                action_token(entry.action),
                entry.commit_id,
                entry.summary.replace('\n', " ")
            )
        })
        .collect()
}

fn compact_todo_for_log(entries: &[RebasePlanEntry]) -> String {
    entries
        .iter()
        .map(|entry| {
            let short = entry.commit_id.chars().take(7).collect::<String>();
            format!("{}:{short}", action_token(entry.action))
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn action_token(action: RebaseAction) -> &'static str {
    match action {
        RebaseAction::Pick => "pick",
        RebaseAction::Reword => "reword",
        RebaseAction::Edit => "edit",
        RebaseAction::Squash => "squash",
        RebaseAction::Fixup => "fixup",
        RebaseAction::Drop => "drop",
    }
}

fn validate_refish(ref_name: &str) -> Result<&str, Error> {
    let ref_name = ref_name.trim();
    if ref_name.is_empty() || ref_name.starts_with('-') {
        return Err(Error::InvalidRefName(ref_name.to_string()));
    }
    Ok(ref_name)
}

fn validate_commit_id(commit_id: &str) -> Result<&str, Error> {
    let commit_id = commit_id.trim();
    if commit_id.is_empty() || commit_id.starts_with('-') {
        return Err(Error::InvalidCommit(commit_id.to_string()));
    }
    Ok(commit_id)
}

fn validate_reword_message(message: &str) -> Result<&str, Error> {
    let message = message.trim();
    if message.is_empty() {
        return Err(Error::InvalidCommitMessage(message.to_string()));
    }
    Ok(message)
}

fn write_script(path: &Path, contents: &str) -> Result<(), Error> {
    fs::write(path, contents).map_err(io_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
    }
    Ok(())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn io_error(source: std::io::Error) -> Error {
    Error::GitCommand {
        command: "prepare naite rebase editor".into(),
        stderr: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    fn history_repo(name: &str) -> TempRepo {
        let repo = TempRepo::init_with_commit(name);
        repo.write("file.txt", "one\n");
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", "one"]);
        repo.write("file.txt", "two\n");
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", "two"]);
        repo.write("file.txt", "three\n");
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", "three"]);
        repo
    }

    fn inter_branch_repo(name: &str) -> TempRepo {
        let repo = TempRepo::init_with_commit(name);
        repo.git(&["branch", "-M", "main"]);
        repo.git(&["switch", "-c", "feature"]);
        repo.write("one.txt", "one\n");
        repo.git(&["add", "one.txt"]);
        repo.git(&["commit", "-m", "one"]);
        repo.write("two.txt", "two\n");
        repo.git(&["add", "two.txt"]);
        repo.git(&["commit", "-m", "two"]);
        repo.write("three.txt", "three\n");
        repo.git(&["add", "three.txt"]);
        repo.git(&["commit", "-m", "three"]);
        repo
    }

    fn plan_entries(repo: &Repository, target_ref: &str) -> Vec<RebasePlanEntry> {
        repo.history_commits_after(target_ref)
            .unwrap()
            .into_iter()
            .map(|commit| RebasePlanEntry {
                action: RebaseAction::Pick,
                commit_id: commit.id,
                summary: commit.summary,
                author_name: commit.author_name,
                author_email: commit.author_email,
            })
            .collect()
    }

    #[test]
    fn reword_commit_rewrites_selected_commit_message() {
        let repo_dir = history_repo("history-reword");
        let target = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(3).unwrap()[1].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.reword_commit(&target, "two rewritten").unwrap();

        let subjects = repo_dir.git_output(&["log", "--format=%s"]);
        assert!(subjects.contains("two rewritten"));
        assert!(!subjects.contains("\ntwo\n"));
        let log_path = Repository::open(&repo_dir.path)
            .unwrap()
            .git_path("naite-history.log")
            .unwrap();
        let log = std::fs::read_to_string(log_path).unwrap();
        assert!(log.contains("interactive-rebase"));
        assert!(log.contains("reword"));
    }

    #[test]
    fn drop_commit_removes_middle_commit_and_replays_descendants() {
        let repo_dir = TempRepo::init_with_commit("history-drop");
        repo_dir.write("one.txt", "one\n");
        repo_dir.git(&["add", "one.txt"]);
        repo_dir.git(&["commit", "-m", "one"]);
        repo_dir.write("two.txt", "two\n");
        repo_dir.git(&["add", "two.txt"]);
        repo_dir.git(&["commit", "-m", "two"]);
        repo_dir.write("three.txt", "three\n");
        repo_dir.git(&["add", "three.txt"]);
        repo_dir.git(&["commit", "-m", "three"]);
        let target = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(3).unwrap()[1].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.drop_commit(&target).unwrap();

        let subjects = repo_dir.git_output(&["log", "--format=%s"]);
        assert!(!subjects.contains("\ntwo\n"));
        assert!(subjects.contains("three"));
    }

    #[test]
    fn fixup_commit_squashes_selected_commit_into_parent() {
        let repo_dir = history_repo("history-fixup");
        let target = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(3).unwrap()[1].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.squash_commit_into_parent(&target, SquashMode::Fixup)
            .unwrap();

        let subjects = repo_dir.git_output(&["log", "--format=%s"]);
        assert!(!subjects.contains("\ntwo\n"));
        assert!(subjects.contains("one"));
        assert!(subjects.contains("three"));
    }

    #[test]
    fn reorder_commit_moves_selected_commit_later() {
        let repo_dir = TempRepo::init_with_commit("history-reorder");
        repo_dir.write("one.txt", "one\n");
        repo_dir.git(&["add", "one.txt"]);
        repo_dir.git(&["commit", "-m", "one"]);
        repo_dir.write("two.txt", "two\n");
        repo_dir.git(&["add", "two.txt"]);
        repo_dir.git(&["commit", "-m", "two"]);
        let target = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(2).unwrap()[1].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.reorder_commit(&target, ReorderDirection::Later)
            .unwrap();

        let subjects = repo_dir.git_output(&["log", "--reverse", "--format=%s"]);
        assert_eq!(
            subjects.lines().collect::<Vec<_>>(),
            vec!["initial", "two", "one"]
        );
    }

    #[test]
    fn apply_rebase_plan_onto_drops_and_reorders_in_one_call() {
        let repo_dir = inter_branch_repo("history-plan-drop-reorder");
        let repo = Repository::open(&repo_dir.path).unwrap();
        let mut entries = plan_entries(&repo, "main");
        entries[1].action = RebaseAction::Drop;
        entries.swap(0, 2);

        repo.apply_rebase_plan_onto("main", &entries, &[]).unwrap();

        let subjects = repo_dir.git_output(&["log", "--reverse", "--format=%s", "main..HEAD"]);
        assert_eq!(subjects.lines().collect::<Vec<_>>(), vec!["three", "one"]);
    }

    #[test]
    fn apply_rebase_plan_onto_reorders_and_fixups_grouped_commits() {
        let repo_dir = inter_branch_repo("history-plan-reorder-fixup");
        let repo = Repository::open(&repo_dir.path).unwrap();
        let mut entries = plan_entries(&repo, "main");
        entries.swap(1, 2);
        entries[0].action = RebaseAction::Reword;
        entries[1].action = RebaseAction::Fixup;
        let messages = vec![(entries[0].commit_id.clone(), "mine squashed".to_string())];

        repo.apply_rebase_plan_onto("main", &entries, &messages)
            .unwrap();

        let subjects = repo_dir.git_output(&["log", "--reverse", "--format=%s", "main..HEAD"]);
        assert_eq!(
            subjects.lines().collect::<Vec<_>>(),
            vec!["mine squashed", "two"]
        );
    }

    #[test]
    fn rebase_plan_entries_include_author_identity() {
        let repo_dir = TempRepo::init_with_commit("history-plan-author");
        repo_dir.git(&["branch", "-M", "main"]);
        repo_dir.git(&["switch", "-c", "feature"]);
        repo_dir.write("mine.txt", "mine\n");
        repo_dir.git(&["add", "mine.txt"]);
        repo_dir.git(&["commit", "-m", "mine"]);
        repo_dir.write("other.txt", "other\n");
        repo_dir.git(&["add", "other.txt"]);
        repo_dir.git(&[
            "-c",
            "user.name=Other Author",
            "-c",
            "user.email=other@example.com",
            "commit",
            "-m",
            "other",
        ]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let entries = repo.rebase_plan_entries_onto("main").unwrap();

        assert_eq!(entries[0].author_name, "naite test");
        assert_eq!(entries[0].author_email, "naite@example.com");
        assert_eq!(entries[1].author_name, "Other Author");
        assert_eq!(entries[1].author_email, "other@example.com");
    }

    #[test]
    fn apply_rebase_plan_onto_rejects_merge_commits_in_range() {
        let repo_dir = TempRepo::init_with_commit("history-plan-merge");
        repo_dir.git(&["branch", "-M", "main"]);
        repo_dir.git(&["switch", "-c", "feature"]);
        repo_dir.write("one.txt", "one\n");
        repo_dir.git(&["add", "one.txt"]);
        repo_dir.git(&["commit", "-m", "one"]);
        repo_dir.git(&["switch", "-c", "side"]);
        repo_dir.write("side.txt", "side\n");
        repo_dir.git(&["add", "side.txt"]);
        repo_dir.git(&["commit", "-m", "side"]);
        repo_dir.git(&["switch", "feature"]);
        repo_dir.git(&["merge", "--no-ff", "side", "-m", "merge side"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let entries = plan_entries(&repo, "main");

        assert!(matches!(
            repo.apply_rebase_plan_onto("main", &entries, &[]),
            Err(Error::UnsupportedHistoryOperation(_))
        ));
    }

    #[test]
    fn apply_rebase_plan_onto_rejects_squash_at_first_row() {
        let repo_dir = inter_branch_repo("history-plan-first-squash");
        let repo = Repository::open(&repo_dir.path).unwrap();
        let mut entries = plan_entries(&repo, "main");
        entries[0].action = RebaseAction::Squash;

        assert!(matches!(
            repo.apply_rebase_plan_onto("main", &entries, &[]),
            Err(Error::UnsupportedHistoryOperation(_))
        ));
    }

    #[test]
    fn apply_rebase_plan_onto_rewords_two_commits_in_one_run() {
        let repo_dir = inter_branch_repo("history-plan-reword-two");
        let repo = Repository::open(&repo_dir.path).unwrap();
        let mut entries = plan_entries(&repo, "main");
        entries[0].action = RebaseAction::Reword;
        entries[2].action = RebaseAction::Reword;
        let messages = vec![
            (entries[0].commit_id.clone(), "one rewritten".to_string()),
            (entries[2].commit_id.clone(), "three rewritten".to_string()),
        ];

        repo.apply_rebase_plan_onto("main", &entries, &messages)
            .unwrap();

        let subjects = repo_dir.git_output(&["log", "--reverse", "--format=%s", "main..HEAD"]);
        assert_eq!(
            subjects.lines().collect::<Vec<_>>(),
            vec!["one rewritten", "two", "three rewritten"]
        );
    }

    #[test]
    fn apply_rebase_plan_onto_rejects_omitted_commit() {
        let repo_dir = inter_branch_repo("history-plan-omitted");
        let repo = Repository::open(&repo_dir.path).unwrap();
        let mut entries = plan_entries(&repo, "main");
        entries.remove(1);

        assert!(matches!(
            repo.apply_rebase_plan_onto("main", &entries, &[]),
            Err(Error::UnsupportedHistoryOperation(_))
        ));
    }

    #[test]
    fn apply_rebase_plan_onto_rejects_empty_range() {
        let repo_dir = TempRepo::init_with_commit("history-plan-empty");
        repo_dir.git(&["branch", "-M", "main"]);
        let repo = Repository::open(&repo_dir.path).unwrap();

        assert!(matches!(
            repo.apply_rebase_plan_onto("HEAD", &[], &[]),
            Err(Error::UnsupportedHistoryOperation(_))
        ));
    }

    #[test]
    fn merge_ref_merges_feature_branch() {
        let repo_dir = TempRepo::init_with_commit("history-merge");
        repo_dir.git(&["switch", "-c", "feature"]);
        repo_dir.write("feature.txt", "feature\n");
        repo_dir.git(&["add", "feature.txt"]);
        repo_dir.git(&["commit", "-m", "feature"]);
        repo_dir.git(&["switch", "main"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.merge_ref("feature").unwrap();

        assert!(repo_dir.path.join("feature.txt").exists());
    }

    #[test]
    fn resolve_conflict_with_side_checks_out_and_stages_side() {
        let repo_dir = TempRepo::init_with_commit("history-conflict");
        repo_dir.git(&["switch", "-c", "feature"]);
        repo_dir.write("file.txt", "feature\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "feature"]);
        repo_dir.git(&["switch", "main"]);
        repo_dir.write("file.txt", "main\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "main"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.merge_ref("feature").unwrap();
        assert!(repo.operation_state().merge_in_progress);

        repo.resolve_conflict_with_side("file.txt", ConflictSide::Theirs)
            .unwrap();
        let detail = repo.status_detail().unwrap();
        assert!(detail.conflicted.is_empty());
        assert!(!detail.staged.is_empty());
    }

    #[test]
    fn reset_hard_to_moves_head_to_checkpoint() {
        let repo_dir = history_repo("history-reset");
        let checkpoint = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(2).unwrap()[1].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.reset_hard_to(&checkpoint).unwrap();

        assert_eq!(repo.list_commits(1).unwrap()[0].id, checkpoint);
    }
}
