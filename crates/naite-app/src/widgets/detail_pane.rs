//! Right pane: commit metadata or WIP detail with diff browser.

use iced::widget::{
    button, column, container, image, mouse_area, rich_text, row, scrollable,
    scrollable::Direction, scrollable::Scrollbar, text, text::Wrapping, tooltip, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use naite_core::{
    ChangeStatus, CommitDiff, CommitSummary, FileChange, GitHubIssueSummary, GitOperationState,
    HighlightedDiff, HighlightedLine, Hunk, PullRequestSummary, StashSummary, TokenKind,
    WorktreeDiffKind, WorktreeDiffTarget, WorktreeStatusDetail,
};

use crate::features::{discard, file_inspect, github_issue, pull_request, stage, stash};
use crate::icons::{self, IconName};
use crate::state::{
    AvatarCache, CommitFormState, ContextMenuKind, DiffViewMode, FileInsightMode, FileInsightState,
    PreferencesState,
};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::common::{action_button, danger_action_button, format_relative_time, inset_text};
use super::forms::commit_form as commit_form_widget;
use super::pills::{ci_pill, muted_pill, review_pill};
use super::status::{status_overview, status_section, wip_diff_panel, StatusSectionProps};

const PR_AUTHOR_AVATAR_SIZE: f32 = 18.0;

pub struct DetailPaneProps<'a> {
    pub commits: &'a [CommitSummary],
    pub selected: Option<usize>,
    pub wip_selected: bool,
    pub selected_pull_request: Option<&'a PullRequestSummary>,
    pub selected_github_issue: Option<&'a GitHubIssueSummary>,
    pub selected_stash: Option<&'a StashSummary>,
    pub status_detail: &'a WorktreeStatusDetail,
    pub diff: Option<&'a CommitDiff>,
    pub diff_highlight: Option<&'a HighlightedDiff>,
    pub diff_loading: bool,
    pub diff_error: Option<&'a str>,
    pub selected_file: Option<usize>,
    pub selected_hunk: Option<usize>,
    pub diff_view_mode: DiffViewMode,
    pub selected_wip_file: Option<&'a WorktreeDiffTarget>,
    pub commit_form: &'a CommitFormState,
    pub head_branch: Option<&'a str>,
    pub actions_disabled: bool,
    pub operation_state: GitOperationState,
    pub file_insight: &'a FileInsightState,
    pub avatars: &'a AvatarCache,
    pub preferences: &'a PreferencesState,
}

pub fn detail_pane<'a>(props: DetailPaneProps<'a>) -> Element<'a, Message> {
    let DetailPaneProps {
        commits,
        selected,
        wip_selected,
        selected_pull_request,
        selected_github_issue,
        selected_stash,
        status_detail,
        diff,
        diff_highlight,
        diff_loading,
        diff_error,
        selected_file,
        selected_hunk,
        diff_view_mode,
        selected_wip_file,
        commit_form,
        head_branch,
        actions_disabled,
        operation_state,
        file_insight,
        avatars,
        preferences,
    } = props;

    let inner: Element<'a, Message> = if wip_selected {
        scrollable(
            container(status_detail_content(StatusDetailContentProps {
                status_detail,
                actions_disabled,
                commit_form,
                head_branch,
                operation_state,
                diff,
                diff_highlight,
                diff_loading,
                diff_error,
                selected_file,
                selected_hunk,
                diff_view_mode,
                selected_wip_file,
                file_insight,
                show_file_inspection: preferences.display.show_file_inspection,
            }))
            .width(Length::Fill)
            .padding(preferences.panel_padding()),
        )
        .direction(styles::thin_scrollbar_dir())
        .style(styles::thin_scrollbar)
        .height(Length::Fill)
        .into()
    } else if let Some(pull_request) = selected_pull_request {
        scrollable(
            container(pull_request_detail(
                pull_request,
                actions_disabled,
                avatars,
                preferences.display.show_pr_metadata,
            ))
            .width(Length::Fill)
            .padding(preferences.panel_padding()),
        )
        .direction(styles::thin_scrollbar_dir())
        .style(styles::thin_scrollbar)
        .height(Length::Fill)
        .into()
    } else if let Some(issue) = selected_github_issue {
        scrollable(
            container(github_issue_detail(issue))
                .width(Length::Fill)
                .padding(preferences.panel_padding()),
        )
        .direction(styles::thin_scrollbar_dir())
        .style(styles::thin_scrollbar)
        .height(Length::Fill)
        .into()
    } else if let Some(stash) = selected_stash {
        scrollable(
            container(
                column![
                    stash_overview(stash, actions_disabled),
                    Space::with_height(theme::SP_MD),
                    diff_content(DiffContentProps {
                        diff,
                        diff_highlight,
                        loading: diff_loading,
                        error: diff_error,
                        selected_file,
                        selected_hunk,
                        diff_view_mode,
                        selected_wip_file: None,
                        actions_disabled: false,
                        file_insight,
                        show_file_inspection: preferences.display.show_file_inspection,
                    }),
                ]
                .width(Length::Fill)
                .spacing(theme::SP_MD),
            )
            .width(Length::Fill)
            .padding(preferences.panel_padding()),
        )
        .direction(styles::thin_scrollbar_dir())
        .style(styles::thin_scrollbar)
        .height(Length::Fill)
        .into()
    } else {
        match selected.and_then(|i| commits.get(i)) {
            Some(commit) => scrollable(
                container(
                    column![
                        copyable_meta_field("Commit", &commit.id),
                        meta_field("Subject", &commit.summary, false),
                        meta_field("Author", &commit.author_name, false),
                        meta_field("Email", &commit.author_email, true),
                        meta_field("Date", &format_relative_time(commit.time_seconds), false),
                        Space::with_height(theme::SP_LG),
                        diff_content(DiffContentProps {
                            diff,
                            diff_highlight,
                            loading: diff_loading,
                            error: diff_error,
                            selected_file,
                            selected_hunk,
                            diff_view_mode,
                            selected_wip_file: None,
                            actions_disabled: false,
                            file_insight,
                            show_file_inspection: preferences.display.show_file_inspection,
                        }),
                    ]
                    .width(Length::Fill)
                    .spacing(theme::SP_MD),
                )
                .width(Length::Fill)
                .padding(preferences.panel_padding()),
            )
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar)
            .height(Length::Fill)
            .into(),
            None => container(
                text("Select a commit to inspect.")
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_SUBTLE),
            )
            .padding(theme::SP_LG)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
        }
    };

    container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::surface_panel)
        .into()
}

