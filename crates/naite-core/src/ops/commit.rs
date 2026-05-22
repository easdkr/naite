use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitOptions {
    pub title: String,
    pub body: String,
    pub co_authors: Vec<String>,
    pub amend: bool,
    pub skip_hooks: bool,
}

impl Repository {
    pub fn commit(&self, title: &str, body: &str) -> Result<(), Error> {
        self.commit_with_options(&CommitOptions {
            title: title.into(),
            body: body.into(),
            ..Default::default()
        })
    }

    pub fn commit_with_options(&self, options: &CommitOptions) -> Result<(), Error> {
        let title = options.title.trim();
        if title.is_empty() {
            return Err(Error::InvalidCommitMessage("title is required".into()));
        }

        let body = commit_body_with_trailers(options.body.trim(), &options.co_authors);
        let mut args = vec!["commit"];
        if options.amend {
            args.push("--amend");
        }
        if options.skip_hooks {
            args.push("--no-verify");
        }
        args.extend(["-m", title]);
        if !body.is_empty() {
            args.extend(["-m", body.as_str()]);
        }

        let _ = self.git(&args)?;
        Ok(())
    }
}

fn commit_body_with_trailers(body: &str, co_authors: &[String]) -> String {
    let trailers = co_authors
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.to_ascii_lowercase().starts_with("co-authored-by:") {
                value.to_string()
            } else {
                format!("Co-authored-by: {value}")
            }
        })
        .collect::<Vec<_>>();

    if trailers.is_empty() {
        body.to_string()
    } else if body.is_empty() {
        trailers.join("\n")
    } else {
        format!("{body}\n\n{}", trailers.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command;
    use crate::test_helpers::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn commit_creates_commit_from_staged_changes() {
        let repo_dir = TempRepo::init_with_commit("commit-staged");
        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.commit("update file", "").unwrap();

        let message = command::run_git(&repo_dir.path, ["log", "-1", "--pretty=%B"]).unwrap();
        assert_eq!(message.trim_end(), "update file");
        assert!(repo.status_detail().unwrap().staged.is_empty());
    }

    #[test]
    fn commit_preserves_title_and_body_message() {
        let repo_dir = TempRepo::init_with_commit("commit-body");
        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.commit("update file", "explain the change").unwrap();

        let message = command::run_git(&repo_dir.path, ["log", "-1", "--pretty=%B"]).unwrap();
        assert_eq!(message.trim_end(), "update file\n\nexplain the change");
    }

    #[test]
    fn commit_options_append_co_author_trailers() {
        let repo_dir = TempRepo::init_with_commit("commit-coauthors");
        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.commit_with_options(&CommitOptions {
            title: "update file".into(),
            body: "explain the change".into(),
            co_authors: vec![
                "Ada Lovelace <ada@example.com>".into(),
                "Co-authored-by: Grace Hopper <grace@example.com>".into(),
            ],
            ..Default::default()
        })
        .unwrap();

        let message = command::run_git(&repo_dir.path, ["log", "-1", "--pretty=%B"]).unwrap();
        assert_eq!(
            message.trim_end(),
            "update file\n\nexplain the change\n\nCo-authored-by: Ada Lovelace <ada@example.com>\nCo-authored-by: Grace Hopper <grace@example.com>"
        );
    }

    #[test]
    fn commit_options_amend_rewrites_previous_commit() {
        let repo_dir = TempRepo::init_with_commit("commit-amend");
        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.commit_with_options(&CommitOptions {
            title: "amended commit".into(),
            amend: true,
            ..Default::default()
        })
        .unwrap();

        let count = command::run_git(&repo_dir.path, ["rev-list", "--count", "HEAD"]).unwrap();
        let message = command::run_git(&repo_dir.path, ["log", "-1", "--pretty=%B"]).unwrap();
        assert_eq!(count.trim(), "1");
        assert_eq!(message.trim_end(), "amended commit");
    }

    #[test]
    fn commit_options_skip_hooks_bypasses_pre_commit_hook() {
        let repo_dir = TempRepo::init_with_commit("commit-skip-hooks");
        let hook_path = repo_dir.path.join(".git/hooks/pre-commit");
        std::fs::write(&hook_path, "#!/bin/sh\necho hook failed >&2\nexit 1\n").unwrap();
        let mut permissions = std::fs::metadata(&hook_path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook_path, permissions).unwrap();

        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.commit("blocked", "").unwrap_err();
        assert!(matches!(
            err,
            Error::GitCommand { stderr, .. } if stderr.contains("hook failed")
        ));

        repo.commit_with_options(&CommitOptions {
            title: "skip hook".into(),
            skip_hooks: true,
            ..Default::default()
        })
        .unwrap();
        let message = command::run_git(&repo_dir.path, ["log", "-1", "--pretty=%B"]).unwrap();
        assert_eq!(message.trim_end(), "skip hook");
    }

    #[test]
    fn commit_rejects_empty_title_before_running_git() {
        let repo_dir = TempRepo::init_with_commit("commit-empty-title");
        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo.commit("  ", "body").unwrap_err();

        assert!(matches!(
            err,
            Error::InvalidCommitMessage(message) if message == "title is required"
        ));
    }

    #[test]
    fn commit_without_staged_changes_returns_git_stderr() {
        let repo_dir = TempRepo::init_with_commit("commit-clean");
        let repo = Repository::open(&repo_dir.path).unwrap();

        let err = repo.commit("empty commit", "").unwrap_err();

        assert!(matches!(
            err,
            Error::GitCommand { command, stderr }
                if command == "git commit -m empty commit" && stderr.contains("nothing to commit")
        ));
    }
}
