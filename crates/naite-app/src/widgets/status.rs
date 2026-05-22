//! Working-tree status: overview, WIP diff panel, and grouped file sections.

use iced::widget::{button, column, container, mouse_area, row, text, text::Wrapping, Space};
use iced::{Alignment, Color, Element, Length, Padding};
use naite_core::{
    ConflictSide, GitOperationState, StatusEntry, StatusKind, WorktreeDiffKind, WorktreeDiffTarget,
    WorktreeStatusDetail,
};

use crate::features::{discard, history, stage};
use crate::state::ContextMenuKind;
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::common::{
    action_button, danger_action_button, status_summary_detail, status_summary_title,
};
use super::detail_pane::{diff_content, DiffContentProps};

pub(super) fn status_overview<'a>(
    status_detail: &WorktreeStatusDetail,
    operation_state: GitOperationState,
    actions_disabled: bool,
) -> Element<'a, Message> {
    let has_conflicts = !status_detail.conflicted.is_empty();
    let can_stage_all = !actions_disabled
        && !has_conflicts
        && (!status_detail.unstaged.is_empty()
            || !status_detail.untracked.is_empty()
            || !status_detail.submodules.is_empty());
    let can_unstage_all = !actions_disabled && !has_conflicts && !status_detail.staged.is_empty();

    let mut controls = row![
        action_button(
            "Stage all",
            can_stage_all,
            Message::from(stage::Message::All),
        ),
        action_button(
            "Unstage all",
            can_unstage_all,
            Message::from(stage::Message::UnstageAll),
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    if operation_state.merge_in_progress {
        controls = controls.push(action_button(
            "Abort merge",
            !actions_disabled,
            Message::from(history::Message::Requested(history::Operation::AbortMerge)),
        ));
    }
    if operation_state.rebase_in_progress {
        controls = controls
            .push(action_button(
                "Continue",
                !actions_disabled && status_detail.conflicted.is_empty(),
                Message::from(history::Message::Requested(
                    history::Operation::ContinueRebase,
                )),
            ))
            .push(action_button(
                "Abort rebase",
                !actions_disabled,
                Message::from(history::Message::Requested(history::Operation::AbortRebase)),
            ));
    }

    column![
        row![
            text("WORKING TREE")
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None),
            Space::with_width(Length::Fill),
            controls,
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        Space::with_height(2),
        text(status_summary_title(status_detail))
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT),
        text(status_summary_detail(status_detail))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_MUTED),
    ]
    .spacing(2)
    .into()
}

pub(super) fn wip_diff_panel<'a>(diff_props: DiffContentProps<'a>) -> Element<'a, Message> {
    column![
        text("SELECTED FILE DIFF")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
        diff_content(diff_props),
    ]
    .width(Length::Fill)
    .spacing(theme::SP_SM)
    .into()
}

pub(super) struct StatusSectionProps<'a> {
    pub label: &'a str,
    pub entries: &'a [StatusEntry],
    pub accent: Color,
    pub action: Option<StatusAction>,
    pub discardable: bool,
    pub actions_disabled: bool,
    pub diff_kind: WorktreeDiffKind,
    pub selected_wip_file: Option<&'a WorktreeDiffTarget>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum StatusAction {
    Stage,
    Unstage,
}

pub(super) fn status_section<'a>(props: StatusSectionProps<'a>) -> Element<'a, Message> {
    let StatusSectionProps {
        label,
        entries,
        accent,
        action,
        discardable,
        actions_disabled,
        diff_kind,
        selected_wip_file,
    } = props;

    let mut rows = column![row![
        text(label.to_uppercase())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(accent),
        Space::with_width(Length::Fill),
        text(entries.len().to_string())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
    ]
    .align_y(Alignment::Center)]
    .spacing(theme::SP_SM);

    if entries.is_empty() {
        rows = rows.push(
            text("None")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_SUBTLE),
        );
    } else {
        for entry in entries {
            let selected = selected_wip_file
                .is_some_and(|target| target.kind == diff_kind && target.path == entry.path);
            rows = rows.push(status_entry_row(
                entry,
                accent,
                action,
                discardable,
                actions_disabled,
                diff_kind,
                selected,
            ));
        }
    }

    container(rows)
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::inset_card)
        .into()
}

