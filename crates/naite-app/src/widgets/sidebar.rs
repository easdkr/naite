//! Repository sidebar: repo manager and refs sections.

use std::collections::BTreeMap;
use std::path::Path;

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text::Wrapping,
    text_input, Space,
};
use iced::{mouse, Alignment, Color, Element, Length, Padding};
use naite_core::{
    GitHubIssueFilter, GitHubIssueSummary, PullRequestFilter, PullRequestSummary, RefKind,
    RefSummary, Refs, StashSummary, WorktreeSummary,
};

use crate::features::{github_issue, pull_request, worktree};
use crate::icons::{self, IconName};
use crate::state::{
    ContextMenuKind, RepositoryCatalog, RepositoryTabsState, SidebarSection, SidebarState,
};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::repo_manager::repository_manager;
use super::tab_strip::tab_strip;

const TREE_CHEVRON_ICON_SIZE: u16 = 12;
const TREE_FOLDER_ICON_SIZE: u16 = 15;
const SIDEBAR_REF_ROW_HEIGHT: f32 = 26.0;

pub struct SidebarProps<'a> {
    pub repo_path: Option<&'a Path>,
    pub refs: &'a Refs,
    pub pull_requests: &'a [PullRequestSummary],
    pub github_issues: &'a [GitHubIssueSummary],
    pub pull_request_filter: PullRequestFilter,
    pub pull_request_search_query: &'a str,
    pub pull_request_loading: bool,
    pub pull_request_error: Option<&'a str>,
    pub pull_request_search_input_id: &'a text_input::Id,
    pub github_issue_filter: GitHubIssueFilter,
    pub github_issue_search_query: &'a str,
    pub github_issue_loading: bool,
    pub github_issue_error: Option<&'a str>,
    pub actions_disabled: bool,
    pub head_branch: Option<&'a str>,
    pub stashes: &'a [StashSummary],
    pub worktrees: &'a [WorktreeSummary],
    pub selected_pull_request_number: Option<u32>,
    pub selected_github_issue_number: Option<u32>,
    pub selected_stash_selector: Option<&'a str>,
    pub selected_worktree_path: Option<&'a Path>,
    pub catalog: &'a RepositoryCatalog,
    pub tabs: &'a RepositoryTabsState,
    pub sidebar_state: &'a SidebarState,
    pub clone_url: &'a str,
    pub clone_open: bool,
    pub new_repo_menu_open: bool,
}

pub fn sidebar<'a>(props: SidebarProps<'a>) -> Element<'a, Message> {
    let SidebarProps {
        repo_path,
        refs,
        pull_requests,
        github_issues,
        pull_request_filter,
        pull_request_search_query,
        pull_request_loading,
        pull_request_error,
        pull_request_search_input_id,
        github_issue_filter,
        github_issue_search_query,
        github_issue_loading,
        github_issue_error,
        actions_disabled,
        head_branch,
        stashes,
        worktrees,
        selected_pull_request_number,
        selected_github_issue_number,
        selected_stash_selector,
        selected_worktree_path,
        catalog,
        tabs,
        sidebar_state,
        clone_url,
        clone_open,
        new_repo_menu_open,
    } = props;
    let manager = repository_manager(repo_path, catalog, sidebar_state, clone_url, clone_open);

    let tabs_block: Element<'a, Message> = column![
        tab_strip(tabs, new_repo_menu_open),
        container(Space::with_height(Length::Fixed(1.0)))
            .width(Length::Fill)
            .style(styles::hairline_divider),
    ]
    .spacing(theme::SP_XS)
    .into();

    let body: Element<'a, Message> = if repo_path.is_some() {
        column![
            tabs_block,
            manager,
            section(
                "Local",
                IconName::GitBranch,
                SidebarSection::LocalBranches,
                sidebar_state.is_expanded(SidebarSection::LocalBranches),
                refs.local.len(),
                ref_tree_to_items(&refs.local, SidebarSection::LocalBranches, sidebar_state),
            ),
            section(
                "Remotes",
                IconName::Cloud,
                SidebarSection::RemoteBranches,
                sidebar_state.is_expanded(SidebarSection::RemoteBranches),
                refs.remote.len(),
                ref_tree_to_items(&refs.remote, SidebarSection::RemoteBranches, sidebar_state),
            ),
            section(
                "Pull Requests",
                IconName::GitCommit,
                SidebarSection::PullRequests,
                sidebar_state.is_expanded(SidebarSection::PullRequests),
                pull_requests.len(),
                pull_request_to_items(PullRequestItemsProps {
                    pull_requests,
                    filter: pull_request_filter,
                    search_query: pull_request_search_query,
                    loading: pull_request_loading,
                    error: pull_request_error,
                    search_input_id: pull_request_search_input_id,
                    actions_disabled,
                    has_branch: head_branch.is_some(),
                    selected_number: selected_pull_request_number,
                }),
            ),
            section(
                "Issues",
                IconName::GitCommit,
                SidebarSection::Issues,
                sidebar_state.is_expanded(SidebarSection::Issues),
                github_issues.len(),
                github_issue_to_items(GitHubIssueItemsProps {
                    issues: github_issues,
                    filter: github_issue_filter,
                    search_query: github_issue_search_query,
                    loading: github_issue_loading,
                    error: github_issue_error,
                    actions_disabled,
                    selected_number: selected_github_issue_number,
                }),
            ),
            section(
                "Tags",
                IconName::Tag,
                SidebarSection::Tags,
                sidebar_state.is_expanded(SidebarSection::Tags),
                refs.tags.len(),
                refs_to_items(&refs.tags, sidebar_state),
            ),
            section(
                "Stashes",
                IconName::GitCommit,
                SidebarSection::Stashes,
                sidebar_state.is_expanded(SidebarSection::Stashes),
                stashes.len(),
                stash_to_items(stashes, selected_stash_selector),
            ),
            section(
                "Worktrees",
                IconName::FolderOpen,
                SidebarSection::Worktrees,
                sidebar_state.is_expanded(SidebarSection::Worktrees),
                worktrees.len(),
                worktree_to_items(worktrees, selected_worktree_path),
            ),
        ]
        .spacing(theme::SP_LG)
        .into()
    } else {
        column![tabs_block, manager].spacing(theme::SP_LG).into()
    };

    container(
        scrollable(body)
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::from([theme::SP_SM, 0]))
    .style(styles::surface_panel)
    .into()
}

