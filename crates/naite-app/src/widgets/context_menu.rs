//! Floating context menu drawn at the cursor position.
//!
//! iced 0.13 does not expose a true overlay/popup primitive; we approximate
//! one with a full-screen `mouse_area` that catches click-outside events plus
//! padding-based absolute positioning inside that area.

use std::path::Path;

use iced::widget::{button, column, container, mouse_area, row, text, text::Wrapping, Space};
use iced::{Element, Length, Padding};
use naite_core::{
    GitHubIssueSummary, PullRequestSummary, RefKind, RefSummary, StashSummary, WorktreeDiffKind,
    WorktreeDiffTarget, WorktreeSummary,
};

use crate::features::{
    branch_create, branch_manage, checkout, cherry_pick, discard, file_inspect, github_issue,
    history, pull_request, rebase, repo_open, reset, revert, stage, stash, tag, terminal, worktree,
};
use crate::state::{ContextMenuKind, ContextMenuState};
use crate::styles;
use crate::theme::{self, color};
use crate::{BranchDeleteTarget, Message};

const MENU_WIDTH: f32 = 220.0;
/// Rough height per row used to keep the menu inside the window.
const MENU_ROW_HEIGHT: f32 = 28.0;
const MENU_PADDING: f32 = 12.0;

pub fn floating_context_menu<'a>(
    state: &'a ContextMenuState,
    window_size: iced::Size,
    force_sync_target: Option<RefSummary>,
) -> Element<'a, Message> {
    let actions = build_actions(&state.kind, force_sync_target);
    let estimated_height = MENU_PADDING * 2.0 + actions.len() as f32 * MENU_ROW_HEIGHT;
    let clamped_x = state
        .position
        .x
        .min(window_size.width - MENU_WIDTH - 8.0)
        .max(8.0);
    let clamped_y = state
        .position
        .y
        .min(window_size.height - estimated_height - 8.0)
        .max(8.0);

    let mut menu_column = column![].spacing(2).width(Length::Fixed(MENU_WIDTH));
    for action in actions {
        menu_column = menu_column.push(action);
    }

    let card = container(menu_column)
        .padding(Padding::from([MENU_PADDING as u16, MENU_PADDING as u16]))
        .width(Length::Fixed(MENU_WIDTH))
        .style(styles::inset_card);

    let positioned = column![
        Space::with_height(Length::Fixed(clamped_y)),
        row![
            Space::with_width(Length::Fixed(clamped_x)),
            card,
            Space::with_width(Length::Fill),
        ],
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    mouse_area(
        container(positioned)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::ContextMenuClosed)
    .on_right_press(Message::ContextMenuClosed)
    .into()
}

fn build_actions<'a>(
    kind: &'a ContextMenuKind,
    force_sync_target: Option<RefSummary>,
) -> Vec<Element<'a, Message>> {
    match kind {
        ContextMenuKind::Ref(ref_summary) => ref_actions(ref_summary, force_sync_target),
        ContextMenuKind::LocalBranchFolder { label, branches } => {
            local_branch_folder_actions(label, branches)
        }
        ContextMenuKind::RemoteBranchFolder { label, branches } => {
            remote_branch_folder_actions(label, branches)
        }
        ContextMenuKind::Stash(stash) => stash_actions(stash),
        ContextMenuKind::StashMenu {
            dirty,
            latest_stash,
        } => stash_menu_actions(*dirty, latest_stash.as_ref()),
        ContextMenuKind::Worktree(worktree) => worktree_actions(worktree),
        ContextMenuKind::PullRequest(pull_request) => pull_request_actions(pull_request),
        ContextMenuKind::GitHubIssue(issue) => github_issue_actions(issue),
        ContextMenuKind::Commit(commit) => commit_actions(commit),
        ContextMenuKind::WipFile(target) => wip_file_actions(target),
        ContextMenuKind::CommitFile { path } => commit_file_actions(path),
        ContextMenuKind::HunkHeader { path, hunk, kind } => hunk_actions(path, hunk, *kind),
        ContextMenuKind::RecentRepo(path) => recent_repo_actions(path),
    }
}

