use crate::diff::parser::diff_from_outputs;
use crate::diff::CommitDiff;
use crate::repo::Repository;
use crate::Error;

const STASH_FIELD_SEPARATOR: char = '\x1f';
const STASH_RECORD_SEPARATOR: char = '\x1e';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashSummary {
    pub selector: String,
    pub short_id: String,
    pub branch: String,
    pub date: String,
    pub message: String,
}

impl Repository {
    pub fn list_stashes(&self) -> Result<Vec<StashSummary>, Error> {
        let output = self.git(&["stash", "list", "--format=%gd%x1f%H%x1f%cr%x1f%gs%x1e"])?;
        parse_stash_list(&output)
    }

    pub fn stash_diff(&self, selector: &str) -> Result<CommitDiff, Error> {
        validate_stash_selector(selector)?;
        let name_status = self.git(&[
            "stash",
            "show",
            "--include-untracked",
            "--name-status",
            "-M",
            "-C",
            selector,
        ])?;
        let patch = self.git(&[
            "stash",
            "show",
            "--include-untracked",
            "--patch",
            "--no-ext-diff",
            selector,
        ])?;
        Ok(diff_from_outputs(&name_status, &patch))
    }

    pub fn create_stash(&self, message: &str, include_untracked: bool) -> Result<(), Error> {
        let mut args = vec!["stash", "push"];
        if include_untracked {
            args.push("--include-untracked");
        }
        let message = message.trim();
        if !message.is_empty() {
            args.push("--message");
            args.push(message);
        }

        let _ = self.git(&args)?;
        Ok(())
    }

    pub fn apply_stash(&self, selector: &str) -> Result<(), Error> {
        validate_stash_selector(selector)?;
        let _ = self.git(&["stash", "apply", selector])?;
        Ok(())
    }

    pub fn pop_stash(&self, selector: &str) -> Result<(), Error> {
        validate_stash_selector(selector)?;
        let _ = self.git(&["stash", "pop", selector])?;
        Ok(())
    }

    pub fn drop_stash(&self, selector: &str) -> Result<(), Error> {
        validate_stash_selector(selector)?;
        let _ = self.git(&["stash", "drop", selector])?;
        Ok(())
    }

    pub fn create_branch_from_stash(&self, branch_name: &str, selector: &str) -> Result<(), Error> {
        let branch_name = branch_name.trim();
        if branch_name.is_empty() || branch_name.starts_with('-') {
            return Err(Error::InvalidRefName(branch_name.to_string()));
        }
        validate_stash_selector(selector)?;

        let _ = self.git(&["check-ref-format", "--branch", branch_name])?;
        let _ = self.git(&["stash", "branch", branch_name, selector])?;
        Ok(())
    }
}

fn parse_stash_list(output: &str) -> Result<Vec<StashSummary>, Error> {
    output
        .split(STASH_RECORD_SEPARATOR)
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            (!record.is_empty()).then_some(record)
        })
        .map(parse_stash_record)
        .collect()
}

fn parse_stash_record(record: &str) -> Result<StashSummary, Error> {
    let mut fields = record.split(STASH_FIELD_SEPARATOR);
    let selector = fields.next().unwrap_or_default().trim().to_string();
    let id = fields.next().unwrap_or_default().trim().to_string();
    let date = fields.next().unwrap_or_default().trim().to_string();
    let subject = fields.next().unwrap_or_default().trim().to_string();

    if selector.is_empty() || id.is_empty() || date.is_empty() || subject.is_empty() {
        return Err(Error::GitCommand {
            command: "git stash list --format=%gd%x1f%H%x1f%cr%x1f%gs%x1e".into(),
            stderr: format!("unexpected stash list record: {record}"),
        });
    }

    let (branch, message) = parse_stash_subject(&subject);

    Ok(StashSummary {
        selector,
        short_id: id.chars().take(7).collect(),
        branch,
        date,
        message,
    })
}

fn parse_stash_subject(subject: &str) -> (String, String) {
    for prefix in ["WIP on ", "On "] {
        if let Some(rest) = subject.strip_prefix(prefix) {
            if let Some((branch, message)) = rest.split_once(": ") {
                return (branch.to_string(), message.to_string());
            }
            return (rest.to_string(), subject.to_string());
        }
    }

    (String::new(), subject.to_string())
}

