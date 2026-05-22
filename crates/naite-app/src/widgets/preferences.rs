use iced::widget::{button, column, container, row, text, text::Wrapping, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::state::{DensityPreference, PreferencesState};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

pub fn display_options_panel(preferences: &PreferencesState) -> Element<'_, Message> {
    container(
        column![
            header("Display options", Message::ToggleDisplayOptions),
            row![
                text("Density")
                    .size(theme::FS_SM)
                    .font(theme::font_semibold())
                    .color(color::TEXT_SUBTLE)
                    .width(Length::Fixed(96.0)),
                option_button(
                    "Comfort",
                    preferences.density == DensityPreference::Comfortable,
                    Message::DensityPreferenceChanged(DensityPreference::Comfortable),
                ),
                option_button(
                    "Compact",
                    preferences.density == DensityPreference::Compact,
                    Message::DensityPreferenceChanged(DensityPreference::Compact),
                ),
                option_button(
                    "Dense",
                    preferences.density == DensityPreference::Dense,
                    Message::DensityPreferenceChanged(DensityPreference::Dense),
                ),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM),
            toggle_row(
                "Graph author column",
                preferences.display.show_commit_author,
                Message::DisplayCommitAuthorToggled,
            ),
            toggle_row(
                "File inspection panels",
                preferences.display.show_file_inspection,
                Message::DisplayFileInspectionToggled,
            ),
            toggle_row(
                "PR metadata cards",
                preferences.display.show_pr_metadata,
                Message::DisplayPrMetadataToggled,
            ),
            toggle_row(
                "Workspace detail rows",
                preferences.display.show_workspace_details,
                Message::DisplayWorkspaceDetailsToggled,
            ),
        ]
        .spacing(theme::SP_SM),
    )
    .padding(theme::SP_LG)
    .width(Length::Fill)
    .style(styles::surface_panel)
    .into()
}

pub fn shortcut_help_overlay() -> Element<'static, Message> {
    let general_rows = [
        ("Cmd/Ctrl K", "Command palette"),
        ("?", "Shortcuts"),
        ("Cmd/Ctrl O", "Open repository"),
        ("Cmd/Ctrl `", "Open terminal"),
        ("Cmd/Ctrl F", "Search commits"),
        ("Cmd/Ctrl Shift R", "Release promotion"),
        ("J / K", "Next / previous commit"),
        ("[ / ]", "Previous / next hunk"),
        ("Enter", "Run command or open single search result"),
        ("Esc", "Close overlays, prompts, or clear selection"),
    ];

    let commit_action_rows = [
        ("R", "Reword commit"),
        ("S", "Squash into parent"),
        ("F", "Fixup into parent"),
        ("E", "Edit commit (pause rebase)"),
        ("D", "Drop commit"),
        ("T", "Tag\u{2026}"),
        ("Y", "Copy hash (yank)"),
    ];

    let mut body =
        column![header("Keyboard shortcuts", Message::ToggleShortcutOverlay)].spacing(theme::SP_SM);

    for (keys, action) in general_rows {
        body = body.push(row![
            text(keys)
                .size(theme::FS_SM)
                .font(iced::Font::MONOSPACE)
                .color(color::TEXT)
                .width(Length::Fixed(120.0)),
            text(action)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED)
                .wrapping(Wrapping::None),
        ]);
    }

    body = body.push(
        text("COMMIT ACTIONS")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
    );

    for (keys, action) in commit_action_rows {
        body = body.push(row![
            text(keys)
                .size(theme::FS_SM)
                .font(iced::Font::MONOSPACE)
                .color(color::TEXT)
                .width(Length::Fixed(120.0)),
            text(action)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED)
                .wrapping(Wrapping::None),
        ]);
    }

    container(body)
        .padding(theme::SP_LG)
        .width(Length::Fill)
        .style(styles::surface_panel)
        .into()
}

fn header<'a>(label: &'a str, close_message: Message) -> Element<'a, Message> {
    row![
        text(label.to_uppercase())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
        Space::with_width(Length::Fill),
        button(text("Close").size(theme::FS_SM))
            .padding(Padding::from([3, 8]))
            .style(styles::subtle_button)
            .on_press(close_message),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn option_button<'a>(label: &'static str, active: bool, message: Message) -> Element<'a, Message> {
    button(
        text(label)
            .size(theme::FS_SM)
            .font(if active {
                theme::font_semibold()
            } else {
                theme::font_regular()
            })
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([4, 10]))
    .style(styles::segmented_chip(active))
    .on_press(message)
    .into()
}

fn toggle_row<'a>(label: &'static str, active: bool, message: Message) -> Element<'a, Message> {
    row![
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_MUTED)
            .width(Length::Fixed(160.0)),
        option_button(if active { "Shown" } else { "Hidden" }, active, message),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into()
}
