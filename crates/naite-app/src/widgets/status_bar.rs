//! Status strips:
//!
//! - [`top_status_bar`] — strip below the toolbar. When idle, renders a
//!   repo-status summary (branch sync, change/conflict counts, in-flight
//!   merge/rebase labels, "Fetched N min ago"). When operations are
//!   active, renders animated spinners for each on the right while the
//!   summary stays anchored on the left.
//! - [`bottom_status_bar`] — strip docked against the window bottom
//!   showing recently-completed foreground operations (success/failure
//!   glyphs + relative time-ago) and dismissable pills for Recoverable
//!   errors. Successful background auto-fetches are summarized by the top
//!   bar instead. Fatal errors are routed through a blocking modal.
//!
//! Both widgets are pure: they read the `OperationTracker` (and, for the
//! top bar, the current repo's `BranchSyncStatus` / `WorktreeStatusDetail`
//! / `GitOperationState` / last-fetch `Instant`) and emit an `Element`.
//! Integration (driving the `frame` counter from a tick subscription,
//! placing both strips in `view.rs`) lives outside this module.

use std::time::Instant;

use iced::widget::{button, container, row, text, text::Wrapping, Space};
use iced::{Alignment, Element, Length, Padding};
use naite_core::{BranchSyncStatus, GitOperationState, WorktreeStatusDetail};

use crate::message::OperationEvent;
use crate::state::{
    ActiveOperation, CompletedOperation, OpResult, OpSeverity, OperationId, OperationKind,
    OperationTracker,
};
use crate::styles;
use crate::theme::{self, color};
use crate::widgets::common::{
    format_duration_ago, spinner_frame, status_summary_title, truncate_with_ellipsis,
};
use crate::Message;

/// Inputs for the top status bar. `last_fetch_completed` is pre-filtered
/// by the caller: it should be `Some(instant)` only when the recorded
/// fetch completion belongs to the currently open repo (see
/// `OperationState::last_fetch_completed`).
pub struct TopStatusBarProps<'a> {
    pub tracker: &'a OperationTracker,
    pub frame: usize,
    pub repo_open: bool,
    pub sync_status: &'a BranchSyncStatus,
    pub status_detail: &'a WorktreeStatusDetail,
    pub operation_state: GitOperationState,
    pub last_fetch_completed: Option<Instant>,
}

/// Render the top status bar.
///
/// The strip always paints its surface. When a repo is open, the idle
/// repo-status summary is rendered on the left; when operations are
/// in flight, their spinners are appended on the right (separated by a
/// fill spacer so the left summary stays anchored). With no repo open
/// the strip is an empty painted band — no layout jump when a repo is
/// opened or closed.
pub fn top_status_bar<'a>(props: TopStatusBarProps<'a>) -> Element<'a, Message> {
    let active = props.tracker.active();
    let mut row = row![]
        .align_y(Alignment::Center)
        .spacing(theme::SP_LG)
        .padding([0, theme::SP_LG]);

    if props.repo_open {
        row = row.push(idle_summary(&props));
    }

    if !active.is_empty() {
        if props.repo_open {
            row = row.push(Space::with_width(Length::Fill));
        }
        for (idx, op) in active.iter().enumerate() {
            if idx > 0 {
                row = row.push(separator_glyph());
            }
            row = row.push(active_op_segment(op, props.frame));
        }
    }

    container(row)
        .width(Length::Fill)
        .height(Length::Fixed(theme::STATUS_BAR_HEIGHT))
        .align_y(Alignment::Center)
        .style(styles::status_bar_surface)
        .into()
}

fn idle_summary(props: &TopStatusBarProps<'_>) -> Element<'static, Message> {
    let mut segments: Vec<Element<'static, Message>> = Vec::new();

    segments.push(sync_segment(props.sync_status));
    segments.push(changes_segment(props.status_detail));
    segments.extend(operation_state_labels(props.operation_state).map(|label| {
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::WARNING)
            .into()
    }));
    if let Some(instant) = props.last_fetch_completed {
        segments.push(
            text(fetched_ago_text(instant))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE)
                .into(),
        );
    }

    let mut joined = row![].align_y(Alignment::Center).spacing(theme::SP_MD);
    for (idx, segment) in segments.into_iter().enumerate() {
        if idx > 0 {
            joined = joined.push(separator_glyph());
        }
        joined = joined.push(segment);
    }
    joined.into()
}