fn validate_stash_selector(selector: &str) -> Result<(), Error> {
    let Some(rest) = selector.strip_prefix("stash@{") else {
        return Err(Error::InvalidStashSelector(selector.to_string()));
    };
    let Some(index) = rest.strip_suffix('}') else {
        return Err(Error::InvalidStashSelector(selector.to_string()));
    };

    if index.is_empty() || !index.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::InvalidStashSelector(selector.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::ChangeStatus;
    use crate::test_helpers::*;

    #[test]
    fn parse_stash_list_extracts_default_and_custom_subjects() {
        let output = concat!(
            "stash@{0}\x1f1234567890abcdef\x1f2 minutes ago\x1fWIP on main: abc1234 initial\x1e\n",
            "stash@{1}\x1ffedcba9876543210\x1fyesterday\x1fOn feature/demo: save progress\x1e\n",
            "stash@{2}\x1fabcdef1234567890\x1f3 days ago\x1fmanual subject\x1e\n",
        );

        let stashes = parse_stash_list(output).unwrap();

        assert_eq!(
            stashes,
            vec![
                StashSummary {
                    selector: "stash@{0}".into(),
                    short_id: "1234567".into(),
                    branch: "main".into(),
                    date: "2 minutes ago".into(),
                    message: "abc1234 initial".into(),
                },
                StashSummary {
                    selector: "stash@{1}".into(),
                    short_id: "fedcba9".into(),
                    branch: "feature/demo".into(),
                    date: "yesterday".into(),
                    message: "save progress".into(),
                },
                StashSummary {
                    selector: "stash@{2}".into(),
                    short_id: "abcdef1".into(),
                    branch: "".into(),
                    date: "3 days ago".into(),
                    message: "manual subject".into(),
                },
            ]
        );
    }

    #[test]
    fn stash_operations_reject_invalid_selectors_before_git_runs() {
        let repo_dir = TempRepo::init_with_commit("stash-invalid-selector");
        let repo = Repository::open(&repo_dir.path).unwrap();

        for selector in ["", "0", "stash@{}", "stash@{-1}", "stash@{main}"] {
            assert!(matches!(
                repo.stash_diff(selector),
                Err(Error::InvalidStashSelector(_))
            ));
            assert!(matches!(
                repo.apply_stash(selector),
                Err(Error::InvalidStashSelector(_))
            ));
            assert!(matches!(
                repo.pop_stash(selector),
                Err(Error::InvalidStashSelector(_))
            ));
            assert!(matches!(
                repo.drop_stash(selector),
                Err(Error::InvalidStashSelector(_))
            ));
            assert!(matches!(
                repo.create_branch_from_stash("feature/from-stash", selector),
                Err(Error::InvalidStashSelector(_))
            ));
        }
    }

    #[test]
    fn create_branch_from_stash_checks_out_branch_and_applies_stash() {
        let repo_dir = TempRepo::init_with_commit("stash-branch");
        repo_dir.write("file.txt", "stashed\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_stash("save branch work", false).unwrap();
        repo.create_branch_from_stash("feature/from-stash", "stash@{0}")
            .unwrap();

        let repo = Repository::open(&repo_dir.path).unwrap();
        assert_eq!(repo.head_branch().as_deref(), Some("feature/from-stash"));
        assert!(repo.status().unwrap().has_unstaged);
        assert!(repo.list_stashes().unwrap().is_empty());
    }

    #[test]
    fn create_branch_from_stash_rejects_invalid_branch_name() {
        let repo_dir = TempRepo::init_with_commit("stash-branch-invalid");
        repo_dir.write("file.txt", "stashed\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_stash("save branch work", false).unwrap();

        let err = repo
            .create_branch_from_stash("feature..bad", "stash@{0}")
            .unwrap_err();
        assert!(matches!(err, Error::GitCommand { .. }));

        let err = repo
            .create_branch_from_stash("-bad", "stash@{0}")
            .unwrap_err();
        assert!(matches!(err, Error::InvalidRefName(_)));
    }

    #[test]
    fn create_stash_saves_tracked_changes() {
        let repo_dir = TempRepo::init_with_commit("stash-tracked");
        repo_dir.write("file.txt", "changed\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_stash("tracked work", false).unwrap();

        assert!(!repo.status_detail().unwrap().is_dirty());
        let stashes = repo.list_stashes().unwrap();
        assert_eq!(stashes.len(), 1);
        assert_eq!(stashes[0].branch, repo.head_branch().unwrap());
        assert_eq!(stashes[0].message, "tracked work");
    }

    #[test]
    fn create_stash_can_include_untracked_files() {
        let repo_dir = TempRepo::init_with_commit("stash-untracked");
        repo_dir.write("new.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_stash("include untracked", true).unwrap();

        assert!(!repo.status_detail().unwrap().is_dirty());
        let diff = repo.stash_diff("stash@{0}").unwrap();
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "new.txt");
        assert_eq!(diff.files[0].status, ChangeStatus::Added);
    }

    #[test]
    fn stash_diff_reports_tracked_and_untracked_changes() {
        let repo_dir = TempRepo::init_with_commit("stash-diff");
        repo_dir.write("file.txt", "initial\nchanged\n");
        repo_dir.write("new.txt", "new\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_stash("mixed diff", true).unwrap();

        let diff = repo.stash_diff("stash@{0}").unwrap();

        assert_eq!(diff.files.len(), 2);
        assert!(diff
            .files
            .iter()
            .any(|file| file.path == "file.txt" && file.status == ChangeStatus::Modified));
        assert!(diff
            .files
            .iter()
            .any(|file| file.path == "new.txt" && file.status == ChangeStatus::Added));
    }

    #[test]
    fn apply_pop_and_drop_have_expected_stash_lifetime() {
        let repo_dir = TempRepo::init_with_commit("stash-lifetime");
        repo_dir.write("file.txt", "apply me\n");

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_stash("apply stash", false).unwrap();
        repo.apply_stash("stash@{0}").unwrap();

        assert_eq!(repo.list_stashes().unwrap().len(), 1);
        repo_dir.git(&["checkout", "--", "file.txt"]);
        repo.pop_stash("stash@{0}").unwrap();
        assert!(repo.list_stashes().unwrap().is_empty());

        repo_dir.git(&["checkout", "--", "file.txt"]);
        repo_dir.write("file.txt", "drop me\n");
        repo.create_stash("drop stash", false).unwrap();
        repo.drop_stash("stash@{0}").unwrap();
        assert!(repo.list_stashes().unwrap().is_empty());
    }
}