fn section<'a>(
    label: &'a str,
    icon: IconName,
    section: SidebarSection,
    expanded: bool,
    count: usize,
    items: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut col = column![collapsible_section_header(
        label, icon, section, expanded, count
    )]
    .spacing(0);
    if !expanded {
        return col.into();
    }

    if items.is_empty() {
        col = col.push(
            container(
                text("None")
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_SUBTLE),
            )
            .padding(Padding::from([2, theme::SP_LG])),
        );
    } else {
        for it in items {
            col = col.push(it);
        }
    }
    col.into()
}

fn collapsible_section_header<'a>(
    label: &'a str,
    icon: IconName,
    section: SidebarSection,
    expanded: bool,
    count: usize,
) -> Element<'a, Message> {
    let indicator = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    button(
        row![
            icons::icon(indicator, TREE_CHEVRON_ICON_SIZE, color::TEXT_SUBTLE),
            icons::icon(icon, 13, color::TEXT_SUBTLE),
            text(label.to_uppercase())
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
    .on_press(Message::SidebarSectionToggled(section))
    .into()
}

fn refs_to_items<'a>(
    refs: &'a [RefSummary],
    sidebar_state: &'a SidebarState,
) -> Vec<Element<'a, Message>> {
    refs.iter()
        .map(|ref_summary| sidebar_item(ref_summary, &ref_summary.short_name, 0, sidebar_state))
        .collect()
}

fn ref_tree_to_items<'a>(
    refs: &'a [RefSummary],
    section: SidebarSection,
    sidebar_state: &'a SidebarState,
) -> Vec<Element<'a, Message>> {
    let tree = RefTreeNode::from_refs(refs);
    let mut items = Vec::new();
    for (name, node) in tree.children {
        render_ref_tree_node(name, node, 0, section, "", sidebar_state, &mut items);
    }
    items
}

fn stash_to_items<'a>(
    stashes: &'a [StashSummary],
    selected_selector: Option<&'a str>,
) -> Vec<Element<'a, Message>> {
    stashes
        .iter()
        .map(|stash| stash_item(stash, selected_selector == Some(stash.selector.as_str())))
        .collect()
}

struct PullRequestItemsProps<'a> {
    pull_requests: &'a [PullRequestSummary],
    filter: PullRequestFilter,
    search_query: &'a str,
    loading: bool,
    error: Option<&'a str>,
    search_input_id: &'a text_input::Id,
    actions_disabled: bool,
    has_branch: bool,
    selected_number: Option<u32>,
}

struct GitHubIssueItemsProps<'a> {
    issues: &'a [GitHubIssueSummary],
    filter: GitHubIssueFilter,
    search_query: &'a str,
    loading: bool,
    error: Option<&'a str>,
    actions_disabled: bool,
    selected_number: Option<u32>,
}

fn pull_request_to_items<'a>(props: PullRequestItemsProps<'a>) -> Vec<Element<'a, Message>> {
    let PullRequestItemsProps {
        pull_requests,
        filter,
        search_query,
        loading,
        error,
        search_input_id,
        actions_disabled,
        has_branch,
        selected_number,
    } = props;

    let _ = actions_disabled;
    let mut items = vec![
        pull_request_controls(loading, actions_disabled, has_branch),
        pull_request_filters(filter, loading, search_query, search_input_id),
    ];
    if loading {
        items.push(sidebar_note("Refreshing pull requests..."));
    }
    if let Some(error) = error {
        items.push(sidebar_note(error));
    }
    items.extend(pull_requests.iter().map(|pull_request| {
        pull_request_item(pull_request, selected_number == Some(pull_request.number))
    }));
    items
}

