//! User-input forms: commit composer, branch creation, branch rename, and stash creation.

use iced::widget::{
    button, checkbox, column, container, mouse_area, row, text, text::Wrapping, text_editor,
    text_input, Space,
};
use iced::{Alignment, Border, Element, Length, Padding};
use naite_core::WorktreeStatusDetail;

use crate::features::{
    branch_create, branch_manage, commit, history, pull_request, stash, tag, worktree,
};
use crate::state::{
    BranchCreateState, BranchManageRenameState, CommitFormState, HistoryRewordState,
    PullRequestCreateState, PullRequestWorktreeCheckoutState, StashBranchState, StashCreateState,
    TagCreateState, TagNameMode, WorktreeCreateState,
};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

pub(super) fn commit_form<'a>(
    status_detail: &WorktreeStatusDetail,
    actions_disabled: bool,
    state: &'a CommitFormState,
    head_branch: Option<&str>,
) -> Element<'a, Message> {
    let can_commit = !actions_disabled
        && !status_detail.staged.is_empty()
        && !state.title.trim().is_empty()
        && (!state.push_after || head_branch.is_some());
    let button_label = if state.amend { "Amend" } else { "Commit" };

    container(
        column![
            text("COMMIT")
                .size(theme::FS_XS)
                .font(theme::font_semibold())
                .color(color::TEXT_SUBTLE),
            text_input("Summary", &state.title)
                .on_input(|title| Message::from(commit::Message::TitleChanged(title)))
                .padding(Padding::from([5, 8]))
                .size(theme::FS_SM)
                .style(styles::form_text_input)
                .width(Length::Fill),
            text_input("Description", &state.body)
                .on_input(|body| Message::from(commit::Message::BodyChanged(body)))
                .padding(Padding::from([5, 8]))
                .size(theme::FS_SM)
                .style(styles::form_text_input)
                .width(Length::Fill),
            text_input("Co-authors: Name <email>; Name <email>", &state.co_authors)
                .style(styles::form_text_input)
                .on_input(|co_authors| {
                    Message::from(commit::Message::CoAuthorsChanged(co_authors))
                })
                .padding(Padding::from([5, 8]))
                .size(theme::FS_SM)
                .width(Length::Fill),
            column![
                checkbox("Amend previous", state.amend)
                    .on_toggle(|checked| Message::from(commit::Message::AmendChanged(checked)))
                    .size(theme::FS_SM),
                checkbox("Skip hooks", state.skip_hooks)
                    .on_toggle(|checked| {
                        Message::from(commit::Message::SkipHooksChanged(checked))
                    })
                    .size(theme::FS_SM),
                checkbox("Push after commit", state.push_after)
                    .on_toggle(|checked| {
                        Message::from(commit::Message::PushAfterChanged(checked))
                    })
                    .size(theme::FS_SM),
            ]
            .spacing(theme::SP_XS),
            row![
                text(commit_readiness_label(status_detail, state, head_branch))
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_SUBTLE),
                Space::with_width(Length::Fill),
                button(form_button_label(button_label))
                    .padding(Padding::from([4, 10]))
                    .style(styles::primary_button)
                    .on_press_maybe(
                        can_commit.then_some(Message::from(commit::Message::Requested))
                    ),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM),
        ]
        .spacing(theme::SP_SM),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn commit_readiness_label(
    status_detail: &WorktreeStatusDetail,
    state: &CommitFormState,
    head_branch: Option<&str>,
) -> String {
    if status_detail.staged.is_empty() {
        "Stage changes to commit".into()
    } else if state.title.trim().is_empty() {
        "Enter a summary".into()
    } else if state.push_after && head_branch.is_none() {
        "Checkout a branch before pushing".into()
    } else if state.amend && state.push_after {
        format!(
            "Amend and push {} staged file(s)",
            status_detail.staged.len()
        )
    } else if state.amend {
        format!("Amend with {} staged file(s)", status_detail.staged.len())
    } else if state.push_after {
        format!(
            "Commit and push {} staged file(s)",
            status_detail.staged.len()
        )
    } else {
        format!("{} staged file(s)", status_detail.staged.len())
    }
}