fn ref_actions<'a>(
    ref_summary: &'a RefSummary,
    force_sync_target: Option<RefSummary>,
) -> Vec<Element<'a, Message>> {
    let mut actions: Vec<Element<'a, Message>> = Vec::new();
    actions.push(label_header(&ref_summary.short_name));

    if matches!(
        ref_summary.kind,
        RefKind::LocalBranch | RefKind::RemoteBranch
    ) {
        match (&force_sync_target, ref_summary.kind) {
            (Some(target), RefKind::RemoteBranch) => actions.push(menu_button(
                "Reset matching local branch",
                styles::danger_button,
                Message::from(checkout::Message::ForceSyncRequested(target.clone())),
            )),
            _ => actions.push(menu_button(
                "Checkout",
                styles::subtle_button,
                Message::from(checkout::Message::Requested(ref_summary.clone())),
            )),
        }
        if let (Some(target), RefKind::LocalBranch) = (&force_sync_target, ref_summary.kind) {
            actions.push(menu_button(
                "Reset local to remote",
                styles::danger_button,
                Message::from(checkout::Message::ForceSyncRequested(target.clone())),
            ));
        }
    }

    if matches!(
        ref_summary.kind,
        RefKind::LocalBranch | RefKind::RemoteBranch
    ) && !ref_summary.is_head
    {
        actions.push(menu_button(
            "Merge",
            styles::subtle_button,
            Message::from(history::Message::Requested(history::Operation::Merge(
                ref_summary.clone(),
            ))),
        ));
        actions.push(menu_button(
            "Rebase",
            styles::subtle_button,
            Message::from(history::Message::Requested(history::Operation::Rebase(
                ref_summary.clone(),
            ))),
        ));
        actions.push(menu_button(
            "Interactive Rebase",
            styles::subtle_button,
            Message::from(rebase::Message::Started(ref_summary.clone())),
        ));
    }

    if ref_summary.kind == RefKind::LocalBranch {
        actions.push(menu_button(
            "Rename",
            styles::subtle_button,
            Message::from(branch_manage::Message::RenameRequested(ref_summary.clone())),
        ));
        if !ref_summary.is_head {
            actions.push(menu_button(
                "Delete",
                styles::danger_button,
                Message::from(branch_manage::Message::DeleteRequested(
                    BranchDeleteTarget::LocalBranch(ref_summary.clone()),
                )),
            ));
        }
    }

    if is_deletable_remote_branch(ref_summary) {
        actions.push(menu_button(
            "Delete remote branch",
            styles::danger_button,
            Message::from(branch_manage::Message::DeleteRequested(
                BranchDeleteTarget::RemoteBranches {
                    label: ref_summary.short_name.clone(),
                    branches: vec![ref_summary.clone()],
                },
            )),
        ));
    }

    if ref_summary.kind == RefKind::Tag {
        actions.push(menu_button(
            "Delete",
            styles::danger_button,
            Message::from(tag::Message::DeleteRequested(ref_summary.clone())),
        ));
    }

    actions.push(menu_button(
        "Copy name",
        styles::subtle_button,
        Message::CopyText(ref_summary.full_name.clone()),
    ));

    actions
}

fn is_deletable_remote_branch(ref_summary: &RefSummary) -> bool {
    if ref_summary.kind != RefKind::RemoteBranch {
        return false;
    }
    let Some(name) = ref_summary.full_name.strip_prefix("refs/remotes/") else {
        return false;
    };
    let Some((_remote, branch)) = name.split_once('/') else {
        return false;
    };
    !branch.is_empty() && branch != "HEAD"
}

fn remote_branch_folder_actions<'a>(
    label: &'a str,
    branches: &'a [RefSummary],
) -> Vec<Element<'a, Message>> {
    vec![
        label_header(label),
        menu_button(
            "Delete remote branches",
            styles::danger_button,
            Message::from(branch_manage::Message::DeleteRequested(
                BranchDeleteTarget::RemoteBranches {
                    label: label.to_string(),
                    branches: branches.to_vec(),
                },
            )),
        ),
        menu_button(
            "Copy name",
            styles::subtle_button,
            Message::CopyText(label.into()),
        ),
    ]
}

