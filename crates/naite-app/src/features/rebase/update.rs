use std::collections::HashMap;

use iced::Task;
use naite_core::{RebaseAction, RebasePlanEntry, RefKind};

use crate::features::rebase::{
    self, task::ApplyOutcome, DragState, InteractiveRebaseSession, Message as RebaseMessage,
    RebaseApplyMode, RebasePlanPreset,
};
use crate::features::{release_prep, repo_open};
use crate::state::ReleasePrepPhase;
use crate::tasks;
use crate::{App, Message, RebasePrompt};

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
                self.load_selected_rebase_diff()
            }
            RebaseMessage::LoadFailed(message) => {
                self.operation.loading = false;
                self.operation.error = Some(message);
                Task::none()
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
            self.operation.error = Some("choose a non-HEAD branch as the rebase target".into());
            return Task::none();
        }
        if self.repo.operation_state.is_busy() {
            self.operation.error = Some("another Git operation is already in progress".into());
            return Task::none();
        }
        if self.repo.status_detail.is_dirty() {
            self.operation.error = Some("worktree has local changes".into());
            return Task::none();
        }
        let Some(current_branch) = self
            .repo
            .refs
            .local
            .iter()
            .find(|branch| branch.is_head)
            .cloned()
        else {
            self.operation.error = Some("current HEAD is detached".into());
            return Task::none();
        };

        self.operation.error = None;
        self.operation.loading = true;
        let target_ref = target.full_name.clone();
        let target_for_message = target.clone();
        Task::perform(
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
        )
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
                self.operation.error = Some(message);
                return Task::none();
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
                    self.operation.error = Some(
                        "release promotion applies the rebase locally; push the target, then sync the source"
                            .into(),
                    );
                    return Task::none();
                }
                if let Err(message) = self.force_push_prompt_for_current_branch() {
                    self.operation.error = Some(message);
                    return Task::none();
                }
            }
            RebaseApplyMode::ReleasePromotionAuto => {
                if self.release_prep.active_profile.is_none() {
                    self.operation.error = Some("Plan a release promotion first".into());
                    return Task::none();
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
            todo_preview: format_todo_preview(session),
            apply_mode,
        });
        Task::none()
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

        Task::perform(
            rebase::task::apply_plan(path, target_ref, entries, reword_messages),
            move |result| Message::from(RebaseMessage::Done { result, apply_mode }),
        )
    }

    fn finish_rebase_operation(
        &mut self,
        result: Result<ApplyOutcome, String>,
        apply_mode: RebaseApplyMode,
    ) -> Task<Message> {
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
                    Task::perform(repo_open::task::load(path), |result| {
                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                    })
                } else {
                    self.set_transient_status("Interactive rebase applied".into());
                    Task::none()
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
                    Task::perform(repo_open::task::load(path), |result| {
                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                    })
                } else {
                    self.set_transient_status("Interactive rebase paused on conflicts".into());
                    self.operation.error = Some(message);
                    Task::none()
                }
            }
            Err(message) => {
                self.rebase = None;
                self.selection.rebase_confirmation = None;
                self.operation.pending_force_push_after_reload = false;
                self.operation.error = Some(message);
                Task::none()
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

fn format_todo_preview(session: &InteractiveRebaseSession) -> String {
    session
        .plan
        .iter()
        .map(|row| {
            format!(
                "{} {} {}",
                action_token(row.action),
                short_id(&row.commit.id),
                row.commit.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn action_token(action: RebaseAction) -> &'static str {
    match action {
        RebaseAction::Pick => "pick",
        RebaseAction::Reword => "reword",
        RebaseAction::Edit => "edit",
        RebaseAction::Squash => "squash",
        RebaseAction::Fixup => "fixup",
        RebaseAction::Drop => "drop",
    }
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