fn sync_segment(sync_status: &BranchSyncStatus) -> Element<'static, Message> {
    match sync_status_text(sync_status) {
        Some(label) => text(label)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_MUTED)
            .into(),
        None => text("no upstream")
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE)
            .into(),
    }
}

fn changes_segment(status_detail: &WorktreeStatusDetail) -> Element<'static, Message> {
    let title = status_summary_title(status_detail);
    let title_color = if status_detail.is_dirty() {
        color::TEXT_MUTED
    } else {
        color::TEXT_SUBTLE
    };

    let title_text = text(title)
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .wrapping(Wrapping::None)
        .color(title_color);

    if status_detail.conflicted.is_empty() {
        title_text.into()
    } else {
        let conflict = text(conflict_count_text(status_detail.conflicted.len()))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::DANGER);
        row![title_text, separator_glyph(), conflict]
            .align_y(Alignment::Center)
            .spacing(theme::SP_MD)
            .into()
    }
}

fn active_op_segment(op: &ActiveOperation, frame: usize) -> Element<'_, Message> {
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

    row![spinner, label]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .into()
}

fn sync_status_text(sync_status: &BranchSyncStatus) -> Option<String> {
    let upstream = sync_status.upstream.as_deref()?;
    match (sync_status.ahead, sync_status.behind) {
        (0, 0) => Some(format!("{upstream} synced")),
        (ahead, 0) => Some(format!("{upstream} ahead {ahead}")),
        (0, behind) => Some(format!("{upstream} behind {behind}")),
        (ahead, behind) => Some(format!("{upstream} ahead {ahead} / behind {behind}")),
    }
}

fn conflict_count_text(count: usize) -> String {
    if count == 1 {
        "1 conflicted".into()
    } else {
        format!("{count} conflicted")
    }
}

fn operation_state_labels(state: GitOperationState) -> OperationStateLabels {
    OperationStateLabels {
        rebase: state.rebase_in_progress,
        merge: state.merge_in_progress,
    }
}

struct OperationStateLabels {
    rebase: bool,
    merge: bool,
}

impl Iterator for OperationStateLabels {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rebase {
            self.rebase = false;
            return Some("Rebase in progress");
        }
        if self.merge {
            self.merge = false;
            return Some("Merge in progress");
        }
        None
    }
}

fn fetched_ago_text(completed_at: Instant) -> String {
    format!(
        "Fetched {}",
        format_duration_ago(completed_at.elapsed().as_secs() as i64)
    )
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
    let visible = visible_recent(&tracker.recent(usize::MAX));

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

/// Newest-first view of completed operations eligible for the bottom bar.
fn visible_recent<'a>(recent: &[&'a CompletedOperation]) -> Vec<&'a CompletedOperation> {
    recent
        .iter()
        .rev()
        .copied()
        .filter(|op| !is_fatal(op) && !is_successful_auto_fetch(op))
        .take(BOTTOM_BAR_RECENT)
        .collect()
}

fn is_fatal(op: &CompletedOperation) -> bool {
    matches!(op.severity, OpSeverity::Fatal)
}

