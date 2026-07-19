use std::collections::HashMap;

use iced::Task;
use naite_core::{RebaseAction, RebasePlanEntry, RefKind};

use crate::features::rebase::{
    self, task::ApplyOutcome, DragState, InteractiveRebaseSession, Message as RebaseMessage,
    RebaseApplyMode, RebasePlanPreset,
};
use crate::features::{release_prep, repo_open};
use crate::message::OperationEvent;
use crate::state::{OpResult, OpSeverity, OperationKind, ReleasePrepPhase};
use crate::tasks;
use crate::{App, Message, RebasePrompt, RebasePromptRow};

use super::state::RebasePlanRow;

impl App {
    pub(crate) fn update_rebase(&mut self, message: RebaseMessage) -> Task<Message> {
        match message {
            RebaseMessage::Started(target) => self.start_interactive_rebase(target),
            RebaseMessage::Loaded {
                target,
                plan,
                current_branch,
                current_author_email,
            } => {
                let completion = match self
                    .operation_tracker
                    .current_id_for(&OperationKind::ManualAction("rebase_load_plan"))
                {
                    Some(id) => Task::done(Message::Operation(OperationEvent::Completed {
                        id,
                        result: OpResult::Success,
                        severity: OpSeverity::Recoverable,
                    })),
                    None => Task::none(),
                };
                self.operation.loading = false;
                self.selection.context_menu = None;
                self.rebase = Some(InteractiveRebaseSession {
                    current_branch,
                    target,
                    current_author_email,
                    plan,
                    selected: 0,
                    drag: None,
                    reword_drafts: Default::default(),
                    applying: false,
                    scroll_offset: 0.0,
                });
                let avatar_fetches = self.resolve_rebase_plan_avatars();
                completion.chain(Task::batch([
                    avatar_fetches,
                    self.load_selected_rebase_diff(),
                ]))
            }
            RebaseMessage::LoadFailed(message) => {
                let completion = match self
                    .operation_tracker
                    .current_id_for(&OperationKind::ManualAction("rebase_load_plan"))
                {
                    Some(id) => Task::done(Message::Operation(OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(message.clone()),
                        severity: OpSeverity::Recoverable,
                    })),
                    None => Task::none(),
                };
                self.operation.loading = false;
                self.operation.error = Some(message);
                completion
            }
            RebaseMessage::Cancelled => {
                self.rebase = None;
                self.selection.rebase_confirmation = None;
                Task::none()
            }
            RebaseMessage::RowSelected(index) => {
                let mut selected = false;
                if let Some(session) = self.rebase.as_mut() {
                    if index < session.plan.len() {
                        session.selected = index;
                        selected = true;
                    }
                }
                if selected {
                    self.load_selected_rebase_diff()
                } else {
                    Task::none()
                }
            }
            RebaseMessage::RowSelectedRelative(delta) => {
                let mut moved = false;
                if let Some(session) = self.rebase.as_mut() {
                    if !session.plan.is_empty() {
                        let before = session.selected;
                        let last = session.plan.len().saturating_sub(1) as isize;
                        session.selected =
                            (session.selected as isize + delta).clamp(0, last) as usize;
                        moved = before != session.selected;
                    }
                }
                if moved {
                    self.load_selected_rebase_diff()
                } else {
                    Task::none()
                }
            }
            RebaseMessage::ActionSet(index, action) => self.set_rebase_action(index, action),
            RebaseMessage::ActionSetSelected(action) => {
                let Some(selected) = self.rebase.as_ref().map(|session| session.selected) else {
                    return Task::none();
                };
                self.set_rebase_action(selected, action)
            }
            RebaseMessage::MoveUp(index) => self.move_rebase_row(index, -1),
            RebaseMessage::MoveDown(index) => self.move_rebase_row(index, 1),
            RebaseMessage::MoveSelected(delta) => {
                let Some(selected) = self.rebase.as_ref().map(|session| session.selected) else {
                    return Task::none();
                };
                self.move_rebase_row(selected, delta)
            }
            RebaseMessage::DragPressed(index) => {
                let press_origin = self.selection.cursor_position;
                if let Some(session) = self.rebase.as_mut() {
                    if index < session.plan.len() {
                        match press_origin {
                            Some(origin) => {
                                session.drag = Some(DragState {
                                    source_index: index,
                                    hover_index: index,
                                    press_origin: origin,
                                    started: false,
                                });
                            }
                            None => {
                                session.selected = index;
                                return self.load_selected_rebase_diff();
                            }
                        }
                    }
                }
                Task::none()
            }
            RebaseMessage::DragEnded => self.finish_rebase_drag(true),
            RebaseMessage::DragCancelled => self.finish_rebase_drag(false),
            RebaseMessage::EscapePressed => {
                if self
                    .rebase
                    .as_ref()
                    .and_then(|session| session.drag.as_ref())
                    .is_some()
                {
                    self.finish_rebase_drag(false)
                } else {
                    self.rebase = None;
                    self.selection.rebase_confirmation = None;
                    Task::none()
                }
            }
            RebaseMessage::Scrolled(offset) => {
                if let Some(session) = self.rebase.as_mut() {
                    session.scroll_offset = offset;
                }
                Task::none()
            }
            RebaseMessage::RewordOpened(index) => {
                self.set_rebase_action(index, RebaseAction::Reword)
            }
            RebaseMessage::RewordChanged(index, message) => {
                if let Some(session) = self.rebase.as_mut() {
                    if let Some(row) = session.plan.get(index) {
                        session.reword_drafts.insert(row.commit.id.clone(), message);
                    }
                }
                Task::none()
            }
            RebaseMessage::RewordCommitted(index) => {
                let mut selected = false;
                if let Some(session) = self.rebase.as_mut() {
                    if index < session.plan.len() {
                        session.selected = index;
                        selected = true;
                    }
                }
                if selected {
                    self.load_selected_rebase_diff()
                } else {
                    Task::none()
                }
            }
            RebaseMessage::PickMineRequested => {
                self.apply_rebase_plan_preset(RebasePlanPreset::KeepMine)
            }
            RebaseMessage::PresetRequested(preset) => self.apply_rebase_plan_preset(preset),
            RebaseMessage::ApplyRequested(mode) => self.open_rebase_prompt(mode),
            RebaseMessage::ApplyCancelled => {
                self.selection.rebase_confirmation = None;
                Task::none()
            }
            RebaseMessage::ApplyConfirmed => self.apply_rebase_plan(),
            RebaseMessage::Done { result, apply_mode } => {
                self.finish_rebase_operation(result, apply_mode)
            }
        }
    }

    fn start_interactive_rebase(&mut self, target: naite_core::RefSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }
        if !matches!(target.kind, RefKind::LocalBranch | RefKind::RemoteBranch) || target.is_head {
            let msg = "choose a non-HEAD branch as the rebase target".to_string();
            self.operation.fatal_error = Some(msg.clone());
            return self.fatal_validation_op(
                OperationKind::ManualAction("rebase_load_plan"),
                "Validating rebase target…".to_string(),
                msg,
            );
        }
        if self.repo.operation_state.is_busy() {
            let msg = "another Git operation is already in progress".to_string();
            self.operation.fatal_error = Some(msg.clone());
            return self.fatal_validation_op(
                OperationKind::ManualAction("rebase_load_plan"),
                "Checking operation state…".to_string(),
                msg,
            );
        }
        if self.repo.status_detail.is_dirty() {
            let msg = "worktree has local changes".to_string();
            self.operation.fatal_error = Some(msg.clone());
            return self.fatal_validation_op(
                OperationKind::ManualAction("rebase_load_plan"),
                "Checking worktree state…".to_string(),
                msg,
            );
        }
        let Some(current_branch) = self
            .repo
            .refs
            .local
            .iter()
            .find(|branch| branch.is_head)
            .cloned()
        else {
            let msg = "current HEAD is detached".to_string();
            self.operation.fatal_error = Some(msg.clone());
            return self.fatal_validation_op(
                OperationKind::ManualAction("rebase_load_plan"),
                "Checking current branch…".to_string(),
                msg,
            );
        };

        self.operation.error = None;
        self.operation.loading = true;
        let label = format!("Loading rebase plan onto {}…", target.short_name);
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("rebase_load_plan"),
            label,
        }));
        let target_ref = target.full_name.clone();
        let target_for_message = target.clone();
        start.chain(Task::perform(
            rebase::task::load_plan(path, target_ref),
            move |result| match result {
                Ok(outcome) => Message::from(RebaseMessage::Loaded {
                    target: target_for_message.clone(),
                    plan: outcome.plan,
                    current_branch: current_branch.clone(),
                    current_author_email: outcome.current_author_email,
                }),
                Err(message) => Message::from(RebaseMessage::LoadFailed(message)),
            },
        ))
    }

    fn fatal_validation_op(
        &mut self,
        kind: OperationKind,
        label: String,
        message: String,
    ) -> Task<Message> {
        let id = self.operation_tracker.next_id();
        let start = Task::done(Message::Operation(OperationEvent::Started {
            id,
            kind,
            label,
        }));
        let complete = Task::done(Message::Operation(OperationEvent::Completed {
            id,
            result: OpResult::Failed(message),
            severity: OpSeverity::Fatal,
        }));
        start.chain(complete)
    }

    fn set_rebase_action(&mut self, index: usize, action: RebaseAction) -> Task<Message> {
        let Some(session) = self.rebase.as_mut() else {
            return Task::none();
        };
        if index >= session.plan.len() || session.applying {
            return Task::none();
        }
        session.selected = index;
        session.plan[index].action = action;
        let commit_id = session.plan[index].commit.id.clone();
        match action {
            RebaseAction::Reword => {
                session
                    .reword_drafts
                    .entry(commit_id)
                    .or_insert_with(|| session.plan[index].commit.summary.clone());
            }
            _ => {
                session.reword_drafts.remove(&commit_id);
            }
        }
        Task::none()
    }

    fn move_rebase_row(&mut self, index: usize, delta: isize) -> Task<Message> {
        let Some(session) = self.rebase.as_mut() else {
            return Task::none();
        };
        if session.applying || index >= session.plan.len() {
            return Task::none();
        }
        let target = (index as isize + delta)
            .clamp(0, session.plan.len().saturating_sub(1) as isize) as usize;
        if target != index {
            session.plan.swap(index, target);
            session.selected = target;
            return self.load_selected_rebase_diff();
        }
        Task::none()
    }

    fn finish_rebase_drag(&mut self, commit_reorder: bool) -> Task<Message> {
        let Some(session) = self.rebase.as_mut() else {
            return Task::none();
        };
        let Some(drag) = session.drag.take() else {
            return Task::none();
        };
        if !drag.started {
            // It was a plain click — select the pressed row.
            if drag.source_index < session.plan.len() && session.selected != drag.source_index {
                session.selected = drag.source_index;
                return self.load_selected_rebase_diff();
            }
            return Task::none();
        }
        if commit_reorder
            && drag.source_index < session.plan.len()
            && drag.hover_index < session.plan.len()
            && drag.source_index != drag.hover_index
        {
            let row = session.plan.remove(drag.source_index);
            let insert_at = drag.hover_index.min(session.plan.len());
            session.plan.insert(insert_at, row);
            session.selected = insert_at;
            return self.load_selected_rebase_diff();
        }
        // Drag ended without a reorder (cancelled or dropped on source). Make
        // sure the pressed row still ends up selected, matching click behaviour.
        if drag.source_index < session.plan.len() && session.selected != drag.source_index {
            session.selected = drag.source_index;
            return self.load_selected_rebase_diff();
        }
        Task::none()
    }

    fn apply_rebase_plan_preset(&mut self, preset: RebasePlanPreset) -> Task<Message> {
        let Some(session) = self.rebase.as_ref() else {
            return Task::none();
        };
        if session.applying {
            return Task::none();
        }

        let preset_result = match preset {
            RebasePlanPreset::KeepMine => keep_mine_plan(session),
            RebasePlanPreset::SquashMine => squash_mine_plan(session),
            RebasePlanPreset::SquashAll => Ok(squash_all_plan(session)),
        };
        let (plan, reword_drafts, status) = match preset_result {
            Ok(result) => result,
            Err(message) => {
                let id = self.operation_tracker.next_id();
                self.operation.error = Some(message.clone());
                let start = Task::done(Message::Operation(OperationEvent::Started {
                    id,
                    kind: OperationKind::ManualAction("rebase_load_plan"),
                    label: "Computing rebase preset…".to_string(),
                }));
                let complete = Task::done(Message::Operation(OperationEvent::Completed {
                    id,
                    result: OpResult::Failed(message),
                    severity: OpSeverity::Recoverable,
                }));
                return start.chain(complete);
            }
        };

        if let Some(session) = self.rebase.as_mut() {
            session.plan = plan;
            session.reword_drafts.clear();
            session.reword_drafts.extend(reword_drafts);
            session.selected = 0;
            session.drag = None;
        }

        self.operation.error = None;
        self.set_transient_status(status);
        self.load_selected_rebase_diff()
    }

    pub(crate) fn load_selected_rebase_diff(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(commit_id) = self
            .rebase
            .as_ref()
            .and_then(InteractiveRebaseSession::selected_row)
            .map(|row| row.commit.id.clone())
        else {
            return Task::none();
        };

        self.selection.selected_commit_id = Some(commit_id.clone());
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_stash = None;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_error = None;
        self.operation.diff_loading = true;
        self.operation.pending_diff_commit_id = Some(commit_id.clone());
        self.operation.pending_wip_diff_target = None;
        self.operation.pending_stash_diff_selector = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;

        Task::perform(tasks::load_diff(path, commit_id), |(commit_id, result)| {
            Message::DiffLoaded { commit_id, result }
        })
    }

    fn open_rebase_prompt(&mut self, apply_mode: RebaseApplyMode) -> Task<Message> {
        let Some(session) = self.rebase.as_ref() else {
            return Task::none();
        };
        if !can_apply(session) {
            return Task::none();
        }
        match apply_mode {
            RebaseApplyMode::RebaseOnly => {}
            RebaseApplyMode::RebaseThenForcePush => {
                if self.release_prep.active_profile.is_some() {
                    let message =
                        "release promotion applies the rebase locally; push the target, then sync the source";
                    let id = self.operation_tracker.next_id();
                    self.operation.error = Some(message.to_string());
                    let start = Task::done(Message::Operation(OperationEvent::Started {
                        id,
                        kind: OperationKind::ManualAction("rebase_apply"),
                        label: "Preparing rebase prompt…".to_string(),
                    }));
                    let complete = Task::done(Message::Operation(OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(message.to_string()),
                        severity: OpSeverity::Recoverable,
                    }));
                    return start.chain(complete);
                }
                if let Err(message) = self.force_push_prompt_for_current_branch() {
                    let id = self.operation_tracker.next_id();
                    self.operation.error = Some(message.clone());
                    let start = Task::done(Message::Operation(OperationEvent::Started {
                        id,
                        kind: OperationKind::ManualAction("rebase_apply"),
                        label: "Preparing rebase prompt…".to_string(),
                    }));
                    let complete = Task::done(Message::Operation(OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(message),
                        severity: OpSeverity::Recoverable,
                    }));
                    return start.chain(complete);
                }
            }
            RebaseApplyMode::ReleasePromotionAuto => {
                if self.release_prep.active_profile.is_none() {
                    let msg = "Plan a release promotion first".to_string();
                    self.operation.fatal_error = Some(msg.clone());
                    return self.fatal_validation_op(
                        OperationKind::ManualAction("rebase_apply"),
                        "Preparing rebase prompt…".to_string(),
                        msg,
                    );
                }
            }
        }
        let follow_up = match apply_mode {
            RebaseApplyMode::RebaseOnly => "",
            RebaseApplyMode::RebaseThenForcePush => {
                " After it succeeds, naite will ask before running git push --force-with-lease."
            }
            RebaseApplyMode::ReleasePromotionAuto => {
                " After it succeeds, naite will update and push the target, then rebase and push the source."
            }
        };
        let preview_rows = rebase_prompt_preview(session);
        self.selection.rebase_confirmation = Some(RebasePrompt {
            title: format!(
                "Interactive rebase {} onto {}",
                session.current_branch.short_name, session.target.short_name
            ),
            detail: format!(
                "Will run git rebase -i {} with {}.{}",
                session.target.short_name,
                plan_counts(session),
                follow_up
            ),
            preview_rows,
            apply_mode,
        });
        Task::none()
    }

    /// Resolve avatar URLs for every author in the active rebase plan and
    /// kick off the image fetches. Resolution order matches the commit list:
    /// the persisted/known author cache first, then the noreply-email
    /// derivation, and finally the GitHub GraphQL lookup (`gh api graphql`)
    /// for commits neither covers.
    pub(crate) fn resolve_rebase_plan_avatars(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let known = self.known_author_avatar_urls_for_path(&path);

        let mut urls = Vec::new();
        let mut unresolved_commit_ids = Vec::new();
        if let Some(session) = self.rebase.as_mut() {
            for row in &mut session.plan {
                let resolved =
                    Self::author_avatar_key(&row.commit.author_email, &row.commit.author_name)
                        .and_then(|key| known.get(&key).cloned())
                        .or_else(|| {
                            naite_core::author_avatar_url_from_email(&row.commit.author_email)
                        });
                match resolved {
                    Some(url) => {
                        urls.push(url.clone());
                        row.author_avatar_url = Some(url);
                    }
                    None => unresolved_commit_ids.push(row.commit.id.clone()),
                }
            }
        }

        let image_fetches = Task::batch(
            urls.iter()
                .map(|url| self.maybe_fetch_avatar(Some(url.as_str()))),
        );
        if unresolved_commit_ids.is_empty() {
            return image_fetches;
        }
        let provider_lookup = Task::perform(
            repo_open::task::load_commit_author_avatars(path, unresolved_commit_ids),
            |(path, result)| {
                Message::from(repo_open::Message::CommitAuthorAvatarsLoaded { path, result })
            },
        );
        Task::batch([image_fetches, provider_lookup])
    }

    fn apply_rebase_plan(&mut self) -> Task<Message> {
        let Some(session) = self.rebase.as_mut() else {
            return Task::none();
        };
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if session.applying || !can_apply(session) {
            return Task::none();
        }

        let entries = plan_entries(&session.plan);
        let reword_messages = reword_messages(session);
        let target_ref = session.target.full_name.clone();
        let apply_mode = self
            .selection
            .rebase_confirmation
            .as_ref()
            .map(|prompt| prompt.apply_mode)
            .unwrap_or(RebaseApplyMode::RebaseOnly);
        session.applying = true;
        self.selection.rebase_confirmation = None;
        self.operation.error = None;
        self.operation.loading = true;
        self.operation.pending_force_push_after_reload = false;
        self.release_prep.auto_running = false;
        self.release_prep.auto_next_action = None;

        let start = Task::done(Message::Operation(OperationEvent::Started {
            id: self.operation_tracker.next_id(),
            kind: OperationKind::ManualAction("rebase_apply"),
            label: "Applying interactive rebase…".to_string(),
        }));
        start.chain(Task::perform(
            rebase::task::apply_plan(path, target_ref, entries, reword_messages),
            move |result| Message::from(RebaseMessage::Done { result, apply_mode }),
        ))
    }

    fn finish_rebase_operation(
        &mut self,
        result: Result<ApplyOutcome, String>,
        apply_mode: RebaseApplyMode,
    ) -> Task<Message> {
        let completion = match self
            .operation_tracker
            .current_id_for(&OperationKind::ManualAction("rebase_apply"))
        {
            Some(id) => {
                let event = match &result {
                    Ok(_) => OperationEvent::Completed {
                        id,
                        result: OpResult::Success,
                        severity: OpSeverity::Recoverable,
                    },
                    Err(message) => OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(message.clone()),
                        severity: OpSeverity::Recoverable,
                    },
                };
                Task::done(Message::Operation(event))
            }
            None => Task::none(),
        };
        self.operation.loading = false;
        if let Some(session) = self.rebase.as_mut() {
            session.applying = false;
        }
        match result {
            Ok(ApplyOutcome::Applied) => {
                self.rebase = None;
                self.selection.rebase_confirmation = None;
                let pending_force_push = apply_mode == RebaseApplyMode::RebaseThenForcePush;
                let pending_auto_promotion = apply_mode == RebaseApplyMode::ReleasePromotionAuto;
                if self.release_prep.active_profile.is_some() {
                    self.release_prep.phase = ReleasePrepPhase::Actions;
                    self.release_prep.animation_frame = 0;
                }
                if pending_auto_promotion {
                    self.release_prep.auto_running = true;
                    self.release_prep.auto_next_action =
                        Some(release_prep::ReleasePrepAction::UpdateTargetFromSource);
                    self.release_prep.active_action = None;
                    self.release_prep.completed_actions.clear();
                }
                if let Some(path) = self.repo.path.clone() {
                    self.operation.pending_transient_status_after_reload = Some(
                        if pending_force_push {
                            "Interactive rebase applied; confirm force push"
                        } else if pending_auto_promotion {
                            "Interactive rebase applied; starting auto promotion"
                        } else {
                            "Interactive rebase applied"
                        }
                        .into(),
                    );
                    self.operation.pending_force_push_after_reload = pending_force_push;
                    self.operation.loading = true;
                    let reload_start = Task::done(Message::Operation(OperationEvent::Started {
                        id: self.operation_tracker.next_id(),
                        kind: OperationKind::RepositoryLoad,
                        label: "Reloading repository…".to_string(),
                    }));
                    completion.chain(
                        reload_start.chain(Task::perform(repo_open::task::load(path), |result| {
                            Message::from(repo_open::Message::Loaded(Box::new(result)))
                        })),
                    )
                } else {
                    self.set_transient_status("Interactive rebase applied".into());
                    completion
                }
            }
            Ok(ApplyOutcome::Paused { message }) => {
                self.rebase = None;
                self.selection.rebase_confirmation = None;
                self.operation.pending_force_push_after_reload = false;
                if let Some(path) = self.repo.path.clone() {
                    self.operation.pending_transient_status_after_reload =
                        Some("Interactive rebase paused on conflicts".into());
                    self.operation.pending_error_after_reload = Some(message);
                    self.operation.loading = true;
                    let reload_start = Task::done(Message::Operation(OperationEvent::Started {
                        id: self.operation_tracker.next_id(),
                        kind: OperationKind::RepositoryLoad,
                        label: "Reloading repository…".to_string(),
                    }));
                    completion.chain(
                        reload_start.chain(Task::perform(repo_open::task::load(path), |result| {
                            Message::from(repo_open::Message::Loaded(Box::new(result)))
                        })),
                    )
                } else {
                    self.set_transient_status("Interactive rebase paused on conflicts".into());
                    self.operation.fatal_error = Some(message);
                    let id = self.operation_tracker.next_id();
                    let start = Task::done(Message::Operation(OperationEvent::Started {
                        id,
                        kind: OperationKind::ManualAction("rebase_apply"),
                        label: "Interactive rebase paused".to_string(),
                    }));
                    let complete = Task::done(Message::Operation(OperationEvent::Completed {
                        id,
                        result: OpResult::Failed(
                            "Interactive rebase paused on conflicts".to_string(),
                        ),
                        severity: OpSeverity::Fatal,
                    }));
                    completion.chain(start.chain(complete))
                }
            }
            Err(message) => {
                self.rebase = None;
                self.selection.rebase_confirmation = None;
                self.operation.pending_force_push_after_reload = false;
                self.operation.error = Some(message);
                completion
            }
        }
    }
}

