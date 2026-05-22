use std::path::PathBuf;
use std::process::Command;

use naite_core::{PullMode, Repository, WorkspaceRepoSummary};

use super::MultiRepoOperationSummary;

pub(crate) async fn load(paths: Vec<PathBuf>) -> Vec<WorkspaceRepoSummary> {
    tokio::task::spawn_blocking(move || {
        paths
            .into_iter()
            .map(Repository::workspace_summary)
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default()
}

pub(crate) async fn fetch_all(paths: Vec<PathBuf>) -> MultiRepoOperationSummary {
    run_multi_repo(paths, |repo| repo.fetch_all_remotes()).await
}

pub(crate) async fn pull_all(paths: Vec<PathBuf>) -> MultiRepoOperationSummary {
    run_multi_repo(paths, |repo| repo.pull(PullMode::FastForwardOnly)).await
}

pub(crate) async fn locate(path: PathBuf) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let output = Command::new("open")
            .arg("-R")
            .arg(&path)
            .output()
            .map_err(|err| format!("failed to reveal {}: {err}", path.display()))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                format!("failed to reveal {}", path.display())
            } else {
                stderr
            })
        }
    })
    .await
    .map_err(|err| format!("worker join error: {err}"))?
}

async fn run_multi_repo<F>(paths: Vec<PathBuf>, operation: F) -> MultiRepoOperationSummary
where
    F: Fn(&Repository) -> Result<(), naite_core::Error> + Send + 'static + Copy,
{
    tokio::task::spawn_blocking(move || {
        let mut summary = MultiRepoOperationSummary {
            attempted: paths.len(),
            ..MultiRepoOperationSummary::default()
        };
        for path in paths {
            match Repository::open(&path).and_then(|repo| operation(&repo)) {
                Ok(()) => summary.succeeded += 1,
                Err(err) => summary.failures.push((path, err.to_string())),
            }
        }
        summary
    })
    .await
    .unwrap_or_else(|err| MultiRepoOperationSummary {
        attempted: 0,
        succeeded: 0,
        failures: vec![(PathBuf::new(), format!("worker join error: {err}"))],
    })
}
