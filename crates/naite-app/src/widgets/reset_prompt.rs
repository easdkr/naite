//! Confirmation prompt for `git reset` with mode selection (soft/mixed/hard).

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding};
use naite_core::ResetMode;

use crate::features::reset;
use crate::styles;
use crate::theme::{self, color};
use crate::{Message, ResetPrompt};

pub fn reset_prompt<'a>(prompt: &'a ResetPrompt, loading: bool) -> Element<'a, Message> {
    let title = format!("Reset to {}", prompt.target.short_id);
    let detail = format!(
        "Move HEAD to {}. Choose how index and worktree are affected.",
        prompt.target.short_id
    );

    container(
        column![
            row![
                column![
                    text(title)
                        .size(theme::FS_BASE)
                        .font(theme::font_semibold())
                        .color(color::TEXT),
                    text(detail)
                        .size(theme::FS_SM)
                        .font(theme::font_regular())
                        .color(color::TEXT_MUTED),
                    text(format!("Subject: {}", prompt.target.summary))
                        .size(theme::FS_SM)
                        .font(theme::font_regular())
                        .color(color::TEXT_MUTED),
                ]
                .spacing(2),
                Space::with_width(Length::Fill),
                button(text("Cancel").size(theme::FS_SM))
                    .padding(Padding::from([5, 10]))
                    .style(styles::subtle_button)
                    .on_press(Message::from(reset::Message::Cancelled)),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_MD),
            row![
                button(text("Soft (keep changes staged)").size(theme::FS_SM))
                    .padding(Padding::from([5, 10]))
                    .style(styles::subtle_button)
                    .on_press_maybe(
                        (!loading)
                            .then_some(Message::from(reset::Message::Confirmed(ResetMode::Soft)))
                    ),
                button(text("Mixed (keep changes unstaged)").size(theme::FS_SM))
                    .padding(Padding::from([5, 10]))
                    .style(styles::subtle_button)
                    .on_press_maybe(
                        (!loading)
                            .then_some(Message::from(reset::Message::Confirmed(ResetMode::Mixed)))
                    ),
                button(text("Hard (discard changes)").size(theme::FS_SM))
                    .padding(Padding::from([5, 10]))
                    .style(styles::danger_button)
                    .on_press_maybe(
                        (!loading)
                            .then_some(Message::from(reset::Message::Confirmed(ResetMode::Hard)))
                    ),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM),
        ]
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
    .into()
}