fn status_entry_row<'a>(
    entry: &'a StatusEntry,
    accent: Color,
    action: Option<StatusAction>,
    discardable: bool,
    actions_disabled: bool,
    diff_kind: WorktreeDiffKind,
    selected: bool,
) -> Element<'a, Message> {
    let detail: Element<'a, Message> = match entry.old_path.as_deref() {
        Some(old_path) => text(format!("from {old_path}"))
            .size(theme::FS_XS)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE)
            .into(),
        None => Space::new(0.0, 0.0).into(),
    };

    let is_conflict = diff_kind == WorktreeDiffKind::Conflict;
    let selectable_enabled = action.is_some() || is_conflict;
    let action: Element<'a, Message> = match action {
        Some(StatusAction::Stage) => action_button(
            "Stage",
            !actions_disabled,
            Message::from(stage::Message::StatusPath(entry.path.clone())),
        ),
        Some(StatusAction::Unstage) => action_button(
            "Unstage",
            !actions_disabled,
            Message::from(stage::Message::UnstageStatusPath(entry.path.clone())),
        ),
        None => Space::new(0.0, 0.0).into(),
    };
    let discard: Element<'a, Message> = if discardable {
        danger_action_button(
            "Discard",
            !actions_disabled,
            Message::from(discard::Message::FileRequested(WorktreeDiffTarget {
                kind: diff_kind,
                path: entry.path.clone(),
            })),
        )
    } else {
        Space::new(0.0, 0.0).into()
    };
    let conflict_actions: Element<'a, Message> = if is_conflict {
        row![
            action_button(
                "Ours",
                !actions_disabled,
                Message::from(history::Message::Requested(
                    history::Operation::ResolveWithSide {
                        path: entry.path.clone(),
                        side: ConflictSide::Ours,
                    },
                )),
            ),
            action_button(
                "Theirs",
                !actions_disabled,
                Message::from(history::Message::Requested(
                    history::Operation::ResolveWithSide {
                        path: entry.path.clone(),
                        side: ConflictSide::Theirs,
                    },
                )),
            ),
            action_button(
                "Stage",
                !actions_disabled,
                Message::from(history::Message::Requested(
                    history::Operation::MarkResolved(entry.path.clone())
                )),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .into()
    } else {
        Space::new(0.0, 0.0).into()
    };

    let content = row![
        container(
            text(status_kind_label(entry.status))
                .size(theme::FS_XS)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None)
                .color(accent)
        )
        .padding(Padding::from([2, 7]))
        .style(styles::status_badge(accent)),
        column![
            text(entry.path.clone())
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(if selected {
                    color::TEXT
                } else {
                    color::TEXT_MUTED
                }),
            detail,
        ]
        .spacing(1)
        .width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let selectable: Element<'a, Message> = if selectable_enabled {
        button(content)
            .padding(Padding::from([5, 8]))
            .width(Length::Fill)
            .style(styles::commit_row_button(selected))
            .on_press(Message::WipStatusPathSelected(WorktreeDiffTarget {
                kind: diff_kind,
                path: entry.path.clone(),
            }))
            .into()
    } else {
        container(content)
            .padding(Padding::from([5, 8]))
            .width(Length::Fill)
            .style(styles::status_row_container(selected))
            .into()
    };

    let target = WorktreeDiffTarget {
        kind: diff_kind,
        path: entry.path.clone(),
    };
    let selectable = mouse_area(selectable)
        .on_right_press(Message::ContextMenuOpened(ContextMenuKind::WipFile(target)));

    row![selectable, action, discard, conflict_actions]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .into()
}

fn status_kind_label(status: StatusKind) -> &'static str {
    match status {
        StatusKind::Added => "Added",
        StatusKind::Modified => "Modified",
        StatusKind::Deleted => "Deleted",
        StatusKind::Renamed => "Renamed",
        StatusKind::Copied => "Copied",
        StatusKind::TypeChanged => "Type changed",
        StatusKind::Untracked => "New",
        StatusKind::Ignored => "Ignored",
        StatusKind::Submodule => "Submodule",
        StatusKind::Unmerged { .. } => "Conflict",
    }
}