type PresetResult = Result<(Vec<RebasePlanRow>, HashMap<String, String>, String), String>;

fn keep_mine_plan(session: &InteractiveRebaseSession) -> PresetResult {
    let author_email = configured_author_email(session)?;
    let picked = session
        .plan
        .iter()
        .filter(|row| emails_match(&row.commit.author_email, &author_email))
        .count();
    if picked == 0 {
        return Err(no_matching_commits_message(session, &author_email));
    }

    let dropped = session.plan.len().saturating_sub(picked);
    let mut picked_rows = Vec::with_capacity(picked);
    let mut dropped_rows = Vec::with_capacity(dropped);
    for mut row in session.plan.iter().cloned() {
        if emails_match(&row.commit.author_email, &author_email) {
            row.action = RebaseAction::Pick;
            picked_rows.push(row);
        } else {
            row.action = RebaseAction::Drop;
            dropped_rows.push(row);
        }
    }
    picked_rows.extend(dropped_rows);
    Ok((
        picked_rows,
        HashMap::new(),
        format!("Kept {picked} authored commits and marked {dropped} for drop"),
    ))
}

fn squash_mine_plan(session: &InteractiveRebaseSession) -> PresetResult {
    let author_email = configured_author_email(session)?;
    let first_mine = session
        .plan
        .iter()
        .position(|row| emails_match(&row.commit.author_email, &author_email))
        .ok_or_else(|| no_matching_commits_message(session, &author_email))?;

    let mut before = Vec::new();
    let mut mine = Vec::new();
    let mut after = Vec::new();
    for (index, mut row) in session.plan.iter().cloned().enumerate() {
        if emails_match(&row.commit.author_email, &author_email) {
            row.action = RebaseAction::Pick;
            mine.push(row);
        } else if index < first_mine {
            row.action = RebaseAction::Pick;
            before.push(row);
        } else {
            row.action = RebaseAction::Pick;
            after.push(row);
        }
    }

    let mine_count = mine.len();
    let mut drafts = HashMap::new();
    if mine_count > 1 {
        if let Some(first) = mine.first_mut() {
            first.action = RebaseAction::Reword;
            drafts.insert(first.commit.id.clone(), first.commit.summary.clone());
        }
        for row in mine.iter_mut().skip(1) {
            row.action = RebaseAction::Fixup;
        }
    }

    let mut plan = Vec::with_capacity(session.plan.len());
    plan.extend(before);
    plan.extend(mine);
    plan.extend(after);

    let status = if mine_count == 1 {
        "Only one authored commit; kept it unchanged".into()
    } else {
        format!("Grouped and squashed {mine_count} authored commits")
    };
    Ok((plan, drafts, status))
}

