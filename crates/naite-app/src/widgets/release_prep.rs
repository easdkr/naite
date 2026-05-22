use iced::widget::{button, checkbox, column, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};
use naite_core::{ReleaseBranchSync, ReleaseProfile};

use crate::features::release_prep::{self, ReleasePrepAction};
use crate::state::ReleasePrepState;
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

const PROGRESS_TRACK_WIDTH: f32 = 320.0;
const PROGRESS_SEGMENT_WIDTH: f32 = 92.0;

pub fn release_prep_config(state: &ReleasePrepState, loading: bool) -> Element<'_, Message> {
    let can_submit = !loading
        && !state.remote.trim().is_empty()
        && !state.source_branch.trim().is_empty()
        && !state.target_branch.trim().is_empty()
        && state.source_branch.trim() != state.target_branch.trim();

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
                progress_line("Fetching remote refs"),
                progress_line("Force syncing source and target branches"),
                progress_line("Building the rebase plan"),
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

    body.push(
        column![
            section_label("Guided mode"),
            action_row(
                "Update target from source",
                ReleasePrepAction::UpdateTargetFromSource,
                state,
                actions_locked,
            ),
            action_row(
                "Push target",
                ReleasePrepAction::PushTarget,
                state,
                actions_locked,
            ),
            action_row(
                "Rebase source onto target",
                ReleasePrepAction::SyncSourceFromTarget,
                state,
                actions_locked,
            ),
        ]
        .spacing(theme::SP_SM),
    )
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
    [
        ReleasePrepAction::UpdateTargetFromSource,
        ReleasePrepAction::PushTarget,
        ReleasePrepAction::SyncSourceFromTarget,
    ]
    .into_iter()
    .all(|action| state.completed_actions.contains(&action))
}

fn progress_line<'a>(label: &'a str) -> Element<'a, Message> {
    row![
        text("-")
            .size(theme::FS_SM)
            .font(iced::Font::MONOSPACE)
            .color(color::TEXT_SUBTLE),
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into()
}

fn spinner_frame(frame: usize) -> &'static str {
    ["|", "/", "-", "\\"][frame % 4]
}

fn animated_dots(frame: usize) -> &'static str {
    match (frame / 4) % 4 {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

fn moving_progress_bar(frame: usize) -> Element<'static, Message> {
    let cycle = (frame % 32) as f32 / 31.0;
    let lead_width = ease_in_out_sine(cycle) * (PROGRESS_TRACK_WIDTH - PROGRESS_SEGMENT_WIDTH);

    container(
        row![
            Space::with_width(Length::Fixed(lead_width)),
            container(Space::new(
                Length::Fixed(PROGRESS_SEGMENT_WIDTH),
                Length::Fixed(3.0)
            ))
            .style(progress_segment_style(frame)),
            Space::with_width(Length::Fill),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fixed(PROGRESS_TRACK_WIDTH))
    .height(Length::Fixed(3.0))
    .style(progress_track_style)
    .into()
}

fn progress_track_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::BORDER, 0.55))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

fn progress_segment_style(frame: usize) -> impl Fn(&Theme) -> container::Style {
    let pulse = 0.65
        + 0.25
            * (((frame % 16) as f32 / 15.0) * std::f32::consts::TAU)
                .sin()
                .abs();
    move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::ACCENT, pulse))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

fn ease_in_out_sine(progress: f32) -> f32 {
    -(std::f32::consts::PI * progress).cos() / 2.0 + 0.5
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
    if error.contains("cannot lock ref") {
        return "Remote branch data was updated by another Git operation. Retry release promotion."
            .into();
    }
    if error.starts_with("git command failed: git fetch") {
        return "Could not fetch the latest remote branches. Check repository access or network state, then retry release promotion.".into();
    }
    if error.starts_with("git command failed:") {
        return "Git could not complete this release step. Check the branch and worktree state, then retry release promotion.".into();
    }
    error.to_string()
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
}