pub fn branch_create_prompt<'a>(
    state: &'a BranchCreateState,
    loading: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    let name_empty = state.name.trim().is_empty();
    let base = state.base.label();

    container(
        row![
            column![
                text("Create branch")
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(format!("Base: {base}"))
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
            text_input("branch/name", &state.name)
                .id(input_id.clone())
                .style(styles::form_text_input)
                .on_input(|name| Message::from(branch_create::Message::NameChanged(name)))
                .on_submit(Message::from(branch_create::Message::Submitted))
                .padding(Padding::from([5, 10]))
                .size(theme::FS_SM)
                .width(Length::FillPortion(2)),
            button(form_button_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(branch_create::Message::Cancelled)),
            button(form_button_label("Create"))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading && !name_empty)
                        .then_some(Message::from(branch_create::Message::Submitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

pub fn worktree_create_prompt<'a>(
    state: &'a WorktreeCreateState,
    loading: bool,
    path_input_id: &text_input::Id,
    start_input_id: &text_input::Id,
    branch_input_id: &text_input::Id,
) -> Element<'a, Message> {
    let ready = !state.path.trim().is_empty() && !state.start_point.trim().is_empty();

    container(
        row![
            column![
                text("Create worktree")
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text("Path, start point, and optional new branch")
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
            column![
                text_input("path", &state.path)
                    .id(path_input_id.clone())
                    .style(styles::form_text_input)
                    .on_input(|path| Message::from(worktree::Message::CreatePathChanged(path)))
                    .on_submit(Message::from(worktree::Message::CreateConfirmed))
                    .padding(Padding::from([5, 10]))
                    .size(theme::FS_SM),
                row![
                    text_input("branch or commit", &state.start_point)
                        .id(start_input_id.clone())
                        .style(styles::form_text_input)
                        .on_input(|start_point| {
                            Message::from(worktree::Message::CreateStartPointChanged(start_point))
                        })
                        .on_submit(Message::from(worktree::Message::CreateConfirmed))
                        .padding(Padding::from([5, 10]))
                        .size(theme::FS_SM)
                        .width(Length::FillPortion(1)),
                    text_input("new branch optional", &state.new_branch)
                        .id(branch_input_id.clone())
                        .style(styles::form_text_input)
                        .on_input(|branch| {
                            Message::from(worktree::Message::CreateBranchChanged(branch))
                        })
                        .on_submit(Message::from(worktree::Message::CreateConfirmed))
                        .padding(Padding::from([5, 10]))
                        .size(theme::FS_SM)
                        .width(Length::FillPortion(1)),
                ]
                .spacing(theme::SP_SM),
            ]
            .spacing(theme::SP_SM)
            .width(Length::FillPortion(3)),
            button(form_button_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(worktree::Message::CreateCancelled)),
            button(form_button_label("Create"))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading && ready)
                        .then_some(Message::from(worktree::Message::CreateConfirmed))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

pub fn branch_rename_prompt<'a>(
    state: &'a BranchManageRenameState,
    loading: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    let Some(target) = state.target.as_ref() else {
        return Space::new(0.0, 0.0).into();
    };
    let name_empty = state.name.trim().is_empty();

    container(
        row![
            column![
                text("Rename branch")
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(format!("Current: {}", target.short_name))
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
            text_input("branch/name", &state.name)
                .id(input_id.clone())
                .style(styles::form_text_input)
                .on_input(|name| Message::from(branch_manage::Message::RenameNameChanged(name)))
                .on_submit(Message::from(branch_manage::Message::RenameSubmitted))
                .padding(Padding::from([5, 10]))
                .size(theme::FS_SM)
                .width(Length::FillPortion(2)),
            button(form_button_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(branch_manage::Message::RenameCancelled)),
            button(form_button_label("Rename"))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading && !name_empty)
                        .then_some(Message::from(branch_manage::Message::RenameSubmitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

pub fn pull_request_create_prompt<'a>(
    state: &'a PullRequestCreateState,
    loading: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    column![
        text("Create pull request")
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        text("Optional base branch and draft mode")
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT_MUTED),
        Space::with_height(theme::SP_SM),
        text_input("base branch (default: current)", &state.base_branch)
            .id(input_id.clone())
            .style(styles::form_text_input)
            .on_input(|base| Message::from(pull_request::Message::CreateBaseChanged(base)))
            .on_submit(Message::from(pull_request::Message::CreateSubmitted))
            .padding(Padding::from([6, 10]))
            .size(theme::FS_SM)
            .width(Length::Fill),
        checkbox("Create as draft", state.draft)
            .on_toggle(|checked| Message::from(pull_request::Message::CreateDraftChanged(checked)))
            .size(theme::FS_SM),
        Space::with_height(theme::SP_SM),
        row![
            Space::with_width(Length::Fill),
            button(modal_button_label("Cancel"))
                .padding(Padding::from([6, 12]))
                .style(styles::subtle_button)
                .on_press(Message::from(pull_request::Message::CreateCancelled)),
            button(modal_button_label("Create"))
                .padding(Padding::from([6, 12]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading).then_some(Message::from(pull_request::Message::CreateSubmitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_SM)
    .into()
}

pub fn pull_request_worktree_prompt<'a>(
    state: &'a PullRequestWorktreeCheckoutState,
    loading: bool,
    path_input_id: &text_input::Id,
    branch_input_id: &text_input::Id,
) -> Element<'a, Message> {
    let Some(pull_request) = state.pull_request.as_ref() else {
        return Space::new(0.0, 0.0).into();
    };
    let ready = !state.path.trim().is_empty();

    column![
        text("PR worktree")
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        text(format!("#{} {}", pull_request.number, pull_request.title))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT_MUTED),
        Space::with_height(theme::SP_SM),
        text_input("worktree path", &state.path)
            .id(path_input_id.clone())
            .style(styles::form_text_input)
            .on_input(|path| {
                Message::from(pull_request::Message::CheckoutWorktreePathChanged(path))
            })
            .on_submit(Message::from(
                pull_request::Message::CheckoutWorktreeSubmitted
            ))
            .padding(Padding::from([6, 10]))
            .size(theme::FS_SM)
            .width(Length::Fill),
        text("Default is pre-filled. Edit to pick a different sibling directory.")
            .size(theme::FS_XS)
            .font(theme::font_regular())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT_SUBTLE),
        text_input("local branch (optional)", &state.branch_name)
            .id(branch_input_id.clone())
            .style(styles::form_text_input)
            .on_input(|branch| {
                Message::from(pull_request::Message::CheckoutWorktreeBranchChanged(branch))
            })
            .on_submit(Message::from(
                pull_request::Message::CheckoutWorktreeSubmitted
            ))
            .padding(Padding::from([6, 10]))
            .size(theme::FS_SM)
            .width(Length::Fill),
        Space::with_height(theme::SP_SM),
        row![
            Space::with_width(Length::Fill),
            button(modal_button_label("Cancel"))
                .padding(Padding::from([6, 12]))
                .style(styles::subtle_button)
                .on_press(Message::from(
                    pull_request::Message::CheckoutWorktreeCancelled
                )),
            button(modal_button_label("Create"))
                .padding(Padding::from([6, 12]))
                .style(styles::primary_button)
                .on_press_maybe((!loading && ready).then_some(Message::from(
                    pull_request::Message::CheckoutWorktreeSubmitted,
                ))),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn modal_button_label<'a>(label: &'a str) -> Element<'a, Message> {
    super::modal::modal_action_label(label)
}

pub fn stash_create_prompt<'a>(
    state: &'a StashCreateState,
    status_detail: &WorktreeStatusDetail,
    loading: bool,
    can_submit: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    container(
        row![
            column![
                text("Create stash")
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(stash_readiness_label(
                    status_detail,
                    state.include_untracked
                ))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
            text_input("Optional message", &state.message)
                .id(input_id.clone())
                .style(styles::form_text_input)
                .on_input(|message| Message::from(stash::Message::DescriptionChanged(message)))
                .on_submit(Message::from(stash::Message::Submitted))
                .padding(Padding::from([5, 10]))
                .size(theme::FS_SM)
                .width(Length::FillPortion(2)),
            checkbox("Include untracked", state.include_untracked)
                .on_toggle(|checked| {
                    Message::from(stash::Message::IncludeUntrackedChanged(checked))
                })
                .size(theme::FS_SM),
            button(form_button_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(stash::Message::Cancelled)),
            button(form_button_label("Stash"))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading && can_submit).then_some(Message::from(stash::Message::Submitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

pub fn history_reword_prompt<'a>(
    state: &'a HistoryRewordState,
    loading: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    let Some(commit) = state.commit.as_ref() else {
        return Space::new(0.0, 0.0).into();
    };
    let title_empty = state.title.trim().is_empty();
    let busy = loading || state.loading;

    let target_label = if state.loading {
        format!("Loading {}…", commit.short_id)
    } else {
        format!("Target: {} {}", commit.short_id, commit.summary)
    };

    container(
        column![
            text("Reword commit")
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .color(color::TEXT),
            text(target_label)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
            text_input("Title", &state.title)
                .id(input_id.clone())
                .style(styles::form_text_input)
                .on_input(|title| Message::from(history::Message::RewordTitleChanged(title)))
                .on_submit(Message::from(history::Message::RewordSubmitted))
                .padding(Padding::from([5, 10]))
                .size(theme::FS_SM)
                .width(Length::Fill),
            text_editor(&state.body_content)
                .placeholder("Body (optional)")
                .style(styles::form_text_editor)
                .on_action(|action| Message::from(history::Message::RewordBodyAction(action)))
                .padding(Padding::from([5, 10]))
                .size(theme::FS_SM)
                .height(Length::Fixed(120.0)),
            row![
                Space::with_width(Length::Fill),
                button(form_button_label("Cancel"))
                    .padding(Padding::from([5, 10]))
                    .style(styles::subtle_button)
                    .on_press(Message::from(history::Message::RewordCancelled)),
                button(form_button_label("Reword"))
                    .padding(Padding::from([5, 10]))
                    .style(styles::primary_button)
                    .on_press_maybe(
                        (!busy && !title_empty)
                            .then_some(Message::from(history::Message::RewordSubmitted)),
                    ),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM),
        ]
        .spacing(theme::SP_SM),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

pub fn tag_create_prompt<'a>(
    state: &'a TagCreateState,
    loading: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    let target = state
        .target_commit
        .as_ref()
        .map(|commit| format!("{} {}", commit.short_id, commit.summary))
        .unwrap_or_else(|| "HEAD".into());
    let name_empty = state.name.trim().is_empty();
    let mut mode_row = row![].spacing(theme::SP_XS);
    for mode in TagNameMode::ALL {
        mode_row = mode_row.push(tag_mode_button(mode, state.name_mode, loading));
    }

    column![
        text("Create tag")
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph)
            .color(color::TEXT),
        text(format!("Target: {target}"))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .width(Length::Fill)
            .color(color::TEXT_MUTED)
            .wrapping(Wrapping::WordOrGlyph),
        Space::with_height(theme::SP_SM),
        mode_row,
        text_input("v1.0.0", &state.name)
            .id(input_id.clone())
            .style(styles::form_text_input)
            .on_input(|name| Message::from(tag::Message::CreateNameChanged(name)))
            .on_submit(Message::from(tag::Message::CreateSubmitted))
            .padding(Padding::from([6, 10]))
            .size(theme::FS_SM)
            .width(Length::Fill),
        tag_push_toggle(state.push_after_create, loading),
        Space::with_height(theme::SP_SM),
        row![
            Space::with_width(Length::Fill),
            button(modal_button_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(tag::Message::CreateCancelled)),
            button(modal_button_label("Create"))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading && !name_empty)
                        .then_some(Message::from(tag::Message::CreateSubmitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn tag_mode_button(
    mode: TagNameMode,
    current: TagNameMode,
    loading: bool,
) -> Element<'static, Message> {
    let active = mode == current;
    button(
        text(mode.label())
            .size(theme::FS_XS)
            .font(if active {
                theme::font_semibold()
            } else {
                theme::font_regular()
            })
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([4, 10]))
    .style(styles::segmented_chip(active))
    .on_press_maybe(
        (!loading && !active).then_some(Message::from(tag::Message::CreateNameModeChanged(mode))),
    )
    .into()
}

fn tag_push_toggle(checked: bool, loading: bool) -> Element<'static, Message> {
    let content = container(
        row![
            tag_toggle_indicator(checked),
            text("Push after create")
                .size(theme::FS_SM)
                .font(if checked {
                    theme::font_semibold()
                } else {
                    theme::font_regular()
                })
                .color(if checked {
                    color::TEXT
                } else {
                    color::TEXT_MUTED
                })
                .wrapping(Wrapping::None),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_XS),
    )
    .padding(Padding::from([2, 0]));

    if loading {
        content.into()
    } else {
        mouse_area(content)
            .on_press(Message::from(tag::Message::CreatePushAfterChanged(
                !checked,
            )))
            .into()
    }
}

fn tag_toggle_indicator(checked: bool) -> Element<'static, Message> {
    let mark: Element<'static, Message> = if checked {
        text("✓")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::ACCENT)
            .into()
    } else {
        Space::new(0.0, 0.0).into()
    };

    container(mark)
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .center_x(Length::Fixed(12.0))
        .center_y(Length::Fixed(12.0))
        .style(move |_| {
            let border_color = if checked {
                color::ACCENT
            } else {
                color::BORDER
            };
            container::Style {
                background: None,
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: theme::R_SM.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

fn form_button_label<'a>(label: &'a str) -> Element<'a, Message> {
    container(
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None),
    )
    .center_x(Length::Fixed(form_button_label_width(label)))
    .into()
}

fn form_button_label_width(label: &str) -> f32 {
    let text_width = label.chars().count() as f32 * 7.0;
    (text_width + 2.0).clamp(42.0, 76.0)
}

pub fn stash_branch_prompt<'a>(
    state: &'a StashBranchState,
    loading: bool,
    input_id: &text_input::Id,
) -> Element<'a, Message> {
    let Some(stash) = state.stash.as_ref() else {
        return Space::new(0.0, 0.0).into();
    };
    let name_empty = state.name.trim().is_empty();

    container(
        row![
            column![
                text("Create branch from stash")
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT),
                text(format!(
                    "Source: {}  Applies changes and removes stash on success.",
                    stash.selector
                ))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_MUTED),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
            text_input("branch/name", &state.name)
                .id(input_id.clone())
                .style(styles::form_text_input)
                .on_input(|name| Message::from(stash::Message::BranchNameChanged(name)))
                .on_submit(Message::from(stash::Message::BranchSubmitted))
                .padding(Padding::from([5, 10]))
                .size(theme::FS_SM)
                .width(Length::FillPortion(2)),
            button(form_button_label("Cancel"))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(stash::Message::BranchCancelled)),
            button(form_button_label("Create"))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(
                    (!loading && !name_empty)
                        .then_some(Message::from(stash::Message::BranchSubmitted))
                ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn stash_readiness_label(status_detail: &WorktreeStatusDetail, include_untracked: bool) -> String {
    let tracked = status_detail.staged.len() + status_detail.unstaged.len();
    let untracked = status_detail.untracked.len();

    if tracked == 0 && untracked > 0 && !include_untracked {
        "Enable untracked files to stash new files".into()
    } else if include_untracked {
        format!("{tracked} tracked, {untracked} untracked file(s)")
    } else {
        format!("{tracked} tracked file(s)")
    }
}
