//! Top toolbar: branch chip, sync status, search, action buttons.

use std::path::Path;

use iced::widget::{button, container, row, svg, text, text::Wrapping, text_input, tooltip, Space};
use iced::{Alignment, Element, Length, Padding};
use naite_core::{BranchSyncStatus, StashSummary, WorktreeStatusDetail};

use crate::features::{branch_create, fetch, pull, release_prep, repo_open};
use crate::icons::{self, IconName};
use crate::state::ContextMenuKind;
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

pub const TOOLBAR_HEIGHT: f32 = 44.0;
const TOOLBAR_LOGO_SIZE: f32 = 28.0;
const TOOLBAR_LOGO_SVG: &[u8] = include_bytes!("../../assets/toolbar-logo.svg");

pub struct ToolbarProps<'a> {
    pub repo_path: Option<&'a Path>,
    pub head_branch: Option<&'a str>,
    pub sync_status: &'a BranchSyncStatus,
    pub status_detail: &'a WorktreeStatusDetail,
    pub stashes: &'a [StashSummary],
    pub transient_status: Option<&'a str>,
    pub loading: bool,
    pub search_query: &'a str,
    pub visible_count: usize,
    pub total_count: usize,
    pub search_input_id: &'a text_input::Id,
    pub window_width: f32,
}

