use std::path::PathBuf;

use naite_core::Repository;

use crate::features::file_inspect::FileInsightResult;
use crate::state::FileInsightMode;

pub(crate) async fn load(
    repo_path: PathBuf,
    path: String,
    mode: FileInsightMode,
) -> (String, FileInsightMode, Result<FileInsightResult, String>) {
    let task_path = path.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&repo_path).map_err(|e| e.to_string())?;
        match mode {
            FileInsightMode::History => repo
                .file_history(&task_path)
                .map(FileInsightResult::History)
                .map_err(|e| e.to_string()),
            FileInsightMode::Blame => repo
                .file_blame(&task_path)
                .map(FileInsightResult::Blame)
                .map_err(|e| e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))
    .and_then(|result| result);

    (path, mode, result)
}