fn squash_all_plan(
    session: &InteractiveRebaseSession,
) -> (Vec<RebasePlanRow>, HashMap<String, String>, String) {
    let mut plan = session.plan.clone();
    let mut drafts = HashMap::new();
    let count = plan.len();
    if count > 1 {
        if let Some(first) = plan.first_mut() {
            first.action = RebaseAction::Reword;
            drafts.insert(first.commit.id.clone(), first.commit.summary.clone());
        }
        for row in plan.iter_mut().skip(1) {
            row.action = RebaseAction::Fixup;
        }
    } else if let Some(first) = plan.first_mut() {
        first.action = RebaseAction::Pick;
    }

    let status = if count <= 1 {
        "Only one commit; kept it unchanged".into()
    } else {
        format!("Squashing {count} commits into one")
    };
    (plan, drafts, status)
}

fn configured_author_email(session: &InteractiveRebaseSession) -> Result<String, String> {
    session
        .current_author_email
        .as_deref()
        .and_then(normalized_email)
        .ok_or_else(|| "git user.email is not configured for this repository".into())
}

fn no_matching_commits_message(session: &InteractiveRebaseSession, author_email: &str) -> String {
    format!(
        "No commits match configured author email {}",
        session
            .current_author_email
            .as_deref()
            .unwrap_or(author_email)
    )
}

