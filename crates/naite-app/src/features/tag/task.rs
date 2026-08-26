use std::path::PathBuf;

use naite_core::Repository;

use crate::features::tag::Operation;

pub(crate) async fn load_local_utc_offset(path: PathBuf) -> Result<i32, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.local_utc_offset_minutes().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn run(path: PathBuf, operation: Operation) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match operation {
            Operation::Create {
                name,
                push_after_create,
                target_commit,
            } => {
                repo.create_tag(
                    &name,
                    target_commit.as_ref().map(|commit| commit.id.as_str()),
                )
                .map_err(|e| e.to_string())?;
                if push_after_create {
                    repo.push_tag(&name).map_err(|e| e.to_string())?;
                }
                Ok(())
            }
            Operation::Delete(target) => repo
                .delete_tag(&target.short_name)
                .map_err(|e| e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
