//! Top status bar widget — thin strip rendered just below the toolbar
//! that surfaces in-flight operations from `OperationTracker`.
//!
//! This is the primary visual feedback for "what is running now" —
//! long-running commands, release promotion preparation steps, auto-fetch,
//! etc. Each in-flight operation renders as `spinner | label [: current/total]`
//! in a single horizontal row.
//!
//! The widget itself is pure: it reads `tracker.active()` and a `frame`
//! counter, and emits an `Element`. The integration (driving `frame`
//! from a tick subscription and placing the widget in `view.rs`) lives in
//! Task 17.

use iced::widget::{container, row, text, text::Wrapping, Space};
use iced::{Alignment, Element, Length};

use crate::state::OperationTracker;
use crate::styles;
use crate::theme::{self, color};
use crate::widgets::common::spinner_frame;
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