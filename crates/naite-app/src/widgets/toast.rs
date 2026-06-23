//! Bottom-right toast notification layer.
//!
//! Renders short-lived success and failure pills stacked above the bottom
//! status bar. Success toasts auto-dismiss after
//! `theme::TOAST_SUCCESS_TTL_SECS`; failure toasts stay until the user
//! dismisses them with the built-in close button.
//!
//! The view is pure — TTL bookkeeping lives in `update.rs` (the existing
//! `Message::TransientStatusTick` arm retains expired toasts) and the
//! failure pill dispatches `Message::ToastDismissed { index }` to drop
//! itself from `App::toasts`. `Task 17` wires the layer into `view.rs` so
//! it overlays every other surface; this module only describes the
//! bottom-right stack.

use std::time::Instant;

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::styles;
use crate::theme::{self, color};
use crate::Message;

/// Maximum number of toasts rendered at once. Older toasts (FIFO by
/// creation time) are still tracked in `App::toasts` for bookkeeping but
/// the layer only paints the most recent `MAX_VISIBLE` so the corner
/// stack never grows past a sane height.
pub const MAX_VISIBLE: usize = 3;

/// Visual variant of a toast. Distinct from `state::OpSeverity`: toasts
/// are only ever drawn for recoverable operations (success or failure),
/// and this enum only carries the color cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Success,
    Failure,
}

impl ToastSeverity {
    pub const fn auto_dismiss(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub severity: ToastSeverity,
    pub created_at: Instant,
}

impl Toast {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            severity: ToastSeverity::Success,
            created_at: Instant::now(),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            severity: ToastSeverity::Failure,
            created_at: Instant::now(),
        }
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        if !self.severity.auto_dismiss() {
            return false;
        }
        let ttl = std::time::Duration::from_secs(theme::TOAST_SUCCESS_TTL_SECS);
        now.duration_since(self.created_at) >= ttl
    }
}

/// Render `toasts` as a bottom-right anchored stack of pills. The caller
/// is expected to compose this on top of the main view (Task 17). An empty
/// `toasts` slice produces a zero-area placeholder so the overlay never
/// swallows pointer events when there is nothing to show.
pub fn toast_layer<'a>(toasts: &'a [Toast]) -> Element<'a, Message> {
    if toasts.is_empty() {
        return Space::new(Length::Shrink, Length::Shrink).into();
    }

    let visible = visible_toasts(toasts);
    // Offset from the visible slice index back to the `App::toasts`
    // index. The dismiss button captures this offset so the handler
    // removes the right entry even when older toasts have scrolled off
    // the visible stack.
    let app_offset = toasts.len() - visible.len();
    let pills: Vec<Element<'a, Message>> = visible
        .iter()
        .enumerate()
        .map(|(visible_index, toast)| toast_pill(app_offset + visible_index, toast))
        .collect();

    let column = column(pills).spacing(theme::SP_SM);

    container(column)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_right(Length::Fill)
        .align_bottom(Length::Fill)
        .padding(iced::Padding {
            top: theme::SP_LG as f32,
            right: theme::SP_LG as f32,
            // Clear the bottom status bar (24px) plus a margin of SP_LG
            // so toasts never sit on top of it. The status bar height is
            // already f32, so only the spacing token needs a cast.
            bottom: theme::STATUS_BAR_HEIGHT + theme::SP_LG as f32,
            left: theme::SP_LG as f32,
        })
        .into()
}

/// Returns the most recent `MAX_VISIBLE` toasts as a contiguous slice,
/// preserving chronological order (oldest at the top, newest at the
/// bottom). Callers index into this slice for the dismiss button payload,
/// so the mapping back to `App::toasts` happens in the caller.
fn visible_toasts(toasts: &[Toast]) -> &[Toast] {
    if toasts.len() <= MAX_VISIBLE {
        toasts
    } else {
        let start = toasts.len() - MAX_VISIBLE;
        &toasts[start..]
    }
}

fn toast_pill(app_toast_index: usize, toast: &Toast) -> Element<'_, Message> {
    let style = match toast.severity {
        ToastSeverity::Success => styles::toast_pill_success,
        ToastSeverity::Failure => styles::toast_pill_failure,
    };
    let accent_color = match toast.severity {
        ToastSeverity::Success => color::SUCCESS,
        ToastSeverity::Failure => color::DANGER,
    };

    let label = text(toast.message.as_str())
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .color(color::TEXT);

    let row: iced::widget::Row<'_, Message> = match toast.severity {
        ToastSeverity::Failure => row![
            label,
            Space::with_width(theme::SP_SM),
            dismiss_button(app_toast_index)
        ]
        .align_y(Alignment::Center),
        // Success pills auto-dismiss; no manual close button.
        ToastSeverity::Success => row![label].align_y(Alignment::Center),
    };

    let badge = text(badge_glyph(toast.severity))
        .size(theme::FS_SM)
        .font(theme::font_semibold())
        .color(accent_color);

    container(row![badge, Space::with_width(theme::SP_SM), row])
        .padding(iced::Padding::from([6, 10]))
        .max_width(360)
        .style(style)
        .into()
}

fn dismiss_button(app_toast_index: usize) -> Element<'static, Message> {
    button(
        text("닫기")
            .size(theme::FS_XS)
            .font(theme::font_regular())
            .color(color::TEXT_MUTED),
    )
    .padding(iced::Padding::from([2, 6]))
    .style(styles::toast_dismiss_button)
    .on_press(Message::ToastDismissed {
        index: app_toast_index,
    })
    .into()
}

fn badge_glyph(severity: ToastSeverity) -> &'static str {
    match severity {
        ToastSeverity::Success => "✓",
        ToastSeverity::Failure => "!",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn severity_drives_auto_dismiss() {
        assert!(ToastSeverity::Success.auto_dismiss());
        assert!(!ToastSeverity::Failure.auto_dismiss());
    }

    #[test]
    fn success_toast_expires_after_ttl() {
        let ttl = Duration::from_secs(theme::TOAST_SUCCESS_TTL_SECS);
        let early = Toast::success("hello");
        let now = early.created_at + ttl - Duration::from_millis(10);
        assert!(!early.is_expired(now));

        let boundary = Toast::success("hello");
        let past = boundary.created_at + ttl + Duration::from_millis(10);
        assert!(boundary.is_expired(past));
    }

    #[test]
    fn failure_toast_never_auto_expires() {
        let toast = Toast::failure("oops");
        let far_future = toast.created_at + Duration::from_secs(60 * 60 * 24);
        assert!(!toast.is_expired(far_future));
    }

    #[test]
    fn visible_toasts_returns_last_three_in_order() {
        let toasts: Vec<Toast> = (0..5).map(|i| Toast::success(format!("{i}"))).collect();
        let visible = visible_toasts(&toasts);
        assert_eq!(visible.len(), MAX_VISIBLE);
        assert_eq!(visible.first().unwrap().message, "2");
        assert_eq!(visible.last().unwrap().message, "4");
    }

    #[test]
    fn visible_toasts_returns_full_slice_when_short() {
        let toasts: Vec<Toast> = (0..2).map(|i| Toast::success(format!("{i}"))).collect();
        let visible = visible_toasts(&toasts);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].message, "0");
        assert_eq!(visible[1].message, "1");
    }
}