fn github_issue_to_items<'a>(props: GitHubIssueItemsProps<'a>) -> Vec<Element<'a, Message>> {
    let GitHubIssueItemsProps {
        issues,
        filter,
        search_query,
        loading,
        error,
        actions_disabled,
        selected_number,
    } = props;

    let mut items = vec![
        github_issue_controls(loading, actions_disabled),
        github_issue_filters(filter, loading, search_query),
    ];
    if loading {
        items.push(sidebar_note("Refreshing GitHub issues..."));
    }
    if let Some(error) = error {
        items.push(sidebar_note(error));
    }
    items.extend(
        issues
            .iter()
            .map(|issue| github_issue_item(issue, selected_number == Some(issue.number))),
    );
    items
}

fn github_issue_controls<'a>(loading: bool, actions_disabled: bool) -> Element<'a, Message> {
    container(
        row![
            compact_action_button(
                "Refresh",
                !loading && !actions_disabled,
                Message::from(github_issue::Message::RefreshRequested),
            ),
            Space::with_width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([3, theme::SP_MD]))
    .into()
}

fn github_issue_filters<'a>(
    filter: GitHubIssueFilter,
    loading: bool,
    search_query: &'a str,
) -> Element<'a, Message> {
    container(
        column![
            text_input("Search issues", search_query)
                .on_input(|query| {
                    Message::from(github_issue::Message::SearchQueryChanged(query))
                })
                .on_submit(Message::from(github_issue::Message::SearchSubmitted))
                .padding(Padding::from([6, 10]))
                .size(theme::FS_SM)
                .width(Length::Fill),
            row![
                issue_filter_chip("Open", GitHubIssueFilter::Open, filter, loading),
                issue_filter_chip("Assigned", GitHubIssueFilter::Assigned, filter, loading),
                issue_filter_chip("Mentioned", GitHubIssueFilter::Mentioned, filter, loading),
                issue_filter_chip("Closed", GitHubIssueFilter::Closed, filter, loading),
            ]
            .align_y(Alignment::Center)
            .spacing(4),
        ]
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([0, theme::SP_MD]))
    .into()
}

fn issue_filter_chip<'a>(
    label: &'static str,
    target: GitHubIssueFilter,
    current: GitHubIssueFilter,
    loading: bool,
) -> Element<'a, Message> {
    let active = current == target;
    button(
        text(label)
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
        (!loading).then_some(Message::from(github_issue::Message::FilterChanged(target))),
    )
    .into()
}

fn github_issue_item<'a>(issue: &'a GitHubIssueSummary, selected: bool) -> Element<'a, Message> {
    let title_color = if selected {
        color::TEXT
    } else {
        color::TEXT_MUTED
    };
    let title_font = if selected {
        theme::font_semibold()
    } else {
        theme::font_regular()
    };

    let title_column: Element<'a, Message> = container(
        text(issue.title.clone())
            .size(theme::FS_SM)
            .font(title_font)
            .color(title_color)
            .wrapping(Wrapping::None),
    )
    .width(Length::Fill)
    .clip(true)
    .into();

    let kebab = button(icons::icon(IconName::DotsVertical, 13, color::TEXT_SUBTLE))
        .padding(Padding::from([4, 6]))
        .style(styles::commit_row_button(false))
        .on_press(Message::ContextMenuOpened(ContextMenuKind::GitHubIssue(
            issue.clone(),
        )));

    let content = row![
        Space::with_width(Length::Fixed(theme::SP_SM as f32)),
        text(format!("#{}", issue.number))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE)
            .wrapping(Wrapping::None),
        title_column,
        text(issue.state.clone())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::TEXT_SUBTLE)
            .wrapping(Wrapping::None),
        kebab,
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let pressable = button(content)
        .padding(Padding::from([5, theme::SP_MD]))
        .width(Length::Fill)
        .style(styles::commit_row_button(selected))
        .on_press(Message::from(github_issue::Message::Selected(
            issue.clone(),
        )));

    mouse_area(pressable)
        .on_right_press(Message::ContextMenuOpened(ContextMenuKind::GitHubIssue(
            issue.clone(),
        )))
        .into()
}

