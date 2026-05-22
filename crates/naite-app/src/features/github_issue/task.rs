use std::path::PathBuf;

use naite_core::{GitHubIssueSummary, ListGitHubIssuesOptions, Repository};

pub(crate) async fn list(
    path: PathBuf,
    options: ListGitHubIssuesOptions,
) -> Result<Vec<GitHubIssueSummary>, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.list_github_issues(options).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn open_in_browser(path: PathBuf, number: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.open_github_issue_in_browser(number)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
