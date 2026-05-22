//! Confirmation modals: force-checkout, branch delete, discard, and stash prompts.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding};
use naite_core::{DiffLine, WorktreeDiffKind};

use crate::features::rebase::RebaseApplyMode;
use crate::features::{
    branch_manage, checkout, discard, history, push, rebase, stash, tag, worktree,
};
use crate::styles;
use crate::theme::{self, color};
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

    container(
        row![
            column![
                text(format!(
                    "Worktree has local changes before checkout {}",
                    prompt.target.short_name
                ))
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .color(color::TEXT),
                text(detail)
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(checkout::Message::Cancelled)),
            button(text("Force checkout").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press(Message::from(checkout::Message::Confirmed {
                    target: prompt.target.clone(),
                    force: true,
                })),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
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
        .color(color::TEXT_MUTED),
        text(status_detail)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_MUTED),
    ]
    .spacing(2);
    if let Some(sync_detail) = sync_detail {
        details = details.push(
            text(sync_detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
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
        .color(color::TEXT),
        details,
        row![
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(checkout::Message::Cancelled)),
            button(text("Reset local branch").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe((!loading).then_some(Message::from(
                    checkout::Message::ForceSyncConfirmed {
                        target: prompt.target.clone(),
                    },
                ))),
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
        text(format!("Force push {}", prompt.branch))
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .color(color::TEXT),
        column![
            text(format!("Upstream: {}", prompt.upstream))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
            text(format!("New remote tip: {}", prompt.head_short_id))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
            text("Uses git push --force-with-lease and will refuse if the remote moved.")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
        ]
        .spacing(2),
        row![
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(push::Message::ForceWithLeaseCancelled)),
            button(text("Force push").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(push::Message::ForceWithLeaseConfirmed))
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
    let can_delete = !loading && (linked_count == 0 || prompt.delete_linked_worktrees);

    column![
        column![
            text(title)
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .color(color::TEXT),
            text(detail)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
        ]
        .spacing(2),
        row![
            maybe_matching_local_toggle_button(
                matching_local_toggle,
                prompt.delete_matching_local_branches
            ),
            maybe_linked_worktree_toggle_button(
                linked_worktree_toggle,
                prompt.delete_linked_worktrees
            ),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(branch_manage::Message::DeleteCancelled)),
            button(text("Delete").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(
                    can_delete.then_some(Message::from(branch_manage::Message::DeleteConfirmed))
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
        button(text(label).size(theme::FS_SM))
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
        button(text(label).size(theme::FS_SM))
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

    container(
        row![
            column![
                text(title)
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(detail)
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(discard::Message::Cancelled)),
            button(text("Discard").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe((!loading).then_some(Message::from(discard::Message::Confirmed))),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
    .into()
}
pub fn history_prompt<'a>(prompt: &'a HistoryPrompt, loading: bool) -> Element<'a, Message> {
    container(
        row![
            column![
                text(prompt.operation.title())
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(prompt.operation.detail())
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(history::Message::Cancelled)),
            button(text(prompt.operation.button_label()).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe((!loading).then_some(Message::from(history::Message::Confirmed))),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
    .into()
}

pub fn rebase_prompt<'a>(prompt: &'a RebasePrompt, loading: bool) -> Element<'a, Message> {
    let confirm_label = match prompt.apply_mode {
        RebaseApplyMode::RebaseOnly => "Apply",
        RebaseApplyMode::RebaseThenForcePush => "Apply rebase",
    };
    container(
        row![
            column![
                text(prompt.title.clone())
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(prompt.detail.clone())
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
                container(
                    text(prompt.todo_preview.clone())
                        .size(theme::FS_SM)
                        .font(theme::font_code())
                        .color(color::TEXT_MUTED)
                )
                .padding(Padding::from([4, 8]))
                .style(styles::inset_card),
            ]
            .spacing(theme::SP_SM),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(rebase::Message::ApplyCancelled)),
            button(text(confirm_label).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(rebase::Message::ApplyConfirmed))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
    .into()
}

pub fn tag_delete_prompt<'a>(prompt: &'a TagDeletePrompt, loading: bool) -> Element<'a, Message> {
    container(
        row![
            column![
                text(format!("Delete tag {}", prompt.target.short_name))
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text("Runs git tag --delete for the selected tag.")
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(tag::Message::DeleteCancelled)),
            button(text("Delete").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe((!loading).then_some(Message::from(tag::Message::DeleteConfirmed))),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
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

    container(
        row![
            column![
                text(format!("{action} {}", prompt.checkpoint.label))
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(format!("Runs git reset --hard {short}."))
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(history::Message::UndoCancelled)),
            button(text(action).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(history::Message::UndoConfirmed))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
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

    container(
        row![
            column![
                text(format!("Remove worktree {}", prompt.target.path.display()))
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(format!("Branch: {branch}. Runs git worktree remove."))
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text(delete_label).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(worktree::Message::RemoveDeleteBranchToggled(
                    !prompt.delete_branch
                ))),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(worktree::Message::RemoveCancelled)),
            button(text("Remove").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(worktree::Message::RemoveConfirmed))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
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

    container(
        row![
            column![
                text(title)
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(detail)
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(stash::Message::ConfirmationCancelled)),
            button(text(action_label).size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe((!loading).then_some(Message::from(stash::Message::Confirmed))),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::warning_card)
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
