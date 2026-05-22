use iced::Task;
use naite_core::ReleaseProfile;

use crate::features::release_prep::{self, Message as ReleasePrepMessage, ReleasePrepAction};
use crate::features::repo_open;
use crate::state::ReleasePrepPhase;
use crate::{features::rebase::InteractiveRebaseSession, App, Message};

impl App {
    pub(crate) fn update_release_prep(&mut self, message: ReleasePrepMessage) -> Task<Message> {
        match message {
            ReleasePrepMessage::Requested => self.start_release_prep(),
            ReleasePrepMessage::SuggestionLoaded(result) => self.open_release_prep_config(result),
            ReleasePrepMessage::RemoteChanged(value) => {
                self.release_prep.remote = value;
                self.release_prep.error = None;
                Task::none()
            }
            ReleasePrepMessage::SourceBranchChanged(value) => {
                self.release_prep.source_branch = value;
                self.release_prep.error = None;
                Task::none()
            }
            ReleasePrepMessage::TargetBranchChanged(value) => {
                self.release_prep.target_branch = value;
                self.release_prep.error = None;
                Task::none()
            }
            ReleasePrepMessage::BackupToggled(value) => {
                self.release_prep.backup_before_rebase = value;
                Task::none()
            }
            ReleasePrepMessage::Cancelled => {
                self.release_prep.phase = ReleasePrepPhase::Idle;
                Task::none()
            }
            ReleasePrepMessage::ProfileSubmitted => self.submit_release_prep_profile(),
            ReleasePrepMessage::Prepared(result) => self.finish_release_prepare(*result),
            ReleasePrepMessage::ActionRequested(action) => self.start_release_prep_action(action),
            ReleasePrepMessage::ActionDone { action, result } => {
                self.finish_release_prep_action(action, *result)
            }
        }
    }

    fn start_release_prep(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }
        if self.repo.status_detail.is_dirty() {
            self.release_prep.error = Some("Commit, stash, or resolve local changes first.".into());
            self.release_prep.phase = ReleasePrepPhase::Configuring;
            self.release_prep.animation_frame = 0;
            return Task::none();
        }