pub fn toolbar<'a>(props: ToolbarProps<'a>) -> Element<'a, Message> {
    let ToolbarProps {
        repo_path,
        head_branch,
        sync_status,
        status_detail,
        stashes,
        transient_status,
        loading,
        search_query,
        visible_count,
        total_count,
        search_input_id,
        window_width,
    } = props;
    let action_mode = toolbar_action_mode(window_width);

    let title = svg(svg::Handle::from_memory(TOOLBAR_LOGO_SVG))
        .width(Length::Fixed(TOOLBAR_LOGO_SIZE))
        .height(Length::Fixed(TOOLBAR_LOGO_SIZE));

    let branch_chip: Element<'a, Message> = match (repo_path, head_branch) {
        (Some(_), Some(name)) => {
            let name = compact_branch_name(name, action_mode);
            container(
                row![
                    icons::icon(IconName::GitBranch, 13, color::SUCCESS),
                    text(name)
                        .size(theme::FS_SM)
                        .font(theme::font_regular())
                        .wrapping(Wrapping::None)
                        .color(color::TEXT),
                ]
                .align_y(Alignment::Center)
                .spacing(6),
            )
            .padding(Padding::from([3, 10]))
            .style(styles::pill_chip)
            .into()
        }
        (Some(_), None) => container(
            text("HEAD detached")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED),
        )
        .padding(Padding::from([3, 10]))
        .style(styles::pill_chip)
        .into(),
        _ => Space::new(0.0, 0.0).into(),
    };

    let search = text_input("Filter commits...", search_query)
        .id(search_input_id.clone())
        .on_input(Message::SearchChanged)
        .padding(Padding::from([5, 10]))
        .size(theme::FS_SM)
        .width(Length::Fixed(toolbar_search_width(window_width)));

    let match_status: Element<'a, Message> = if total_count > 0 {
        text(format!("{visible_count} of {total_count}"))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE)
            .into()
    } else {
        Space::new(0.0, 0.0).into()
    };

    let status: Element<'a, Message> = if let Some(message) = transient_status {
        container(
            text(message.to_string())
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::SUCCESS),
        )
        .padding(Padding::from([3, 8]))
        .style(styles::pill_chip)
        .into()
    } else {
        Space::new(0.0, 0.0).into()
    };

    let branch_create_enabled = repo_path.is_some() && !loading;
    let branch_btn = toolbar_action_button(
        IconName::GitBranch,
        "Branch",
        action_mode,
        branch_create_enabled.then_some(Message::from(branch_create::Message::Requested)),
    );

    let release_promotion_enabled = repo_path.is_some() && !loading && !status_detail.is_dirty();
    let release_promotion_btn = toolbar_action_button(
        IconName::GitMerge,
        "Promotion",
        action_mode,
        release_promotion_enabled.then_some(Message::from(release_prep::Message::Requested)),
    );

    let stash_can_create = status_detail.is_dirty() && status_detail.conflicted.is_empty();
    let stash_menu_enabled =
        repo_path.is_some() && !loading && (stash_can_create || !stashes.is_empty());
    let stash_btn = toolbar_action_button(
        IconName::GitCommit,
        "Stash",
        action_mode,
        stash_menu_enabled.then(|| {
            Message::ContextMenuOpened(ContextMenuKind::StashMenu {
                dirty: stash_can_create,
                latest_stash: stashes.first().cloned(),
            })
        }),
    );

    let fetch_enabled = repo_path.is_some() && !loading && sync_status.upstream.is_some();
    let fetch_btn = toolbar_action_button(
        IconName::Cloud,
        "Fetch",
        action_mode,
        fetch_enabled.then_some(Message::from(fetch::Message::Requested(
            fetch::FetchScope::CurrentRemote,
        ))),
    );

    let pull_enabled = repo_path.is_some() && !loading && sync_status.upstream.is_some();
    let pull_btn = toolbar_action_button(
        IconName::ChevronDown,
        "Pull",
        action_mode,
        pull_enabled.then_some(Message::from(pull::Message::Requested(
            pull::PullMode::FastForwardOnly,
        ))),
    );

    let push_enabled = repo_path.is_some() && !loading && head_branch.is_some();
    let force_push_available =
        push_enabled && sync_status.upstream.is_some() && !status_detail.is_dirty();
    let push_btn = toolbar_action_button(
        IconName::ChevronUp,
        "Push",
        action_mode,
        push_enabled.then_some(Message::ContextMenuOpened(ContextMenuKind::PushMenu {
            force_with_lease_available: force_push_available,
        })),
    );

    let open_btn = toolbar_action_button(
        IconName::FolderOpen,
        "Open",
        action_mode,
        Some(Message::from(repo_open::Message::OpenClicked)),
    );

    let display_btn = toolbar_action_button(
        IconName::Wrench,
        "Display",
        action_mode,
        Some(Message::ToggleDisplayOptions),
    );

    let shortcuts_btn = toolbar_action_button(
        IconName::DotsVertical,
        "Shortcuts",
        action_mode,
        Some(Message::ToggleShortcutOverlay),
    );

    let actions = row![
        branch_btn,
        release_promotion_btn,
        stash_btn,
        toolbar_group_divider(),
        fetch_btn,
        pull_btn,
        push_btn,
        toolbar_group_divider(),
        display_btn,
        open_btn,
        shortcuts_btn,
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    container(
        row![
            title,
            branch_chip,
            Space::with_width(Length::Fill),
            icons::icon(IconName::Search, 14, color::TEXT_SUBTLE),
            search,
            match_status,
            status,
            actions,
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(Padding::from([8, theme::SP_LG]))
    .width(Length::Fill)
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .style(styles::surface_panel)
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarActionMode {
    Text,
    IconOnly,
}

fn toolbar_action_mode(width: f32) -> ToolbarActionMode {
    if width >= 1400.0 {
        ToolbarActionMode::Text
    } else {
        ToolbarActionMode::IconOnly
    }
}

fn toolbar_search_width(width: f32) -> f32 {
    if width >= 1500.0 {
        220.0
    } else if width >= 1050.0 {
        180.0
    } else {
        150.0
    }
}

fn toolbar_action_button<'a>(
    icon: IconName,
    label: &'static str,
    mode: ToolbarActionMode,
    message: Option<Message>,
) -> Element<'a, Message> {
    let btn = match mode {
        ToolbarActionMode::IconOnly => button(icons::icon(icon, 15, color::TEXT_MUTED))
            .width(Length::Fixed(TOOLBAR_ICON_BUTTON_SIZE))
            .padding(TOOLBAR_BUTTON_PAD_ICON_ONLY)
            .style(styles::toolbar_button)
            .on_press_maybe(message),
        ToolbarActionMode::Text => {
            let content = row![
                icons::icon(icon, 14, color::TEXT_MUTED),
                text(label)
                    .size(theme::FS_SM)
                    .font(theme::font_semibold())
                    .wrapping(Wrapping::None),
            ]
            .align_y(Alignment::Center)
            .spacing(6);
            button(content)
                .padding(TOOLBAR_BUTTON_PAD_TEXT)
                .style(styles::toolbar_button)
                .on_press_maybe(message)
        }
    };

    if mode == ToolbarActionMode::IconOnly {
        tooltip(
            btn,
            container(
                text(label)
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .wrapping(Wrapping::None)
                    .color(color::TEXT),
            )
            .padding(Padding::from([4, 8]))
            .style(styles::inset_card),
            tooltip::Position::Bottom,
        )
        .into()
    } else {
        btn.into()
    }
}

fn toolbar_group_divider<'a>() -> Element<'a, Message> {
    container(Space::new(
        Length::Fixed(1.0),
        Length::Fixed(TOOLBAR_DIVIDER_HEIGHT),
    ))
    .style(styles::solid_bar(color::with_alpha(color::BORDER, 0.7)))
    .into()
}

const TOOLBAR_DIVIDER_HEIGHT: f32 = 18.0;
const TOOLBAR_ICON_BUTTON_SIZE: f32 = 34.0;
const TOOLBAR_BUTTON_PAD_ICON_ONLY: Padding = Padding {
    top: 5.0,
    right: 8.0,
    bottom: 5.0,
    left: 8.0,
};
const TOOLBAR_BUTTON_PAD_TEXT: Padding = Padding {
    top: 5.0,
    right: 10.0,
    bottom: 5.0,
    left: 10.0,
};

fn compact_branch_name(name: &str, mode: ToolbarActionMode) -> String {
    let max_chars = match mode {
        ToolbarActionMode::Text => 26,
        ToolbarActionMode::IconOnly => 18,
    };
    compact_text(name, max_chars)
}

fn compact_text(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let head: String = value.chars().take(max_chars - 3).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_action_mode_steps_down_at_width_breakpoint() {
        assert_eq!(toolbar_action_mode(1600.0), ToolbarActionMode::Text);
        assert_eq!(toolbar_action_mode(1400.0), ToolbarActionMode::Text);
        assert_eq!(toolbar_action_mode(1300.0), ToolbarActionMode::IconOnly);
        assert_eq!(toolbar_action_mode(900.0), ToolbarActionMode::IconOnly);
    }

    #[test]
    fn compact_branch_name_uses_ascii_truncation() {
        assert_eq!(
            compact_branch_name("feature/select-prd-next-task", ToolbarActionMode::Text),
            "feature/select-prd-next..."
        );
    }
}
