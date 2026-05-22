//! Command palette overlay.

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input, Space,
};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::features::command_palette as command_palette_feature;
use crate::styles;
use crate::theme::{self, color};
use crate::{CommandPaletteItem, Message};

const COMMAND_PALETTE_WIDTH: f32 = 640.0;
const COMMAND_PALETTE_TOP_MARGIN: f32 = 72.0;
const COMMAND_PALETTE_BODY_MAX_HEIGHT: f32 = 420.0;

pub fn command_palette_overlay<'a>(
    query: &'a str,
    commands: Vec<CommandPaletteItem>,
    selected: usize,
    input_id: &text_input::Id,
    window_height: f32,
) -> Element<'a, Message> {
    let body_height = command_palette_body_height(window_height);
    let palette = command_palette(query, commands, selected, input_id, body_height);

    let backdrop: Element<'a, Message> = mouse_area(
        container(Space::new(Length::Fill, Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(command_palette_backdrop),
    )
    .on_press(Message::from(command_palette_feature::Message::Closed))
    .into();

    let sheet: Element<'a, Message> = container(
        column![
            Space::with_height(Length::Fixed(COMMAND_PALETTE_TOP_MARGIN)),
            container(palette)
                .width(Length::Fill)
                .max_width(COMMAND_PALETTE_WIDTH),
            Space::with_height(Length::Fill),
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::from([0, theme::SP_LG]))
    .into();

    stack![backdrop, sheet].into()
}

pub fn command_palette<'a>(
    query: &'a str,
    commands: Vec<CommandPaletteItem>,
    selected: usize,
    input_id: &text_input::Id,
    body_height: f32,
) -> Element<'a, Message> {
    let header = row![
        text("COMMAND PALETTE")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
        Space::with_width(Length::Fill),
        button(text("Close").size(theme::FS_SM))
            .padding(Padding::from([3, 8]))
            .style(styles::subtle_button)
            .on_press(Message::from(command_palette_feature::Message::Closed)),
    ]
    .align_y(Alignment::Center);

    let search = text_input("Search commands...", query)
        .id(input_id.clone())
        .on_input(|query| Message::from(command_palette_feature::Message::QueryChanged(query)))
        .padding(Padding::from([6, 10]))
        .size(theme::FS_SM)
        .width(Length::Fill);

    let body: Element<'a, Message> = if commands.is_empty() {
        container(
            text("No commands match this filter.")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_SUBTLE),
        )
        .width(Length::Fill)
        .padding(Padding::from([8, 0]))
        .into()
    } else {
        let mut command_rows = column![].spacing(2);
        for (index, command) in commands.into_iter().enumerate() {
            command_rows =
                command_rows.push(command_palette_row(command, index, selected == index));
        }

        scrollable(command_rows)
            .width(Length::Fill)
            .height(Length::Fixed(body_height))
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar)
            .into()
    };

    let rows = column![header, search, body].spacing(theme::SP_SM);

    container(rows)
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::inset_card)
        .into()
}

fn command_palette_body_height(window_height: f32) -> f32 {
    (window_height - 220.0).clamp(160.0, COMMAND_PALETTE_BODY_MAX_HEIGHT)
}

fn command_palette_backdrop(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::BG, 0.42))),
        ..Default::default()
    }
}

fn command_palette_row<'a>(
    command: CommandPaletteItem,
    index: usize,
    selected: bool,
) -> Element<'a, Message> {
    let enabled = command.enabled();
    let label_color = if enabled {
        color::TEXT
    } else {
        color::TEXT_SUBTLE
    };
    let helper = command.disabled_reason.unwrap_or(command.description);
    let helper_color = if enabled {
        color::TEXT_MUTED
    } else {
        color::DANGER
    };
    let shortcut: Element<'a, Message> = if command.shortcut.is_empty() {
        Space::new(0.0, 0.0).into()
    } else {
        container(
            text(command.shortcut)
                .size(theme::FS_XS)
                .font(iced::Font::MONOSPACE)
                .color(if selected {
                    color::TEXT
                } else {
                    color::TEXT_SUBTLE
                }),
        )
        .padding(Padding::from([2, 6]))
        .style(styles::command_palette_shortcut(selected))
        .into()
    };

    let content = row![
        container(Space::new(Length::Fixed(2.0), Length::Fixed(32.0)))
            .style(styles::command_palette_selection_rail(selected)),
        column![
            text(command.label)
                .size(theme::FS_BASE)
                .font(if selected {
                    theme::font_semibold()
                } else {
                    theme::font_regular()
                })
                .color(label_color),
            text(helper)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(helper_color),
        ]
        .spacing(1)
        .width(Length::Fill),
        shortcut,
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    mouse_area(
        button(content)
            .padding(Padding::from([6, 8]))
            .width(Length::Fill)
            .style(styles::command_palette_button(selected))
            .on_press_maybe(enabled.then_some(Message::from(
                command_palette_feature::Message::Run(command.id),
            ))),
    )
    .on_enter(Message::from(command_palette_feature::Message::Selected(
        index,
    )))
    .into()
}
