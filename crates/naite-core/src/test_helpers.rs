use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commits::CommitSummary;
use crate::diff::{CommitDiff, DiffLine};

pub(crate) fn commit(id: &str, parents: &[&str]) -> CommitSummary {
    CommitSummary {
        id: id.to_string(),
        short_id: id.chars().take(7).collect(),
        summary: id.to_string(),
        author_name: "author".into(),
        author_email: "author@example.com".into(),
        author_avatar_url: None,
        time_seconds: 0,
        parent_ids: parents.iter().map(|p| p.to_string()).collect(),
    }
}

pub(crate) struct TempRepo {
    pub(crate) path: PathBuf,
}

impl TempRepo {
    pub(crate) fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("naite-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub(crate) fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub(crate) fn git_output(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    pub(crate) fn write(&self, path: &str, contents: &str) {
        fs::write(self.path.join(path), contents).unwrap();
    }

    pub(crate) fn init_with_commit(name: &str) -> Self {
        let repo = Self::new(name);
        repo.git(&["init"]);
        repo.git(&["config", "user.name", "naite test"]);
        repo.git(&["config", "user.email", "naite@example.com"]);
        repo.write("file.txt", "initial\n");
        repo.git(&["add", "file.txt"]);
        repo.git(&["commit", "-m", "initial"]);
        repo
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn expanded_file() -> String {
    (1..=30).map(|n| format!("line {n}\n")).collect()
}

pub(crate) fn diff_contains_added(diff: &CommitDiff, path: &str, value: &str) -> bool {
    diff.hunks_by_file.get(path).is_some_and(|hunks| {
        hunks
            .iter()
            .flat_map(|hunk| hunk.lines.iter())
            .any(|line| line == &DiffLine::Add(value.into()))
    })
}

pub(crate) fn clone_main(remote: &TempRepo, parent: &TempRepo) -> PathBuf {
    let local_path = parent.path.join("local");
    let output = Command::new("git")
        .args([
            "clone",
            "--branch",
            "main",
            remote.path.to_str().unwrap(),
            local_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    local_path
}
