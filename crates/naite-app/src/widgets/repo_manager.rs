//! Recent-repositories, open / init / clone, and favorite controls.

use std::path::Path;

use iced::widget::{
    button, column, container, mouse_area, row, text, text::Wrapping, text_input, tooltip, Space,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::features::repo_open;
use crate::icons::{self, IconName};
use crate::state::{ContextMenuKind, RepositoryCatalog, SidebarSection, SidebarState};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::common::section_header;

const REPO_ROW_ICON_SIZE: u16 = 11;
const REPO_ROW_ACTION_SIZE: f32 = 24.0;
const REPO_SECTION_CHEVRON_ICON_SIZE: u16 = 12;

pub(super) fn repository_manager<'a>(
    repo_path: Option<&'a Path>,
    catalog: &'a RepositoryCatalog,
    sidebar_state: &'a SidebarState,
    clone_url: &'a str,
    clone_open: bool,
) -> Element<'a, Message> {
    let show_clone_form = clone_open || !clone_url.trim().is_empty();
    let mut col = column![].spacing(theme::SP_SM);

    if show_clone_form {
        col = col.push(
            container(
                row![
                    text_input("Clone URL", clone_url)
                        .on_input(|url| Message::from(repo_open::Message::CloneUrlChanged(url)))
                        .padding(Padding::from([5, 8]))
                        .size(theme::FS_SM)
                        .width(Length::Fill),
                    repo_action_button("Start", Message::from(repo_open::Message::CloneClicked)),
                ]
                .spacing(theme::SP_SM)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([0, theme::SP_LG])),
        );
    }

    if let Some(path) = repo_path {
        let favorite = catalog.is_favorite(path);
        let favorite_tooltip = if favorite {
            "Remove favorite"
        } else {
            "Add favorite"
        };
        col = col.push(
            container(
                row![
                    text(path_label(path))
                        .size(theme::FS_XS)
                        .font(theme::font_regular())
                        .wrapping(Wrapping::None)
                        .color(color::TEXT_SUBTLE),
                    Space::with_width(Length::Fill),
                    repo_icon_button(
                        favorite_icon(favorite),
                        favorite_tint(favorite),
                        favorite_tooltip,
                        Message::from(repo_open::Message::ToggleFavorite(path.to_path_buf())),
                    ),
                ]
                .align_y(Alignment::Center)
                .spacing(theme::SP_SM),
            )
            .padding(Padding::from([0, theme::SP_LG])),
        );
    }

    let favorites: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| entry.favorite)
        .collect();
    if !favorites.is_empty() {
        col = col.push(repo_entries("Favorites", favorites, true));
    }

    col = col.push(recent_entries(
        catalog.entries.iter().collect(),
        sidebar_state.is_expanded(SidebarSection::RecentRepositories),
    ));

    col.into()
}

fn repo_entries<'a>(
    label: &'a str,
    entries: Vec<&'a crate::state::RepositoryEntry>,
    favorite_list: bool,
) -> Element<'a, Message> {
    let mut col = column![section_header(label, IconName::FolderOpen)].spacing(0);
    for entry in entries {
        col = col.push(repo_entry_row(entry, favorite_list));
    }
    col.into()
}

fn recent_entries<'a>(
    entries: Vec<&'a crate::state::RepositoryEntry>,
    expanded: bool,
) -> Element<'a, Message> {
    let mut col = column![collapsible_recent_header(entries.len(), expanded)].spacing(0);
    if !expanded {
        return col.into();
    }

    if entries.is_empty() {
        col = col.push(
            container(
                text("No recent repositories.")
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_SUBTLE),
            )
            .padding(Padding::from([2, theme::SP_LG])),
        );
    } else {
        for entry in entries {
            col = col.push(repo_entry_row(entry, false));
        }
    }
    col.into()
}

