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
        body = body.push(moving_progress_bar(state.animation_frame)).push(
            text(format!(
                "Running release action{}",
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

    let action = |label: &'static str, action: ReleasePrepAction| {
        button(text(label).size(theme::FS_SM))
            .padding(Padding::from([5, 10]))
            .style(styles::primary_button)
            .on_press_maybe((!loading).then_some(Message::from(
                release_prep::Message::ActionRequested(action),
            )))
    };

    body.push(
        column![
            action(
                "Update target from source",
                ReleasePrepAction::UpdateTargetFromSource
            ),
            action("Push target", ReleasePrepAction::PushTarget),
            action(
                "Sync source from target",
                ReleasePrepAction::SyncSourceFromTarget
            ),
            button(text("Close").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(release_prep::Message::Cancelled)),
        ]
        .spacing(theme::SP_SM),
    )
    .into()
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
    container(
        text(format!(
            "{error}\n\nRelease promotion could not complete. Check the branch names, worktree state, or Git operation state and retry."
        ))
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .color(color::DANGER),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn profile_label(profile: &ReleaseProfile) -> String {
    format!(
        "{} / {} -> {}",
        profile.remote, profile.source_branch, profile.target_branch
    )
}
