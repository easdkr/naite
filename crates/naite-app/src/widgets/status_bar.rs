//! Status strips:
//!
//! - [`top_status_bar`] — strip below the toolbar showing in-flight
//!   operations from `tracker.active()` with animated spinners.
//! - [`bottom_status_bar`] — strip docked against the window bottom
//!   showing recently-completed operations (success/failure glyphs +
//!   relative time-ago) and dismissable pills for Recoverable errors.
//!   Fatal errors are deliberately NOT rendered here — Task 19 routes
//!   them through a blocking modal instead.
//!
//! Both widgets are pure: they read the `OperationTracker` and emit an
//! `Element`. Integration (driving the `frame` counter from a tick
//! subscription, placing both strips in `view.rs`) lives in Task 17.

use std::time::Instant;

use iced::widget::{button, container, row, text, text::Wrapping, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::message::OperationEvent;
use crate::state::{CompletedOperation, OpResult, OpSeverity, OperationId, OperationTracker};
use crate::styles;
use crate::theme::{self, color};
use crate::widgets::common::{format_duration_ago, spinner_frame, truncate_with_ellipsis};
use crate::Message;

/// Render the top status bar.
///
/// - When `tracker.active()` is empty, returns a `Space` of
///   `STATUS_BAR_HEIGHT` so the vertical slot is preserved (no UI jump
///   when operations start/stop).
/// - When there are in-flight operations, renders each as
///   `spinner | label [: current/total]` in a horizontally-spaced row.
///   The `frame` argument is the 80ms-spaced animation counter supplied
///   by the caller (Task 17 will wire the tick).
pub fn top_status_bar<'a>(tracker: &'a OperationTracker, frame: usize) -> Element<'a, Message> {
    let active = tracker.active();

    if active.is_empty() {
        // No operations in flight: reserve the slot but draw nothing.
        Space::with_height(Length::Fixed(theme::STATUS_BAR_HEIGHT)).into()
    } else {
        let mut row = row![]
            .align_y(Alignment::Center)
            .spacing(theme::SP_LG)
            .padding([0, theme::SP_LG]);

        // Each in-flight operation gets one entry: animated spinner +
        // label + optional "current/total" step counter.
        for (idx, op) in active.iter().enumerate() {
            if idx > 0 {
                // Visual separator between concurrent operations.
                row = row.push(
                    text("·")
                        .size(theme::FS_SM)
                        .font(theme::font_regular())
                        .color(color::TEXT_SUBTLE),
                );
            }

            let spinner = text(spinner_frame(frame))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::ACCENT);

            let label_text = match op.step {
                Some((current, total)) => format!("{}: {}/{}", op.label, current, total),
                None => op.label.clone(),
            };

            let label = text(label_text)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED);

            row = row.push(spinner).push(label);
        }

        container(row)
            .width(Length::Fill)
            .height(Length::Fixed(theme::STATUS_BAR_HEIGHT))
            .align_y(Alignment::Center)
            .style(styles::status_bar_surface)
            .into()
    }
}

/// Maximum number of completed operations to show on the bottom bar.
const BOTTOM_BAR_RECENT: usize = 3;

/// Soft cap on the inline error message length inside a pill — keeps
/// the bar readable when the underlying Git error spans multiple lines.
const ERROR_TEXT_MAX_CHARS: usize = 64;

/// Render the bottom status bar.
///
/// - When `tracker.recent(BOTTOM_BAR_RECENT)` is empty (or all visible
///   entries are Fatal, which are routed to the Task 19 modal), returns
///   a `Space` of `STATUS_BAR_HEIGHT` so the vertical slot is preserved.
/// - Otherwise renders each non-Fatal completed operation in
///   newest-first order:
///   - `OpResult::Success` → flat row `[✓] label · time-ago`
///   - `OpResult::Failed` + `OpSeverity::Recoverable` → red pill
///     `[✗] label · truncated-error · Dismiss`
///
/// The widget is pure: it reads the tracker by reference and emits an
/// `Element`. `Task 19` will route the `OperationEvent::Dismissed`
/// message to `tracker.dismiss(id)`.
pub fn bottom_status_bar<'a>(tracker: &'a OperationTracker) -> Element<'a, Message> {
    let visible = visible_recent(&tracker.recent(BOTTOM_BAR_RECENT));

    if visible.is_empty() {
        // Nothing to surface — reserve the slot height to avoid UI jump
        // when the first completion lands.
        Space::with_height(Length::Fixed(theme::STATUS_BAR_HEIGHT)).into()
    } else {
        let mut row = row![]
            .align_y(Alignment::Center)
            .spacing(theme::SP_MD)
            .padding([0, theme::SP_LG]);

        for (idx, op) in visible.iter().enumerate() {
            if idx > 0 {
                row = row.push(separator_glyph());
            }
            row = row.push(operation_entry(op));
        }

        container(row)
            .width(Length::Fill)
            .height(Length::Fixed(theme::STATUS_BAR_HEIGHT))
            .align_y(Alignment::Center)
            .style(styles::bottom_status_bar_surface)
            .into()
    }
}