fn local_branch_folder_actions<'a>(
    label: &'a str,
    branches: &'a [RefSummary],
) -> Vec<Element<'a, Message>> {
    vec![
        label_header(label),
        menu_button(
            "Delete local branches",
            styles::danger_button,
            Message::from(branch_manage::Message::DeleteRequested(
                BranchDeleteTarget::LocalBranches {
                    label: label.to_string(),
                    branches: branches.to_vec(),
                },
            )),
        ),
        menu_button(
            "Copy name",
            styles::subtle_button,
            Message::CopyText(label.into()),
        ),
    ]
}

fn stash_menu_actions<'a>(
    dirty: bool,
    latest_stash: Option<&'a StashSummary>,
) -> Vec<Element<'a, Message>> {
    let mut actions: Vec<Element<'a, Message>> = vec![label_header("Stash")];
    if dirty {
        actions.push(menu_button(
            "Stash changes",
            styles::subtle_button,
            Message::from(stash::Message::CreateRequested),
        ));
    }
    if let Some(latest) = latest_stash {
        actions.push(menu_button(
            "Pop latest",
            styles::subtle_button,
            Message::from(stash::Message::PopRequested(latest.clone())),
        ));
        actions.push(menu_button(
            "Apply latest",
            styles::subtle_button,
            Message::from(stash::Message::ApplyRequested(latest.clone())),
        ));
    }
    actions
}

fn stash_actions<'a>(stash: &'a StashSummary) -> Vec<Element<'a, Message>> {
    vec![
        label_header(&stash.selector),
        menu_button(
            "Apply",
            styles::subtle_button,
            Message::from(stash::Message::ApplyRequested(stash.clone())),
        ),
        menu_button(
            "Pop",
            styles::subtle_button,
            Message::from(stash::Message::PopRequested(stash.clone())),
        ),
        menu_button(
            "Branch from stash",
            styles::subtle_button,
            Message::from(stash::Message::BranchRequested(stash.clone())),
        ),
        menu_button(
            "Drop",
            styles::danger_button,
            Message::from(stash::Message::DropRequested(stash.clone())),
        ),
        menu_button(
            "Copy selector",
            styles::subtle_button,
            Message::CopyText(stash.selector.clone()),
        ),
    ]
}

fn worktree_actions<'a>(summary: &'a WorktreeSummary) -> Vec<Element<'a, Message>> {
    let label = summary
        .branch
        .as_deref()
        .unwrap_or(summary.path.to_str().unwrap_or("worktree"));
    let mut actions = vec![
        label_header(label),
        menu_button(
            "Select",
            styles::subtle_button,
            Message::from(worktree::Message::Selected(summary.clone())),
        ),
        menu_button(
            "Open",
            styles::subtle_button,
            Message::from(worktree::Message::OpenRequested(summary.clone())),
        ),
        menu_button(
            "Terminal here",
            styles::subtle_button,
            Message::from(terminal::Message::SessionSelected(
                terminal::SessionSelection::Target {
                    cwd: summary.path.clone(),
                    label: label.to_string(),
                    worktree_hint: Some(label.to_string()),
                },
            )),
        ),
    ];
    if summary.locked {
        actions.push(menu_button(
            "Unlock",
            styles::subtle_button,
            Message::from(worktree::Message::UnlockRequested(summary.clone())),
        ));
    } else {
        actions.push(menu_button(
            "Lock",
            styles::subtle_button,
            Message::from(worktree::Message::LockRequested(summary.clone())),
        ));
    }
    if !summary.is_current && !summary.locked {
        actions.push(menu_button(
            "Remove",
            styles::danger_button,
            Message::from(worktree::Message::RemoveRequested(summary.clone())),
        ));
    }
    actions.push(menu_button(
        "Copy path",
        styles::subtle_button,
        Message::CopyText(summary.path.display().to_string()),
    ));
    actions
}