fn github_issue_detail<'a>(issue: &'a GitHubIssueSummary) -> Element<'a, Message> {
    let title_clip: Element<'a, Message> = container(
        text(issue.title.clone())
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT),
    )
    .width(Length::Fill)
    .clip(true)
    .into();

    let mut labels = row![].align_y(Alignment::Center).spacing(theme::SP_XS);
    if issue.labels.is_empty() {
        labels = labels.push(muted_pill("No labels"));
    } else {
        for label in &issue.labels {
            labels = labels.push(muted_pill(label));
        }
    }

    column![
        text("GITHUB ISSUE")
            .size(theme::FS_XS)
            .color(color::TEXT_SUBTLE)
            .font(theme::font_semibold()),
        row![
            text(format!("#{}", issue.number))
                .size(theme::FS_BASE)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE),
            title_clip,
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        row![
            muted_pill(&issue.state),
            Space::with_width(Length::Fill),
            action_button(
                "Open",
                true,
                Message::from(github_issue::Message::OpenInBrowserRequested(issue.clone())),
            ),
            action_button("Copy", true, Message::CopyText(issue.url.clone())),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        group_card(
            "Issue",
            column![
                labeled_row(
                    "Author",
                    text(issue.author.clone())
                        .size(theme::FS_SM)
                        .font(theme::font_regular())
                        .color(color::TEXT)
                        .into(),
                ),
                labeled_row(
                    "Updated",
                    text(issue.updated_at.clone())
                        .size(theme::FS_SM)
                        .font(theme::font_regular())
                        .color(color::TEXT_MUTED)
                        .into(),
                ),
                labeled_row("Labels", labels.into()),
            ]
            .spacing(theme::SP_SM)
            .into(),
        ),
        group_card(
            "Link",
            text(issue.url.clone())
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE)
                .into(),
        ),
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn stash_overview<'a>(
    stash_summary: &'a StashSummary,
    actions_disabled: bool,
) -> Element<'a, Message> {
    let branch = if stash_summary.branch.is_empty() {
        "Unknown branch"
    } else {
        stash_summary.branch.as_str()
    };

    column![
        row![
            text("STASH")
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_semibold()),
            Space::with_width(Length::Fill),
            action_button(
                "Branch",
                !actions_disabled,
                Message::from(stash::Message::BranchRequested(stash_summary.clone())),
            ),
            action_button(
                "Apply",
                !actions_disabled,
                Message::from(stash::Message::ApplyRequested(stash_summary.clone())),
            ),
            action_button(
                "Pop",
                !actions_disabled,
                Message::from(stash::Message::PopRequested(stash_summary.clone())),
            ),
            danger_action_button(
                "Drop",
                !actions_disabled,
                Message::from(stash::Message::DropRequested(stash_summary.clone())),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
        meta_field("Selector", &stash_summary.selector, true),
        meta_field("Commit", &stash_summary.short_id, true),
        meta_field("Branch", branch, false),
        meta_field("Date", &stash_summary.date, false),
        meta_field("Message", &stash_summary.message, false),
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn pull_request_detail<'a>(
    pull_request: &'a PullRequestSummary,
    actions_disabled: bool,
    avatars: &'a AvatarCache,
    show_metadata: bool,
) -> Element<'a, Message> {
    let issues: Element<'a, Message> = if pull_request.issue_links.is_empty() {
        inset_text("No linked GitHub issues.")
    } else {
        let mut rows = column![].spacing(theme::SP_XS);
        for issue in &pull_request.issue_links {
            let url_clip: Element<'a, Message> = container(
                text(issue.url.clone())
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .wrapping(Wrapping::None)
                    .color(color::TEXT_SUBTLE),
            )
            .width(Length::Fill)
            .clip(true)
            .into();
            rows = rows.push(
                row![
                    text(format!("#{}", issue.number))
                        .size(theme::FS_SM)
                        .font(theme::font_semibold())
                        .wrapping(Wrapping::None)
                        .color(color::TEXT_MUTED),
                    url_clip,
                    action_button("Copy", true, Message::CopyText(issue.url.clone()),),
                ]
                .align_y(Alignment::Center)
                .spacing(theme::SP_SM),
            );
        }
        container(rows)
            .padding(theme::SP_MD)
            .width(Length::Fill)
            .style(styles::inset_card)
            .into()
    };

    // Title sits on its own row so action buttons never compete with it for
    // horizontal space. Inner text uses Wrapping::None + container clip so
    // long titles render single-line and clip on overflow (iced 0.13 will
    // otherwise per-glyph wrap a Fill-width text when the row is tight).
    let title_clip: Element<'a, Message> = container(
        text(pull_request.title.clone())
            .size(theme::FS_BASE)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT),
    )
    .width(Length::Fill)
    .clip(true)
    .into();

    let title_row = row![
        text(format!("#{}", pull_request.number))
            .size(theme::FS_BASE)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
        title_clip,
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let actions_row = row![
        Space::with_width(Length::Fill),
        action_button(
            "Open ↗",
            true,
            Message::from(pull_request::Message::OpenInBrowserRequested(
                pull_request.clone(),
            )),
        ),
        action_button(
            "Checkout",
            !actions_disabled,
            Message::from(pull_request::Message::CheckoutRequested(
                pull_request.clone(),
            )),
        ),
        action_button(
            "Worktree",
            !actions_disabled,
            Message::from(pull_request::Message::CheckoutWorktreeRequested(
                pull_request.clone(),
            )),
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let mut subheader = row![
        text(format!(
            "{} → {}",
            pull_request.head_branch, pull_request.base_branch
        ))
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .wrapping(Wrapping::None)
        .color(color::TEXT_SUBTLE),
        review_pill(pull_request.review_status),
        ci_pill(pull_request.ci_status),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);
    if !pull_request.merge_state.trim().is_empty() {
        subheader = subheader.push(muted_pill(&pull_request.merge_state));
    }
    if pull_request.draft {
        subheader = subheader.push(muted_pill("Draft"));
    }

    let author_value: Element<'a, Message> = row![
        avatar_widget(pull_request.author_avatar_url.as_deref(), avatars),
        text(pull_request.author.clone())
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into();

    let reviewers_value: Element<'a, Message> = if pull_request.reviewers.is_empty() {
        text("No reviewers")
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE)
            .into()
    } else {
        text(pull_request.reviewers.join(", "))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT)
            .into()
    };

    let people_card = group_card(
        "People",
        column![
            labeled_row("Author", author_value),
            labeled_row("Reviewers", reviewers_value),
        ]
        .spacing(theme::SP_SM)
        .into(),
    );

    let labels_value: Element<'a, Message> = if pull_request.labels.is_empty() {
        text("No labels")
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE)
            .into()
    } else {
        let mut chips = row![].align_y(Alignment::Center).spacing(theme::SP_XS);
        for label in &pull_request.labels {
            chips = chips.push(muted_pill(label));
        }
        chips.into()
    };

    let updated_value: Element<'a, Message> = text(pull_request.updated_at.clone())
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .color(color::TEXT_MUTED)
        .into();

    let status_card = group_card(
        "Status",
        column![
            labeled_row("Labels", labels_value),
            labeled_row("Updated", updated_value),
        ]
        .spacing(theme::SP_SM)
        .into(),
    );

    let people_section: Element<'a, Message> = if show_metadata {
        people_card
    } else {
        Space::new(0, 0).into()
    };
    let status_section: Element<'a, Message> = if show_metadata {
        status_card
    } else {
        Space::new(0, 0).into()
    };

    column![
        text("PULL REQUEST")
            .size(theme::FS_XS)
            .color(color::TEXT_SUBTLE)
            .font(theme::font_semibold()),
        title_row,
        subheader,
        actions_row,
        Space::with_height(theme::SP_SM),
        people_section,
        status_section,
        Space::with_height(theme::SP_SM),
        text("LINKED ISSUES")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
        issues,
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn group_card<'a>(label: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(
        column![
            text(label.to_uppercase())
                .size(theme::FS_XS)
                .font(theme::font_semibold())
                .color(color::TEXT_SUBTLE),
            body,
        ]
        .spacing(theme::SP_SM),
    )
    .padding(theme::SP_MD)
    .width(Length::Fill)
    .style(styles::inset_card)
    .into()
}

