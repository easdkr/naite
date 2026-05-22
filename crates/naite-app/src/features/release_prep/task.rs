use std::path::PathBuf;

use naite_core::{
    HistoryCommit, RebaseAction, RefKind, RefSummary, ReleaseProfile, ReleaseSyncCheck, Repository,
};

use crate::features::rebase::RebasePlanRow;
use crate::features::repo_open::{self, LoadedRepo};

use super::message::ReleasePrepAction;

#[derive(Debug, Clone)]
pub struct PrepareOutcome {
    pub sync_check: ReleaseSyncCheck,
    pub backup_branch: Option<String>,
    pub current_branch: RefSummary,
    pub target: RefSummary,
    pub current_author_email: Option<String>,
    pub plan: Vec<RebasePlanRow>,
    pub repo_snapshot: LoadedRepo,
}

pub(crate) async fn load_suggestion(
    path: PathBuf,
) -> Result<naite_core::ReleaseProfileSuggestion, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.suggest_release_profile().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn prepare(
    path: PathBuf,
    profile: ReleaseProfile,
    backup_before_rebase: bool,
) -> Result<PrepareOutcome, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        if repo.operation_state().is_busy() {
            return Err("another Git operation is already in progress".into());
        }
        if repo.status_detail().map_err(|e| e.to_string())?.is_dirty() {
            return Err("worktree has local changes".into());
        }

        repo.fetch_release_remote(&profile.remote)
            .map_err(|e| e.to_string())?;
        repo.sync_release_branches_with_remote(&profile)
            .map_err(|e| e.to_string())?;
        let sync_check = repo
            .check_release_sync(&profile)
            .map_err(|e| e.to_string())?;
        if !sync_check.is_ready() {
            return Err(format_sync_failure(&sync_check));
        }

        repo.checkout_release_source(&profile)
            .map_err(|e| e.to_string())?;
        let backup_branch = if backup_before_rebase {
            Some(
                repo.create_release_backup_branch(&profile)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };

        let current_author_email = repo.configured_user_email().map_err(|e| e.to_string())?;
        let author_email = current_author_email.as_deref().and_then(normalized_email);

        let target_ref = format!("refs/heads/{}", profile.target_branch);
        let entries = repo
            .rebase_plan_entries_onto(&target_ref)
            .map_err(|e| e.to_string())?;

        let plan = entries
            .into_iter()
            .map(|entry| {
                let action =
                    release_prep_default_action(&entry.author_email, author_email.as_deref());
                RebasePlanRow {
                    action,
                    commit: HistoryCommit {
                        id: entry.commit_id,
                        summary: entry.summary,
                        author_name: entry.author_name,
                        author_email: entry.author_email,
                    },
                }
            })
            .collect();
        let repo_snapshot = repo_open::task::load_blocking(path)?;

        Ok(PrepareOutcome {
            current_branch: branch_ref(&profile.source_branch, true, &sync_check.source),
            target: branch_ref(&profile.target_branch, false, &sync_check.target),
            current_author_email,
            sync_check,
            backup_branch,
            plan,
            repo_snapshot,
        })
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn run_action(
    path: PathBuf,
    profile: ReleaseProfile,
    action: ReleasePrepAction,
) -> Result<ReleaseSyncCheck, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        match action {
            ReleasePrepAction::UpdateTargetFromSource => repo.fast_forward_release_target(&profile),
            ReleasePrepAction::PushTarget => repo.push_release_target(&profile),
            ReleasePrepAction::SyncSourceFromTarget => {
                repo.sync_release_source_from_target(&profile)
            }
        }
        .map_err(|e| e.to_string())?;
        repo.fetch_release_remote(&profile.remote)
            .map_err(|e| e.to_string())?;
        repo.check_release_sync(&profile).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

fn branch_ref(branch: &str, is_head: bool, sync: &naite_core::ReleaseBranchSync) -> RefSummary {
    RefSummary {
        kind: RefKind::LocalBranch,
        short_name: branch.to_string(),
        full_name: format!("refs/heads/{branch}"),
        target_short_id: sync.local_oid.as_deref().map(short_id).unwrap_or_default(),
        is_head,
        sync_status: None,
    }
}

fn format_sync_failure(sync: &ReleaseSyncCheck) -> String {
    let mut lines = vec!["Release branches still differ after automatic force sync.".to_string()];
    for branch in [&sync.source, &sync.target] {
        let state = match (&branch.local_oid, &branch.remote_oid) {
            (None, _) => format!("{} is missing locally", branch.local_ref),
            (_, None) => format!("{} is missing", branch.remote_ref),
            (Some(_), Some(_)) => {
                format!(
                    "{} differs from {} (ahead {}, behind {})",
                    branch.local_ref, branch.remote_ref, branch.ahead, branch.behind
                )
            }
        };
        if !branch.is_ready() {
            lines.push(state);
        }
    }
    lines.join("\n")
}

fn normalized_email(email: &str) -> Option<String> {
    let email = email.trim();
    (!email.is_empty()).then(|| email.to_ascii_lowercase())
}

fn emails_match(commit_email: &str, configured_email: &str) -> bool {
    normalized_email(commit_email).as_deref() == Some(configured_email)
}

fn release_prep_default_action(commit_email: &str, configured_email: Option<&str>) -> RebaseAction {
    if configured_email.is_some_and(|email| emails_match(commit_email, email)) {
        RebaseAction::Pick
    } else {
        RebaseAction::Drop
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_prep_defaults_non_matching_or_missing_author_to_drop() {
        assert_eq!(
            release_prep_default_action("other@example.com", Some("mine@example.com")),
            RebaseAction::Drop
        );
        assert_eq!(
            release_prep_default_action("mine@example.com", None),
            RebaseAction::Drop
        );
    }

    #[test]
    fn release_prep_defaults_matching_author_to_pick_case_insensitively() {
        assert_eq!(
            release_prep_default_action("Mine@Example.com", Some("mine@example.com")),
            RebaseAction::Pick
        );
    }
}