fn pull_request_controls<'a>(
    loading: bool,
    actions_disabled: bool,
    has_branch: bool,
) -> Element<'a, Message> {
    container(
        row![
            compact_action_button(
                "Refresh",
                !loading,
                Message::from(pull_request::Message::RefreshRequested),
            ),
            Space::with_width(Length::Fill),
            compact_action_button(
                "Create PR",
                !loading && !actions_disabled && has_branch,
                Message::from(pull_request::Message::CreateRequested),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([3, theme::SP_MD]))
    .into()
}

fn pull_request_filters<'a>(
    filter: PullRequestFilter,
    loading: bool,
    search_query: &'a str,
    search_input_id: &'a text_input::Id,
) -> Element<'a, Message> {
    container(
        column![
            text_input("Search pull requests", search_query)
                .id(search_input_id.clone())
                .on_input(|query| {
                    Message::from(pull_request::Message::SearchQueryChanged(query))
                })
                .on_submit(Message::from(pull_request::Message::SearchSubmitted))
                .padding(Padding::from([6, 10]))
                .size(theme::FS_SM)
                .width(Length::Fill),
            row![
                filter_chip("All", PullRequestFilter::All, filter, loading),
                filter_chip("Mine", PullRequestFilter::Mine, filter, loading),
                filter_chip("Review", PullRequestFilter::NeedsReview, filter, loading),
                filter_chip("Draft", PullRequestFilter::Draft, filter, loading),
                filter_chip("Failing", PullRequestFilter::FailingChecks, filter, loading),
                filter_chip("Branch", PullRequestFilter::CurrentBranch, filter, loading),
            ]
            .align_y(Alignment::Center)
            .spacing(4),
        ]
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([0, theme::SP_MD]))
    .into()
}

fn filter_chip<'a>(
    label: &'static str,
    target: PullRequestFilter,
    current: PullRequestFilter,
    loading: bool,
) -> Element<'a, Message> {
    let active = current == target;
    button(
        text(label)
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
        (!loading).then_some(Message::from(pull_request::Message::FilterChanged(target))),
    )
    .into()
}

fn pull_request_item<'a>(
    pull_request: &'a PullRequestSummary,
    selected: bool,
) -> Element<'a, Message> {
    let (title_color, title_font) = if selected {
        (color::TEXT, theme::font_semibold())
    } else {
        (color::TEXT_MUTED, theme::font_regular())
    };

    let kebab = button(icons::icon(IconName::DotsVertical, 13, color::TEXT_SUBTLE))
        .padding(Padding::from([4, 6]))
        .style(styles::commit_row_button(false))
        .on_press(Message::ContextMenuOpened(ContextMenuKind::PullRequest(
            pull_request.clone(),
        )));

    // Title column wraps text in a Fill-width container so iced gives the
    // text widget a definite width budget. Inner text is Shrink with
    // Wrapping::None so it renders single-line and clips on overflow —
    // putting Wrapping::None on a Fill-width text widget directly leaks
    // through to row layout and produces per-character wrapping when the
    // remaining space is small (long branch + pill + kebab).
    let title_column: Element<'a, Message> = container(
        text(pull_request.title.clone())
            .size(theme::FS_SM)
            .font(title_font)
            .color(title_color)
            .wrapping(Wrapping::None),
    )
    .width(Length::Fill)
    .clip(true)
    .into();

    let content = row![
        Space::with_width(Length::Fixed(theme::SP_SM as f32)),
        super::pills::status_dot(super::pills::ci_status_color(pull_request.ci_status)),
        text(format!("#{}", pull_request.number))
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE)
            .wrapping(Wrapping::None),
        title_column,
        super::pills::review_pill(pull_request.review_status),
        kebab,
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let pressable = button(content)
        .padding(Padding::from([5, theme::SP_MD]))
        .width(Length::Fill)
        .style(styles::commit_row_button(selected))
        .on_press(Message::from(pull_request::Message::Selected(
            pull_request.clone(),
        )));

    mouse_area(pressable)
        .on_right_press(Message::ContextMenuOpened(ContextMenuKind::PullRequest(
            pull_request.clone(),
        )))
        .into()
}

fn worktree_to_items<'a>(
    worktrees: &'a [WorktreeSummary],
    selected_path: Option<&'a Path>,
) -> Vec<Element<'a, Message>> {
    let mut items = vec![button(
        row![
            Space::with_width(Length::Fixed(theme::SP_SM as f32)),
            icons::icon(IconName::FolderOpen, 13, color::TEXT_SUBTLE),
            text("Create worktree")
                .size(theme::FS_SM)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED),
            Space::with_width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .padding(Padding::from([5, theme::SP_MD]))
    .width(Length::Fill)
    .style(styles::commit_row_button(false))
    .on_press(Message::from(worktree::Message::CreateRequested))
    .into()];

    items.extend(
        worktrees.iter().map(|worktree| {
            worktree_item(worktree, selected_path == Some(worktree.path.as_path()))
        }),
    );
    items
}