fn can_apply(session: &InteractiveRebaseSession) -> bool {
    !session.plan.is_empty()
        && !session.drag.as_ref().is_some_and(|drag| drag.started)
        && !session.applying
        && !matches!(
            session.plan.first().map(|row| row.action),
            Some(RebaseAction::Squash | RebaseAction::Fixup)
        )
        && session
            .plan
            .iter()
            .filter(|row| row.action == RebaseAction::Reword)
            .all(|row| {
                session
                    .reword_drafts
                    .get(&row.commit.id)
                    .is_some_and(|message| !message.trim().is_empty())
            })
}

fn plan_entries(plan: &[RebasePlanRow]) -> Vec<RebasePlanEntry> {
    plan.iter()
        .map(|row| RebasePlanEntry {
            action: row.action,
            commit_id: row.commit.id.clone(),
            summary: row.commit.summary.clone(),
            author_name: row.commit.author_name.clone(),
            author_email: row.commit.author_email.clone(),
        })
        .collect()
}

fn reword_messages(session: &InteractiveRebaseSession) -> Vec<(String, String)> {
    session
        .plan
        .iter()
        .filter(|row| row.action == RebaseAction::Reword)
        .filter_map(|row| {
            session
                .reword_drafts
                .get(&row.commit.id)
                .map(|message| (row.commit.id.clone(), message.clone()))
        })
        .collect()
}

