use crate::state::RepositoryCatalog;

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<RepositoryCatalog, String>),
    Saved(Result<(), String>),
}
