use iced::Task;
use naite_core::ReleaseProfile;

use crate::features::release_prep::{self, Message as ReleasePrepMessage, ReleasePrepAction};
use crate::features::repo_open;
use crate::state::ReleasePrepPhase;
use crate::{features::rebase::InteractiveRebaseSession, App, Message};

pub(crate) const DIRTY_WORKTREE_RELEASE_ERROR: &str =
    "Commit, stash, or resolve local changes first.";

impl App {
    pub(crate) fn update_release_prep(&mut self, message: ReleasePrepMessage) -> Task<Message> {
        match message {
            ReleasePrepMessage::Requested => self.start_release_prep(),
            ReleasePrepMessage::ConfigureRequested => self.start_release_prep_configure(),
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
            ReleasePrepMessage::ValidationScriptChanged(value) => {
                self.release_prep.validation_script = value;
                self.release_prep.error = None;
                self.sync_active_release_validation_script();
                Task::none()
            }
            ReleasePrepMessage::BackupToggled(value) => {
                self.release_prep.backup_before_rebase = value;
                Task::none()
            }
            ReleasePrepMessage::Cancelled => {
                if self.release_prep.auto_running {
                    return Task::none();
                }
                // Keep script edits made in the actions modal across sessions.
                let save = if self.release_prep.phase == ReleasePrepPhase::Actions {
                    self.persist_active_release_profile_if_changed()
                } else {
                    Task::none()
                };
                self.release_prep.phase = ReleasePrepPhase::Idle;
                self.release_prep.auto_running = false;
                self.release_prep.auto_next_action = None;
                self.release_prep.active_action = None;
                self.release_prep.completed_actions.clear();
                save
            }
            ReleasePrepMessage::ProfileSubmitted => self.submit_release_prep_profile(),
            ReleasePrepMessage::Prepared(result) => self.finish_release_prepare(*result),
            ReleasePrepMessage::PrepareStepStarted(_step) => Task::none(),
            ReleasePrepMessage::PrepareStepDone { .. } => Task::none(),
            ReleasePrepMessage::AutoRequested => self.start_release_prep_auto(),
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

        self.operation.error = None;
        self.operation.transient_status = None;
        self.release_prep.error = None;
        if self.repo.status_detail.is_dirty() {
            self.release_prep.error = Some(DIRTY_WORKTREE_RELEASE_ERROR.into());
            self.release_prep.phase = ReleasePrepPhase::Preparing;
            self.release_prep.animation_frame = 0;
            self.operation.loading = true;
            return Task::perform(release_prep::task::load_suggestion(path), |result| {
                Message::from(ReleasePrepMessage::SuggestionLoaded(result))
            });
        }
        if let Some(profile) = self.preferences.release_profiles.get(&path).cloned() {
            self.release_prep.remote = profile.remote.clone();
            self.release_prep.source_branch = profile.source_branch.clone();
            self.release_prep.target_branch = profile.target_branch.clone();
            self.release_prep.validation_script =
                profile.validation_script.clone().unwrap_or_default();
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

    fn start_release_prep_configure(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading || self.release_prep.auto_running {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.transient_status = None;
        self.release_prep.error = None;
        self.release_prep.force_config = true;
        self.release_prep.phase = ReleasePrepPhase::Preparing;
        self.release_prep.animation_frame = 0;
        self.operation.loading = true;
        Task::perform(release_prep::task::load_suggestion(path), |result| {
            Message::from(ReleasePrepMessage::SuggestionLoaded(result))
        })
    }

    fn open_release_prep_config(
        &mut self,
        result: Result<naite_core::ReleaseProfileSuggestion, String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        let force_config = std::mem::take(&mut self.release_prep.force_config);
        match result {
            Ok(suggestion) => {
                let pending_error = self.release_prep.error.take();
                let saved = force_config
                    .then(|| {
                        self.repo
                            .path
                            .as_ref()
                            .and_then(|path| self.preferences.release_profiles.get(path))
                            .cloned()
                    })
                    .flatten();
                let base = saved.unwrap_or_else(|| suggestion.default_profile.clone());
                self.release_prep.remote = base.remote.clone();
                self.release_prep.source_branch = base.source_branch.clone();
                self.release_prep.target_branch = base.target_branch.clone();
                self.release_prep.validation_script =
                    base.validation_script.clone().unwrap_or_default();
                self.release_prep.backup_before_rebase = false;
                self.release_prep.error = pending_error;
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
                if !validation_script_is_single_line(profile.validation_script.as_deref()) {
                    "Validation script must be a single line (no tabs or newlines).".into()
                } else {
                    "Remote, source branch, and target branch are required and must differ.".into()
                },
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
        self.release_prep.auto_running = false;
        self.release_prep.auto_next_action = None;
        self.release_prep.active_action = None;
        self.release_prep.completed_actions.clear();
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
                // The snapshot replaced the commit list, so re-run the same
                // avatar pipeline the repo-open path uses (image prefetch for
                // preserved URLs + provider lookup for uncached authors)
                // alongside the rebase plan's own resolution.
                let commit_avatar_task = self.prefetch_commit_avatars();
                let provider_commit_avatar_task = match self.repo.path.clone() {
                    Some(path) => self.load_provider_commit_author_avatars(path),
                    None => Task::none(),
                };
                let avatar_fetches = self.resolve_rebase_plan_avatars();
                Task::batch([
                    commit_avatar_task,
                    provider_commit_avatar_task,
                    avatar_fetches,
                    self.load_selected_rebase_diff(),
                ])
            }
            Err(message) => {
                self.release_prep.phase = ReleasePrepPhase::Configuring;
                self.release_prep.animation_frame = 0;
                self.release_prep.error = Some(message);
                if let Some(path) = self.repo.path.clone() {
                    self.operation.loading = true;
                    Task::perform(repo_open::task::load(path), |result| {
                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                    })
                } else {
                    Task::none()
                }
            }
        }
    }

    fn apply_release_prep_repo_snapshot(&mut self, snapshot: repo_open::LoadedRepo) {
        let (
            path,
            mut commits,
            commit_page_cursor,
            refs,
            stashes,
            worktrees,
            head_branch,
            status_detail,
            sync_status,
            operation_state,
        ) = snapshot;

        // The snapshot comes fresh from git with only noreply-derived avatar
        // URLs; restore the provider-resolved ones so the commit graph keeps
        // its avatars across the release-prep reload.
        self.preserve_known_commit_avatar_urls(&path, &mut commits);
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
        if self.release_prep.auto_running || self.release_prep.completed_actions.contains(&action) {
            return Task::none();
        }
        self.release_prep.auto_running = false;
        self.release_prep.auto_next_action = None;
        self.start_release_prep_action_internal(action)
    }

    pub(crate) fn release_has_script(&self) -> bool {
        self.release_prep
            .active_profile
            .as_ref()
            .is_some_and(ReleaseProfile::has_validation_script)
    }

    /// Apply edits from the actions-modal script input to the active
    /// profile so gating and the auto sequence pick them up immediately.
    /// The config form only writes to the input buffer; submit re-snapshots
    /// the profile there.
    fn sync_active_release_validation_script(&mut self) {
        if self.release_prep.phase != ReleasePrepPhase::Actions
            || self.release_prep.auto_running
            || self.operation.loading
        {
            return;
        }
        let script = self.release_prep.validation_script.trim();
        let script = (!script.is_empty()).then(|| script.to_string());
        let Some(profile) = self.release_prep.active_profile.as_mut() else {
            return;
        };
        if profile.validation_script == script {
            return;
        }
        profile.validation_script = script;
        // The edited script has not run yet, so any earlier validation
        // result no longer applies; this also re-locks "Push target".
        self.release_prep
            .completed_actions
            .retain(|action| *action != ReleasePrepAction::ValidateTarget);
    }

    pub(crate) fn persist_active_release_profile_if_changed(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(profile) = self.release_prep.active_profile.clone() else {
            return Task::none();
        };
        if self
            .preferences
            .release_profiles
            .get(&path)
            .is_some_and(|saved| saved == &profile)
        {
            return Task::none();
        }
        self.preferences.release_profiles.insert(path, profile);
        self.save_preferences()
    }

    pub(crate) fn continue_release_prep_auto(&mut self) -> Task<Message> {
        if !self.release_prep.auto_running
            || self.operation.loading
            || self.operation.auto_fetch_path.is_some()
        {
            return Task::none();
        }
        let Some(action) = self.release_prep.auto_next_action.take() else {
            self.release_prep.auto_running = false;
            self.release_prep.phase = ReleasePrepPhase::Actions;
            self.set_transient_status("Auto promotion complete".into());
            return Task::none();
        };
        self.start_release_prep_action_internal(action)
    }

    fn start_release_prep_auto(&mut self) -> Task<Message> {
        if self.release_prep.auto_running {
            return Task::none();
        }
        if self.release_prep.active_profile.is_none() {
            self.operation.error = Some("Plan a release promotion first".into());
            return Task::none();
        }
        let Some(next_action) = next_incomplete_release_prep_action(
            &self.release_prep.completed_actions,
            self.release_has_script(),
        ) else {
            self.set_transient_status("Auto promotion already complete".into());
            return Task::none();
        };
        self.release_prep.auto_running = true;
        self.release_prep.auto_next_action = Some(next_action);
        self.continue_release_prep_auto()
    }

    fn start_release_prep_action_internal(&mut self, action: ReleasePrepAction) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(profile) = self.release_prep.active_profile.clone() else {
            return Task::none();
        };
        if self.operation.loading || self.operation.auto_fetch_path.is_some() {
            return Task::none();
        }
        // Running the validation step is the natural moment to keep a
        // script edited in the actions modal for future promotions.
        let save = if action == ReleasePrepAction::ValidateTarget {
            self.persist_active_release_profile_if_changed()
        } else {
            Task::none()
        };
        self.operation.loading = true;
        self.operation.error = None;
        self.release_prep.phase = ReleasePrepPhase::RunningAction;
        self.release_prep.active_action = Some(action);
        self.release_prep.animation_frame = 0;
        let run = Task::perform(
            release_prep::task::run_action(path, profile, action),
            move |result| {
                Message::from(ReleasePrepMessage::ActionDone {
                    action,
                    result: Box::new(result),
                })
            },
        );
        Task::batch([save, run])
    }

    fn finish_release_prep_action(
        &mut self,
        action: ReleasePrepAction,
        result: Result<naite_core::ReleaseSyncCheck, String>,
    ) -> Task<Message> {
        self.operation.loading = false;
        self.release_prep.active_action = None;
        match result {
            Ok(sync_check) => {
                let was_auto_action = self.release_prep.auto_running;
                self.release_prep.sync_check = Some(sync_check);
                self.release_prep.phase = ReleasePrepPhase::Actions;
                self.release_prep.animation_frame = 0;
                if !self.release_prep.completed_actions.contains(&action) {
                    self.release_prep.completed_actions.push(action);
                }
                if was_auto_action {
                    self.release_prep.auto_next_action =
                        next_release_prep_action(action, self.release_has_script());
                    if self.release_prep.auto_next_action.is_none() {
                        self.release_prep.auto_running = false;
                    }
                }
                self.operation.pending_transient_status_after_reload =
                    Some(if was_auto_action && self.release_prep.auto_running {
                        format!("{} complete; continuing auto promotion", action.label())
                    } else if was_auto_action
                        && self.release_prep.auto_next_action.is_none()
                        && action == ReleasePrepAction::SyncSourceFromTarget
                    {
                        "Auto promotion complete".into()
                    } else {
                        format!("{} complete", action.label())
                    });
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
                self.release_prep.auto_running = false;
                self.release_prep.auto_next_action = None;
                self.release_prep.phase = ReleasePrepPhase::Actions;
                self.release_prep.animation_frame = 0;
                self.operation.error = Some(message);
                Task::none()
            }
        }
    }
}

/// Ordered release promotion actions; `ValidateTarget` only participates when
/// the active profile configures a validation script.
pub(crate) fn release_prep_actions_for(has_script: bool) -> &'static [ReleasePrepAction] {
    const WITH_VALIDATION: [ReleasePrepAction; 4] = [
        ReleasePrepAction::UpdateTargetFromSource,
        ReleasePrepAction::ValidateTarget,
        ReleasePrepAction::PushTarget,
        ReleasePrepAction::SyncSourceFromTarget,
    ];
    const WITHOUT_VALIDATION: [ReleasePrepAction; 3] = [
        ReleasePrepAction::UpdateTargetFromSource,
        ReleasePrepAction::PushTarget,
        ReleasePrepAction::SyncSourceFromTarget,
    ];
    if has_script {
        &WITH_VALIDATION
    } else {
        &WITHOUT_VALIDATION
    }
}

fn next_release_prep_action(
    action: ReleasePrepAction,
    has_script: bool,
) -> Option<ReleasePrepAction> {
    let actions = release_prep_actions_for(has_script);
    actions
        .iter()
        .position(|candidate| *candidate == action)
        .and_then(|index| actions.get(index + 1))
        .copied()
}

fn next_incomplete_release_prep_action(
    completed_actions: &[ReleasePrepAction],
    has_script: bool,
) -> Option<ReleasePrepAction> {
    release_prep_actions_for(has_script)
        .iter()
        .find(|action| !completed_actions.contains(action))
        .copied()
}

fn validation_script_is_single_line(script: Option<&str>) -> bool {
    script.is_none_or(|script| !script.contains(['\t', '\n', '\r']))
}

fn valid_profile_input(profile: &ReleaseProfile) -> bool {
    validation_script_is_single_line(profile.validation_script.as_deref())
        && !profile.remote.trim().is_empty()
        && !profile.source_branch.trim().is_empty()
        && !profile.target_branch.trim().is_empty()
        && profile.source_branch != profile.target_branch
}
