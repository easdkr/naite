use crate::repo::Repository;
use crate::text::compose_hangul;
use crate::Error;

/// Full commit message split into title (subject) and body. Loaded on
/// demand for editors like reword — the per-row [`CommitSummary`] only
/// carries the title to keep the commit list cheap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitMessage {
    pub title: String,
    pub body: String,
}

/// Short, UI-friendly snapshot of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub author_avatar_url: Option<String>,
    pub time_seconds: i64,
    pub parent_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitPageCursor {
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPage {
    pub commits: Vec<CommitSummary>,
    pub next_cursor: Option<CommitPageCursor>,
}

impl Repository {
    /// Walk commits starting from HEAD, newest first, up to `limit`.
    pub fn list_commits(&self, limit: usize) -> Result<Vec<CommitSummary>, Error> {
        let head_id = self.inner.head_id().map_err(|_| Error::NoHead)?;

        let walk = self
            .inner
            .rev_walk([head_id])
            .all()
            .map_err(|e| Error::Walk(Box::new(e)))?;

        let mut out = Vec::with_capacity(limit);
        for info in walk.take(limit) {
            let info = info.map_err(|e| Error::Walk(Box::new(e)))?;
            let commit = info.object().map_err(|e| Error::ReadCommit(Box::new(e)))?;

            let id = info.id;
            let short_id = id.to_hex_with_len(7).to_string();

            let message = commit
                .message()
                .map_err(|e| Error::ReadCommit(Box::new(e)))?;
            let summary = compose_hangul(&message.title.to_string());

            let author = commit
                .author()
                .map_err(|e| Error::ReadCommit(Box::new(e)))?;
            let author_name = compose_hangul(&author.name.to_string());
            let author_email = author.email.to_string();
            let author_avatar_url = author_avatar_url_from_email(&author_email);
            let time_seconds = author.time.seconds;
            let parent_ids = commit.parent_ids().map(|id| id.to_string()).collect();

            out.push(CommitSummary {
                id: id.to_string(),
                short_id,
                summary,
                author_name,
                author_email,
                author_avatar_url,
                time_seconds,
                parent_ids,
            });
        }

        Ok(out)
    }

    /// Walk commits reachable from the provided refs, newest first, up to `limit`.
    ///
    /// An empty ref set preserves the default HEAD-only history view.
    pub fn list_commits_from_refs(
        &self,
        ref_names: &[String],
        limit: usize,
    ) -> Result<Vec<CommitSummary>, Error> {
        Ok(self
            .list_commit_page_from_refs(ref_names, None, limit)?
            .commits)
    }

    /// Walk commits reachable from the provided refs, newest first.
    ///
    /// `cursor` is an opaque page position for callers; callers should pass the
    /// returned `next_cursor` back unchanged when loading the following page.
    pub fn list_commit_page_from_refs(
        &self,
        ref_names: &[String],
        cursor: Option<CommitPageCursor>,
        limit: usize,
    ) -> Result<CommitPage, Error> {
        if limit == 0 {
            return Ok(CommitPage {
                commits: Vec::new(),
                next_cursor: cursor,
            });
        }

        let offset = cursor.map(|cursor| cursor.offset).unwrap_or(0);
        let page_size = limit.saturating_add(1);
        let mut args = vec![
            "log".to_string(),
            "--date-order".to_string(),
            format!("--skip={offset}"),
            format!("--max-count={page_size}"),
            "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%at%x1f%P%x1f%s%x1e".to_string(),
        ];
        if ref_names.is_empty() {
            let head_id = self.inner.head_id().map_err(|_| Error::NoHead)?;
            args.push(head_id.to_string());
        } else {
            args.extend(ref_names.iter().cloned());
        }

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = self.git(&arg_refs)?;
        let mut commits = parse_commit_log(&output)?;
        let next_cursor = if commits.len() > limit {
            commits.truncate(limit);
            Some(CommitPageCursor {
                offset: offset + limit,
            })
        } else {
            None
        };

        Ok(CommitPage {
            commits,
            next_cursor,
        })
    }

    pub(crate) fn commit_parent_ids(&self, commit_id: &str) -> Result<Vec<String>, Error> {
        let output = self.git(&["rev-list", "--parents", "-n", "1", commit_id])?;
        Ok(output
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect())
    }

    /// Load the full commit message (title and body) for a single commit.
    /// Used by editor surfaces that need to round-trip the existing body,
    /// not just the short subject carried by [`CommitSummary`].
    pub fn commit_message(&self, commit_id: &str) -> Result<CommitMessage, Error> {
        let title = self.git(&["show", "-s", "--format=%s", commit_id])?;
        let body = self.git(&["show", "-s", "--format=%b", commit_id])?;
        Ok(CommitMessage {
            title: compose_hangul(title.trim_end_matches('\n')),
            body: compose_hangul(body.trim_end_matches('\n')),
        })
    }
}

fn parse_commit_log(output: &str) -> Result<Vec<CommitSummary>, Error> {
    let mut commits = Vec::new();
    for record in output.split('\x1e') {
        let record = record.trim_matches('\n');
        if record.is_empty() {
            continue;
        }

        let mut fields = record.splitn(7, '\x1f');
        let id = fields.next().unwrap_or_default();
        let short_id = fields.next().unwrap_or_default();
        let author_name = fields.next().unwrap_or_default();
        let author_email = fields.next().unwrap_or_default();
        let time_seconds = fields
            .next()
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or_else(|| invalid_commit_log(output))?;
        let parent_ids = fields
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let summary = fields.next().unwrap_or_default();

        if id.is_empty() || short_id.is_empty() {
            return Err(invalid_commit_log(output));
        }

        commits.push(CommitSummary {
            id: id.to_string(),
            short_id: short_id.to_string(),
            summary: compose_hangul(summary),
            author_name: compose_hangul(author_name),
            author_email: author_email.to_string(),
            author_avatar_url: author_avatar_url_from_email(author_email),
            time_seconds,
            parent_ids,
        });
    }

    Ok(commits)
}

fn author_avatar_url_from_email(email: &str) -> Option<String> {
    let local = email
        .trim()
        .strip_suffix("@users.noreply.github.com")?
        .trim();
    if local.is_empty() || local.contains('@') {
        return None;
    }

    let login = local
        .rsplit_once('+')
        .map_or(local, |(_, login)| login)
        .trim();
    if login.is_empty()
        || login.starts_with('-')
        || login.ends_with('-')
        || !login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }

    Some(format!("https://github.com/{login}.png?size=128"))
}

