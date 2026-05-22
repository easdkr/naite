use crate::repo::Repository;
use crate::worktree::validate_status_path;
use crate::Error;

const FIELD_SEPARATOR: char = '\x1f';
const RECORD_SEPARATOR: char = '\x1e';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistoryEntry {
    pub commit_id: String,
    pub short_id: String,
    pub author_name: String,
    pub time_seconds: i64,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    pub line_number: u32,
    pub commit_id: String,
    pub short_id: String,
    pub author_name: String,
    pub time_seconds: i64,
    pub summary: String,
    pub contents: String,
}

impl Repository {
    pub fn file_history(&self, path: &str) -> Result<Vec<FileHistoryEntry>, Error> {
        validate_status_path(path)?;
        let output = self.git(&[
            "log",
            "--follow",
            "--format=%H%x1f%h%x1f%an%x1f%at%x1f%s%x1e",
            "--",
            path,
        ])?;
        parse_file_history(&output)
    }

    pub fn file_blame(&self, path: &str) -> Result<Vec<BlameLine>, Error> {
        validate_status_path(path)?;
        let output = self.git(&["blame", "--line-porcelain", "--", path])?;
        Ok(parse_blame(&output))
    }
}

fn parse_file_history(output: &str) -> Result<Vec<FileHistoryEntry>, Error> {
    output
        .split(RECORD_SEPARATOR)
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            (!record.is_empty()).then_some(record)
        })
        .map(parse_file_history_record)
        .collect()
}

fn parse_file_history_record(record: &str) -> Result<FileHistoryEntry, Error> {
    let mut fields = record.split(FIELD_SEPARATOR);
    let commit_id = fields.next().unwrap_or_default().trim();
    let short_id = fields.next().unwrap_or_default().trim();
    let author_name = fields.next().unwrap_or_default().trim();
    let time_seconds = fields
        .next()
        .unwrap_or_default()
        .trim()
        .parse::<i64>()
        .unwrap_or_default();
    let summary = fields.next().unwrap_or_default().trim();

    if commit_id.is_empty() || short_id.is_empty() {
        return Err(Error::GitCommand {
            command: "git log --follow --format=%H%x1f%h%x1f%an%x1f%at%x1f%s%x1e".into(),
            stderr: format!("unexpected file history record: {record}"),
        });
    }

    Ok(FileHistoryEntry {
        commit_id: commit_id.to_string(),
        short_id: short_id.to_string(),
        author_name: author_name.to_string(),
        time_seconds,
        summary: summary.to_string(),
    })
}

fn parse_blame(output: &str) -> Vec<BlameLine> {
    let mut lines = Vec::new();
    let mut commit_id = String::new();
    let mut final_line = 0u32;
    let mut author_name = String::new();
    let mut author_time = 0i64;
    let mut summary = String::new();

    for line in output.lines() {
        if let Some(contents) = line.strip_prefix('\t') {
            let short_id = commit_id.chars().take(7).collect();
            lines.push(BlameLine {
                line_number: final_line,
                commit_id: commit_id.clone(),
                short_id,
                author_name: author_name.clone(),
                time_seconds: author_time,
                summary: summary.clone(),
                contents: contents.to_string(),
            });
            continue;
        }

        if let Some(rest) = line.strip_prefix("author ") {
            author_name = rest.to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("author-time ") {
            author_time = rest.parse::<i64>().unwrap_or_default();
            continue;
        }
        if let Some(rest) = line.strip_prefix("summary ") {
            summary = rest.to_string();
            continue;
        }

        let mut fields = line.split_whitespace();
        let first = fields.next().unwrap_or_default();
        if first.len() >= 7 && first.chars().all(|c| c.is_ascii_hexdigit()) {
            commit_id = first.to_string();
            let _original = fields.next();
            final_line = fields
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or_default();
            author_name.clear();
            author_time = 0;
            summary.clear();
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn file_history_lists_commits_touching_path() {
        let repo_dir = TempRepo::init_with_commit("file-history");
        repo_dir.write("other.txt", "other\n");
        repo_dir.git(&["add", "other.txt"]);
        repo_dir.git(&["commit", "-m", "other"]);
        repo_dir.write("file.txt", "changed\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "change file"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let history = repo.file_history("file.txt").unwrap();

        assert_eq!(history[0].summary, "change file");
        assert!(history.iter().all(|entry| entry.summary != "other"));
    }

    #[test]
    fn file_blame_reports_line_owners() {
        let repo_dir = TempRepo::init_with_commit("file-blame");
        repo_dir.write("file.txt", "initial\nsecond\n");
        repo_dir.git(&["add", "file.txt"]);
        repo_dir.git(&["commit", "-m", "second line"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let blame = repo.file_blame("file.txt").unwrap();

        assert_eq!(blame.len(), 2);
        assert_eq!(blame[1].line_number, 2);
        assert_eq!(blame[1].contents, "second");
        assert_eq!(blame[1].summary, "second line");
    }

    #[test]
    fn file_inspection_rejects_empty_path() {
        let repo_dir = TempRepo::init_with_commit("file-invalid");
        let repo = Repository::open(&repo_dir.path).unwrap();

        assert!(matches!(repo.file_history(""), Err(Error::InvalidPath(_))));
        assert!(matches!(repo.file_blame(""), Err(Error::InvalidPath(_))));
    }
}