fn plan_counts(session: &InteractiveRebaseSession) -> String {
    let mut pick = 0;
    let mut drop = 0;
    let mut reword = 0;
    let mut squash = 0;
    let mut fixup = 0;
    let mut edit = 0;
    for row in &session.plan {
        match row.action {
            RebaseAction::Pick => pick += 1,
            RebaseAction::Reword => reword += 1,
            RebaseAction::Drop => drop += 1,
            RebaseAction::Squash => squash += 1,
            RebaseAction::Fixup => fixup += 1,
            RebaseAction::Edit => edit += 1,
        }
    }
    format!(
        "{pick} picks, {drop} drops, {reword} rewords, {squash} squashes, {fixup} fixups, {edit} edits"
    )
}

const REBASE_PROMPT_SUMMARY_MAX_CHARS: usize = 96;

fn rebase_prompt_preview(session: &InteractiveRebaseSession) -> Vec<RebasePromptRow> {
    session
        .plan
        .iter()
        .map(|row| RebasePromptRow {
            action: row.action,
            short_id: short_id(&row.commit.id),
            summary: compact_end(&row.commit.summary, REBASE_PROMPT_SUMMARY_MAX_CHARS),
            author_name: row.commit.author_name.clone(),
            author_avatar_url: row.author_avatar_url.clone(),
        })
        .collect()
}

