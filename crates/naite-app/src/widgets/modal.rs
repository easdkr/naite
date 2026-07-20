//! Full-screen modal overlay primitive.
//!
//! Renders a dimmed backdrop that captures clicks (dispatching an
//! `on_dismiss` message) and a centered card with the caller-supplied
//! content. Intended for true blocking flows (multi-field forms); use the
//! inline cards in `prompts.rs` for lightweight confirmations.

use iced::widget::text::Wrapping;
use iced::widget::{container, mouse_area, scrollable, stack, text, Space};
use iced::{Background, Border, Element, Length, Padding, Theme};

use crate::styles;
use crate::theme::{self, color, MAX_MODAL_HEIGHT};
use crate::Message;

const MODAL_MAX_WIDTH: f32 = 480.0;
/// Wider card for content-heavy confirmations (e.g. the rebase plan preview)
/// where per-row columns need horizontal room to stay on one line.
const MODAL_WIDE_MAX_WIDTH: f32 = 680.0;
const MODAL_ENTRY_FRAMES: f32 = 10.0;

pub fn modal<'a>(content: Element<'a, Message>, on_dismiss: Message) -> Element<'a, Message> {
    modal_with_progress(content, on_dismiss, 1.0, MODAL_MAX_WIDTH)
}

pub fn wide_modal<'a>(content: Element<'a, Message>, on_dismiss: Message) -> Element<'a, Message> {
    modal_with_progress(content, on_dismiss, 1.0, MODAL_WIDE_MAX_WIDTH)
}

pub fn animated_modal<'a>(
    content: Element<'a, Message>,
    on_dismiss: Message,
    frame: usize,
) -> Element<'a, Message> {
    let progress = (frame as f32 / MODAL_ENTRY_FRAMES).clamp(0.0, 1.0);
    modal_with_progress(
        content,
        on_dismiss,
        ease_out_cubic(progress),
        MODAL_MAX_WIDTH,
    )
}

pub(super) fn modal_action_label<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .size(theme::FS_SM)
        .font(theme::font_semibold())
        .wrapping(Wrapping::None)
        .into()
}

fn modal_with_progress<'a, Renderer>(
    content: Element<'a, Message, Theme, Renderer>,
    on_dismiss: Message,
    progress: f32,
    max_width: f32,
) -> Element<'a, Message, Theme, Renderer>
where
    Renderer: iced_core::Renderer + 'a,
{
    let backdrop: Element<'a, Message, Theme, Renderer> = mouse_area(
        container(Space::new(Length::Fill, Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| backdrop_progress_style(progress)),
    )
    .on_press(on_dismiss)
    .into();

    let card_surface: Element<'a, Message, Theme, Renderer> = container(
        mouse_area(
            scrollable(
                container(content)
                    .width(Length::Fill)
                    .max_width(max_width)
                    .max_height(MAX_MODAL_HEIGHT)
                    .padding(Padding::from(theme::SP_LG))
                    .style(move |_| card_progress_style(progress)),
            )
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar),
        )
        .on_press(Message::NoOp),
    )
    .width(Length::Fill)
    .max_width(max_width)
    .into();

    let card: Element<'a, Message, Theme, Renderer> = container(card_surface)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();

    stack![backdrop, card].into()
}

fn backdrop_progress_style(progress: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(
            color::BG,
            0.6 * progress,
        ))),
        ..Default::default()
    }
}

fn card_progress_style(progress: f32) -> container::Style {
    let background_alpha = 0.76 + 0.24 * progress;
    container::Style {
        background: Some(Background::Color(color::with_alpha(
            color::SURFACE_2,
            background_alpha,
        ))),
        border: Border {
            color: color::with_alpha(color::BORDER, 0.35 + 0.65 * progress),
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

fn ease_out_cubic(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::{layout, widget::Tree, Size};

    #[test]
    fn modal_card_is_centered_within_window_bounds() {
        let content: Element<'_, Message, Theme, ()> =
            Space::new(Length::Fill, Length::Fixed(100.0)).into();
        let modal = modal_with_progress(content, Message::NoOp, 1.0, MODAL_MAX_WIDTH);
        let mut tree = Tree::new(modal.as_widget());
        let limits = layout::Limits::new(Size::ZERO, Size::new(1_200.0, 800.0));
        let node = modal.as_widget().layout(&mut tree, &(), &limits);

        let card_layer = layout::Layout::new(&node)
            .children()
            .nth(1)
            .expect("modal stack contains the card layer");
        let scrollable = card_layer
            .children()
            .next()
            .expect("card layer contains the scrollable surface");
        let card_surface = scrollable
            .children()
            .next()
            .expect("scrollable contains the styled card surface");

        assert_eq!(card_surface.bounds().width, MODAL_MAX_WIDTH);
        assert_eq!(card_surface.bounds().x, (1_200.0 - MODAL_MAX_WIDTH) / 2.0);
    }
}