fn labeled_row<'a>(label: &'a str, value: Element<'a, Message>) -> Element<'a, Message> {
    row![
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE)
            .width(Length::Fixed(96.0)),
        value,
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into()
}

fn avatar_widget<'a>(url: Option<&str>, avatars: &'a AvatarCache) -> Element<'a, Message> {
    let size = Length::Fixed(PR_AUTHOR_AVATAR_SIZE);
    match url.and_then(|u| avatars.handles.get(u)) {
        Some(handle) => image::Image::new(handle.clone())
            .width(size)
            .height(size)
            .into(),
        None => container(Space::new(size, size))
            .width(size)
            .height(size)
            .style(|_| container::Style {
                background: Some(Background::Color(color::TEXT_SUBTLE)),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: theme::R_PILL.into(),
                },
                ..Default::default()
            })
            .into(),
    }
}

struct StatusDetailContentProps<'a> {
    status_detail: &'a WorktreeStatusDetail,
    actions_disabled: bool,
    commit_form: &'a CommitFormState,
    head_branch: Option<&'a str>,
    operation_state: GitOperationState,
    diff: Option<&'a CommitDiff>,
    diff_highlight: Option<&'a HighlightedDiff>,
    diff_loading: bool,
    diff_error: Option<&'a str>,
    selected_file: Option<usize>,
    selected_hunk: Option<usize>,
    diff_view_mode: DiffViewMode,
    selected_wip_file: Option<&'a WorktreeDiffTarget>,
    file_insight: &'a FileInsightState,
    show_file_inspection: bool,
}

fn status_detail_content<'a>(props: StatusDetailContentProps<'a>) -> Element<'a, Message> {
    let StatusDetailContentProps {
        status_detail,
        actions_disabled,
        commit_form,
        head_branch,
        operation_state,
        diff,
        diff_highlight,
        diff_loading,
        diff_error,
        selected_file,
        selected_hunk,
        diff_view_mode,
        selected_wip_file,
        file_insight,
        show_file_inspection,
    } = props;

    column![
        status_overview(status_detail, operation_state, actions_disabled),
        commit_form_widget(status_detail, actions_disabled, commit_form, head_branch),
        wip_diff_panel(DiffContentProps {
            diff,
            diff_highlight,
            loading: diff_loading,
            error: diff_error,
            selected_file,
            selected_hunk,
            diff_view_mode,
            selected_wip_file,
            actions_disabled,
            file_insight,
            show_file_inspection,
        }),
        Space::with_height(theme::SP_MD),
        status_section(StatusSectionProps {
            label: "Staged changes",
            entries: &status_detail.staged,
            accent: color::SUCCESS,
            action: Some(super::status::StatusAction::Unstage),
            discardable: false,
            actions_disabled,
            diff_kind: WorktreeDiffKind::Staged,
            selected_wip_file,
        },),
        status_section(StatusSectionProps {
            label: "Modified files",
            entries: &status_detail.unstaged,
            accent: color::WARNING,
            action: Some(super::status::StatusAction::Stage),
            discardable: true,
            actions_disabled,
            diff_kind: WorktreeDiffKind::Unstaged,
            selected_wip_file,
        },),
        status_section(StatusSectionProps {
            label: "New files",
            entries: &status_detail.untracked,
            accent: color::ACCENT,
            action: Some(super::status::StatusAction::Stage),
            discardable: true,
            actions_disabled,
            diff_kind: WorktreeDiffKind::Untracked,
            selected_wip_file,
        },),
        status_section(StatusSectionProps {
            label: "Conflicts",
            entries: &status_detail.conflicted,
            accent: color::DANGER,
            action: None,
            discardable: false,
            actions_disabled,
            diff_kind: WorktreeDiffKind::Conflict,
            selected_wip_file,
        },),
        status_section(StatusSectionProps {
            label: "Submodules",
            entries: &status_detail.submodules,
            accent: color::TEXT_MUTED,
            action: None,
            discardable: false,
            actions_disabled,
            diff_kind: WorktreeDiffKind::Unstaged,
            selected_wip_file: None,
        },),
        status_section(StatusSectionProps {
            label: "Ignored files",
            entries: &status_detail.ignored,
            accent: color::TEXT_SUBTLE,
            action: None,
            discardable: false,
            actions_disabled,
            diff_kind: WorktreeDiffKind::Unstaged,
            selected_wip_file: None,
        },),
    ]
    .width(Length::Fill)
    .spacing(theme::SP_MD)
    .into()
}

