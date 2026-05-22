use crate::persistence;
use crate::state::RepositoryCatalog;

pub(crate) async fn load() -> Result<RepositoryCatalog, String> {
    tokio::task::spawn_blocking(persistence::load_repository_catalog)
        .await
        .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn save(catalog: RepositoryCatalog) -> Result<(), String> {
    tokio::task::spawn_blocking(move || persistence::save_repository_catalog(&catalog))
        .await
        .map_err(|e| format!("worker join error: {e}"))?
}