fn render_ref_tree_node<'a>(
    name: &'a str,
    node: RefTreeNode<'a>,
    depth: usize,
    section: SidebarSection,
    parent_path: &str,
    sidebar_state: &'a SidebarState,
    items: &mut Vec<Element<'a, Message>>,
) {
    if node.children.is_empty() {
        if let Some(ref_summary) = node.ref_summary {
            items.push(sidebar_item(ref_summary, name, depth, sidebar_state));
        }
        return;
    }

    let path = if parent_path.is_empty() {
        name.to_string()
    } else {
        format!("{parent_path}/{name}")
    };
    let expanded = sidebar_state.is_tree_folder_expanded(section, &path);
    let local_branches = (section == SidebarSection::LocalBranches)
        .then(|| node.deletable_local_refs())
        .filter(|branches| !branches.is_empty());
    let remote_branches = (section == SidebarSection::RemoteBranches)
        .then(|| node.deletable_remote_refs())
        .filter(|branches| !branches.is_empty());
    let folder_actions = BranchFolderActions {
        local_branches,
        remote_branches,
    };
    items.push(folder_item(
        name,
        depth,
        node.is_head(),
        expanded,
        section,
        &path,
        folder_actions,
    ));

    if !expanded {
        return;
    }

    if let Some(ref_summary) = node.ref_summary {
        items.push(sidebar_item(ref_summary, name, depth + 1, sidebar_state));
    }

    for (child_name, child) in node.children {
        render_ref_tree_node(
            child_name,
            child,
            depth + 1,
            section,
            &path,
            sidebar_state,
            items,
        );
    }
}

fn sidebar_item<'a>(
    ref_summary: &'a RefSummary,
    label_text: &'a str,
    depth: usize,
    sidebar_state: &'a SidebarState,
) -> Element<'a, Message> {
    let active = ref_summary.is_head;
    let hovered = sidebar_state.is_ref_hovered(ref_summary);
    let bar_bg = if active {
        color::ACCENT
    } else {
        Color::TRANSPARENT
    };
    let bar = container(Space::new(Length::Fixed(2.0), Length::Fixed(20.0)))
        .style(styles::solid_bar(bar_bg));

    let label: Element<'a, Message> = container(
        text(label_text.to_string())
            .size(theme::FS_BASE)
            .wrapping(Wrapping::None)
            .color(if active {
                color::TEXT
            } else {
                color::TEXT_MUTED
            })
            .font(if active {
                theme::font_semibold()
            } else {
                theme::font_regular()
            }),
    )
    .width(Length::Fill)
    .clip(true)
    .into();

    let icon = ref_summary.icon_name();
    let mut content_row = row![
        bar,
        Space::with_width(Length::Fixed((theme::SP_SM + depth as u16 * 12) as f32)),
        icons::icon(icon, 13, color::TEXT_SUBTLE),
        label,
    ];
    for badge in sidebar_sync_badge_specs(ref_summary) {
        content_row = content_row.push(sync_badge(badge));
    }

    let content = container(content_row.align_y(Alignment::Center).spacing(theme::SP_SM))
        .width(Length::Fill)
        .center_y(Length::Fixed(SIDEBAR_REF_ROW_HEIGHT));

    let pressable = button(content)
        .padding(Padding::from([0, theme::SP_MD]))
        .width(Length::Fill)
        .height(Length::Fixed(SIDEBAR_REF_ROW_HEIGHT))
        .style(styles::sidebar_ref_button(hovered))
        .on_press(Message::SidebarRefPressed(ref_summary.clone()));

    mouse_area(pressable)
        .on_enter(Message::SidebarRefHovered(ref_summary.clone()))
        .on_exit(Message::SidebarRefUnhovered(ref_summary.clone()))
        .on_right_press(Message::ContextMenuOpened(ContextMenuKind::Ref(
            ref_summary.clone(),
        )))
        .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SidebarSyncBadgeSpec {
    kind: SidebarSyncBadgeKind,
    count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarSyncBadgeKind {
    Pull,
    Push,
}

impl SidebarSyncBadgeSpec {
    fn label(self) -> String {
        match self.kind {
            SidebarSyncBadgeKind::Pull => format!("pull {}", self.count),
            SidebarSyncBadgeKind::Push => format!("push {}", self.count),
        }
    }

    fn accent(self) -> Color {
        match self.kind {
            SidebarSyncBadgeKind::Pull => color::WARNING,
            SidebarSyncBadgeKind::Push => color::SUCCESS,
        }
    }
}

fn sidebar_sync_badge_specs(ref_summary: &RefSummary) -> Vec<SidebarSyncBadgeSpec> {
    if ref_summary.kind != RefKind::LocalBranch {
        return Vec::new();
    }

    let Some(sync_status) = &ref_summary.sync_status else {
        return Vec::new();
    };
    let mut badges = Vec::new();
    if sync_status.behind > 0 {
        badges.push(SidebarSyncBadgeSpec {
            kind: SidebarSyncBadgeKind::Pull,
            count: sync_status.behind,
        });
    }
    if sync_status.ahead > 0 {
        badges.push(SidebarSyncBadgeSpec {
            kind: SidebarSyncBadgeKind::Push,
            count: sync_status.ahead,
        });
    }
    badges
}

fn sync_badge<'a>(badge: SidebarSyncBadgeSpec) -> Element<'a, Message> {
    let accent = badge.accent();
    container(
        text(badge.label())
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(accent),
    )
    .padding(Padding::from([2, 6]))
    .style(styles::status_badge(accent))
    .into()
}

fn stash_item<'a>(stash: &'a StashSummary, selected: bool) -> Element<'a, Message> {
    let title = if stash.message.is_empty() {
        stash.selector.clone()
    } else {
        format!("{} {}", stash.selector, stash.message)
    };
    let detail = if stash.branch.is_empty() {
        stash.date.clone()
    } else {
        format!("{} · {}", stash.branch, stash.date)
    };

    let content = row![
        Space::with_width(Length::Fixed(theme::SP_SM as f32)),
        icons::icon(IconName::GitCommit, 13, color::TEXT_SUBTLE),
        column![
            text(title)
                .size(theme::FS_SM)
                .wrapping(Wrapping::None)
                .font(if selected {
                    theme::font_semibold()
                } else {
                    theme::font_regular()
                })
                .color(if selected {
                    color::TEXT
                } else {
                    color::TEXT_MUTED
                }),
            text(detail)
                .size(theme::FS_XS)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE),
        ]
        .spacing(1)
        .width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let pressable = button(content)
        .padding(Padding::from([5, theme::SP_MD]))
        .width(Length::Fill)
        .style(styles::commit_row_button(selected))
        .on_press(Message::StashSelected(stash.clone()));

    mouse_area(pressable)
        .on_right_press(Message::ContextMenuOpened(ContextMenuKind::Stash(
            stash.clone(),
        )))
        .into()
}