/// Newest-first, Fatal-filtered view of the tracker's recent history.
fn visible_recent<'a>(recent: &[&'a CompletedOperation]) -> Vec<&'a CompletedOperation> {
    recent
        .iter()
        .rev()
        .copied()
        .filter(|op| !is_fatal(op))
        .collect()
}

fn is_fatal(op: &CompletedOperation) -> bool {
    matches!(op.severity, OpSeverity::Fatal)
}

fn separator_glyph<'a>() -> Element<'a, Message> {
    text("·")
        .size(theme::FS_SM)
        .color(color::TEXT_SUBTLE)
        .into()
}

fn operation_entry<'a>(op: &'a CompletedOperation) -> Element<'a, Message> {
    match (&op.result, op.severity) {
        (OpResult::Failed(_), OpSeverity::Recoverable) => error_pill(op),
        _ => success_entry(op),
    }
}

fn success_entry<'a>(op: &'a CompletedOperation) -> Element<'a, Message> {
    row![
        result_glyph("✓", color::SUCCESS),
        text(op.label.as_str())
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_MUTED),
        text(format_instant_ago(op.completed_at))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into()
}

fn error_pill<'a>(op: &'a CompletedOperation) -> Element<'a, Message> {
    let error_msg = match &op.result {
        OpResult::Failed(msg) => msg.as_str(),
        OpResult::Success => "",
    };
    let truncated = truncate_error(error_msg);

    container(
        row![
            result_glyph("✗", color::DANGER),
            text(op.label.as_str())
                .size(theme::FS_SM)
                .font(theme::font_semibold())
                .color(color::TEXT),
            text("·").size(theme::FS_SM).color(color::TEXT_SUBTLE),
            text(truncated)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
            Space::with_width(Length::Fill),
            dismiss_button(op.id),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([2, theme::SP_MD]))
    .style(styles::error_pill)
    .into()
}

fn result_glyph<'a>(glyph: &'a str, color: iced::Color) -> Element<'a, Message> {
    text(glyph)
        .size(theme::FS_SM)
        .font(theme::font_semibold())
        .color(color)
        .into()
}

fn dismiss_button<'a>(id: OperationId) -> Element<'a, Message> {
    button(
        text("Dismiss")
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([2, theme::SP_SM]))
    .style(styles::subtle_button)
    .on_press(Message::Operation(OperationEvent::Dismissed { id }))
    .into()
}

fn truncate_error(msg: &str) -> String {
    let cleaned = msg.trim();
    if cleaned.is_empty() {
        return "(no detail)".into();
    }
    truncate_with_ellipsis(cleaned, ERROR_TEXT_MAX_CHARS)
}

fn format_instant_ago(instant: Instant) -> String {
    format_duration_ago(instant.elapsed().as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{OperationKind, OperationTracker};

    #[test]
    fn bottom_bar_returns_reserved_space_when_history_empty() {
        let tracker = OperationTracker::default();
        let element = bottom_status_bar(&tracker);
        let _ = element;
    }

    #[test]
    fn bottom_bar_skips_fatal_entries_and_keeps_recoverable() {
        let mut tracker = OperationTracker::default();

        let ok_id = tracker.start(OperationKind::AutoFetch, "fetch origin");
        tracker
            .complete(ok_id, OpResult::Success, OpSeverity::Recoverable)
            .unwrap();

        let fatal_id = tracker.start(OperationKind::ReleasePrep, "promote");
        tracker.fail(fatal_id, "boom", OpSeverity::Fatal).unwrap();

        let recoverable_id = tracker.start(OperationKind::ManualAction("rebase"), "rebase");
        tracker
            .fail(recoverable_id, "conflict", OpSeverity::Recoverable)
            .unwrap();

        let visible = visible_recent(&tracker.recent(BOTTOM_BAR_RECENT));
        assert_eq!(visible.len(), 2, "fatal entry must be filtered out");
        assert!(visible.iter().all(|op| !is_fatal(op)));
    }

    #[test]
    fn format_instant_ago_uses_monotonic_buckets() {
        let now = Instant::now();
        assert_eq!(format_instant_ago(now), "just now");
    }

    #[test]
    fn truncate_error_caps_long_messages_with_ellipsis() {
        let long = "x".repeat(ERROR_TEXT_MAX_CHARS + 50);
        let out = truncate_error(&long);
        assert!(out.chars().count() <= ERROR_TEXT_MAX_CHARS);
        assert!(out.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_error_returns_no_detail_for_empty_message() {
        assert_eq!(truncate_error(""), "(no detail)");
        assert_eq!(truncate_error("   "), "(no detail)");
    }
}
