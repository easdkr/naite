use std::path::PathBuf;

use naite_core::{RebasePlanEntry, Repository};

use super::state::RebasePlanRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadPlanOutcome {
    pub plan: Vec<RebasePlanRow>,
    pub current_author_email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    Paused { message: String },
}

pub(crate) async fn load_plan(
    path: PathBuf,
    target_ref: String,
) -> Result<LoadPlanOutcome, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        if repo.operation_state().is_busy() {
            return Err("another Git operation is already in progress".into());
        }
        if repo.status_detail().map_err(|e| e.to_string())?.is_dirty() {
            return Err("worktree has local changes".into());
        }

        let current_author_email = repo.configured_user_email().map_err(|e| e.to_string())?;
        let plan = repo
            .rebase_plan_entries_onto(&target_ref)
            .map_err(|e| e.to_string())
            .map(|entries| {
                entries
                    .into_iter()
                    .map(|entry| RebasePlanRow {
                        action: entry.action,
                        commit: naite_core::HistoryCommit {
                            id: entry.commit_id,
                            summary: entry.summary,
                            author_name: entry.author_name,
                            author_email: entry.author_email,
                        },
                        author_avatar_url: None,
                    })
                    .collect()
            })?;

        Ok(LoadPlanOutcome {
            plan,
            current_author_email,
        })
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn apply_plan(
    path: PathBuf,
    target_ref: String,
    entries: Vec<RebasePlanEntry>,
    reword_messages: Vec<(String, String)>,
) -> Result<ApplyOutcome, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match repo.apply_rebase_plan_onto(&target_ref, &entries, &reword_messages) {
            Ok(()) => Ok(ApplyOutcome::Applied),
            Err(err) if repo.operation_state().rebase_in_progress => Ok(ApplyOutcome::Paused {
                message: err.to_string(),
            }),
            Err(err) => Err(err.to_string()),
        }
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}