fn invalid_commit_log(output: &str) -> Error {
    Error::GitCommand {
        command: "git log --format=%H%x1f%h%x1f%an%x1f%ae%x1f%at%x1f%P%x1f%s%x1e".into(),
        stderr: format!("unexpected commit log output: {}", output.trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn github_noreply_email_maps_to_avatar_url() {
        assert_eq!(
            author_avatar_url_from_email("123456+octocat@users.noreply.github.com").as_deref(),
            Some("https://github.com/octocat.png?size=128")
        );
        assert_eq!(
            author_avatar_url_from_email("octocat@users.noreply.github.com").as_deref(),
            Some("https://github.com/octocat.png?size=128")
        );
    }

    #[test]
    fn non_github_or_invalid_email_has_no_avatar_url() {
        assert_eq!(author_avatar_url_from_email("octocat@example.com"), None);
        assert_eq!(author_avatar_url_from_email(""), None);
        assert_eq!(
            author_avatar_url_from_email("+@users.noreply.github.com"),
            None
        );
        assert_eq!(
            author_avatar_url_from_email("-octocat@users.noreply.github.com"),
            None
        );
        assert_eq!(
            author_avatar_url_from_email("octocat_1@users.noreply.github.com"),
            None
        );
    }

    #[test]
    fn parse_commit_log_derives_author_avatar_url() {
        let output = concat!(
            "abc123456789\x1fabc1234\x1fOcto Cat\x1f",
            "123456+octocat@users.noreply.github.com\x1f1716000000\x1f\x1f",
            "Add avatar support\x1e"
        );

        let commits = parse_commit_log(output).unwrap();

        assert_eq!(
            commits[0].author_avatar_url.as_deref(),
            Some("https://github.com/octocat.png?size=128")
        );
    }

    #[test]
    fn repository_reports_no_head_for_empty_repo() {
        let repo_dir = TempRepo::new("empty");

        Repository::init(&repo_dir.path).unwrap();
        let repo = Repository::open(&repo_dir.path).unwrap();

        assert!(matches!(repo.list_commits(1), Err(Error::NoHead)));
    }

    #[test]
    fn repository_lists_commits_from_selected_refs() {
        let repo_dir = TempRepo::init_with_commit("selected-refs");
        repo_dir.git(&["branch", "-M", "main"]);
        repo_dir.git(&["switch", "-c", "feature/a"]);
        repo_dir.write("a.txt", "a\n");
        repo_dir.git(&["add", "a.txt"]);
        repo_dir.git(&["commit", "-m", "feature a"]);
        repo_dir.git(&["switch", "main"]);
        repo_dir.git(&["switch", "-c", "feature/b"]);
        repo_dir.write("b.txt", "b\n");
        repo_dir.git(&["add", "b.txt"]);
        repo_dir.git(&["commit", "-m", "feature b"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let commits = repo
            .list_commits_from_refs(
                &["refs/heads/feature/a".into(), "refs/heads/feature/b".into()],
                10,
            )
            .unwrap();

        let summaries: Vec<&str> = commits
            .iter()
            .map(|commit| commit.summary.as_str())
            .collect();
        assert!(summaries.contains(&"feature a"));
        assert!(summaries.contains(&"feature b"));
        assert!(summaries.contains(&"initial"));
    }

    #[test]
    fn repository_pages_commits_from_refs_with_cursor() {
        let repo_dir = TempRepo::init_with_commit("paged-commits");
        repo_dir.git(&["branch", "-M", "main"]);
        for index in 1..=3 {
            repo_dir.write("file.txt", &format!("{index}\n"));
            repo_dir.git(&["add", "file.txt"]);
            repo_dir.git(&["commit", "-m", &format!("commit {index}")]);
        }

        let repo = Repository::open(&repo_dir.path).unwrap();
        let first_page = repo.list_commit_page_from_refs(&[], None, 2).unwrap();

        assert_eq!(
            first_page
                .commits
                .iter()
                .map(|commit| commit.summary.as_str())
                .collect::<Vec<_>>(),
            vec!["commit 3", "commit 2"]
        );

        let second_page = repo
            .list_commit_page_from_refs(&[], first_page.next_cursor, 2)
            .unwrap();

        assert_eq!(
            second_page
                .commits
                .iter()
                .map(|commit| commit.summary.as_str())
                .collect::<Vec<_>>(),
            vec!["commit 1", "initial"]
        );
        assert_eq!(second_page.next_cursor, None);
    }
}