fn worktree_item<'a>(worktree: &'a WorktreeSummary, selected: bool) -> Element<'a, Message> {
    let title = worktree
        .branch
        .clone()
        .unwrap_or_else(|| format!("HEAD {}", worktree.head_short_id));
    let mut detail = worktree.path.display().to_string();
    let mut badges = Vec::new();
    if worktree.is_current {
        badges.push("current");
    }
    if worktree.dirty {
        badges.push("dirty");
    }
    if worktree.locked {
        badges.push("locked");
    }
    if worktree.ahead > 0 || worktree.behind > 0 {
        badges.push("sync");
    }
    if !badges.is_empty() {
        detail = format!("{} · {}", badges.join(" / "), detail);
    }

    let tint = if worktree.locked {
        color::WARNING
    } else if worktree.dirty {
        color::ACCENT
    } else {
        color::TEXT_SUBTLE
    };

    let content = row![
        Space::with_width(Length::Fixed(theme::SP_SM as f32)),
        icons::icon(IconName::FolderOpen, 13, tint),
        column![
            text(title)
                .size(theme::FS_SM)
                .wrapping(Wrapping::None)
                .font(if selected {
                    theme::font_semibold()
                } else {
                    theme::font_regular()
                })
                .color(if selected {
                    color::TEXT
                } else {
                    color::TEXT_MUTED
                }),
            text(detail)
                .size(theme::FS_XS)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE),
        ]
        .spacing(1)
        .width(Length::Fill),
        worktree_action_button("Open", worktree::Message::OpenRequested(worktree.clone())),
        if worktree.locked {
            worktree_action_button(
                "Unlock",
                worktree::Message::UnlockRequested(worktree.clone()),
            )
        } else {
            worktree_action_button("Lock", worktree::Message::LockRequested(worktree.clone()))
        },
        worktree_action_button(
            "Remove",
            worktree::Message::RemoveRequested(worktree.clone())
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let pressable = button(content)
        .padding(Padding::from([5, theme::SP_MD]))
        .width(Length::Fill)
        .style(styles::commit_row_button(selected))
        .on_press(Message::from(worktree::Message::Selected(worktree.clone())));

    mouse_area(pressable)
        .on_right_press(Message::ContextMenuOpened(ContextMenuKind::Worktree(
            worktree.clone(),
        )))
        .into()
}

fn worktree_action_button<'a>(
    label: &'static str,
    message: worktree::Message,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([3, 6]))
    .style(styles::subtle_button)
    .on_press(Message::from(message))
    .into()
}

fn sidebar_note<'a>(value: &'a str) -> Element<'a, Message> {
    container(
        text(value)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .color(color::TEXT_SUBTLE),
    )
    .padding(Padding::from([2, theme::SP_LG]))
    .width(Length::Fill)
    .into()
}