pub(super) struct DiffContentProps<'a> {
    pub diff: Option<&'a CommitDiff>,
    pub diff_highlight: Option<&'a HighlightedDiff>,
    pub loading: bool,
    pub error: Option<&'a str>,
    pub selected_file: Option<usize>,
    pub selected_hunk: Option<usize>,
    pub diff_view_mode: DiffViewMode,
    pub selected_wip_file: Option<&'a WorktreeDiffTarget>,
    pub actions_disabled: bool,
    pub file_insight: &'a FileInsightState,
    pub show_file_inspection: bool,
}

pub(super) fn diff_content<'a>(props: DiffContentProps<'a>) -> Element<'a, Message> {
    let DiffContentProps {
        diff,
        diff_highlight,
        loading,
        error,
        selected_file,
        selected_hunk,
        diff_view_mode,
        selected_wip_file,
        actions_disabled,
        file_insight,
        show_file_inspection,
    } = props;

    if loading {
        return inset_text("Loading diff...");
    }

    if let Some(error) = error {
        return container(
            text(format!("Diff error: {error}"))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::DANGER),
        )
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::error_card)
        .into();
    }

    let Some(diff) = diff else {
        return inset_text("Diff not loaded.");
    };

    if diff.files.is_empty() {
        return inset_text("No file changes.");
    }

    let selected = selected_file.unwrap_or(0).min(diff.files.len() - 1);
    let file = &diff.files[selected];
    let hunks = diff.hunks_by_file.get(&file.path);
    let selected_hunk = hunks
        .filter(|hunks| !hunks.is_empty())
        .map(|hunks| selected_hunk.unwrap_or(0).min(hunks.len() - 1));
    let hunk_actions = if actions_disabled || file.is_binary || file.is_truncated {
        HunkActions::default()
    } else {
        selected_wip_file
            .filter(|target| target.path == file.path)
            .map(HunkActions::for_kind)
            .unwrap_or_default()
    };

    let wip_kind = selected_wip_file.map(|target| target.kind);
    container(
        column![
            file_list(&diff.files, selected, wip_kind),
            Space::with_height(theme::SP_MD),
            diff_controls(
                diff_view_mode,
                hunks.map(Vec::len).unwrap_or(0),
                selected_hunk,
            ),
            file_diff(
                file,
                hunks.map(Vec::as_slice),
                diff_highlight.and_then(|hl| hl.by_file.get(&file.path).map(Vec::as_slice)),
                hunk_actions,
                diff_view_mode,
                selected_hunk,
                wip_kind,
            ),
            if show_file_inspection {
                file_insight_controls(file, actions_disabled)
            } else {
                Space::new(0, 0).into()
            },
            if show_file_inspection {
                file_insight_panel(file, file_insight)
            } else {
                Space::new(0, 0).into()
            },
        ]
        .width(Length::Fill)
        .spacing(theme::SP_SM),
    )
    .width(Length::Fill)
    .into()
}

fn file_insight_controls<'a>(file: &'a FileChange, actions_disabled: bool) -> Element<'a, Message> {
    row![
        text("FILE INSPECTION")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE),
        Space::with_width(Length::Fill),
        file_insight_action_group(file, actions_disabled),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM)
    .into()
}

fn file_insight_action_group<'a>(
    file: &'a FileChange,
    actions_disabled: bool,
) -> Element<'a, Message> {
    let buttons = row![
        file_insight_action_button(
            "History",
            IconName::FileClock,
            !actions_disabled,
            "Show the commits that changed this file",
            Message::from(file_inspect::Message::HistoryRequested(file.path.clone())),
        ),
        file_insight_action_button(
            "Blame",
            IconName::FileUser,
            !actions_disabled,
            "Show the last author and commit for each line",
            Message::from(file_inspect::Message::BlameRequested(file.path.clone())),
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(2);

    container(scrollable(buttons).direction(Direction::Horizontal(
        Scrollbar::new().width(0).scroller_width(0).margin(0),
    )))
    .padding(2)
    .width(Length::Shrink)
    .style(styles::inset_card)
    .into()
}

fn file_insight_action_button<'a>(
    label: &'a str,
    icon: IconName,
    enabled: bool,
    tooltip_text: &'a str,
    message: Message,
) -> Element<'a, Message> {
    let tint = if enabled {
        color::TEXT_MUTED
    } else {
        color::TEXT_SUBTLE
    };
    let button = button(
        row![
            icons::icon(icon, 12, tint),
            text(label)
                .size(theme::FS_XS)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None)
                .color(if enabled {
                    color::TEXT
                } else {
                    color::TEXT_SUBTLE
                }),
        ]
        .align_y(Alignment::Center)
        .spacing(5),
    )
    .width(Length::Fixed(file_insight_action_width(label)))
    .padding(Padding::from([3, 7]))
    .style(styles::subtle_button)
    .on_press_maybe(enabled.then_some(message));

    tooltip(
        button,
        container(
            text(tooltip_text)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT),
        )
        .padding(Padding::from([4, 8]))
        .style(styles::inset_card),
        tooltip::Position::Top,
    )
    .into()
}

