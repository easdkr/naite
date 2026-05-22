//! Vertical tab strip rendered at the top of the sidebar. Each row represents
//! an open repository; clicking the row activates the tab, the trailing X
//! closes it. A tiny dot on the right edge indicates a background refresh in
//! flight. A "+" button at the end of the strip toggles a dropdown menu with
//! actions to add a new repository (Open / Init / Clone) and to access the
//! Workspace dashboard / Terminal.

use std::path::Path;

use iced::widget::{button, column, container, row, text, text::Wrapping, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::features::{repo_open, terminal, workspace};
use crate::icons::{self, IconName};
use crate::message::TabsMessage;
use crate::state::RepositoryTabsState;
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

const TAB_ICON_SIZE: u16 = 12;
const TAB_CLOSE_SIZE: u16 = 12;
const TAB_HEIGHT: f32 = 28.0;
const TAB_CLOSE_BUTTON_WIDTH: f32 = 20.0;
const MENU_ICON_SIZE: u16 = 13;

pub fn tab_strip<'a>(
    tabs: &'a RepositoryTabsState,
    new_repo_menu_open: bool,
) -> Element<'a, Message> {
    let mut col = column![].spacing(2);
    for path in &tabs.open {
        let active = tabs.active.as_deref() == Some(path.as_path());
        col = col.push(tab_row(path, active, tabs.is_refreshing(path)));
    }
    col = col.push(new_repo_button(new_repo_menu_open));
    if new_repo_menu_open {
        col = col.push(new_repo_menu());
    }

    container(col)
        .width(Length::Fill)
        .padding(Padding::from([theme::SP_XS, theme::SP_SM]))
        .into()
}

fn tab_row<'a>(path: &'a Path, active: bool, refreshing: bool) -> Element<'a, Message> {
    let activate_button = button(
        row![
            icons::icon(IconName::FolderOpen, TAB_ICON_SIZE, color::TEXT_SUBTLE),
            text(path_label(path))
                .size(theme::FS_SM)
                .font(if active {
                    theme::font_semibold()
                } else {
                    theme::font_regular()
                })
                .wrapping(Wrapping::None)
                .color(if active {
                    color::TEXT
                } else {
                    color::TEXT_MUTED
                }),
            Space::with_width(Length::Fill),
            refresh_dot(refreshing),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .height(Length::Fixed(TAB_HEIGHT)),
    )
    .padding(Padding::from([0, theme::SP_SM]))
    .width(Length::Fill)
    .style(styles::tab_strip_button(active))
    .on_press(Message::from(TabsMessage::Activate(path.to_path_buf())));

    let close_button = button(icons::icon(
        IconName::Close,
        TAB_CLOSE_SIZE,
        color::TEXT_SUBTLE,
    ))
    .padding(Padding::from([4, 4]))
    .width(Length::Fixed(TAB_CLOSE_BUTTON_WIDTH))
    .height(Length::Fixed(TAB_HEIGHT))
    .style(styles::tab_strip_button(false))
    .on_press(Message::from(TabsMessage::Close(path.to_path_buf())));

    row![activate_button, close_button]
        .align_y(Alignment::Center)
        .spacing(2)
        .into()
}

fn new_repo_button<'a>(open: bool) -> Element<'a, Message> {
    button(
        row![
            icons::icon(IconName::FolderOpen, TAB_ICON_SIZE, color::TEXT_SUBTLE),
            text("New repo")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED),
            Space::with_width(Length::Fill),
            icons::icon(
                new_repo_toggle_icon(open),
                TAB_ICON_SIZE,
                color::TEXT_SUBTLE,
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .height(Length::Fixed(TAB_HEIGHT)),
    )
    .padding(Padding::from([0, theme::SP_SM]))
    .width(Length::Fill)
    .style(styles::tab_strip_button(open))
    .on_press(Message::from(repo_open::Message::NewRepoMenuToggled))
    .into()
}

fn new_repo_menu<'a>() -> Element<'a, Message> {
    container(
        column![
            menu_item(
                IconName::FolderOpen,
                "Open…",
                Message::from(repo_open::Message::OpenClicked),
            ),
            menu_item(
                IconName::GitBranch,
                "Init…",
                Message::from(repo_open::Message::InitClicked),
            ),
            menu_item(
                IconName::Cloud,
                "Clone…",
                Message::from(repo_open::Message::CloneFormToggled),
            ),
            container(Space::with_height(Length::Fixed(1.0)))
                .width(Length::Fill)
                .style(styles::hairline_divider),
            menu_item(
                IconName::GitCommit,
                "Workspace dashboard",
                Message::from(workspace::Message::DashboardToggled),
            ),
            menu_item(
                IconName::DotsVertical,
                "Terminal",
                Message::from(terminal::Message::OpenRequested),
            ),
        ]
        .spacing(2),
    )
    .padding(Padding::from([theme::SP_XS, theme::SP_XS]))
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn new_repo_toggle_icon(open: bool) -> IconName {
    if open {
        IconName::ChevronUp
    } else {
        IconName::ChevronDown
    }
}

fn menu_item<'a>(icon: IconName, label: &'a str, message: Message) -> Element<'a, Message> {
    button(
        row![
            icons::icon(icon, MENU_ICON_SIZE, color::TEXT_SUBTLE),
            text(label)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED),
            Space::with_width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .height(Length::Fixed(24.0)),
    )
    .padding(Padding::from([0, theme::SP_SM]))
    .width(Length::Fill)
    .style(styles::tab_strip_button(false))
    .on_press(message)
    .into()
}

fn refresh_dot<'a>(refreshing: bool) -> Element<'a, Message> {
    if refreshing {
        container(Space::new(Length::Fixed(6.0), Length::Fixed(6.0)))
            .style(styles::solid_bar(color::ACCENT))
            .into()
    } else {
        Space::with_width(Length::Fixed(6.0)).into()
    }
}

fn path_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}