fn compact_action_button<'a>(
    label: &'static str,
    enabled: bool,
    message: Message,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([3, 6]))
    .style(styles::subtle_button)
    .on_press_maybe(enabled.then_some(message))
    .into()
}

fn folder_item<'a>(
    name: &'a str,
    depth: usize,
    active: bool,
    expanded: bool,
    section: SidebarSection,
    path: &str,
    branch_actions: BranchFolderActions,
) -> Element<'a, Message> {
    let indicator = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let pressable = button(
        row![
            Space::with_width(Length::Fixed(2.0)),
            Space::with_width(Length::Fixed((theme::SP_SM + depth as u16 * 12) as f32)),
            icons::icon(indicator, TREE_CHEVRON_ICON_SIZE, color::TEXT_SUBTLE),
            icons::icon(
                IconName::FolderOpen,
                TREE_FOLDER_ICON_SIZE,
                color::TEXT_SUBTLE
            ),
            text(name.to_string())
                .size(theme::FS_BASE)
                .wrapping(Wrapping::None)
                .color(if active {
                    color::TEXT
                } else {
                    color::TEXT_SUBTLE
                })
                .font(if active {
                    theme::font_semibold()
                } else {
                    theme::font_regular()
                }),
            Space::with_width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_SM)
        .height(Length::Fixed(24.0)),
    )
    .width(Length::Fill)
    .padding(Padding::from([0, theme::SP_MD]))
    .style(styles::commit_row_button(false))
    .on_press(Message::SidebarTreeFolderToggled {
        section,
        path: path.to_string(),
    });

    let pointer_idle_layer = mouse_area(Space::new(Length::Fill, Length::Fixed(24.0)))
        .interaction(mouse::Interaction::Idle);
    let item = stack![pressable, pointer_idle_layer]
        .width(Length::Fill)
        .height(Length::Fixed(24.0));

    if let Some(branches) = branch_actions.local_branches {
        mouse_area(item)
            .on_right_press(Message::ContextMenuOpened(
                ContextMenuKind::LocalBranchFolder {
                    label: format!("{path}/"),
                    branches,
                },
            ))
            .into()
    } else if let Some(branches) = branch_actions.remote_branches {
        mouse_area(item)
            .on_right_press(Message::ContextMenuOpened(
                ContextMenuKind::RemoteBranchFolder {
                    label: format!("{path}/"),
                    branches,
                },
            ))
            .into()
    } else {
        item.into()
    }
}

struct BranchFolderActions {
    local_branches: Option<Vec<RefSummary>>,
    remote_branches: Option<Vec<RefSummary>>,
}

#[derive(Default)]
struct RefTreeNode<'a> {
    ref_summary: Option<&'a RefSummary>,
    children: BTreeMap<&'a str, RefTreeNode<'a>>,
}

impl<'a> RefTreeNode<'a> {
    fn from_refs(refs: &'a [RefSummary]) -> Self {
        let mut root = Self::default();
        for ref_summary in refs {
            root.insert(ref_summary);
        }
        root
    }

    fn insert(&mut self, ref_summary: &'a RefSummary) {
        let mut current = self;
        for segment in ref_summary
            .short_name
            .split('/')
            .filter(|segment| !segment.is_empty())
        {
            current = current.children.entry(segment).or_default();
        }
        current.ref_summary = Some(ref_summary);
    }

    fn is_head(&self) -> bool {
        self.ref_summary
            .map(|ref_summary| ref_summary.is_head)
            .unwrap_or(false)
            || self.children.values().any(RefTreeNode::is_head)
    }

    fn deletable_remote_refs(&self) -> Vec<RefSummary> {
        let mut refs = Vec::new();
        self.collect_deletable_remote_refs(&mut refs);
        refs
    }

    fn deletable_local_refs(&self) -> Vec<RefSummary> {
        let mut refs = Vec::new();
        self.collect_deletable_local_refs(&mut refs);
        refs
    }

    fn collect_deletable_local_refs(&self, refs: &mut Vec<RefSummary>) {
        if let Some(ref_summary) = self.ref_summary {
            if ref_summary.kind == RefKind::LocalBranch && !ref_summary.is_head {
                refs.push(ref_summary.clone());
            }
        }
        for child in self.children.values() {
            child.collect_deletable_local_refs(refs);
        }
    }

