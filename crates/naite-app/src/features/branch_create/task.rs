use std::path::PathBuf;

use naite_core::Repository;

pub(crate) async fn run(
    path: PathBuf,
    branch_name: String,
    start_point: Option<String>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.create_branch_and_checkout(&branch_name, start_point.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
