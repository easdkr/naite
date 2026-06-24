//! Shared primitives used across more than one widget submodule.

use std::borrow::Cow;

use iced::widget::{button, column, container, row, text, text::Wrapping, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};
use naite_core::WorktreeStatusDetail;

use crate::icons::{self, IconName};
use crate::state::OpSeverity;
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

pub(super) fn section_header<'a>(label: &'a str, icon: IconName) -> Element<'a, Message> {
    container(
        row![
            icons::icon(icon, 13, color::TEXT_SUBTLE),
            text(label.to_uppercase())
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([theme::SP_SM, theme::SP_LG]))
    .into()
}

pub struct ErrorRecovery<'a> {
    pub label: &'a str,
    pub message: Message,
}

pub(super) fn error_card<'a>(
    err: &'a str,
    severity: OpSeverity,
    recovery: Option<ErrorRecovery<'a>>,
) -> Element<'a, Message> {
    let display_error: Cow<'a, str> = crate::error_display::format_git_error_for_display(err);
    match severity {
        OpSeverity::Fatal => fatal_error_card(display_error, recovery),
        OpSeverity::Recoverable => recoverable_error_card(display_error),
    }
}

/// Full blocking card for Fatal errors (with optional recovery action).
fn fatal_error_card<'a>(
    display_error: Cow<'a, str>,
    recovery: Option<ErrorRecovery<'a>>,
) -> Element<'a, Message> {
    let mut actions = row![].align_y(Alignment::Center).spacing(theme::SP_SM);
    if let Some(recovery) = recovery {
        actions = actions.push(
            button(
                text(recovery.label)
                    .size(theme::FS_SM)
                    .wrapping(Wrapping::None),
            )
            .padding(Padding::from([4, 8]))
            .style(styles::danger_button)
            .on_press(recovery.message),
        );
    }
    actions = actions.push(
        button(text("Dismiss").size(theme::FS_SM).wrapping(Wrapping::None))
            .padding(Padding::from([4, 8]))
            .style(styles::subtle_button)
            .on_press(Message::ClearError),
    );

    container(
        row![
            container(
                text(format!("Error: {display_error}"))
                    .size(theme::FS_BASE)
                    .font(theme::font_regular())
                    .color(color::DANGER),
            )
            .width(Length::Fill),
            actions,
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_LG)
    .style(styles::error_card)
    .into()
}

/// Compact pill for Recoverable errors (mirrors `status_bar::error_pill`).
fn recoverable_error_card<'a>(display_error: Cow<'a, str>) -> Element<'a, Message> {
    container(
        row![
            text("✗")
                .size(theme::FS_SM)
                .font(theme::font_semibold())
                .color(color::DANGER),
            text(display_error)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT),
            Space::with_width(Length::Fill),
            button(text("Dismiss").size(theme::FS_SM).wrapping(Wrapping::None))
                .padding(Padding::from([2, theme::SP_SM]))
                .style(styles::subtle_button)
                .on_press(Message::ClearError),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([2, theme::SP_MD]))
    .style(styles::error_pill)
    .into()
}

pub(super) fn empty_state<'a>() -> Element<'a, Message> {
    container(
        column![
            text("naite")
                .size(theme::FS_XL)
                .font(theme::font_semibold())
                .color(color::TEXT_MUTED),
            Space::with_height(theme::SP_SM),
            text("Open a repository to begin.")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_SUBTLE),
        ]
        .align_x(Alignment::Center)
        .spacing(2),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

pub(super) fn uses_ui_font_fallback(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x11ff
            | 0x3130..=0x318f
            | 0xac00..=0xd7af
            | 0x3040..=0x30ff
            | 0x3400..=0x9fff
    )
}

pub(super) fn empty_filter_state<'a>() -> Element<'a, Message> {
    container(
        text("No commits match this filter.")
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

pub(super) fn inset_text<'a>(value: &'a str) -> Element<'a, Message> {
    container(
        text(value)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

pub(super) fn action_button<'a>(
    label: &'a str,
    enabled: bool,
    message: Message,
) -> Element<'a, Message> {
    button(action_label(label))
        .width(Length::Fixed(action_button_width(label)))
        .padding(Padding::from([3, 8]))
        .style(styles::subtle_button)
        .on_press_maybe(enabled.then_some(message))
        .into()
}

pub(super) fn danger_action_button<'a>(
    label: &'a str,
    enabled: bool,
    message: Message,
) -> Element<'a, Message> {
    button(action_label(label))
        .width(Length::Fixed(action_button_width(label)))
        .padding(Padding::from([3, 8]))
        .style(styles::danger_button)
        .on_press_maybe(enabled.then_some(message))
        .into()
}