fn pull_request_actions<'a>(pull_request: &'a PullRequestSummary) -> Vec<Element<'a, Message>> {
    vec![
        label_header(&pull_request.title),
        menu_button(
            "Select",
            styles::subtle_button,
            Message::from(pull_request::Message::Selected(pull_request.clone())),
        ),
        menu_button(
            "Open in browser",
            styles::subtle_button,
            Message::from(pull_request::Message::OpenInBrowserRequested(
                pull_request.clone(),
            )),
        ),
        menu_button(
            "Checkout",
            styles::subtle_button,
            Message::from(pull_request::Message::CheckoutRequested(
                pull_request.clone(),
            )),
        ),
        menu_button(
            "Checkout into worktree",
            styles::subtle_button,
            Message::from(pull_request::Message::CheckoutWorktreeRequested(
                pull_request.clone(),
            )),
        ),
        menu_button(
            "Copy URL",
            styles::subtle_button,
            Message::CopyText(pull_request.url.clone()),
        ),
    ]
}

fn github_issue_actions<'a>(issue: &'a GitHubIssueSummary) -> Vec<Element<'a, Message>> {
    vec![
        label_header(&issue.title),
        menu_button(
            "Select",
            styles::subtle_button,
            Message::from(github_issue::Message::Selected(issue.clone())),
        ),
        menu_button(
            "Open in browser",
            styles::subtle_button,
            Message::from(github_issue::Message::OpenInBrowserRequested(issue.clone())),
        ),
        menu_button(
            "Copy URL",
            styles::subtle_button,
            Message::CopyText(issue.url.clone()),
        ),
    ]
}

fn commit_actions<'a>(commit: &'a naite_core::CommitSummary) -> Vec<Element<'a, Message>> {
    vec![
        label_header(&commit.short_id),
        menu_button(
            "Reword",
            styles::subtle_button,
            Message::from(history::Message::RewordRequested(commit.clone())),
        ),
        menu_button(
            "Squash into parent",
            styles::subtle_button,
            Message::from(history::Message::Requested(history::Operation::Squash(
                commit.clone(),
            ))),
        ),
        menu_button(
            "Fixup into parent",
            styles::subtle_button,
            Message::from(history::Message::Requested(history::Operation::Fixup(
                commit.clone(),
            ))),
        ),
        menu_button(
            "Edit",
            styles::subtle_button,
            Message::from(history::Message::Requested(history::Operation::Edit(
                commit.clone(),
            ))),
        ),
        menu_button(
            "Cherry-pick",
            styles::subtle_button,
            Message::from(cherry_pick::Message::Requested(commit.clone())),
        ),
        menu_button(
            "Tag here",
            styles::subtle_button,
            Message::from(tag::Message::CreateRequested(Some(commit.clone()))),
        ),
        menu_button(
            "Create branch from here",
            styles::subtle_button,
            Message::from(branch_create::Message::RequestedFromCommit(commit.clone())),
        ),
        menu_button(
            "Reset to here",
            styles::danger_button,
            Message::from(reset::Message::Requested(commit.clone())),
        ),
        menu_button(
            "Revert",
            styles::danger_button,
            Message::from(revert::Message::Requested(commit.clone())),
        ),
        menu_button(
            "Drop",
            styles::danger_button,
            Message::from(history::Message::Requested(history::Operation::Drop(
                commit.clone(),
            ))),
        ),
        menu_button(
            "Copy hash",
            styles::subtle_button,
            Message::CopyText(commit.id.clone()),
        ),
        menu_button(
            "Copy summary",
            styles::subtle_button,
            Message::CopyText(commit.summary.clone()),
        ),
    ]
}

