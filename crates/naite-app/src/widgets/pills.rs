//! Small status pills and dots for PR / CI / review state.
//!
//! Colored dots and low-saturation chips that let the eye scan rows without
//! competing with body text. Keep these widgets constant-size so they compose
//! cleanly in dense list rows.

use iced::widget::text::Wrapping;
use iced::widget::{container, row, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};
use naite_core::{PullRequestCiStatus, PullRequestReviewStatus};

use crate::theme::{self, color};
use crate::Message;

const DOT_SIZE: f32 = 6.0;

fn dot_style(fill: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

fn pill_style(accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(accent, 0.12))),
        border: Border {
            color: color::with_alpha(accent, 0.30),
            width: 1.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

pub fn status_dot(fill: Color) -> Element<'static, Message> {
    container(Space::new(Length::Fixed(DOT_SIZE), Length::Fixed(DOT_SIZE)))
        .width(Length::Fixed(DOT_SIZE))
        .height(Length::Fixed(DOT_SIZE))
        .style(dot_style(fill))
        .into()
}

pub fn ci_pill(status: PullRequestCiStatus) -> Element<'static, Message> {
    let (accent, label) = match status {
        PullRequestCiStatus::Passing => (color::SUCCESS, "Passing"),
        PullRequestCiStatus::Failing => (color::DANGER, "Failing"),
        PullRequestCiStatus::Pending => (color::WARNING, "Pending"),
        PullRequestCiStatus::NoChecks => (color::TEXT_SUBTLE, "No checks"),
        PullRequestCiStatus::Unknown => (color::TEXT_SUBTLE, "Unknown"),
    };
    pill_with(accent, label)
}

pub fn review_pill(status: PullRequestReviewStatus) -> Element<'static, Message> {
    let (accent, label) = match status {
        PullRequestReviewStatus::Approved => (color::SUCCESS, "Approved"),
        PullRequestReviewStatus::ChangesRequested => (color::DANGER, "Changes requested"),
        PullRequestReviewStatus::ReviewRequired => (color::WARNING, "Review required"),
        PullRequestReviewStatus::Unknown => (color::TEXT_SUBTLE, "Unknown"),
    };
    pill_with(accent, label)
}

pub fn muted_pill(label: &str) -> Element<'static, Message> {
    pill_with(color::TEXT_SUBTLE, label)
}

fn pill_with(accent: Color, label: &str) -> Element<'static, Message> {
    container(
        row![text(label.to_string())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(accent)]
        .spacing(theme::SP_XS),
    )
    .padding(Padding::from([2, 8]))
    .style(pill_style(accent))
    .into()
}

pub fn ci_status_color(status: PullRequestCiStatus) -> Color {
    match status {
        PullRequestCiStatus::Passing => color::SUCCESS,
        PullRequestCiStatus::Failing => color::DANGER,
        PullRequestCiStatus::Pending => color::WARNING,
        PullRequestCiStatus::NoChecks | PullRequestCiStatus::Unknown => color::TEXT_SUBTLE,
    }
}
