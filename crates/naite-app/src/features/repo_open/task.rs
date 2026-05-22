use std::path::PathBuf;

use naite_core::{
    CommitAuthorAvatar, CommitPage, CommitPageCursor, Error as CoreError, Refs, Repository,
};

use crate::features::repo_open::LoadedRepo;

const COMMIT_PAGE_SIZE: usize = 500;

pub(crate) async fn pick_folder(title: &'static str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
}

pub(crate) async fn load(path: PathBuf) -> Result<LoadedRepo, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> { load_blocking(path) })
        .await
        .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) fn load_blocking(path: PathBuf) -> Result<LoadedRepo, String> {
    let repo = Repository::open(&path).map_err(|e| e.to_string())?;
    let refs = repo.refs().map_err(|e| e.to_string())?;
    let graph_ref_names = graph_refs(&refs);
    let commit_page =
        match repo.list_commit_page_from_refs(&graph_ref_names, None, COMMIT_PAGE_SIZE) {
            Ok(page) => page,
            Err(CoreError::NoHead) => CommitPage {
                commits: Vec::new(),
                next_cursor: None,
            },
            Err(err) => return Err(err.to_string()),
        };
    let stashes = repo.list_stashes().map_err(|e| e.to_string())?;
    let worktrees = repo.list_worktrees().map_err(|e| e.to_string())?;
    let head_branch = repo.head_branch();
    let status_detail = repo.status_detail().map_err(|e| e.to_string())?;
    let sync_status = repo.branch_sync_status().unwrap_or_default();
    let operation_state = repo.operation_state();
    let path = repo
        .workdir()
        .unwrap_or(repo.path())
        .canonicalize()
        .unwrap_or_else(|_| repo.workdir().unwrap_or(repo.path()).to_path_buf());
    Ok((
        path,
        commit_page.commits,
        commit_page.next_cursor,
        refs,
        stashes,
        worktrees,
        head_branch,
        status_detail,
        sync_status,
        operation_state,
    ))
}

pub(crate) async fn load_more_commits(
    path: PathBuf,
    cursor: CommitPageCursor,
) -> (PathBuf, Result<CommitPage, String>) {
    let path_for_task = path.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path_for_task).map_err(|e| e.to_string())?;
        let refs = repo.refs().map_err(|e| e.to_string())?;
        let graph_ref_names = graph_refs(&refs);
        repo.list_commit_page_from_refs(&graph_ref_names, Some(cursor), COMMIT_PAGE_SIZE)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))
    .and_then(|result| result);

    (path, result)
}

pub(crate) async fn load_commit_author_avatars(
    path: PathBuf,
    commit_ids: Vec<String>,
) -> (PathBuf, Result<Vec<CommitAuthorAvatar>, String>) {
    let path_for_task = path.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path_for_task).map_err(|e| e.to_string())?;
        repo.resolve_github_commit_author_avatars(&commit_ids)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))
    .and_then(|result| result);

    (path, result)
}

fn graph_refs(refs: &Refs) -> Vec<String> {
    refs.local
        .iter()
        .chain(refs.remote.iter())
        .chain(refs.tags.iter())
        .filter(|ref_summary| !ref_summary.target_short_id.is_empty())
        .map(|ref_summary| ref_summary.full_name.clone())
        .collect()
}

pub(crate) async fn clone_repo(url: String, parent: PathBuf) -> Result<PathBuf, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        Repository::clone_from_url(&url, parent).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn init(path: PathBuf) -> Result<PathBuf, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        Repository::init(path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use naite_core::{RefKind, RefSummary};

    fn ref_summary(kind: RefKind, short_name: &str, target_short_id: &str) -> RefSummary {
        let prefix = match kind {
            RefKind::LocalBranch => "refs/heads",
            RefKind::RemoteBranch => "refs/remotes",
            RefKind::Tag => "refs/tags",
        };

        RefSummary {
            kind,
            short_name: short_name.into(),
            full_name: format!("{prefix}/{short_name}"),
            target_short_id: target_short_id.into(),
            is_head: false,
            sync_status: None,
        }
    }

    #[test]
    fn graph_refs_include_local_remote_and_tag_refs() {
        let refs = Refs {
            local: vec![ref_summary(RefKind::LocalBranch, "main", "abc1234")],
            remote: vec![ref_summary(RefKind::RemoteBranch, "origin/main", "abc1234")],
            tags: vec![ref_summary(RefKind::Tag, "v1.0.0", "abc1234")],
        };

        assert_eq!(
            graph_refs(&refs),
            vec![
                "refs/heads/main",
                "refs/remotes/origin/main",
                "refs/tags/v1.0.0"
            ]
        );
    }

    #[test]
    fn graph_refs_skip_refs_without_targets() {
        let refs = Refs {
            local: vec![ref_summary(RefKind::LocalBranch, "main", "abc1234")],
            remote: vec![ref_summary(RefKind::RemoteBranch, "origin/missing", "")],
            tags: vec![ref_summary(RefKind::Tag, "dangling", "")],
        };

        assert_eq!(graph_refs(&refs), vec!["refs/heads/main"]);
    }
}