fn wip_file_actions<'a>(target: &'a WorktreeDiffTarget) -> Vec<Element<'a, Message>> {
    let mut actions: Vec<Element<'a, Message>> = vec![label_header(&target.path)];
    match target.kind {
        WorktreeDiffKind::Staged => {
            actions.push(menu_button(
                "Unstage",
                styles::subtle_button,
                Message::from(stage::Message::UnstageStatusPath(target.path.clone())),
            ));
        }
        WorktreeDiffKind::Unstaged => {
            actions.push(menu_button(
                "Stage",
                styles::subtle_button,
                Message::from(stage::Message::StatusPath(target.path.clone())),
            ));
            actions.push(menu_button(
                "Discard",
                styles::danger_button,
                Message::from(discard::Message::FileRequested(target.clone())),
            ));
        }
        WorktreeDiffKind::Untracked => {
            actions.push(menu_button(
                "Stage",
                styles::subtle_button,
                Message::from(stage::Message::StatusPath(target.path.clone())),
            ));
            actions.push(menu_button(
                "Discard",
                styles::danger_button,
                Message::from(discard::Message::FileRequested(target.clone())),
            ));
        }
        WorktreeDiffKind::Conflict => {}
    }
    actions.push(menu_button(
        "Show history",
        styles::subtle_button,
        Message::from(file_inspect::Message::HistoryRequested(target.path.clone())),
    ));
    actions.push(menu_button(
        "Show blame",
        styles::subtle_button,
        Message::from(file_inspect::Message::BlameRequested(target.path.clone())),
    ));
    actions.push(menu_button(
        "Copy path",
        styles::subtle_button,
        Message::CopyText(target.path.clone()),
    ));
    actions
}

fn commit_file_actions<'a>(path: &'a str) -> Vec<Element<'a, Message>> {
    vec![
        label_header(path),
        menu_button(
            "Show history",
            styles::subtle_button,
            Message::from(file_inspect::Message::HistoryRequested(path.to_string())),
        ),
        menu_button(
            "Show blame",
            styles::subtle_button,
            Message::from(file_inspect::Message::BlameRequested(path.to_string())),
        ),
        menu_button(
            "Copy path",
            styles::subtle_button,
            Message::CopyText(path.to_string()),
        ),
    ]
}

fn hunk_actions<'a>(
    path: &'a str,
    hunk: &'a naite_core::Hunk,
    kind: WorktreeDiffKind,
) -> Vec<Element<'a, Message>> {
    let mut actions: Vec<Element<'a, Message>> = vec![label_header(&hunk.header)];
    match kind {
        WorktreeDiffKind::Staged => {
            actions.push(menu_button(
                "Unstage hunk",
                styles::subtle_button,
                Message::from(stage::Message::UnstageHunkRequested {
                    path: path.to_string(),
                    hunk: hunk.clone(),
                }),
            ));
        }
        WorktreeDiffKind::Unstaged => {
            actions.push(menu_button(
                "Stage hunk",
                styles::subtle_button,
                Message::from(stage::Message::HunkRequested {
                    path: path.to_string(),
                    hunk: hunk.clone(),
                }),
            ));
            actions.push(menu_button(
                "Discard hunk",
                styles::danger_button,
                Message::from(discard::Message::HunkRequested {
                    path: path.to_string(),
                    hunk: hunk.clone(),
                }),
            ));
        }
        WorktreeDiffKind::Untracked | WorktreeDiffKind::Conflict => {}
    }
    actions
}

fn recent_repo_actions<'a>(path: &'a Path) -> Vec<Element<'a, Message>> {
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string());
    vec![
        label_header(&label),
        menu_button(
            "Open",
            styles::subtle_button,
            Message::from(repo_open::Message::OpenRecent(path.to_path_buf())),
        ),
        menu_button(
            "Toggle favorite",
            styles::subtle_button,
            Message::from(repo_open::Message::ToggleFavorite(path.to_path_buf())),
        ),
        menu_button(
            "Remove from recent",
            styles::subtle_button,
            Message::from(repo_open::Message::RemoveRecent(path.to_path_buf())),
        ),
        menu_button(
            "Copy path",
            styles::subtle_button,
            Message::CopyText(path.display().to_string()),
        ),
    ]
}

fn label_header<'a>(label: &str) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
    )
    .padding(Padding::from([2, 6]))
    .into()
}

fn menu_button<'a>(
    label: &'a str,
    style: fn(&iced::Theme, button::Status) -> button::Style,
    on_press: Message,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([4, 8]))
    .width(Length::Fill)
    .style(style)
    .on_press(on_press)
    .into()
}
