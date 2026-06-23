use iced::widget::{button, checkbox, column, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Color, Element, Length, Padding};
use naite_core::{ReleaseBranchSync, ReleaseProfile};

use crate::features::release_prep::{self, ReleasePrepAction};
use crate::state::{ReleasePrepState, ReleasePrepStep};
use crate::styles;
use crate::theme::{self, color};
use crate::widgets::common::{animated_dots, moving_progress_bar, spinner_frame};
use crate::Message;

pub fn release_prep_config(state: &ReleasePrepState, loading: bool) -> Element<'_, Message> {
    let can_submit = !loading
        && !release_error_blocks_submit(state.error.as_deref())
        && !state.remote.trim().is_empty()
        && !state.source_branch.trim().is_empty()
        && !state.target_branch.trim().is_empty()
        && state.source_branch.trim() != state.target_branch.trim()
        && !state.validation_script.contains(['\t', '\n', '\r']);

    let hint = state
        .suggestion
        .as_ref()
        .map(|suggestion| {
            let sources = suggestion.source_candidates.join(", ");
            let targets = suggestion.target_candidates.join(", ");
            format!("Suggested source: {sources}; target: {targets}")
        })
        .unwrap_or_else(|| "Choose the release source and target branches.".into());

    let mut body = column![
        text("Release Promotion")
            .size(theme::FS_LG)
            .font(theme::font_semibold())
            .color(color::TEXT),
        text(hint)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    ]
    .spacing(theme::SP_MD);

    if let Some(error) = &state.error {
        body = body.push(release_error(error));
    }

    body.push(selection_field(
        "Remote",
        &state.remote,
        state
            .suggestion
            .as_ref()
            .map(|suggestion| suggestion.remotes.as_slice())
            .unwrap_or(&[]),
        release_prep::Message::RemoteChanged,
    ))
    .push(selection_field(
        "Source branch",
        &state.source_branch,
        state
            .suggestion
            .as_ref()
            .map(|suggestion| suggestion.source_candidates.as_slice())
            .unwrap_or(&[]),
        release_prep::Message::SourceBranchChanged,
    ))
    .push(selection_field(
        "Target branch",
        &state.target_branch,
        state
            .suggestion
            .as_ref()
            .map(|suggestion| suggestion.target_candidates.as_slice())
            .unwrap_or(&[]),
        release_prep::Message::TargetBranchChanged,
    ))
    .push(field(
        "Validation script (optional)",
        &state.validation_script,
        release_prep::Message::ValidationScriptChanged,
    ))
    .push(
        text(
            "Runs via `sh -c` from the repo root before pushing the target; \
             non-zero exit blocks the push. \
             Env: NAITE_REMOTE, NAITE_SOURCE_BRANCH, NAITE_TARGET_BRANCH.",
        )
        .size(theme::FS_XS)
        .font(theme::font_regular())
        .color(color::TEXT_MUTED),
    )
    .push(
        checkbox(
            "Create backup branch before rebase",
            state.backup_before_rebase,
        )
        .on_toggle(|value| Message::from(release_prep::Message::BackupToggled(value)))
        .size(theme::FS_SM)
        .text_size(theme::FS_SM),
    )
    .push(
        row![
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(release_prep::Message::Cancelled)),
            button(text("Fetch and open rebase").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    can_submit.then_some(Message::from(release_prep::Message::ProfileSubmitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .into()
}

pub fn release_prep_progress(state: &ReleasePrepState) -> Element<'_, Message> {
    let profile = state.profile_from_inputs();
    let spinner = spinner_frame(state.animation_frame);
    column![
        row![
            text(spinner)
                .size(theme::FS_LG)
                .font(iced::Font::MONOSPACE)
                .color(color::ACCENT),
            text("Release Promotion")
                .size(theme::FS_LG)
                .font(theme::font_semibold())
                .color(color::TEXT),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        text(profile_label(&profile))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
        moving_progress_bar(state.animation_frame),
        container(
            column![
                step_progress_row(ReleasePrepStep::FetchingRemote, state),
                step_progress_row(ReleasePrepStep::SyncingBranches, state),
                step_progress_row(ReleasePrepStep::CheckingSync, state),
                step_progress_row(ReleasePrepStep::CheckingOutSource, state),
                step_progress_row(ReleasePrepStep::CreatingBackup, state),
                step_progress_row(ReleasePrepStep::BuildingPlan, state),
            ]
            .spacing(theme::SP_SM),
        )
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::inset_card),
        text(format!(
            "Preparing release rebase{}",
            animated_dots(state.animation_frame)
        ))
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .color(color::TEXT_MUTED),
    ]
    .spacing(theme::SP_MD)
    .into()
}

pub fn release_prep_actions(state: &ReleasePrepState, loading: bool) -> Element<'_, Message> {
    let Some(profile) = state.active_profile.as_ref() else {
        return Space::new(Length::Shrink, Length::Shrink).into();
    };

    let mut body = column![
        row![
            text(if loading {
                spinner_frame(state.animation_frame)
            } else {
                " "
            })
            .size(theme::FS_LG)
            .font(iced::Font::MONOSPACE)
            .color(if loading {
                color::ACCENT
            } else {
                Color::TRANSPARENT
            }),
            text("Release promotion")
                .size(theme::FS_LG)
                .font(theme::font_semibold())
                .color(color::TEXT),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        text(profile_label(profile))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    ]
    .spacing(theme::SP_MD);

    if loading {
        let running_label = state
            .active_action
            .map(|action| format!("Running: {}", action.label()))
            .unwrap_or_else(|| "Running release action".to_string());
        body = body.push(moving_progress_bar(state.animation_frame)).push(
            text(format!(
                "{}{}",
                running_label,
                animated_dots(state.animation_frame)
            ))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_MUTED),
        );
    }

    if let Some(sync) = &state.sync_check {
        body = body
            .push(sync_row("Source", &sync.source))
            .push(sync_row("Target", &sync.target));
    }

    let has_script = profile.has_validation_script();
    let validate_complete = state
        .completed_actions
        .contains(&ReleasePrepAction::ValidateTarget);
    let auto_running = state.auto_running;
    let auto_complete = release_prep_complete(state);
    let auto_enabled =
        !loading && state.active_profile.is_some() && !auto_running && !auto_complete;
    let actions_locked = loading || auto_running;
    let auto_label_text = if auto_running {
        if let Some(active) = state.active_action {
            format!("Auto promotion running: {}", active.label())
        } else if let Some(next) = state.auto_next_action {
            format!("Auto promotion — next: {}", next.label())
        } else {
            "Auto promotion finishing…".to_string()
        }
    } else if auto_complete {
        "Auto promotion complete".to_string()
    } else {
        "Run remaining steps automatically".to_string()
    };
    let auto_button_label = if auto_running {
        "Running…"
    } else if auto_complete {
        "Complete"
    } else {
        "Run all"
    };

    let mut guided = column![
        section_label("Guided mode"),
        action_row(
            "Update target from source",
            ReleasePrepAction::UpdateTargetFromSource,
            state,
            actions_locked,
        ),
    ]
    .spacing(theme::SP_SM);
    guided = guided.push(validation_script_row(state, actions_locked));
    // The push gate: with a validation script configured, the target cannot
    // be pushed until the script has passed.
    let push_locked = actions_locked || (has_script && !validate_complete);
    guided = guided
        .push(action_row(
            "Push target",
            ReleasePrepAction::PushTarget,
            state,
            push_locked,
        ))
        .push(action_row(
            "Rebase source onto target",
            ReleasePrepAction::SyncSourceFromTarget,
            state,
            actions_locked,
        ));

    body.push(guided)
        .push(
            column![
                section_label("Auto mode"),
                container(
                    row![
                        text(auto_label_text)
                            .size(theme::FS_SM)
                            .font(theme::font_regular())
                            .color(if auto_running {
                                color::ACCENT
                            } else {
                                color::TEXT
                            }),
                        Space::with_width(Length::Fill),
                        button(text(auto_button_label).size(theme::FS_SM))
                            .padding(Padding::from([5, 10]))
                            .style(styles::danger_button)
                            .on_press_maybe(
                                auto_enabled
                                    .then_some(Message::from(release_prep::Message::AutoRequested))
                            ),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([6, 8]))
                .style(styles::inset_card),
            ]
            .spacing(theme::SP_SM),
        )
        .push(
            row![
                Space::with_width(Length::Fill),
                button(text("Close").size(theme::FS_SM))
                    .padding(Padding::from([5, 10]))
                    .style(styles::subtle_button)
                    .on_press_maybe(
                        (!auto_running).then_some(Message::from(release_prep::Message::Cancelled))
                    ),
            ]
            .align_y(Alignment::Center),
        )
        .into()
}

fn section_label(label: &str) -> Element<'_, Message> {
    text(label)
        .size(theme::FS_XS)
        .font(theme::font_semibold())
        .color(color::TEXT_SUBTLE)
        .into()
}

/// The validation step row doubles as the editor for the script, so a
/// planned promotion can gain (or drop) validation directly from the
/// actions modal. Edits apply to the active profile immediately and are
/// persisted when the script runs or the modal closes.
fn validation_script_row(state: &ReleasePrepState, locked: bool) -> Element<'_, Message> {
    let action = ReleasePrepAction::ValidateTarget;
    let is_active = state.active_action == Some(action);
    let is_completed = state.completed_actions.contains(&action);
    let has_script = !state.validation_script.trim().is_empty();
    let frame = state.animation_frame;

    let (status_glyph, status_color) = if is_active {
        (spinner_frame(frame), color::ACCENT)
    } else if is_completed {
        ("✓", color::SUCCESS)
    } else if has_script {
        ("•", color::TEXT_SUBTLE)
    } else {
        ("•", color::TEXT_MUTED)
    };

    let mut input = text_input(
        "Validation script (optional, runs via sh -c before push)",
        &state.validation_script,
    )
    .padding(Padding::from([4, 8]))
    .size(theme::FS_SM)
    .style(styles::form_text_input)
    .width(Length::Fill);
    if !locked {
        input = input
            .on_input(|value| Message::from(release_prep::Message::ValidationScriptChanged(value)));
    }

    let button_label = if is_completed { "Done" } else { "Run" };
    let button_locked = locked || is_completed || !has_script;

    container(
        row![
            text(status_glyph)
                .size(theme::FS_SM)
                .font(iced::Font::MONOSPACE)
                .color(status_color),
            input,
            button(text(button_label).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe((!button_locked).then_some(Message::from(
                    release_prep::Message::ActionRequested(action),
                ))),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([6, 8]))
    .style(styles::inset_card)
    .into()
}

fn action_row<'a>(
    label: &'a str,
    action: ReleasePrepAction,
    state: &'a ReleasePrepState,
    locked: bool,
) -> Element<'a, Message> {
    let is_active = state.active_action == Some(action);
    let is_completed = state.completed_actions.contains(&action);
    let frame = state.animation_frame;

    let (status_glyph, status_color) = if is_active {
        (spinner_frame(frame).to_string(), color::ACCENT)
    } else if is_completed {
        ("✓".to_string(), color::SUCCESS)
    } else {
        ("•".to_string(), color::TEXT_SUBTLE)
    };

    let label_text = if is_active {
        format!("{label} — running{}", animated_dots(frame))
    } else if is_completed {
        format!("{label} — done")
    } else {
        label.to_string()
    };

    let label_color = if is_active {
        color::ACCENT
    } else if is_completed {
        color::TEXT_SUBTLE
    } else {
        color::TEXT
    };

    let button_label = if is_completed { "Done" } else { "Run" };
    let button_locked = locked || is_completed;

    container(
        row![
            text(status_glyph)
                .size(theme::FS_SM)
                .font(iced::Font::MONOSPACE)
                .color(status_color),
            text(label_text)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(label_color),
            Space::with_width(Length::Fill),
            button(text(button_label).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe((!button_locked).then_some(Message::from(
                    release_prep::Message::ActionRequested(action),
                ))),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([6, 8]))
    .style(styles::inset_card)
    .into()
}

fn release_prep_complete(state: &ReleasePrepState) -> bool {
    let has_script = state
        .active_profile
        .as_ref()
        .is_some_and(ReleaseProfile::has_validation_script);
    crate::features::release_prep::update::release_prep_actions_for(has_script)
        .iter()
        .all(|action| state.completed_actions.contains(action))
}

/// Per-step progress row mirroring the `action_row` glyph convention:
/// spinner+ACCENT while running, ✓+SUCCESS once done, •+TEXT_SUBTLE pending.
fn step_progress_row<'a>(
    step: ReleasePrepStep,
    state: &'a ReleasePrepState,
) -> Element<'a, Message> {
    let is_active = state.preparing_step == Some(step);
    let is_completed = state.completed_preparing_steps.contains(&step);
    let frame = state.animation_frame;

    let (status_glyph, status_color) = if is_active {
        (spinner_frame(frame), color::ACCENT)
    } else if is_completed {
        ("✓", color::SUCCESS)
    } else {
        ("•", color::TEXT_SUBTLE)
    };

    let label_text = if is_active {
        format!("{} — running{}", step.label(), animated_dots(frame))
    } else if is_completed {
        format!("{} — done", step.label())
    } else {
        step.label().to_string()
    };

    let label_color = if is_active {
        color::ACCENT
    } else if is_completed {
        color::TEXT_SUBTLE
    } else {
        color::TEXT
    };

    row![
        text(status_glyph)
            .size(theme::FS_SM)
            .font(iced::Font::MONOSPACE)
            .color(status_color),
        text(label_text)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(label_color),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into()
}

fn field<'a>(
    label: &'a str,
    value: &'a str,
    on_input: fn(String) -> release_prep::Message,
) -> Element<'a, Message> {
    column![
        text(label)
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
        text_input(label, value)
            .on_input(move |value| Message::from(on_input(value)))
            .padding(Padding::from([6, 10]))
            .size(theme::FS_SM),
    ]
    .spacing(4)
    .into()
}