fn is_successful_auto_fetch(op: &CompletedOperation) -> bool {
    matches!(&op.kind, OperationKind::AutoFetch) && matches!(&op.result, OpResult::Success)
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
    use naite_core::StatusEntry;

    #[test]
    fn bottom_bar_returns_reserved_space_when_history_empty() {
        let tracker = OperationTracker::default();
        let element = bottom_status_bar(&tracker);
        let _ = element;
    }

    #[test]
    fn bottom_bar_skips_fatal_entries_and_keeps_recoverable() {
        let mut tracker = OperationTracker::default();

        let ok_id = tracker.start(OperationKind::ManualAction("fetch"), "fetch origin");
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
    fn bottom_bar_hides_successful_auto_fetches_without_consuming_history_slots() {
        let mut tracker = OperationTracker::default();

        let manual_id = tracker.start(OperationKind::ManualAction("pull"), "pull");
        tracker
            .complete(manual_id, OpResult::Success, OpSeverity::Recoverable)
            .unwrap();
        for label in ["first auto-fetch", "second auto-fetch", "third auto-fetch"] {
            let id = tracker.start(OperationKind::AutoFetch, label);
            tracker
                .complete(id, OpResult::Success, OpSeverity::Recoverable)
                .unwrap();
        }
        let failed_id = tracker.start(OperationKind::AutoFetch, "failed auto-fetch");
        tracker
            .fail(failed_id, "offline", OpSeverity::Recoverable)
            .unwrap();

        let visible = visible_recent(&tracker.recent(usize::MAX));

        assert_eq!(
            visible.iter().map(|op| op.id).collect::<Vec<_>>(),
            vec![failed_id, manual_id],
            "successful background fetches belong only in the top status bar"
        );
        assert!(matches!(visible[0].result, OpResult::Failed(_)));
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

    #[test]
    fn sync_status_text_renders_all_upstream_states() {
        let mk = |upstream: Option<&str>, ahead: u32, behind: u32| BranchSyncStatus {
            upstream: upstream.map(Into::into),
            ahead,
            behind,
        };

        assert_eq!(
            sync_status_text(&mk(Some("origin/main"), 0, 0)).as_deref(),
            Some("origin/main synced")
        );
        assert_eq!(
            sync_status_text(&mk(Some("origin/main"), 3, 0)).as_deref(),
            Some("origin/main ahead 3")
        );
        assert_eq!(
            sync_status_text(&mk(Some("origin/main"), 0, 2)).as_deref(),
            Some("origin/main behind 2")
        );
        assert_eq!(
            sync_status_text(&mk(Some("origin/main"), 1, 4)).as_deref(),
            Some("origin/main ahead 1 / behind 4")
        );
        assert!(sync_status_text(&mk(None, 0, 0)).is_none());
    }

    #[test]
    fn fetched_ago_text_uses_just_now_for_now() {
        assert_eq!(fetched_ago_text(Instant::now()), "Fetched just now");
    }

    #[test]
    fn operation_state_labels_covers_rebase_merge_both_and_neither() {
        let neither = GitOperationState {
            rebase_in_progress: false,
            merge_in_progress: false,
        };
        assert_eq!(
            operation_state_labels(neither).collect::<Vec<_>>(),
            Vec::<&str>::new()
        );

        let rebase_only = GitOperationState {
            rebase_in_progress: true,
            merge_in_progress: false,
        };
        assert_eq!(
            operation_state_labels(rebase_only).collect::<Vec<_>>(),
            vec!["Rebase in progress"]
        );

        let merge_only = GitOperationState {
            rebase_in_progress: false,
            merge_in_progress: true,
        };
        assert_eq!(
            operation_state_labels(merge_only).collect::<Vec<_>>(),
            vec!["Merge in progress"]
        );

        let both = GitOperationState {
            rebase_in_progress: true,
            merge_in_progress: true,
        };
        assert_eq!(
            operation_state_labels(both).collect::<Vec<_>>(),
            vec!["Rebase in progress", "Merge in progress"]
        );
    }

    #[test]
    fn conflict_count_text_singular_plural() {
        assert_eq!(conflict_count_text(1), "1 conflicted");
        assert_eq!(conflict_count_text(3), "3 conflicted");
    }

    #[test]
    fn top_status_bar_renders_painted_band_for_empty_tracker() {
        let tracker = OperationTracker::default();
        let sync_status = BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        };
        let status_detail = WorktreeStatusDetail::default();
        let props = TopStatusBarProps {
            tracker: &tracker,
            frame: 0,
            repo_open: true,
            sync_status: &sync_status,
            status_detail: &status_detail,
            operation_state: GitOperationState::default(),
            last_fetch_completed: Some(Instant::now()),
        };
        let _ = top_status_bar(props);
    }

    #[test]
    fn top_status_bar_painted_band_with_no_repo_open() {
        let tracker = OperationTracker::default();
        let sync_status = BranchSyncStatus::default();
        let status_detail = WorktreeStatusDetail::default();
        let props = TopStatusBarProps {
            tracker: &tracker,
            frame: 0,
            repo_open: false,
            sync_status: &sync_status,
            status_detail: &status_detail,
            operation_state: GitOperationState::default(),
            last_fetch_completed: None,
        };
        let _ = top_status_bar(props);
    }

    #[test]
    fn changes_segment_switches_color_for_dirty_worktree() {
        let dirty = WorktreeStatusDetail {
            unstaged: vec![StatusEntry {
                path: "a.rs".into(),
                old_path: None,
                status: naite_core::StatusKind::Modified,
            }],
            ..Default::default()
        };
        let _ = changes_segment(&dirty);

        let clean = WorktreeStatusDetail::default();
        let _ = changes_segment(&clean);
    }
}
