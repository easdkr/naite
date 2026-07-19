//! Central progress overlay for long-running operations.
//!
//! Renders a dimmed backdrop with a centered card surfacing the
//! in-flight operation's label, the animated `moving_progress_bar`,
//! and (when the operation exposes one) a `current/total` step counter.
//! The overlay is non-modal: the backdrop does not capture pointer
//! events so the user can keep interacting with the underlying UI
//! while they wait. v1 deliberately omits a cancel button — the plan
//! excludes cancel support, and Task 20 owns the trigger condition
//! that shows this overlay. release_prep's full per-step glyph list
//! is added by Task 21.

use iced::widget::text::Wrapping;
use iced::widget::{column, container, stack, text, Space};
use iced::{alignment, Alignment, Element, Length, Padding};

use crate::state::ActiveOperation;
use crate::styles;
use crate::theme::{self, color};
use crate::widgets::common::moving_progress_bar;
use crate::Message;

const OVERLAY_CARD_WIDTH: f32 = 420.0;

/// Build the central progress overlay. The caller supplies the current
/// animation `frame` so the moving bar pulses without driving its own
/// clock. The result is a full-screen `stack`: dimmed backdrop below,
/// centered card on top.
pub fn progress_overlay<'a>(op: &'a ActiveOperation, frame: usize) -> Element<'a, Message> {
    let backdrop: Element<'a, Message> = container(Space::new(Length::Fill, Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::progress_overlay_backdrop)
        .into();

    let card_content = progress_card_content(op, frame);

    let card: Element<'a, Message> = container(card_content)
        .width(Length::Fixed(OVERLAY_CARD_WIDTH))
        .padding(Padding::from(theme::SP_LG))
        .style(styles::progress_overlay_card)
        .into();

    let card_center: Element<'a, Message> = container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();

    stack![backdrop, card_center].into()
}

fn progress_card_content<'a>(op: &'a ActiveOperation, frame: usize) -> Element<'a, Message> {
    let mut content = column![
        moving_progress_bar(frame),
        text(&op.label)
            .size(theme::FS_LG)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
    ]
    .width(Length::Fill)
    .spacing(theme::SP_MD)
    .align_x(Alignment::Center);

    if let Some((current, total)) = op.step {
        content = content.push(
            text(format!("Step {current}/{total}"))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .color(color::TEXT_MUTED),
        );
    }

    content.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_card_content_fills_the_card_width() {
        let operation = ActiveOperation {
            id: 1,
            kind: crate::state::OperationKind::ManualAction("layout_test"),
            label: "Checking out a pull request into a long worktree path".to_string(),
            started_at: std::time::Instant::now(),
            step: Some((1, 3)),
        };

        let content: Element<'_, Message> = progress_card_content(&operation, 0);

        assert_eq!(content.as_widget().size().width, Length::Fill);
    }
}
