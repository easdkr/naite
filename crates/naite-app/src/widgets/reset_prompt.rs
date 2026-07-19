//! Confirmation prompt for `git reset` with mode selection (soft/mixed/hard).

use iced::widget::text::Wrapping;
use iced::widget::{button, column, row, text, Space};
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

    column![
        text(title)
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        column![
            text(detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
            text(format!("Subject: {}", prompt.target.summary))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        column![
            mode_hint("Soft", "keep changes staged"),
            mode_hint("Mixed", "keep changes unstaged"),
            mode_hint("Hard", "discard changes"),
        ]
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            button(super::modal::modal_action_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(reset::Message::Cancelled)),
            button(super::modal::modal_action_label("Soft"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(reset::Message::Confirmed(ResetMode::Soft)))
                ),
            button(super::modal::modal_action_label("Mixed"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press_maybe(
                    (!loading)
                        .then_some(Message::from(reset::Message::Confirmed(ResetMode::Mixed)))
                ),
            button(super::modal::modal_action_label("Hard"))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(reset::Message::Confirmed(ResetMode::Hard)))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

fn mode_hint<'a>(name: &'a str, description: &'a str) -> Element<'a, Message> {
    row![
        text(name)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .color(color::TEXT),
        text(format!(" — {description}"))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_MUTED),
    ]
    .into()
}