fn file_insight_action_width(label: &str) -> f32 {
    match label {
        "History" => 82.0,
        "Blame" => 72.0,
        _ => 76.0,
    }
}

fn file_insight_panel<'a>(
    file: &'a FileChange,
    insight: &'a FileInsightState,
) -> Element<'a, Message> {
    if insight.path.as_deref() != Some(file.path.as_str()) {
        return inset_text("Select History or Blame for this file.");
    }
    if insight.loading {
        return inset_text("Loading file inspection...");
    }
    if let Some(error) = insight.error.as_deref() {
        return container(
            text(error)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::DANGER),
        )
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::error_card)
        .into();
    }

    match insight.mode {
        FileInsightMode::History => file_history_panel(&insight.history),
        FileInsightMode::Blame => file_blame_panel(&insight.blame),
    }
}

fn file_history_panel<'a>(entries: &'a [naite_core::FileHistoryEntry]) -> Element<'a, Message> {
    if entries.is_empty() {
        return inset_text("No file history.");
    }

    let mut col = column![].spacing(theme::SP_XS);
    for entry in entries.iter().take(20) {
        col = col.push(
            row![
                text(entry.short_id.clone())
                    .size(theme::FS_XS)
                    .font(iced::Font::MONOSPACE)
                    .color(color::TEXT_SUBTLE),
                text(entry.summary.clone())
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .width(Length::Fill)
                    .color(color::TEXT_MUTED),
                text(format_relative_time(entry.time_seconds))
                    .size(theme::FS_XS)
                    .font(theme::font_regular())
                    .color(color::TEXT_SUBTLE),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM),
        );
    }

    container(col)
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::inset_card)
        .into()
}

fn file_blame_panel<'a>(lines: &'a [naite_core::BlameLine]) -> Element<'a, Message> {
    if lines.is_empty() {
        return inset_text("No blame data.");
    }

    let mut col = column![].spacing(0);
    for line in lines.iter().take(80) {
        col = col.push(
            row![
                text(format!("{:>4}", line.line_number))
                    .size(theme::FS_XS)
                    .font(iced::Font::MONOSPACE)
                    .color(color::TEXT_SUBTLE),
                text(line.short_id.clone())
                    .size(theme::FS_XS)
                    .font(iced::Font::MONOSPACE)
                    .color(color::TEXT_SUBTLE),
                text(line.contents.clone())
                    .size(theme::FS_SM)
                    .font(theme::font_code())
                    .width(Length::Fill)
                    .color(color::TEXT_MUTED),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM),
        );
    }

    container(col)
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .style(styles::inset_card)
        .into()
}

fn file_list<'a>(
    files: &'a [FileChange],
    selected: usize,
    wip_kind: Option<WorktreeDiffKind>,
) -> Element<'a, Message> {
    let mut col = column![text("FILES")
        .size(theme::FS_XS)
        .font(theme::font_semibold())
        .color(color::TEXT_SUBTLE)]
    .spacing(2);

    for (i, file) in files.iter().enumerate() {
        let label = format!("{} {}", status_label(file.status), file.path);
        let pressable = button(
            row![
                text(label)
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(if selected == i {
                        color::TEXT
                    } else {
                        color::TEXT_MUTED
                    }),
                Space::with_width(Length::Fill),
                icons::icon(IconName::ChevronRight, 13, color::TEXT_SUBTLE),
            ]
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([5, 8]))
        .width(Length::Fill)
        .style(styles::commit_row_button(selected == i))
        .on_press(Message::DetailFileSelected(i));

        let kind = match wip_kind {
            Some(kind) => ContextMenuKind::WipFile(WorktreeDiffTarget {
                kind,
                path: file.path.clone(),
            }),
            None => ContextMenuKind::CommitFile {
                path: file.path.clone(),
            },
        };
        col = col.push(mouse_area(pressable).on_right_press(Message::ContextMenuOpened(kind)));
    }

    col.width(Length::Fill).into()
}

fn diff_controls<'a>(
    mode: DiffViewMode,
    hunk_count: usize,
    selected_hunk: Option<usize>,
) -> Element<'a, Message> {
    if hunk_count == 0 {
        return Space::with_height(0).into();
    }

    let selected = selected_hunk.unwrap_or(0).min(hunk_count - 1);
    let mut controls = row![diff_mode_group(mode)]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM);
    if hunk_count > 1 {
        controls = controls
            .push(Space::with_width(Length::Fill))
            .push(hunk_nav_group(selected, hunk_count));
    }
    container(controls.width(Length::Fill))
        .width(Length::Fill)
        .clip(true)
        .into()
}

fn diff_mode_group<'a>(mode: DiffViewMode) -> Element<'a, Message> {
    let buttons = row![
        diff_mode_button("All", DiffViewMode::Unified, mode),
        diff_mode_button("Hunk", DiffViewMode::FocusedHunk, mode),
        diff_mode_button("Inline", DiffViewMode::Inline, mode),
        diff_mode_button("Split", DiffViewMode::Split, mode),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    scrollable(buttons)
        .direction(Direction::Horizontal(
            Scrollbar::new().width(0).scroller_width(0).margin(0),
        ))
        .width(Length::Shrink)
        .into()
}

fn hunk_nav_group<'a>(selected: usize, hunk_count: usize) -> Element<'a, Message> {
    row![
        hunk_nav_button(
            IconName::ChevronLeft,
            selected > 0,
            Message::DetailPreviousHunk,
        ),
        text(format!("{} / {}", selected + 1, hunk_count))
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT_MUTED),
        hunk_nav_button(
            IconName::ChevronRight,
            selected + 1 < hunk_count,
            Message::DetailNextHunk,
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_XS)
    .into()
}

fn hunk_nav_button<'a>(icon: IconName, enabled: bool, message: Message) -> Element<'a, Message> {
    button(icons::icon(
        icon,
        12,
        if enabled {
            color::TEXT_MUTED
        } else {
            color::TEXT_SUBTLE
        },
    ))
    .padding(Padding::from([2, 4]))
    .style(styles::toolbar_button)
    .on_press_maybe(enabled.then_some(message))
    .into()
}

fn diff_mode_button<'a>(
    label: &'a str,
    mode: DiffViewMode,
    current: DiffViewMode,
) -> Element<'a, Message> {
    let selected = mode == current;
    button(
        text(label)
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(if selected {
                color::TEXT
            } else {
                color::TEXT_SUBTLE
            }),
    )
    .padding(Padding::from([2, 6]))
    .style(styles::toolbar_button)
    .on_press(Message::DiffViewModeChanged(mode))
    .into()
}

