//! Confirmation modals: force-checkout, branch delete, discard, and stash prompts.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::widget::{button as iced_button, text::Wrapping};
use iced::{Alignment, Color, Element, Length, Padding, Theme};
use naite_core::{DiffLine, RebaseAction, WorktreeDiffKind};

use crate::features::rebase::RebaseApplyMode;
use crate::features::{
    branch_manage, checkout, discard, history, push, rebase, stash, tag, worktree,
};
use crate::state::AvatarCache;
use crate::styles;
use crate::theme::{self, color};

use super::ROW_HEIGHT;
use crate::{
    BranchDeletePrompt, BranchDeleteTarget, CheckoutPrompt, DiscardPrompt, DiscardTarget,
    ForcePushPrompt, ForceSyncPrompt, HistoryPrompt, Message, RebasePrompt, StashPrompt,
    StashPromptAction, TagDeletePrompt, UndoPrompt, WorktreeRemovePrompt,
};

pub fn checkout_prompt<'a>(prompt: &'a CheckoutPrompt) -> Element<'a, Message> {
    let detail = format!(
        "Unstaged: {}  Staged: {}  Untracked: {}",
        yes_no(prompt.status.has_unstaged),
        yes_no(prompt.status.has_staged),
        yes_no(prompt.status.has_untracked)
    );

    column![
        column![
            text(format!(
                "Worktree has local changes before checkout {}",
                prompt.target.short_name
            ))
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
            text(detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(checkout::Message::Cancelled)),
            ),
            prompt_action_button(
                "Force checkout",
                styles::danger_button,
                Some(Message::from(checkout::Message::Confirmed {
                    target: prompt.target.clone(),
                    force: true,
                })),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn force_sync_prompt<'a>(prompt: &'a ForceSyncPrompt, loading: bool) -> Element<'a, Message> {
    let status_detail = format!(
        "Working tree: unstaged {}  staged {}  untracked {}",
        yes_no(prompt.status.has_unstaged),
        yes_no(prompt.status.has_staged),
        yes_no(prompt.status.has_untracked)
    );
    let sync_detail = prompt.sync_status.as_ref().map(|sync_status| {
        format!(
            "Branch state: ahead {}  behind {}",
            sync_status.ahead, sync_status.behind
        )
    });

    let mut details = column![
        text(format!(
            "Local commits and working-tree changes on {} may be discarded.",
            prompt.local_branch
        ))
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .width(Length::Fill)
        .wrapping(Wrapping::WordOrGlyph)
        .color(color::TEXT_MUTED),
        text(status_detail)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT_MUTED),
    ]
    .width(Length::Fill)
    .spacing(2);
    if let Some(sync_detail) = sync_detail {
        details = details.push(
            text(sync_detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        );
    }

    column![
        text(format!(
            "Reset local {} to {}",
            prompt.local_branch, prompt.target.short_name
        ))
        .size(theme::FS_BASE)
        .font(theme::font_semibold())
        .width(Length::Fill)
        .wrapping(Wrapping::WordOrGlyph)
        .color(color::TEXT),
        details,
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(checkout::Message::Cancelled)),
            ),
            prompt_action_button(
                "Reset local branch",
                styles::danger_button,
                (!loading).then_some(Message::from(checkout::Message::ForceSyncConfirmed {
                    target: prompt.target.clone(),
                },)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn force_push_prompt<'a>(prompt: &'a ForcePushPrompt, loading: bool) -> Element<'a, Message> {
    column![
        text(format!("Update remote {}", prompt.branch))
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        column![
            text(format!("Remote branch: {}", prompt.upstream))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
            text(format!("New tip: {}", prompt.head_short_id))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
            text("If the remote changed since the last fetch, naite will stop before updating it.")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(push::Message::ForceWithLeaseCancelled)),
            ),
            prompt_action_button(
                "Update remote",
                styles::danger_button,
                (!loading).then_some(Message::from(push::Message::ForceWithLeaseConfirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn branch_delete_prompt<'a>(
    prompt: &'a BranchDeletePrompt,
    loading: bool,
) -> Element<'a, Message> {
    let linked_count = prompt.linked_worktrees.len();
    let linked_paths = prompt
        .linked_worktrees
        .iter()
        .map(|worktree| worktree.path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let linked_detail = if linked_count > 0 {
        format!(" Linked worktree path(s): {linked_paths}.")
    } else {
        String::new()
    };
    let (title, detail, matching_local_toggle): (String, String, Option<String>) = match &prompt
        .target
    {
        BranchDeleteTarget::LocalBranch(target) => (
            format!("Delete branch {}", target.short_name),
            if linked_count > 0 {
                format!(
                    "Removes linked worktree path(s), then force-deletes the branch.{linked_detail}"
                )
            } else {
                "Uses git branch --delete --force; unmerged branches are deleted.".into()
            },
            None,
        ),
        BranchDeleteTarget::LocalBranches { branches, .. } => {
            let branch_count = branches.len();
            (
                format!("Delete local branches {}", prompt.target.label()),
                if linked_count > 0 {
                    format!(
                        "Removes linked worktree path(s), then deletes {branch_count} local branches. Linked branches are force-deleted.{linked_detail}"
                    )
                } else {
                    format!(
                        "Force-deletes {branch_count} local branches; unmerged branches are deleted."
                    )
                },
                None,
            )
        }
        BranchDeleteTarget::RemoteBranches { branches, .. } => {
            let branch_count = branches.len();
            let local_count = prompt.matching_local_branches.len();
            let detail = if branch_count == 1 {
                "Runs git push <remote> --delete for the selected remote branch.".into()
            } else {
                format!("Runs batched remote delete for {branch_count} remote branches.")
            };
            let toggle = if prompt.delete_matching_local_branches {
                format!("Delete matching local branches: yes ({local_count})")
            } else {
                format!("Delete matching local branches: no ({local_count})")
            };
            (
                format!("Delete remote branches {}", prompt.target.label()),
                detail,
                Some(toggle),
            )
        }
    };
    let linked_worktree_toggle = (linked_count > 0).then(|| {
        if prompt.delete_linked_worktrees {
            format!("Remove linked worktrees: yes ({linked_count})")
        } else {
            format!("Remove linked worktrees: no ({linked_count})")
        }
    });
    let any_dirty_linked = prompt
        .linked_worktrees
        .iter()
        .any(|worktree| worktree.dirty);
    let force_linked_worktree_toggle = if prompt.delete_linked_worktrees && any_dirty_linked {
        Some(if prompt.force_linked_worktrees {
            "Force linked worktrees: yes".to_string()
        } else {
            "Force linked worktrees: no".to_string()
        })
    } else {
        None
    };
    let can_delete = !loading && (linked_count == 0 || prompt.delete_linked_worktrees);
    let mut options = column![].spacing(theme::SP_SM).width(Length::Fill);
    if matching_local_toggle.is_some() {
        options = options.push(maybe_matching_local_toggle_button(
            matching_local_toggle,
            prompt.delete_matching_local_branches,
        ));
    }
    if linked_worktree_toggle.is_some() {
        options = options.push(maybe_linked_worktree_toggle_button(
            linked_worktree_toggle,
            prompt.delete_linked_worktrees,
        ));
    }
    if force_linked_worktree_toggle.is_some() {
        options = options.push(maybe_force_linked_worktree_toggle_button(
            force_linked_worktree_toggle,
            prompt.force_linked_worktrees,
        ));
    }

    column![
        column![
            text(title)
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT),
            text(detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        options,
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(branch_manage::Message::DeleteCancelled)),
            ),
            prompt_action_button(
                "Delete",
                styles::danger_button,
                can_delete.then_some(Message::from(branch_manage::Message::DeleteConfirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_LG)
    .width(Length::Fill)
    .into()
}

fn maybe_matching_local_toggle_button<'a>(
    label: Option<String>,
    checked: bool,
) -> Element<'a, Message> {
    if let Some(label) = label {
        button(text(label).size(theme::FS_SM).wrapping(Wrapping::None))
            .padding(Padding::from([5, 10]))
            .style(styles::subtle_button)
            .on_press(Message::from(
                branch_manage::Message::DeleteMatchingLocalBranchesToggled(!checked),
            ))
            .into()
    } else {
        Space::with_width(Length::Shrink).into()
    }
}

fn maybe_linked_worktree_toggle_button<'a>(
    label: Option<String>,
    checked: bool,
) -> Element<'a, Message> {
    if let Some(label) = label {
        button(text(label).size(theme::FS_SM).wrapping(Wrapping::None))
            .padding(Padding::from([5, 10]))
            .style(styles::subtle_button)
            .on_press(Message::from(
                branch_manage::Message::DeleteLinkedWorktreesToggled(!checked),
            ))
            .into()
    } else {
        Space::with_width(Length::Shrink).into()
    }
}

fn maybe_force_linked_worktree_toggle_button<'a>(
    label: Option<String>,
    checked: bool,
) -> Element<'a, Message> {
    if let Some(label) = label {
        button(text(label).size(theme::FS_SM).wrapping(Wrapping::None))
            .padding(Padding::from([5, 10]))
            .style(styles::subtle_button)
            .on_press(Message::from(
                branch_manage::Message::DeleteForceLinkedWorktreesToggled(!checked),
            ))
            .into()
    } else {
        Space::with_width(Length::Shrink).into()
    }
}