fn action_label<'a>(label: &'a str) -> Element<'a, Message> {
    container(
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None),
    )
    .center_x(Length::Fixed(action_label_width(label)))
    .into()
}

fn action_label_width(label: &str) -> f32 {
    let text_width = label.chars().count() as f32 * 7.0;
    (text_width + 4.0).clamp(36.0, 92.0)
}

fn action_button_width(label: &str) -> f32 {
    action_label_width(label) + 16.0
}

const CHAR_WIDTH_PX: f32 = 7.0;

pub(super) fn max_chars_for_width(width_px: f32) -> usize {
    if width_px <= 0.0 {
        return 0;
    }
    ((width_px / CHAR_WIDTH_PX).floor() as usize)
        .saturating_sub(1)
        .max(1)
}

pub(super) fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let total_width: usize = s.chars().map(display_width).sum();
    if total_width <= max_chars {
        return s.to_string();
    }
    let mut width = 0usize;
    let mut out: String = s
        .chars()
        .take_while(|ch| {
            let units = display_width(*ch);
            if width + units > max_chars.saturating_sub(1) {
                return false;
            }
            width += units;
            true
        })
        .collect();
    out.push('\u{2026}');
    out
}

fn display_width(ch: char) -> usize {
    match ch as u32 {
        0x1100..=0x115F
        | 0x2329..=0x232A
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE10..=0xFE19
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x2FFFD
        | 0x30000..=0x3FFFD => 2,
        _ => 1,
    }
}

pub(super) fn ghost_icon_button<'a>(icon: IconName, on_press: Message) -> Element<'a, Message> {
    button(icons::icon(icon, 15, color::TEXT_MUTED))
        .padding(Padding::from([4, 6]))
        .style(styles::ghost_icon_button)
        .on_press(on_press)
        .into()
}

pub(super) fn status_summary_title(status_detail: &WorktreeStatusDetail) -> String {
    let total = status_detail.staged.len()
        + status_detail.unstaged.len()
        + status_detail.untracked.len()
        + status_detail.conflicted.len()
        + status_detail.submodules.len();

    if total == 0 {
        "Clean".into()
    } else if total == 1 {
        "1 changed file".into()
    } else {
        format!("{total} changed files")
    }
}

pub(super) fn status_summary_detail(status_detail: &WorktreeStatusDetail) -> String {
    let counts = [
        (status_detail.staged.len(), "staged", "staged"),
        (
            status_detail.unstaged.len(),
            "modified file",
            "modified files",
        ),
        (status_detail.untracked.len(), "new file", "new files"),
        (status_detail.conflicted.len(), "conflict", "conflicts"),
        (
            status_detail.submodules.len(),
            "submodule change",
            "submodule changes",
        ),
        (status_detail.ignored.len(), "ignored file", "ignored files"),
    ];

    counts
        .into_iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, singular, plural)| pluralized_count(count, singular, plural))
        .collect::<Vec<_>>()
        .join(", ")
}

fn pluralized_count(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

pub(crate) fn format_duration_ago(diff_secs: i64) -> String {
    if diff_secs < 60 {
        "just now".into()
    } else if diff_secs < 3600 {
        format!("{} min ago", diff_secs / 60)
    } else if diff_secs < 86_400 {
        format!("{} hr ago", diff_secs / 3600)
    } else if diff_secs < 604_800 {
        format!("{} d ago", diff_secs / 86_400)
    } else if diff_secs < 2_592_000 {
        format!("{} w ago", diff_secs / 604_800)
    } else if diff_secs < 31_536_000 {
        format!("{} mo ago", diff_secs / 2_592_000)
    } else {
        format!("{} y ago", diff_secs / 31_536_000)
    }
}

pub(super) fn format_relative_time(secs: i64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(secs);
    let diff = (now_secs - secs).max(0);
    format_duration_ago(diff)
}

const PROGRESS_TRACK_WIDTH: f32 = 320.0;
const PROGRESS_SEGMENT_WIDTH: f32 = 92.0;

pub fn spinner_frame(frame: usize) -> &'static str {
    ["|", "/", "-", "\\"][frame % 4]
}

pub fn animated_dots(frame: usize) -> &'static str {
    match (frame / 4) % 4 {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

pub fn moving_progress_bar(frame: usize) -> Element<'static, Message> {
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

pub fn ease_in_out_sine(progress: f32) -> f32 {
    -(std::f32::consts::PI * progress).cos() / 2.0 + 0.5
}