fn selection_field<'a>(
    label: &'a str,
    value: &'a str,
    candidates: &'a [String],
    on_select: fn(String) -> release_prep::Message,
) -> Element<'a, Message> {
    let options = selection_options(value, candidates);
    if options.is_empty() {
        return field(label, value, on_select);
    }

    column![
        text(label)
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
        pick_list(options, non_empty_value(value), move |value| {
            Message::from(on_select(value))
        })
        .placeholder(label)
        .padding(Padding::from([6, 10]))
        .text_size(theme::FS_SM)
        .style(styles::release_prep_pick_list)
        .menu_style(styles::release_prep_pick_list_menu)
        .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}

fn selection_options(value: &str, candidates: &[String]) -> Vec<String> {
    let current = value.trim();
    let mut options = Vec::new();
    for candidate in candidates.iter().map(|candidate| candidate.trim()) {
        if !candidate.is_empty() && !options.iter().any(|option| option == candidate) {
            options.push(candidate.to_string());
        }
    }

    if !current.is_empty() && !options.iter().any(|candidate| candidate == current) {
        options.insert(0, current.to_string());
    }
    options
}

fn non_empty_value(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn sync_row<'a>(label: &'a str, branch: &'a ReleaseBranchSync) -> Element<'a, Message> {
    let status = if branch.is_ready() {
        "synced".to_string()
    } else if branch.local_oid.is_none() {
        "missing local branch".to_string()
    } else if branch.remote_oid.is_none() {
        "missing remote branch".to_string()
    } else {
        format!("ahead {}, behind {}", branch.ahead, branch.behind)
    };
    container(
        row![
            text(label)
                .size(theme::FS_SM)
                .font(theme::font_semibold())
                .color(color::TEXT),
            Space::with_width(Length::Fill),
            text(format!("{}: {status}", branch.branch))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(if branch.is_ready() {
                    color::SUCCESS
                } else {
                    color::DANGER
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([6, 8]))
    .style(styles::inset_card)
    .into()
}

fn release_error<'a>(error: &'a str) -> Element<'a, Message> {
    let message = release_error_message(error);
    container(
        text(message)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::DANGER),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn release_error_message(error: &str) -> String {
    if crate::error_display::is_git_index_lock_error(error) {
        return crate::error_display::GIT_INDEX_LOCK_MESSAGE.into();
    }
    if error.contains("cannot lock ref") {
        return "Remote branch data was updated by another Git operation. Retry release promotion."
            .into();
    }
    if error.starts_with("git command failed: git fetch") {
        return "Could not fetch the latest remote branches. Check repository access or network state, then retry release promotion.".into();
    }
    if error.starts_with("git command failed:") {
        return format!(
            "Git could not complete this release step. Resolve the Git error below before retrying.\n\n{error}"
        );
    }
    error.to_string()
}

fn release_error_blocks_submit(error: Option<&str>) -> bool {
    matches!(
        error,
        Some(crate::features::release_prep::update::DIRTY_WORKTREE_RELEASE_ERROR)
            | Some("worktree has local changes")
    )
}

fn profile_label(profile: &ReleaseProfile) -> String {
    format!(
        "{} / {} -> {}",
        profile.remote, profile.source_branch, profile.target_branch
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_error_message_summarizes_fetch_ref_races() {
        let raw = "git command failed: git fetch origin: error: cannot lock ref 'refs/remotes/origin/staging': is at 414a6de but expected 07343dd";

        assert_eq!(
            release_error_message(raw),
            "Remote branch data was updated by another Git operation. Retry release promotion."
        );
    }

    #[test]
    fn release_error_message_summarizes_index_lock_errors() {
        let raw = "git command failed: git status --porcelain=v1: fatal: Unable to create '/repo/.git/index.lock': File exists";

        assert_eq!(
            release_error_message(raw),
            crate::error_display::GIT_INDEX_LOCK_MESSAGE
        );
    }

    #[test]
    fn release_error_message_preserves_non_fetch_git_details() {
        let raw = "git command failed: git checkout staging: fatal: 'staging' is already checked out at '/tmp/staging-worktree'";

        assert_eq!(
            release_error_message(raw),
            "Git could not complete this release step. Resolve the Git error below before retrying.\n\n\
             git command failed: git checkout staging: fatal: 'staging' is already checked out at '/tmp/staging-worktree'"
        );
    }

    #[test]
    fn release_error_blocks_submit_for_dirty_worktree_only() {
        assert!(release_error_blocks_submit(Some(
            crate::features::release_prep::update::DIRTY_WORKTREE_RELEASE_ERROR
        )));
        assert!(release_error_blocks_submit(Some(
            "worktree has local changes"
        )));
        assert!(!release_error_blocks_submit(Some(
            "git command failed: git checkout staging"
        )));
        assert!(!release_error_blocks_submit(None));
    }
}