pub fn discard_prompt<'a>(prompt: &'a DiscardPrompt, loading: bool) -> Element<'a, Message> {
    let (title, detail) = match &prompt.target {
        DiscardTarget::File(target) => {
            let action = match target.kind {
                WorktreeDiffKind::Unstaged => "Restore tracked file from HEAD/index",
                WorktreeDiffKind::Untracked => "Delete untracked file from disk",
                WorktreeDiffKind::Staged => "Unsupported staged discard",
                WorktreeDiffKind::Conflict => "Unsupported conflict discard",
            };
            (
                format!("Discard changes in {}", target.path),
                format!("{action}. This cannot be undone by naite."),
            )
        }
        DiscardTarget::Hunk { path, hunk } => (
            format!("Discard hunk in {path}"),
            format!(
                "{}  Removed lines: {}  Added lines: {}",
                hunk.header,
                hunk.lines
                    .iter()
                    .filter(|line| matches!(line, DiffLine::Del(_)))
                    .count(),
                hunk.lines
                    .iter()
                    .filter(|line| matches!(line, DiffLine::Add(_)))
                    .count()
            ),
        ),
    };

    column![
        column![
            text(title)
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT),
            text(detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(discard::Message::Cancelled)),
            ),
            prompt_action_button(
                "Discard",
                styles::danger_button,
                (!loading).then_some(Message::from(discard::Message::Confirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}
pub fn history_prompt<'a>(prompt: &'a HistoryPrompt, loading: bool) -> Element<'a, Message> {
    column![
        column![
            text(prompt.operation.title())
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT),
            text(prompt.operation.detail())
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(history::Message::Cancelled)),
            ),
            prompt_action_button(
                prompt.operation.button_label(),
                styles::danger_button,
                (!loading).then_some(Message::from(history::Message::Confirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn rebase_prompt<'a>(
    prompt: &'a RebasePrompt,
    avatars: &'a AvatarCache,
    loading: bool,
) -> Element<'a, Message> {
    let (confirm_label, confirm_style): (
        &str,
        fn(&Theme, iced_button::Status) -> iced_button::Style,
    ) = match prompt.apply_mode {
        RebaseApplyMode::RebaseOnly => ("Apply", styles::primary_button),
        RebaseApplyMode::RebaseThenForcePush => ("Apply rebase", styles::danger_button),
        RebaseApplyMode::ReleasePromotionAuto => ("Auto promote", styles::danger_button),
    };

    let body = column![
        text(prompt.title.clone())
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        text(prompt.detail.clone())
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT_MUTED),
        rebase_prompt_preview(prompt, avatars),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill);

    column![
        scrollable(body)
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar)
            .height(Length::Fill),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(rebase::Message::ApplyCancelled)),
            ),
            prompt_action_button(
                confirm_label,
                confirm_style,
                (!loading).then_some(Message::from(rebase::Message::ApplyConfirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

/// Cap the preview list height so a long rebase plan scrolls inside the modal
/// instead of pushing the confirm/cancel buttons off-screen. ~11 rows at the
/// 34px row height + 1px separators; fits within the 900x600 min window once
/// the title/detail/buttons/padding overhead is accounted for.
const REBASE_PROMPT_PREVIEW_MAX_HEIGHT: f32 = 400.0;

fn rebase_prompt_preview<'a>(
    prompt: &'a RebasePrompt,
    avatars: &'a AvatarCache,
) -> Element<'a, Message> {
    let mut rows = column![].spacing(0);
    for (index, row) in prompt.preview_rows.iter().enumerate() {
        if index > 0 {
            rows = rows.push(rebase_prompt_row_separator());
        }
        rows = rows.push(rebase_prompt_preview_row(row, avatars));
    }

    let list = scrollable(rows)
        .direction(styles::thin_scrollbar_dir())
        .style(styles::thin_scrollbar);

    container(list)
        .padding(Padding::from([4, 0]))
        .max_height(REBASE_PROMPT_PREVIEW_MAX_HEIGHT)
        .width(Length::Fill)
        .style(styles::rebase_prompt_preview_surface)
        .into()
}

/// Hairline between preview rows so each commit reads as its own entry.
fn rebase_prompt_row_separator<'a>() -> Element<'a, Message> {
    container(Space::new(Length::Fill, Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(styles::solid_bar(color::with_alpha(color::BORDER, 0.55)))
        .into()
}

fn rebase_prompt_preview_row<'a>(
    row_data: &'a crate::RebasePromptRow,
    avatars: &'a AvatarCache,
) -> Element<'a, Message> {
    let action_tint = rebase_prompt_action_color(row_data.action);
    let action = container(
        text(action_token(row_data.action))
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(action_tint)
            .wrapping(Wrapping::None),
    )
    .center_x(Length::Fixed(56.0))
    .padding(Padding::from([3, 0]))
    .style(styles::rebase_prompt_action_chip(action_tint));

    let avatar = super::commit_list::avatar_badge(
        &row_data.author_name,
        row_data.author_avatar_url.as_deref(),
        avatars,
        color::with_alpha(color::BORDER, 0.9),
    );

    let sha = text(row_data.short_id.clone())
        .size(theme::FS_SM)
        .font(iced::Font::MONOSPACE)
        .color(color::TEXT_MUTED)
        .wrapping(Wrapping::None);

    let dropped = row_data.action == RebaseAction::Drop;
    let summary: Element<'a, Message> = container(
        text(row_data.summary.clone())
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(if dropped {
                color::with_alpha(color::TEXT, 0.55)
            } else {
                color::TEXT
            })
            .wrapping(Wrapping::None),
    )
    .width(Length::Fill)
    .clip(true)
    .into();

    container(
        row![
            action,
            avatar,
            container(sha).width(Length::Fixed(58.0)),
            summary,
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .height(Length::Fixed(ROW_HEIGHT))
    .padding(Padding::from([0, 10]))
    .width(Length::Fill)
    .style(styles::rebase_prompt_preview_row)
    .into()
}

fn action_token(action: RebaseAction) -> &'static str {
    match action {
        RebaseAction::Pick => "pick",
        RebaseAction::Reword => "reword",
        RebaseAction::Edit => "edit",
        RebaseAction::Squash => "squash",
        RebaseAction::Fixup => "fixup",
        RebaseAction::Drop => "drop",
    }
}

fn rebase_prompt_action_color(action: RebaseAction) -> Color {
    match action {
        RebaseAction::Pick => color::TEXT_MUTED,
        RebaseAction::Reword | RebaseAction::Squash => color::ACCENT,
        RebaseAction::Fixup => color::with_alpha(color::ACCENT, 0.75),
        RebaseAction::Edit => color::WARNING,
        RebaseAction::Drop => color::DANGER,
    }
}

fn prompt_action_button<'a>(
    label: &'a str,
    style: fn(&Theme, iced_button::Status) -> iced_button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(prompt_action_label(label))
        .padding(Padding::from([5, 10]))
        .style(style)
        .on_press_maybe(on_press)
        .into()
}