fn collapsible_recent_header<'a>(count: usize, expanded: bool) -> Element<'a, Message> {
    let indicator = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };

    button(
        row![
            icons::icon(
                indicator,
                REPO_SECTION_CHEVRON_ICON_SIZE,
                color::TEXT_SUBTLE
            ),
            icons::icon(IconName::FolderOpen, 13, color::TEXT_SUBTLE),
            text("RECENT")
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None),
            Space::with_width(Length::Fill),
            text(count.to_string())
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_regular())
                .wrapping(Wrapping::None),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([theme::SP_SM, theme::SP_LG]))
    .width(Length::Fill)
    .style(styles::commit_row_button(false))
    .on_press(Message::SidebarSectionToggled(
        SidebarSection::RecentRepositories,
    ))
    .into()
}

fn repo_entry_row<'a>(
    entry: &'a crate::state::RepositoryEntry,
    favorite_list: bool,
) -> Element<'a, Message> {
    let open_button = repo_open_button(entry);

    let open_row = mouse_area(open_button).on_right_press(Message::ContextMenuOpened(
        ContextMenuKind::RecentRepo(entry.path.clone()),
    ));

    container(
        row![
            open_row,
            Space::with_width(Length::Fixed(theme::SP_XS as f32)),
            repo_entry_actions(entry, favorite_list),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0, theme::SP_MD]))
    .into()
}

fn repo_open_button<'a>(entry: &'a crate::state::RepositoryEntry) -> Element<'a, Message> {
    button(
        row![
            text(path_label(&entry.path))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED),
            Space::with_width(Length::Fill),
            icons::icon(
                IconName::ChevronRight,
                REPO_ROW_ICON_SIZE,
                muted_action_tint()
            ),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([5, 8]))
    .width(Length::Fill)
    .style(styles::commit_row_button(false))
    .on_press(Message::from(repo_open::Message::OpenRecent(
        entry.path.clone(),
    )))
    .into()
}

fn repo_entry_actions<'a>(
    entry: &'a crate::state::RepositoryEntry,
    favorite_list: bool,
) -> Element<'a, Message> {
    if favorite_list {
        return repo_icon_button(
            IconName::StarFilled,
            active_favorite_tint(),
            "Remove favorite",
            Message::from(repo_open::Message::RemoveFavorite(entry.path.clone())),
        );
    }

    row![
        repo_icon_button(
            favorite_icon(entry.favorite),
            favorite_tint(entry.favorite),
            if entry.favorite {
                "Remove favorite"
            } else {
                "Add favorite"
            },
            Message::from(repo_open::Message::ToggleFavorite(entry.path.clone(),)),
        ),
        repo_icon_button(
            IconName::Trash,
            muted_action_tint(),
            "Remove from recent",
            Message::from(repo_open::Message::RemoveRecent(entry.path.clone(),)),
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_XS)
    .into()
}

fn repo_icon_button<'a>(
    icon: IconName,
    tint: Color,
    label: &'static str,
    message: Message,
) -> Element<'a, Message> {
    tooltip(
        button(icons::icon(icon, REPO_ROW_ICON_SIZE, tint))
            .width(Length::Fixed(REPO_ROW_ACTION_SIZE))
            .height(Length::Fixed(REPO_ROW_ACTION_SIZE))
            .padding(Padding::from([3, 5]))
            .style(styles::commit_row_button(false))
            .on_press(message),
        container(
            text(label)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT),
        )
        .padding(Padding::from([4, 8]))
        .style(styles::inset_card),
        tooltip::Position::Right,
    )
    .into()
}

fn favorite_icon(favorite: bool) -> IconName {
    if favorite {
        IconName::StarFilled
    } else {
        IconName::Star
    }
}

fn favorite_tint(favorite: bool) -> Color {
    if favorite {
        active_favorite_tint()
    } else {
        muted_action_tint()
    }
}

fn active_favorite_tint() -> Color {
    color::with_alpha(color::WARNING, 0.80)
}

fn muted_action_tint() -> Color {
    color::with_alpha(color::TEXT_SUBTLE, 0.76)
}

fn path_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn repo_action_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(
        container(
            text(label)
                .size(theme::FS_SM)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(28.0))
    .padding(0)
    .style(styles::subtle_button)
    .on_press(message)
    .into()
}