    fn collect_deletable_remote_refs(&self, refs: &mut Vec<RefSummary>) {
        if let Some(ref_summary) = self.ref_summary {
            if is_deletable_remote_branch(ref_summary) {
                refs.push(ref_summary.clone());
            }
        }
        for child in self.children.values() {
            child.collect_deletable_remote_refs(refs);
        }
    }
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

trait RefSummaryUi {
    fn icon_name(&self) -> IconName;
}

#[cfg(test)]
fn ref_tree_debug_rows(refs: &[RefSummary]) -> Vec<(usize, String, bool)> {
    fn collect<'a>(
        name: &'a str,
        node: RefTreeNode<'a>,
        depth: usize,
        rows: &mut Vec<(usize, String, bool)>,
    ) {
        if node.children.is_empty() {
            if let Some(ref_summary) = node.ref_summary {
                rows.push((depth, name.to_string(), ref_summary.is_head));
            }
            return;
        }

        rows.push((depth, format!("{name}/"), node.is_head()));
        if let Some(ref_summary) = node.ref_summary {
            rows.push((depth + 1, name.to_string(), ref_summary.is_head));
        }
        for (child_name, child) in node.children {
            collect(child_name, child, depth + 1, rows);
        }
    }

    let tree = RefTreeNode::from_refs(refs);
    let mut rows = Vec::new();
    for (name, node) in tree.children {
        collect(name, node, 0, &mut rows);
    }
    rows
}

impl RefSummaryUi for RefSummary {
    fn icon_name(&self) -> IconName {
        match self.kind {
            naite_core::RefKind::LocalBranch => IconName::GitBranch,
            naite_core::RefKind::RemoteBranch => IconName::Cloud,
            naite_core::RefKind::Tag => IconName::Tag,
        }
    }
}

#[cfg(test)]
pub(crate) fn is_checkout_supported(ref_summary: &RefSummary) -> bool {
    matches!(
        ref_summary.kind,
        naite_core::RefKind::LocalBranch | naite_core::RefKind::RemoteBranch
    )
}

#[cfg(test)]
mod tests {
    use naite_core::{BranchSyncStatus, RefKind, RefSummary};

    use super::{ref_tree_debug_rows, sidebar_sync_badge_specs, SidebarSyncBadgeKind};

    fn branch(name: &str, is_head: bool) -> RefSummary {
        RefSummary {
            kind: RefKind::LocalBranch,
            short_name: name.into(),
            full_name: format!("refs/heads/{name}"),
            target_short_id: "abc1234".into(),
            is_head,
            sync_status: None,
        }
    }

    fn synced_branch(kind: RefKind, name: &str, ahead: u32, behind: u32) -> RefSummary {
        let prefix = match kind {
            RefKind::LocalBranch => "refs/heads",
            RefKind::RemoteBranch => "refs/remotes",
            RefKind::Tag => "refs/tags",
        };

        RefSummary {
            kind,
            short_name: name.into(),
            full_name: format!("{prefix}/{name}"),
            target_short_id: "abc1234".into(),
            is_head: false,
            sync_status: Some(BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead,
                behind,
            }),
        }
    }

    #[test]
    fn branch_tree_groups_slash_separated_ref_names() {
        let rows = ref_tree_debug_rows(&[
            branch("feature/auth/login", false),
            branch("feature/cart", true),
            branch("main", false),
        ]);

        assert_eq!(
            rows,
            vec![
                (0, "feature/".into(), true),
                (1, "auth/".into(), false),
                (2, "login".into(), false),
                (1, "cart".into(), true),
                (0, "main".into(), false),
            ]
        );
    }

    #[test]
    fn sidebar_sync_badges_use_pull_then_push_counts_for_local_branches() {
        let behind = synced_branch(RefKind::LocalBranch, "main", 0, 2);
        let ahead = synced_branch(RefKind::LocalBranch, "feature/demo", 1, 0);
        let diverged = synced_branch(RefKind::LocalBranch, "release", 3, 4);

        assert_eq!(
            sidebar_sync_badge_specs(&behind),
            vec![super::SidebarSyncBadgeSpec {
                kind: SidebarSyncBadgeKind::Pull,
                count: 2,
            }]
        );
        assert_eq!(
            sidebar_sync_badge_specs(&ahead),
            vec![super::SidebarSyncBadgeSpec {
                kind: SidebarSyncBadgeKind::Push,
                count: 1,
            }]
        );
        assert_eq!(
            sidebar_sync_badge_specs(&diverged),
            vec![
                super::SidebarSyncBadgeSpec {
                    kind: SidebarSyncBadgeKind::Pull,
                    count: 4,
                },
                super::SidebarSyncBadgeSpec {
                    kind: SidebarSyncBadgeKind::Push,
                    count: 3,
                },
            ]
        );
    }

    #[test]
    fn sidebar_sync_badges_ignore_remote_branches_tags_and_synced_branches() {
        let remote = synced_branch(RefKind::RemoteBranch, "origin/main", 1, 2);
        let tag = synced_branch(RefKind::Tag, "v1.0.0", 1, 2);
        let synced = synced_branch(RefKind::LocalBranch, "main", 0, 0);

        assert!(sidebar_sync_badge_specs(&remote).is_empty());
        assert!(sidebar_sync_badge_specs(&tag).is_empty());
        assert!(sidebar_sync_badge_specs(&synced).is_empty());
    }
}
