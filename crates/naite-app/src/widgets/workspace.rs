//! Local workspace dashboard for multi-repo status and actions.

use iced::widget::{button, column, container, row, scrollable, text, text::Wrapping, Space};
use iced::{Alignment, Element, Length, Padding};
use naite_core::WorkspaceRepoSummary;

use crate::features::workspace;
use crate::state::{PreferencesState, RepositoryTabsState, WorkspaceState};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::common::{format_relative_time, section_header};
use crate::icons::IconName;

pub fn workspace_dashboard<'a>(
    workspace_state: &'a WorkspaceState,
    tabs: &'a RepositoryTabsState,
    loading: bool,
    preferences: &'a PreferencesState,
) -> Element<'a, Message> {
    let dirty_count = workspace_state
        .summaries
        .iter()
        .filter(|summary| summary.dirty || summary.dirty_worktree_count > 0)
        .count();
    let failure_count = workspace_state
        .summaries
        .iter()
        .filter(|summary| summary.error.is_some())
        .count();

    let mut body = column![
        row![
            section_header("Workspace", IconName::FolderOpen),
            Space::with_width(Length::Fill),
            action_button(
                "Refresh",
                !workspace_state.loading,
                workspace::Message::RefreshRequested,
            ),
            action_button(
                "Fetch all",
                !loading && !workspace_state.summaries.is_empty(),
                workspace::Message::FetchAllRequested,
            ),
            action_button(
                "Pull all",
                !loading && !workspace_state.summaries.is_empty(),
                workspace::Message::PullAllRequested,
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        container(
            row![
                metric("Repos", workspace_state.summaries.len()),
                metric("Dirty", dirty_count),
                metric("Errors", failure_count),
                metric("Tabs", tabs.open.len()),
            ]
            .spacing(theme::SP_MD)
        )
        .padding(Padding::from([0, theme::SP_LG])),
    ]
    .spacing(theme::SP_SM);

    if workspace_state.loading {
        body = body.push(inset("Refreshing workspace..."));
    }

    if let Some(error) = &workspace_state.error {
        body = body.push(inset(error));
    }

    if workspace_state.summaries.is_empty() && !workspace_state.loading {
        body = body.push(inset("No local workspace repositories yet."));
    } else {
        for summary in &workspace_state.summaries {
            body = body.push(repo_row(
                summary,
                tabs.active.as_ref() == Some(&summary.path),
                preferences.display.show_workspace_details,
                preferences.row_padding(),
            ));
        }
    }

    container(
        scrollable(body.padding(theme::SP_LG).spacing(theme::SP_SM))
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(styles::bg_panel)
    .into()
}

fn repo_row<'a>(
    summary: &'a WorkspaceRepoSummary,
    active: bool,
    show_details: bool,
    row_padding: u16,
) -> Element<'a, Message> {
    let branch = summary.current_branch.as_deref().unwrap_or("detached");
    let dirty = if summary.dirty { "dirty" } else { "clean" };
    let sync = match (summary.ahead, summary.behind) {
        (0, 0) => "up to date".into(),
        (ahead, 0) => format!("ahead {ahead}"),
        (0, behind) => format!("behind {behind}"),
        (ahead, behind) => format!("ahead {ahead} / behind {behind}"),
    };
    let fetch = summary
        .last_fetch_seconds
        .map(format_relative_time)
        .unwrap_or_else(|| "never fetched".into());
    let remote = summary.remote.as_deref().unwrap_or("no origin");

    let status_color = if summary.error.is_some() {
        color::DANGER
    } else if summary.dirty || summary.dirty_worktree_count > 0 {
        color::WARNING
    } else {
        color::SUCCESS
    };

    let mut details = column![row![
        text(summary.name.clone())
            .size(theme::FS_BASE)
            .font(if active {
                theme::font_semibold()
            } else {
                theme::font_regular()
            })
            .wrapping(Wrapping::None)
            .color(color::TEXT),
        text(format!("{branch} · {dirty} · {sync}"))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(status_color),
    ]
    .spacing(theme::SP_MD)
    .align_y(Alignment::Center),]
    .spacing(2)
    .width(Length::Fill);
    if show_details {
        details = details
            .push(
                text(format!(
                    "{} · {} worktree(s), {} dirty · last fetch {}",
                    remote, summary.worktree_count, summary.dirty_worktree_count, fetch
                ))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE),
            )
            .push(
                text(summary.path.display().to_string())
                    .size(theme::FS_XS)
                    .font(theme::font_regular())
                    .wrapping(Wrapping::None)
                    .color(color::TEXT_SUBTLE),
            );
    }
    if let Some(error) = &summary.error {
        details = details.push(
            text(error.clone())
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::DANGER),
        );
    }

    container(
        row![
            details,
            action_button(
                "Open",
                summary.error.is_none(),
                workspace::Message::OpenRepo(summary.path.clone()),
            ),
            action_button(
                "Locate",
                summary.error.is_none(),
                workspace::Message::LocateRepo(summary.path.clone()),
            ),
            action_button(
                "Remove",
                true,
                workspace::Message::RemoveRepo(summary.path.clone()),
            ),
        ]
        .spacing(theme::SP_MD)
        .align_y(Alignment::Center),
    )
    .padding(row_padding)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn metric<'a>(label: &'a str, value: usize) -> Element<'a, Message> {
    container(
        row![
            text(label)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE),
            text(value.to_string())
                .size(theme::FS_SM)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None)
                .color(color::TEXT),
        ]
        .spacing(theme::SP_XS),
    )
    .padding(Padding::from([4, 8]))
    .style(styles::pill_chip)
    .into()
}

fn action_button<'a>(
    label: &'static str,
    enabled: bool,
    message: workspace::Message,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([4, 8]))
    .style(styles::subtle_button)
    .on_press_maybe(enabled.then_some(Message::from(message)))
    .into()
}

fn inset<'a>(value: &'a str) -> Element<'a, Message> {
    container(
        text(value)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}