fn actions_scrollable<'a>(actions: Element<'a, Message>) -> Element<'a, Message> {
    scrollable(actions)
        .direction(Direction::Horizontal(
            Scrollbar::new().width(0).scroller_width(0).margin(0),
        ))
        .width(Length::Shrink)
        .into()
}

fn file_diff<'a>(
    file: &'a FileChange,
    hunks: Option<&'a [Hunk]>,
    hl_hunks: Option<&'a [naite_core::HighlightedHunk]>,
    hunk_actions: HunkActions,
    mode: DiffViewMode,
    selected_hunk: Option<usize>,
    wip_kind: Option<WorktreeDiffKind>,
) -> Element<'a, Message> {
    if file.is_binary {
        return inset_text("Binary file");
    }

    let Some(hunks) = hunks else {
        return inset_text("No text diff for this file.");
    };

    let selected_hunk = selected_hunk
        .unwrap_or(0)
        .min(hunks.len().saturating_sub(1));
    let visible_hunks: Vec<(usize, &'a Hunk)> = match mode {
        DiffViewMode::Unified | DiffViewMode::Inline | DiffViewMode::Split => {
            hunks.iter().enumerate().collect()
        }
        DiffViewMode::FocusedHunk => hunks
            .get(selected_hunk)
            .map(|hunk| vec![(selected_hunk, hunk)])
            .unwrap_or_default(),
    };

    let mut col = column![].spacing(0);
    for (hunk_index, hunk) in visible_hunks {
        let selected = hunk_index == selected_hunk;
        let header: Element<'a, Message> = if hunk_actions.any() {
            let mut actions = row![].spacing(theme::SP_SM);
            if hunk_actions.stage {
                actions = actions.push(action_button(
                    "Stage hunk",
                    true,
                    Message::from(stage::Message::HunkRequested {
                        path: file.path.clone(),
                        hunk: hunk.clone(),
                    }),
                ));
            }
            if hunk_actions.unstage {
                actions = actions.push(action_button(
                    "Unstage hunk",
                    true,
                    Message::from(stage::Message::UnstageHunkRequested {
                        path: file.path.clone(),
                        hunk: hunk.clone(),
                    }),
                ));
            }
            if hunk_actions.discard {
                actions = actions.push(danger_action_button(
                    "Discard hunk",
                    true,
                    Message::from(discard::Message::HunkRequested {
                        path: file.path.clone(),
                        hunk: hunk.clone(),
                    }),
                ));
            }

            column![
                row![
                    hunk_index_label(hunk_index, selected),
                    no_wrap_hunk_header(hunk.header.clone()),
                ]
                .align_y(Alignment::Center)
                .spacing(theme::SP_SM),
                row![
                    Space::with_width(Length::Fill),
                    actions_scrollable(actions.into()),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill),
            ]
            .width(Length::Fill)
            .spacing(theme::SP_XS)
            .into()
        } else {
            row![
                hunk_index_label(hunk_index, selected),
                no_wrap_hunk_header(hunk.header.clone()),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_SM)
            .into()
        };

        let header_container = container(header)
            .padding(Padding::from([5, 8]))
            .width(Length::Fill)
            .style(if selected {
                styles::selected_hunk_header as fn(&iced::Theme) -> _
            } else {
                styles::inset_card as fn(&iced::Theme) -> _
            });

        let header_element: Element<'a, Message> = match wip_kind {
            Some(kind) => mouse_area(header_container)
                .on_right_press(Message::ContextMenuOpened(ContextMenuKind::HunkHeader {
                    path: file.path.clone(),
                    hunk: hunk.clone(),
                    kind,
                }))
                .into(),
            None => header_container.into(),
        };
        col = col.push(header_element);

        let mut old_line = hunk.old_start;
        let mut new_line = hunk.new_start;
        static EMPTY_HL_LINE: naite_core::HighlightedLine =
            naite_core::HighlightedLine { spans: Vec::new() };
        let hl_hunk = hl_hunks.and_then(|hs| hs.get(hunk_index));
        for (line_index, line) in hunk.lines.iter().enumerate() {
            let hl_line = hl_hunk
                .and_then(|h| h.lines.get(line_index))
                .unwrap_or(&EMPTY_HL_LINE);
            col = col.push(match mode {
                DiffViewMode::Unified | DiffViewMode::FocusedHunk => diff_line(line, hl_line),
                DiffViewMode::Inline => {
                    diff_inline_line(line, &mut old_line, &mut new_line, hl_line)
                }
                DiffViewMode::Split => diff_split_line(line, &mut old_line, &mut new_line, hl_line),
            });
        }
    }

    if file.is_truncated {
        col = col.push(
            container(
                text("Diff truncated at 50KB.")
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::WARNING),
            )
            .padding(theme::SP_SM)
            .width(Length::Fill)
            .style(styles::warning_card),
        );
    }

    col.width(Length::Fill).into()
}

fn hunk_index_label<'a>(index: usize, selected: bool) -> Element<'a, Message> {
    button(
        text(format!("#{}", index + 1))
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(if selected {
                color::ACCENT
            } else {
                color::TEXT_SUBTLE
            }),
    )
    .padding(Padding::from([2, 7]))
    .style(styles::commit_row_button(selected))
    .on_press(Message::DetailHunkSelected(index))
    .into()
}

#[derive(Debug, Clone, Copy, Default)]
struct HunkActions {
    stage: bool,
    unstage: bool,
    discard: bool,
}