        self.operation.error = None;
        self.operation.transient_status = None;
        self.release_prep.error = None;
        if let Some(profile) = self.preferences.release_profiles.get(&path).cloned() {
            self.release_prep.remote = profile.remote.clone();
            self.release_prep.source_branch = profile.source_branch.clone();
            self.release_prep.target_branch = profile.target_branch.clone();
            self.begin_release_prepare(path, profile, self.release_prep.backup_before_rebase)
        } else {
            self.release_prep.phase = ReleasePrepPhase::Preparing;
            self.release_prep.animation_frame = 0;
            self.operation.loading = true;
            Task::perform(release_prep::task::load_suggestion(path), |result| {
                Message::from(ReleasePrepMessage::SuggestionLoaded(result))
            })
        }
    }

    fn open_release_prep_config(
        &mut self,
        result: Result<naite_core::ReleaseProfileSuggestion, String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        match result {
            Ok(suggestion) => {
                self.release_prep.remote = suggestion.default_profile.remote.clone();
                self.release_prep.source_branch = suggestion.default_profile.source_branch.clone();
                self.release_prep.target_branch = suggestion.default_profile.target_branch.clone();
                self.release_prep.backup_before_rebase = false;
                self.release_prep.error = None;
                self.release_prep.suggestion = Some(suggestion);
                self.release_prep.phase = ReleasePrepPhase::Configuring;
                self.release_prep.animation_frame = 0;
            }
            Err(message) => {
                self.release_prep.phase = ReleasePrepPhase::Idle;
                self.operation.error = Some(message);
            }
        }
        Task::none()
    }

    fn submit_release_prep_profile(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let profile = self.release_prep.profile_from_inputs();
        if !valid_profile_input(&profile) {
            self.release_prep.error = Some(
                "Remote, source branch, and target branch are required and must differ.".into(),
            );
            return Task::none();
        }
        self.preferences
            .release_profiles
            .insert(path.clone(), profile.clone());
        let save = self.save_preferences();
        let prepare =
            self.begin_release_prepare(path, profile, self.release_prep.backup_before_rebase);
        Task::batch([save, prepare])
    }

    fn begin_release_prepare(
        &mut self,
        path: std::path::PathBuf,
        profile: ReleaseProfile,
        backup_before_rebase: bool,
    ) -> Task<Message> {
        self.release_prep.phase = ReleasePrepPhase::Preparing;
        self.release_prep.active_profile = Some(profile.clone());
        self.release_prep.sync_check = None;
        self.release_prep.error = None;
        self.release_prep.animation_frame = 0;
        self.operation.loading = true;
        self.operation.error = None;
        self.operation.pending_transient_status_after_reload = None;
        Task::perform(
            release_prep::task::prepare(path, profile, backup_before_rebase),
            |result| Message::from(ReleasePrepMessage::Prepared(Box::new(result))),
        )
    }

    fn finish_release_prepare(
        &mut self,
        result: Result<release_prep::task::PrepareOutcome, String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        match result {
            Ok(outcome) => {
                let picked = outcome
                    .plan
                    .iter()
                    .filter(|row| row.action == naite_core::RebaseAction::Pick)
                    .count();
                let dropped = outcome.plan.len().saturating_sub(picked);
                self.apply_release_prep_repo_snapshot(outcome.repo_snapshot);
                self.release_prep.phase = ReleasePrepPhase::Idle;
                self.release_prep.sync_check = Some(outcome.sync_check.clone());
                self.rebase = Some(InteractiveRebaseSession {
                    current_branch: outcome.current_branch,
                    target: outcome.target,
                    current_author_email: outcome.current_author_email,
                    plan: outcome.plan,
                    selected: 0,
                    drag: None,
                    reword_drafts: Default::default(),
                    applying: false,
                    scroll_offset: 0.0,
                });
                let mut status =
                    format!("Release promotion ready: {picked} picks, {dropped} drops");
                if let Some(backup) = outcome.backup_branch {
                    status.push_str(&format!("; backup {backup}"));
                }
                self.set_transient_status(status);
                self.load_selected_rebase_diff()
            }
            Err(message) => {
                self.release_prep.phase = ReleasePrepPhase::Configuring;
                self.release_prep.animation_frame = 0;
                self.release_prep.error = Some(message);
                Task::none()
            }
        }
    }

    fn apply_release_prep_repo_snapshot(&mut self, snapshot: repo_open::LoadedRepo) {
        let (
            path,
            commits,
            commit_page_cursor,
            refs,
            stashes,
            worktrees,
            head_branch,
            status_detail,
            sync_status,
            operation_state,
        ) = snapshot;

        self.repo.path = Some(path.clone());
        self.repo.commits = commits;
        self.repo.commit_page_cursor = commit_page_cursor;
        self.repo.commits_loading_more = false;
        self.repo.refs = refs;
        self.repo.stashes = stashes;
        self.repo.worktrees = worktrees;
        self.repo.head_branch = head_branch;
        self.repo.status_detail = status_detail;
        self.repo.sync_status = sync_status;
        self.repo.operation_state = operation_state;
        self.tabs
            .last_refreshed
            .insert(path, std::time::Instant::now());
        self.refresh_graph_layout();
    }

    fn start_release_prep_action(&mut self, action: ReleasePrepAction) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(profile) = self.release_prep.active_profile.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }
        self.operation.loading = true;
        self.operation.error = None;
        self.release_prep.phase = ReleasePrepPhase::RunningAction;
        self.release_prep.animation_frame = 0;
        Task::perform(
            release_prep::task::run_action(path, profile, action),
            move |result| {
                Message::from(ReleasePrepMessage::ActionDone {
                    action,
                    result: Box::new(result),
                })
            },
        )
    }

    fn finish_release_prep_action(
        &mut self,
        action: ReleasePrepAction,
        result: Result<naite_core::ReleaseSyncCheck, String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        match result {
            Ok(sync_check) => {
                self.release_prep.sync_check = Some(sync_check);
                self.release_prep.phase = ReleasePrepPhase::Actions;
                self.release_prep.animation_frame = 0;
                self.operation.pending_transient_status_after_reload =
                    Some(format!("{} complete", action.label()));
                if let Some(path) = self.repo.path.clone() {
                    self.operation.loading = true;
                    Task::perform(repo_open::task::load(path), |result| {
                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                    })
                } else {
                    self.set_transient_status(format!("{} complete", action.label()));
                    Task::none()
                }
            }
            Err(message) => {
                self.release_prep.phase = ReleasePrepPhase::Actions;
                self.release_prep.animation_frame = 0;
                self.operation.error = Some(message);
                Task::none()
            }
        }
    }
}

fn valid_profile_input(profile: &ReleaseProfile) -> bool {
    !profile.remote.trim().is_empty()
        && !profile.source_branch.trim().is_empty()
        && !profile.target_branch.trim().is_empty()
        && profile.source_branch != profile.target_branch
}