fn compact_end(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let keep = max_chars - 3;
    let head = value.chars().take(keep).collect::<String>();
    format!("{head}...")
}

fn short_id(id: &str) -> String {
    id.chars().take(7).collect()
}

fn normalized_email(email: &str) -> Option<String> {
    let email = email.trim();
    (!email.is_empty()).then(|| email.to_ascii_lowercase())
}

fn emails_match(commit_email: &str, configured_email: &str) -> bool {
    normalized_email(commit_email).as_deref() == Some(configured_email)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use naite_core::{HistoryCommit, RefKind, RefSummary};

    use super::*;

    #[test]
    fn rebase_prompt_preview_compacts_long_summary() {
        let long_summary = "a".repeat(120);
        let session = session_with_rows(vec![plan_row(
            "abcdef1234567890",
            RebaseAction::Drop,
            &long_summary,
        )]);

        let rows = rebase_prompt_preview(&session);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].summary.chars().count(),
            REBASE_PROMPT_SUMMARY_MAX_CHARS
        );
        assert!(rows[0].summary.ends_with("..."));
    }

    #[test]
    fn rebase_prompt_preview_returns_all_rows() {
        let rows = (0..10)
            .map(|index| {
                plan_row(
                    &format!("{index:07}abcdef"),
                    RebaseAction::Pick,
                    &format!("commit {index}"),
                )
            })
            .collect();
        let session = session_with_rows(rows);

        let preview_rows = rebase_prompt_preview(&session);

        assert_eq!(preview_rows.len(), 10);
    }

    #[test]
    fn rebase_prompt_preview_preserves_action_and_short_sha() {
        let session = session_with_rows(vec![plan_row(
            "1234567890abcdef",
            RebaseAction::Reword,
            "rewrite commit message",
        )]);

        let rows = rebase_prompt_preview(&session);

        assert_eq!(rows[0].action, RebaseAction::Reword);
        assert_eq!(rows[0].short_id, "1234567");
        assert_eq!(rows[0].summary, "rewrite commit message");
    }

    fn session_with_rows(plan: Vec<RebasePlanRow>) -> InteractiveRebaseSession {
        InteractiveRebaseSession {
            current_branch: ref_summary("feature"),
            target: ref_summary("main"),
            current_author_email: None,
            plan,
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: false,
            scroll_offset: 0.0,
        }
    }

    fn plan_row(id: &str, action: RebaseAction, summary: &str) -> RebasePlanRow {
        RebasePlanRow {
            action,
            commit: HistoryCommit {
                id: id.into(),
                summary: summary.into(),
                author_name: "june".into(),
                author_email: "june@example.com".into(),
            },
            author_avatar_url: None,
        }
    }

    fn ref_summary(name: &str) -> RefSummary {
        RefSummary {
            kind: RefKind::LocalBranch,
            short_name: name.into(),
            full_name: format!("refs/heads/{name}"),
            target_short_id: "abc1234".into(),
            is_head: false,
            sync_status: None,
        }
    }
}