impl HunkActions {
    fn for_kind(target: &WorktreeDiffTarget) -> Self {
        match target.kind {
            WorktreeDiffKind::Staged => Self {
                unstage: true,
                ..Default::default()
            },
            WorktreeDiffKind::Unstaged => Self {
                stage: true,
                discard: true,
                ..Default::default()
            },
            WorktreeDiffKind::Untracked => Self {
                stage: true,
                ..Default::default()
            },
            WorktreeDiffKind::Conflict => Self::default(),
        }
    }

    fn any(self) -> bool {
        self.stage || self.unstage || self.discard
    }
}

#[derive(Debug, Clone, Copy)]
enum DiffRowKind {
    Add,
    Del,
    Ctx,
}

fn token_spans<'a>(
    body: &'a str,
    hl: &HighlightedLine,
    row_kind: DiffRowKind,
) -> Vec<iced::widget::text::Span<'a, Message, iced::Font>> {
    use iced::widget::text::Span;
    let fallback = match row_kind {
        DiffRowKind::Add => color::SUCCESS,
        DiffRowKind::Del => color::DANGER,
        DiffRowKind::Ctx => color::TEXT_MUTED,
    };
    let alpha = match row_kind {
        DiffRowKind::Add | DiffRowKind::Del => 0.8_f32,
        DiffRowKind::Ctx => 1.0_f32,
    };
    if hl.spans.is_empty() || body.is_empty() {
        return vec![Span::new(body).color(fallback)];
    }
    let bytes = body.as_bytes();
    let mut out: Vec<Span<'a, Message, iced::Font>> = Vec::with_capacity(hl.spans.len() * 2);
    let mut cursor: usize = 0;
    for tok in &hl.spans {
        let start = tok.start as usize;
        let end = tok.end as usize;
        if end > bytes.len() || start >= end || start < cursor {
            continue;
        }
        if start > cursor {
            if let Some(slice) = body.get(cursor..start) {
                out.push(Span::new(slice).color(fallback));
            }
        }
        if let Some(slice) = body.get(start..end) {
            let color = match tok.kind {
                TokenKind::Keyword => color::with_alpha(color::SYNTAX_KEYWORD, alpha),
                TokenKind::Type => color::with_alpha(color::SYNTAX_TYPE, alpha),
                TokenKind::String => color::with_alpha(color::SYNTAX_STRING, alpha),
                TokenKind::Number => color::with_alpha(color::SYNTAX_NUMBER, alpha),
                TokenKind::Comment => color::with_alpha(color::SYNTAX_COMMENT, alpha),
                TokenKind::Function => color::with_alpha(color::SYNTAX_FUNCTION, alpha),
                TokenKind::Punct | TokenKind::Plain => fallback,
            };
            out.push(Span::new(slice).color(color));
        }
        cursor = end;
    }
    if cursor < bytes.len() {
        if let Some(slice) = body.get(cursor..) {
            out.push(Span::new(slice).color(fallback));
        }
    }
    if out.len() > naite_core::MAX_SPANS_PER_LINE {
        out.truncate(naite_core::MAX_SPANS_PER_LINE);
    }
    out
}

fn diff_line<'a>(line: &'a naite_core::DiffLine, hl: &HighlightedLine) -> Element<'a, Message> {
    use naite_core::DiffLine;
    let (prefix, value, style, text_color, row_kind) = match line {
        DiffLine::Ctx(value) => (
            " ",
            value,
            styles::diff_ctx as fn(&iced::Theme) -> _,
            color::TEXT_MUTED,
            DiffRowKind::Ctx,
        ),
        DiffLine::Add(value) => (
            "+",
            value,
            styles::diff_add as fn(&iced::Theme) -> _,
            color::SUCCESS,
            DiffRowKind::Add,
        ),
        DiffLine::Del(value) => (
            "-",
            value,
            styles::diff_del as fn(&iced::Theme) -> _,
            color::DANGER,
            DiffRowKind::Del,
        ),
    };

    let spans = token_spans(value.as_str(), hl, row_kind);
    container(
        row![
            text(prefix)
                .size(theme::FS_SM)
                .font(iced::Font::MONOSPACE)
                .wrapping(Wrapping::None)
                .color(text_color),
            no_wrap_code(spans),
        ]
        .align_y(Alignment::Center)
        .spacing(0),
    )
    .padding(Padding::from([1, 8]))
    .width(Length::Fill)
    .style(style)
    .into()
}

fn diff_inline_line<'a>(
    line: &'a naite_core::DiffLine,
    old_line: &mut u32,
    new_line: &mut u32,
    hl: &HighlightedLine,
) -> Element<'a, Message> {
    use naite_core::DiffLine;

    let (old_number, new_number) = next_line_numbers(line, old_line, new_line);
    let (prefix, value, style, text_color, row_kind) = match line {
        DiffLine::Ctx(value) => (
            " ",
            value,
            styles::diff_ctx as fn(&iced::Theme) -> _,
            color::TEXT_MUTED,
            DiffRowKind::Ctx,
        ),
        DiffLine::Add(value) => (
            "+",
            value,
            styles::diff_add as fn(&iced::Theme) -> _,
            color::SUCCESS,
            DiffRowKind::Add,
        ),
        DiffLine::Del(value) => (
            "-",
            value,
            styles::diff_del as fn(&iced::Theme) -> _,
            color::DANGER,
            DiffRowKind::Del,
        ),
    };

    let spans = token_spans(value.as_str(), hl, row_kind);
    container(
        row![
            line_number(old_number),
            line_number(new_number),
            text(prefix)
                .size(theme::FS_SM)
                .font(iced::Font::MONOSPACE)
                .wrapping(Wrapping::None)
                .color(text_color),
            no_wrap_code(spans),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_XS),
    )
    .padding(Padding::from([1, 8]))
    .width(Length::Fill)
    .style(style)
    .into()
}