fn prompt_action_label<'a>(label: &'a str) -> Element<'a, Message> {
    super::modal::modal_action_label(label)
}

pub fn tag_delete_prompt<'a>(prompt: &'a TagDeletePrompt, loading: bool) -> Element<'a, Message> {
    column![
        column![
            text(format!("Delete tag {}", prompt.target.short_name))
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT),
            text("Runs git tag --delete for the selected tag.")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(tag::Message::DeleteCancelled)),
            ),
            prompt_action_button(
                "Delete",
                styles::danger_button,
                (!loading).then_some(Message::from(tag::Message::DeleteConfirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn undo_prompt<'a>(prompt: &'a UndoPrompt, loading: bool) -> Element<'a, Message> {
    let action = match prompt.action {
        crate::UndoPromptAction::Undo => "Undo",
        crate::UndoPromptAction::Redo => "Redo",
    };
    let short = prompt
        .checkpoint
        .head_id
        .chars()
        .take(7)
        .collect::<String>();

    column![
        column![
            text(format!("{action} {}", prompt.checkpoint.label))
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT),
            text(format!("Runs git reset --hard {short}."))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(history::Message::UndoCancelled)),
            ),
            prompt_action_button(
                action,
                styles::danger_button,
                (!loading).then_some(Message::from(history::Message::UndoConfirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn worktree_remove_prompt<'a>(
    prompt: &'a WorktreeRemovePrompt,
    loading: bool,
) -> Element<'a, Message> {
    let branch = prompt.target.branch.as_deref().unwrap_or("detached HEAD");
    let delete_label = if prompt.delete_branch {
        "Delete branch: yes"
    } else {
        "Delete branch: no"
    };
    let toggle_delete_branch = Message::from(worktree::Message::RemoveDeleteBranchToggled(
        !prompt.delete_branch,
    ));
    let (command_detail, action_label) = if prompt.force {
        (
            format!("Branch: {branch}. Runs git worktree remove --force."),
            "Force remove",
        )
    } else {
        (
            format!("Branch: {branch}. Runs git worktree remove."),
            "Remove",
        )
    };
    let mut details = column![
        text(format!("Remove worktree {}", prompt.target.path.display()))
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        text(command_detail)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT_MUTED),
    ]
    .width(Length::Fill)
    .spacing(2);
    if prompt.force {
        details = details.push(
            text(
                "Warning: modified and untracked files in this worktree will be permanently deleted. naite cannot undo this action.",
            )
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::DANGER),
        );
    }

    column![
        details,
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                delete_label,
                styles::subtle_button,
                Some(toggle_delete_branch),
            ),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(worktree::Message::RemoveCancelled)),
            ),
            prompt_action_button(
                action_label,
                styles::danger_button,
                (!loading).then_some(Message::from(worktree::Message::RemoveConfirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

pub fn stash_prompt<'a>(prompt: &'a StashPrompt, loading: bool) -> Element<'a, Message> {
    let (title, action_label, detail) = match prompt.action {
        StashPromptAction::Pop => (
            format!("Pop {}", prompt.stash.selector),
            "Pop",
            format!(
                "Apply and remove {} ({}) from {}.",
                prompt.stash.selector,
                prompt.stash.message,
                stash_branch_label(prompt)
            ),
        ),
        StashPromptAction::Drop => (
            format!("Drop {}", prompt.stash.selector),
            "Drop",
            format!(
                "Remove {} ({}) from the stash list. This cannot be undone by naite.",
                prompt.stash.selector, prompt.stash.message
            ),
        ),
    };

    column![
        column![
            text(title)
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT),
            text(detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .width(Length::Fill)
                .wrapping(Wrapping::WordOrGlyph)
                .color(color::TEXT_MUTED),
        ]
        .width(Length::Fill)
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            prompt_action_button(
                "Cancel",
                styles::subtle_button,
                Some(Message::from(stash::Message::ConfirmationCancelled)),
            ),
            prompt_action_button(
                action_label,
                styles::danger_button,
                (!loading).then_some(Message::from(stash::Message::Confirmed)),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

fn stash_branch_label(prompt: &StashPrompt) -> &str {
    if prompt.stash.branch.is_empty() {
        "the saved branch"
    } else {
        &prompt.stash.branch
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_action_label_uses_natural_width_for_long_labels() {
        let label = prompt_action_label("Delete linked worktrees and matching local branches");

        assert_eq!(label.as_widget().size().width, Length::Shrink);
    }
}