fn diff_split_line<'a>(
    line: &'a naite_core::DiffLine,
    old_line: &mut u32,
    new_line: &mut u32,
    hl: &HighlightedLine,
) -> Element<'a, Message> {
    use naite_core::DiffLine;

    let (old_number, new_number) = next_line_numbers(line, old_line, new_line);
    let (
        old_value,
        old_style,
        old_color,
        old_row_kind,
        new_value,
        new_style,
        new_color,
        new_row_kind,
    ) = match line {
        DiffLine::Ctx(value) => (
            value.as_str(),
            styles::diff_ctx as fn(&iced::Theme) -> _,
            color::TEXT_MUTED,
            DiffRowKind::Ctx,
            value.as_str(),
            styles::diff_ctx as fn(&iced::Theme) -> _,
            color::TEXT_MUTED,
            DiffRowKind::Ctx,
        ),
        DiffLine::Add(value) => (
            "",
            styles::diff_ctx as fn(&iced::Theme) -> _,
            color::TEXT_SUBTLE,
            DiffRowKind::Ctx,
            value.as_str(),
            styles::diff_add as fn(&iced::Theme) -> _,
            color::SUCCESS,
            DiffRowKind::Add,
        ),
        DiffLine::Del(value) => (
            value.as_str(),
            styles::diff_del as fn(&iced::Theme) -> _,
            color::DANGER,
            DiffRowKind::Del,
            "",
            styles::diff_ctx as fn(&iced::Theme) -> _,
            color::TEXT_SUBTLE,
            DiffRowKind::Ctx,
        ),
    };

    row![
        split_diff_cell(
            old_number,
            old_value,
            old_style,
            old_color,
            old_row_kind,
            hl
        ),
        split_diff_cell(
            new_number,
            new_value,
            new_style,
            new_color,
            new_row_kind,
            hl
        ),
    ]
    .width(Length::Fill)
    .spacing(theme::SP_XS)
    .into()
}

fn split_diff_cell<'a>(
    line_number_value: Option<u32>,
    value: &'a str,
    style: fn(&iced::Theme) -> iced::widget::container::Style,
    _text_color: Color,
    row_kind: DiffRowKind,
    hl: &HighlightedLine,
) -> Element<'a, Message> {
    let spans = token_spans(value, hl, row_kind);
    container(
        row![line_number(line_number_value), no_wrap_code(spans),]
            .align_y(Alignment::Center)
            .spacing(theme::SP_XS),
    )
    .padding(Padding::from([1, 8]))
    .width(Length::FillPortion(1))
    .style(style)
    .into()
}

fn no_wrap_code<'a>(
    spans: Vec<iced::widget::text::Span<'a, Message, iced::Font>>,
) -> Element<'a, Message> {
    scrollable(rich_text(spans).size(theme::FS_SM).font(theme::font_code()))
        .direction(Direction::Horizontal(
            Scrollbar::new().width(0).scroller_width(0).margin(0),
        ))
        .width(Length::Fill)
        .into()
}

fn no_wrap_hunk_header<'a>(header: String) -> Element<'a, Message> {
    scrollable(
        text(header)
            .size(theme::FS_SM)
            .font(theme::font_code())
            .color(color::TEXT_MUTED),
    )
    .direction(Direction::Horizontal(
        Scrollbar::new().width(0).scroller_width(0).margin(0),
    ))
    .width(Length::Fill)
    .into()
}

fn line_number<'a>(value: Option<u32>) -> Element<'a, Message> {
    let label = value.map(|value| value.to_string()).unwrap_or_default();
    container(
        text(label)
            .size(theme::FS_XS)
            .font(iced::Font::MONOSPACE)
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
    )
    .width(Length::Fixed(36.0))
    .into()
}

fn next_line_numbers(
    line: &naite_core::DiffLine,
    old_line: &mut u32,
    new_line: &mut u32,
) -> (Option<u32>, Option<u32>) {
    use naite_core::DiffLine;

    match line {
        DiffLine::Ctx(_) => {
            let current = (Some(*old_line), Some(*new_line));
            *old_line += 1;
            *new_line += 1;
            current
        }
        DiffLine::Add(_) => {
            let current = (None, Some(*new_line));
            *new_line += 1;
            current
        }
        DiffLine::Del(_) => {
            let current = (Some(*old_line), None);
            *old_line += 1;
            current
        }
    }
}

/// Like [`meta_field`] but renders the value as a click-to-copy button.
/// The value styling matches the regular `mono` meta field so the row
/// reads identically; the only signal is a subtle hover background and
/// a pointer cursor.
fn copyable_meta_field<'a>(label: &'a str, value: &str) -> Element<'a, Message> {
    let value_owned = value.to_string();
    let value_text = text(value_owned.clone())
        .size(theme::FS_SM)
        .font(iced::Font::MONOSPACE)
        .color(color::TEXT);

    let copy_button = tooltip(
        button(value_text)
            .padding(Padding::from([2, 6]))
            .style(styles::ghost_icon_button)
            .on_press(Message::CopyText(value_owned)),
        container(
            text("Click to copy")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT),
        )
        .padding(Padding::from([4, 8]))
        .style(styles::inset_card),
        tooltip::Position::Bottom,
    );

    column![
        text(label.to_uppercase())
            .size(theme::FS_XS)
            .color(color::TEXT_SUBTLE)
            .font(theme::font_semibold()),
        Space::with_height(2),
        copy_button,
    ]
    .spacing(0)
    .into()
}

fn meta_field<'a>(label: &'a str, value: &str, mono: bool) -> Element<'a, Message> {
    let value_text = if mono {
        text(value.to_string())
            .size(theme::FS_SM)
            .font(iced::Font::MONOSPACE)
            .color(color::TEXT)
    } else {
        text(value.to_string())
            .size(theme::FS_BASE)
            .font(theme::font_regular())
            .color(color::TEXT)
    };

    column![
        text(label.to_uppercase())
            .size(theme::FS_XS)
            .color(color::TEXT_SUBTLE)
            .font(theme::font_semibold()),
        Space::with_height(2),
        value_text,
    ]
    .spacing(0)
    .into()
}

pub(super) fn status_label(status: ChangeStatus) -> &'static str {
    match status {
        ChangeStatus::Added => "A",
        ChangeStatus::Modified => "M",
        ChangeStatus::Deleted => "D",
        ChangeStatus::Renamed => "R",
        ChangeStatus::Copied => "C",
    }
}
