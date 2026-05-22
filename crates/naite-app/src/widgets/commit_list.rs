//! Center pane: commit list with the graph canvas and the WIP row.

use iced::widget::{
    button, canvas, column, container, image, mouse_area, row, scrollable, stack, text,
    text::Wrapping, Space,
};
use iced::{
    mouse, Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle,
    Renderer, Size, Theme,
};
use naite_core::{
    CommitSummary, GraphLayout, GraphRow, RefKind, RefSummary, Refs, WorktreeStatusDetail,
};

use crate::features::repo_open;
use crate::state::{AvatarCache, ContextMenuKind, PreferencesState};
use crate::styles;
use crate::theme::{self, color};
use crate::{icons, Message};

use crate::icons::IconName;

use super::common::{
    empty_filter_state, empty_state, error_card, format_relative_time, ghost_icon_button,
    status_summary_title, ErrorRecovery,
};
use super::ROW_HEIGHT;

const GRAPH_MIN_WIDTH: f32 = 26.0;
pub(crate) const GRAPH_LANE_GAP: f32 = 22.0;
const GRAPH_MIN_LANE_GAP: f32 = 12.0;
const GRAPH_LANE_SPAN_MAX: f32 = 220.0;
pub(crate) const GRAPH_STROKE_WIDTH: f32 = 2.0;
pub(crate) const GRAPH_CORNER_RADIUS: f32 = 4.0;
const COMMIT_AVATAR_SIZE: f32 = 18.0;
const AVATAR_BORDER_WIDTH: f32 = 2.0;
const AVATAR_VISUAL_SIZE: f32 = COMMIT_AVATAR_SIZE + 2.0 * AVATAR_BORDER_WIDTH;
const AVATAR_HALF: f32 = AVATAR_VISUAL_SIZE / 2.0;
/// Left pad inside the graph canvas before lane 0. Equal to AVATAR_HALF so the
/// commit avatar's left edge sits flush with the canvas's x=0.
pub(crate) const GRAPH_LANE_LEFT: f32 = AVATAR_HALF;
const MAX_PRIMARY_REF_LABELS: usize = 1;
const MAX_GRAPH_REF_LABEL_CHARS: usize = 16;
const GRAPH_REF_LABEL_SPACING: f32 = 6.0;
const GRAPH_REF_PILL_PAD_X: f32 = 8.0;
const GRAPH_REF_PILL_PAD_Y: f32 = 2.0;
const GRAPH_REF_PILL_RADIUS: f32 = 4.0;
const SHA_COLUMN_WIDTH: f32 = 96.0;
const AUTHOR_COLUMN_WIDTH: f32 = 132.0;
const WHEN_COLUMN_WIDTH: f32 = 86.0;
const MIN_SUBJECT_COLUMN_WIDTH: f32 = 260.0;
const MIN_SUBJECT_WITH_WHEN_WIDTH: f32 = 220.0;
const SELECTION_BAR_WIDTH: f32 = 2.0;

pub struct CommitListProps<'a> {
    pub commits: &'a [CommitSummary],
    pub visible_indices: Vec<usize>,
    pub selected: Option<usize>,
    pub wip_selected: bool,
    pub status_detail: &'a WorktreeStatusDetail,
    pub error: Option<&'a str>,
    pub error_recovery: Option<ErrorRecovery<'a>>,
    pub graph_layout: &'a GraphLayout,
    pub refs: &'a Refs,
    pub scroll_id: &'a scrollable::Id,
    pub preferences: &'a PreferencesState,
    pub avatars: &'a AvatarCache,
    pub list_width: f32,
    pub has_more_commits: bool,
    pub loading_more_commits: bool,
}

pub fn commit_list<'a>(props: CommitListProps<'a>) -> Element<'a, Message> {
    let CommitListProps {
        commits,
        visible_indices,
        selected,
        wip_selected,
        status_detail,
        error,
        error_recovery,
        graph_layout,
        refs,
        scroll_id,
        preferences,
        avatars,
        list_width,
        has_more_commits,
        loading_more_commits,
    } = props;

    let inner: Element<'a, Message> = if let Some(err) = error {
        container(error_card(err, error_recovery))
            .padding(theme::SP_LG)
            .into()
    } else if commits.is_empty() && !status_detail.is_dirty() {
        empty_state()
    } else if visible_indices.is_empty() && !status_detail.is_dirty() {
        empty_filter_state()
    } else {
        let layout = commit_list_layout(
            commits,
            &visible_indices,
            graph_layout,
            refs,
            preferences.display.show_commit_author,
            list_width,
        );
        let mut col = column![commit_list_header(layout)].spacing(0);
        if status_detail.is_dirty() {
            col = col.push(wip_row(status_detail, wip_selected));
        }
        for commit_index in visible_indices {
            if let Some(commit) = commits.get(commit_index) {
                col = col.push(commit_row(CommitRowProps {
                    commit,
                    index: commit_index,
                    selected: selected == Some(commit_index),
                    graph_row: graph_layout.rows.get(commit_index),
                    refs,
                    layout,
                    avatars,
                }));
            }
        }
        if has_more_commits {
            col = col.push(load_more_row(loading_more_commits));
        }
        scrollable(col)
            .id(scroll_id.clone())
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar)
            .height(Length::Fill)
            .on_scroll(Message::CommitListScrolled)
            .into()
    };

    container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::bg_panel)
        .into()
}

fn load_more_row<'a>(loading: bool) -> Element<'a, Message> {
    let label = if loading {
        "Loading more commits..."
    } else {
        "Load more commits"
    };
    let button = button(
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None),
    )
    .padding(Padding::from([4, 10]))
    .style(styles::subtle_button)
    .on_press_maybe(
        (!loading).then_some(Message::from(repo_open::Message::LoadMoreCommitsRequested)),
    );

    container(button)
        .padding(Padding::from([theme::SP_SM, theme::SP_LG]))
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn wip_row<'a>(status_detail: &'a WorktreeStatusDetail, selected: bool) -> Element<'a, Message> {
    let bar_color = if selected {
        color::ACCENT
    } else {
        Color::TRANSPARENT
    };
    let bar = container(Space::new(
        Length::Fixed(SELECTION_BAR_WIDTH),
        Length::Fixed(ROW_HEIGHT),
    ))
    .style(styles::solid_bar(bar_color));

    let marker = container(
        text("WIP")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(color::WARNING),
    )
    .width(Length::Fixed(80.0));

    let counts = status_summary_title(status_detail);
    let subject_font = if selected {
        theme::font_semibold()
    } else {
        theme::font_regular()
    };

    let content = row![
        bar,
        marker,
        text("status")
            .size(theme::FS_SM)
            .font(iced::Font::MONOSPACE)
            .wrapping(Wrapping::None)
            .color(color::TEXT_MUTED),
        Space::with_width(Length::Fixed(theme::SP_MD as f32)),
        text("Working tree changes")
            .size(theme::FS_SM)
            .font(subject_font)
            .wrapping(Wrapping::None)
            .color(color::TEXT),
        Space::with_width(Length::Fill),
        text(counts)
            .size(theme::FS_SM)
            .font(theme::font_regular())
            .wrapping(Wrapping::None)
            .color(color::TEXT_MUTED),
        Space::with_width(Length::Fixed(theme::SP_LG as f32)),
        container(
            text("now")
                .size(theme::FS_SM)
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE)
        )
        .width(Length::Fixed(100.0)),
    ]
    .align_y(Alignment::Center);

    button(content)
        .on_press(Message::WipSelected)
        .padding(Padding::from([0, theme::SP_MD]))
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .style(styles::commit_row_button(selected))
        .into()
}

fn commit_list_header<'a>(layout: CommitListLayout) -> Element<'a, Message> {
    let author_header: Element<'a, Message> = if layout.show_author {
        container(
            text("AUTHOR")
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None),
        )
        .width(Length::Fixed(AUTHOR_COLUMN_WIDTH))
        .clip(true)
        .into()
    } else {
        Space::new(0.0, 0.0).into()
    };
    let when_header: Element<'a, Message> = if layout.show_when {
        container(
            text("WHEN")
                .size(theme::FS_XS)
                .color(color::TEXT_SUBTLE)
                .font(theme::font_semibold())
                .wrapping(Wrapping::None),
        )
        .width(Length::Fixed(WHEN_COLUMN_WIDTH))
        .clip(true)
        .into()
    } else {
        Space::new(0.0, 0.0).into()
    };
    container(
        row![
            Space::with_width(Length::Fixed(SELECTION_BAR_WIDTH)),
            Space::with_width(Length::Fixed(layout.graph.width)),
            container(
                text("SHA")
                    .size(theme::FS_XS)
                    .color(color::TEXT_SUBTLE)
                    .font(theme::font_semibold())
                    .wrapping(Wrapping::None),
            )
            .width(Length::Fixed(SHA_COLUMN_WIDTH))
            .clip(true),
            container(
                text("MESSAGE")
                    .size(theme::FS_XS)
                    .color(color::TEXT_SUBTLE)
                    .font(theme::font_semibold())
                    .wrapping(Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            author_header,
            when_header,
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD),
    )
    .padding(Padding::from([10, theme::SP_LG]))
    .width(Length::Fill)
    .style(styles::commit_list_header)
    .into()
}

struct CommitRowProps<'a> {
    commit: &'a CommitSummary,
    index: usize,
    selected: bool,
    graph_row: Option<&'a GraphRow>,
    refs: &'a Refs,
    layout: CommitListLayout,
    avatars: &'a AvatarCache,
}

fn commit_row<'a>(props: CommitRowProps<'a>) -> Element<'a, Message> {
    let CommitRowProps {
        commit,
        index,
        selected,
        graph_row,
        refs,
        layout,
        avatars,
    } = props;

    let bar_color = if selected {
        color::ACCENT
    } else {
        Color::TRANSPARENT
    };
    let bar = container(Space::new(
        Length::Fixed(SELECTION_BAR_WIDTH),
        Length::Fixed(ROW_HEIGHT),
    ))
    .style(styles::solid_bar(bar_color));

    let graph_row = graph_row.cloned().unwrap_or(GraphRow {
        lane: 0,
        lanes_in: Vec::new(),
        lanes_out: Vec::new(),
        parent_lanes: Vec::new(),
        total_lanes: 1,
    });
    let ref_labels = graph_ref_labels(commit, refs);
    let lane_color = color::LANES[graph_row.lane as usize % color::LANES.len()];

    let graph = canvas(GraphCanvas {
        row: graph_row.clone(),
        lane_gap: layout.graph.lane_gap,
    })
    .width(Length::Fixed(layout.graph.width))
    .height(Length::Fixed(ROW_HEIGHT));
    let graph_node = stack![
        graph,
        author_avatar(commit, avatars, graph_row.lane, layout.graph)
    ]
    .width(Length::Fixed(layout.graph.width))
    .height(Length::Fixed(ROW_HEIGHT));

    let sha: Element<'a, Message> = container(
        text(commit.short_id.clone())
            .size(theme::FS_SM)
            .font(iced::Font::MONOSPACE)
            .wrapping(Wrapping::None)
            .color(color::TEXT_MUTED),
    )
    .width(Length::Fixed(SHA_COLUMN_WIDTH))
    .center_y(Length::Fixed(ROW_HEIGHT))
    .clip(true)
    .into();

    let subject = subject_with_labels(commit, ref_labels, lane_color, selected);

    let author: Element<'a, Message> = if layout.show_author {
        container(
            text(commit.author_name.clone())
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_MUTED),
        )
        .width(Length::Fixed(AUTHOR_COLUMN_WIDTH))
        .center_y(Length::Fixed(ROW_HEIGHT))
        .clip(true)
        .into()
    } else {
        Space::new(0.0, 0.0).into()
    };

    let when: Element<'a, Message> = if layout.show_when {
        container(
            text(format_relative_time(commit.time_seconds))
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(color::TEXT_SUBTLE),
        )
        .width(Length::Fixed(WHEN_COLUMN_WIDTH))
        .center_y(Length::Fixed(ROW_HEIGHT))
        .clip(true)
        .into()
    } else {
        Space::new(0.0, 0.0).into()
    };

    let row_body = row![graph_node, sha, subject, author, when]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD)
        .width(Length::Fill);

    let content = row![bar, row_body].align_y(Alignment::Center);

    let pressable = button(content)
        .on_press(Message::CommitSelected(index))
        // Vertical padding must stay 0 so adjacent rows touch and the
        // graph canvas (which spans the full row height) renders a
        // continuous lane line from one commit to the next.
        .padding(Padding::from([0, theme::SP_MD]))
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .style(styles::commit_row_button(selected));

    let pointer_idle_layer = mouse_area(Space::new(Length::Fill, Length::Fixed(ROW_HEIGHT)))
        .interaction(mouse::Interaction::Idle);

    let more_button = container(ghost_icon_button(
        IconName::DotsVertical,
        Message::ContextMenuOpened(ContextMenuKind::Commit(commit.clone())),
    ))
    .align_x(Alignment::End)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .height(Length::Fixed(ROW_HEIGHT))
    .padding(Padding::from([0, theme::SP_SM]));

    mouse_area(
        stack![pressable, pointer_idle_layer, more_button]
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT)),
    )
    .on_right_press(Message::ContextMenuOpened(ContextMenuKind::Commit(
        commit.clone(),
    )))
    .into()
}

#[derive(Debug, Clone)]
struct GraphRefLabel {
    kind: RefKind,
    text: String,
    full_text: String,
    is_head: bool,
}

#[derive(Debug, Clone, Copy)]
struct GraphMetrics {
    width: f32,
    lane_gap: f32,
}

#[derive(Debug, Clone, Copy)]
struct CommitListLayout {
    graph: GraphMetrics,
    show_author: bool,
    show_when: bool,
}

fn graph_ref_labels(commit: &CommitSummary, refs: &Refs) -> Vec<GraphRefLabel> {
    let mut matching_refs: Vec<&RefSummary> = refs
        .local
        .iter()
        .chain(refs.remote.iter())
        .chain(refs.tags.iter())
        .filter(|ref_summary| is_graph_label_ref(ref_summary))
        .filter(|ref_summary| ref_points_to_commit(ref_summary, commit))
        .collect();

    matching_refs.sort_by(|a, b| {
        graph_ref_display_priority(a)
            .cmp(&graph_ref_display_priority(b))
            .then_with(|| a.short_name.cmp(&b.short_name))
    });

    matching_refs.into_iter().map(graph_ref_label).collect()
}

fn graph_ref_display_priority(ref_summary: &RefSummary) -> (u8, u8, u8) {
    let lifecycle_rank = if matches!(
        ref_summary.kind,
        RefKind::LocalBranch | RefKind::RemoteBranch
    ) {
        long_lived_branch_rank(ref_summary).unwrap_or(u8::MAX)
    } else {
        u8::MAX
    };
    let kind_rank = match ref_summary.kind {
        RefKind::LocalBranch => 0,
        RefKind::RemoteBranch => 1,
        RefKind::Tag => 2,
    };
    let head_rank = if ref_summary.is_head { 0 } else { 1 };

    (lifecycle_rank, kind_rank, head_rank)
}

fn long_lived_branch_rank(ref_summary: &RefSummary) -> Option<u8> {
    let branch_name = canonical_branch_name(ref_summary);

    match branch_name {
        "main" | "master" => Some(0),
        "staging" | "stage" => Some(1),
        "dev" | "develop" | "development" => Some(2),
        _ => None,
    }
}

fn canonical_branch_name(ref_summary: &RefSummary) -> &str {
    match ref_summary.kind {
        RefKind::RemoteBranch => ref_summary
            .short_name
            .split_once('/')
            .map_or(ref_summary.short_name.as_str(), |(_, branch)| branch),
        _ => ref_summary.short_name.as_str(),
    }
}

fn split_visible_graph_ref_labels(labels: &[GraphRefLabel]) -> (Vec<GraphRefLabel>, usize) {
    if labels.is_empty() {
        return (Vec::new(), 0);
    }

    let primary_count = labels.len().min(MAX_PRIMARY_REF_LABELS);
    let mut visible: Vec<GraphRefLabel> = labels[..primary_count].to_vec();

    // Always expose the HEAD label so the current commit stands out, even when
    // a higher-priority long-lived branch points to the same commit.
    if !visible.iter().any(|label| label.is_head) {
        if let Some(head) = labels.iter().find(|label| label.is_head) {
            if !visible_contains_label(&visible, head) {
                visible.push(head.clone());
            }
        }
    }

    let hidden_count = labels.len().saturating_sub(visible.len());
    (visible, hidden_count)
}

fn visible_contains_label(visible: &[GraphRefLabel], candidate: &GraphRefLabel) -> bool {
    visible
        .iter()
        .any(|label| label.kind == candidate.kind && label.full_text == candidate.full_text)
}

fn overflow_label(hidden_count: usize) -> GraphRefLabel {
    let text = format!("+{hidden_count}");
    GraphRefLabel {
        kind: RefKind::LocalBranch,
        text: text.clone(),
        full_text: text,
        is_head: false,
    }
}

fn graph_ref_label(ref_summary: &RefSummary) -> GraphRefLabel {
    let full_text = full_graph_ref_label(ref_summary);
    let text = compact_label(&full_text, MAX_GRAPH_REF_LABEL_CHARS);
    GraphRefLabel {
        kind: ref_summary.kind,
        text,
        full_text,
        is_head: ref_summary.is_head,
    }
}

fn is_graph_label_ref(ref_summary: &RefSummary) -> bool {
    if ref_summary.target_short_id.is_empty() {
        return false;
    }

    match ref_summary.kind {
        RefKind::LocalBranch | RefKind::Tag => true,
        RefKind::RemoteBranch => !ref_summary.short_name.ends_with("/HEAD"),
    }
}

fn ref_points_to_commit(ref_summary: &RefSummary, commit: &CommitSummary) -> bool {
    let target = ref_summary.target_short_id.as_str();
    let commit = commit.short_id.as_str();

    !target.is_empty() && (commit.starts_with(target) || target.starts_with(commit))
}

fn full_graph_ref_label(ref_summary: &RefSummary) -> String {
    // HEAD is conveyed visually via `graph_ref_pill` styling instead of a "HEAD "
    // text prefix — the filled accent pill is the "you are here" cue.
    ref_summary.short_name.clone()
}

fn compact_label(label: &str, max_chars: usize) -> String {
    let chars: Vec<char> = label.chars().collect();
    if chars.len() <= max_chars {
        return label.to_string();
    }

    let head_count = max_chars.saturating_sub(3) / 2;
    let tail_count = max_chars.saturating_sub(3 + head_count);
    let head: String = chars.iter().take(head_count).collect();
    let tail: String = chars
        .iter()
        .skip(chars.len().saturating_sub(tail_count))
        .collect();
    format!("{head}...{tail}")
}

fn inline_graph_ref_pills<'a>(
    labels: Vec<GraphRefLabel>,
    lane_color: Color,
) -> Element<'a, Message> {
    if labels.is_empty() {
        return Space::with_width(Length::Fixed(0.0)).into();
    }

    let (visible, hidden_count) = split_visible_graph_ref_labels(&labels);

    let mut items: Vec<Element<'a, Message>> = Vec::with_capacity((visible.len() + 1) * 2);
    for (i, label) in visible.into_iter().enumerate() {
        if i > 0 {
            items.push(Space::with_width(Length::Fixed(GRAPH_REF_LABEL_SPACING)).into());
        }
        let pill = if hidden_count == 0 {
            graph_ref_pill_with_full_text_tooltip(label, lane_color)
        } else {
            graph_ref_pill(label, lane_color)
        };
        items.push(pill);
    }
    if hidden_count > 0 {
        if !items.is_empty() {
            items.push(Space::with_width(Length::Fixed(GRAPH_REF_LABEL_SPACING)).into());
        }
        items.push(graph_ref_pill(overflow_label(hidden_count), lane_color));
    }

    let pills_row: Element<'a, Message> = iced::widget::Row::with_children(items)
        .align_y(Alignment::Center)
        .height(Length::Fixed(ROW_HEIGHT))
        .into();

    if hidden_count == 0 {
        return pills_row;
    }

    let pills: Vec<Element<'a, Message>> = labels
        .into_iter()
        .map(|label| graph_ref_pill(full_text_graph_ref_label(label), lane_color))
        .collect();
    let tooltip_body = container(iced::widget::Column::with_children(pills).spacing(theme::SP_XS))
        .padding(Padding::from([theme::SP_XS, theme::SP_SM]))
        .style(styles::inset_card);

    iced::widget::tooltip(
        pills_row,
        tooltip_body,
        iced::widget::tooltip::Position::Bottom,
    )
    .into()
}

fn graph_ref_pill_with_full_text_tooltip<'a>(
    label: GraphRefLabel,
    lane_color: Color,
) -> Element<'a, Message> {
    if !graph_ref_label_needs_full_text_tooltip(&label) {
        return graph_ref_pill(label, lane_color);
    }

    let tooltip_body = container(graph_ref_pill(
        full_text_graph_ref_label(label.clone()),
        lane_color,
    ))
    .padding(Padding::from([theme::SP_XS, theme::SP_SM]))
    .style(styles::inset_card);

    iced::widget::tooltip(
        graph_ref_pill(label, lane_color),
        tooltip_body,
        iced::widget::tooltip::Position::Bottom,
    )
    .into()
}

fn graph_ref_label_needs_full_text_tooltip(label: &GraphRefLabel) -> bool {
    label.kind == RefKind::Tag || label.text != label.full_text
}

fn full_text_graph_ref_label(label: GraphRefLabel) -> GraphRefLabel {
    GraphRefLabel {
        kind: label.kind,
        text: label.full_text.clone(),
        full_text: label.full_text,
        is_head: label.is_head,
    }
}

fn subject_with_labels<'a>(
    commit: &'a CommitSummary,
    ref_labels: Vec<GraphRefLabel>,
    lane_color: Color,
    selected: bool,
) -> Element<'a, Message> {
    let subject_font = if selected {
        theme::font_semibold()
    } else {
        theme::font_regular()
    };
    let subject_text: Element<'a, Message> = text(commit.summary.clone())
        .size(theme::FS_SM)
        .color(color::TEXT)
        .font(subject_font)
        .wrapping(Wrapping::None)
        .into();

    let inner: Element<'a, Message> = if ref_labels.is_empty() {
        subject_text
    } else {
        iced::widget::Row::new()
            .push(inline_graph_ref_pills(ref_labels, lane_color))
            .push(Space::with_width(Length::Fixed(theme::SP_SM as f32)))
            .push(subject_text)
            .align_y(Alignment::Center)
            .height(Length::Fixed(ROW_HEIGHT))
            .into()
    };

    container(inner)
        .width(Length::Fill)
        .center_y(Length::Fixed(ROW_HEIGHT))
        .clip(true)
        .into()
}

fn graph_ref_pill<'a>(label: GraphRefLabel, lane_color: Color) -> Element<'a, Message> {
    let is_head = label.is_head;
    // HEAD uses SUCCESS green (a semantic "current/active" cue) instead of
    // ACCENT, because ACCENT is also `LANES[0]` and would visually blend with
    // ordinary lane-0 pills. A leading filled-dot glyph reinforces the
    // "you are here" cue without spelling out "HEAD".
    let (background, border_color, text_color, display_text) = if is_head {
        (
            color::SUCCESS,
            color::SUCCESS,
            Color::WHITE,
            format!("\u{25CF} {}", label.text),
        )
    } else if label.kind == RefKind::Tag {
        (
            color::SURFACE_2,
            color::with_alpha(color::TEXT_SUBTLE, 0.45),
            color::TEXT_MUTED,
            label.text,
        )
    } else {
        (
            color::with_alpha(lane_color, 0.55),
            lane_color,
            color::TEXT,
            label.text,
        )
    };

    let content: Element<'a, Message> = if label.kind == RefKind::Tag && !is_head {
        row![
            icons::icon(IconName::Tag, 10, text_color),
            text(display_text)
                .size(theme::FS_XS)
                .font(theme::font_semibold())
                .color(text_color)
                .wrapping(Wrapping::None)
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_XS)
        .into()
    } else {
        text(display_text)
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .color(text_color)
            .wrapping(Wrapping::None)
            .into()
    };

    container(content)
        .padding(Padding::from([
            GRAPH_REF_PILL_PAD_Y as u16,
            GRAPH_REF_PILL_PAD_X as u16,
        ]))
        .style(styles::graph_ref_pill(
            background,
            border_color,
            text_color,
            GRAPH_REF_PILL_RADIUS,
        ))
        .into()
}

fn author_avatar<'a>(
    commit: &'a CommitSummary,
    avatars: &'a AvatarCache,
    lane: u8,
    graph_metrics: GraphMetrics,
) -> Element<'a, Message> {
    let image_size = Length::Fixed(COMMIT_AVATAR_SIZE);
    let visual_size = Length::Fixed(AVATAR_VISUAL_SIZE);
    let border_color = color::LANES[lane as usize % color::LANES.len()];
    let content: Element<'a, Message> = commit
        .author_avatar_url
        .as_deref()
        .and_then(|url| avatars.handles.get(url))
        .map(|handle| {
            stack![
                container(
                    image::Image::new(handle.clone())
                        .width(image_size)
                        .height(image_size),
                )
                .width(visual_size)
                .height(visual_size)
                .center_x(visual_size)
                .center_y(visual_size),
                container(Space::new(visual_size, visual_size))
                    .width(visual_size)
                    .height(visual_size)
                    .style(move |_| avatar_border_overlay_style(border_color))
            ]
            .width(visual_size)
            .height(visual_size)
            .into()
        })
        .unwrap_or_else(|| {
            container(
                container(
                    text(author_initials(&commit.author_name))
                        .size(theme::FS_XS)
                        .font(theme::font_semibold())
                        .color(color::TEXT),
                )
                .width(image_size)
                .height(image_size)
                .center_x(image_size)
                .center_y(image_size)
                .style(avatar_fill_style),
            )
            .width(visual_size)
            .height(visual_size)
            .center_x(visual_size)
            .center_y(visual_size)
            .style(move |_| avatar_border_overlay_style(border_color))
            .into()
        });

    let lane_center = lane_x(lane, graph_metrics.lane_gap);
    let leading = snap((lane_center - AVATAR_VISUAL_SIZE / 2.0).clamp(0.0, graph_metrics.width));
    container(
        row![Space::with_width(Length::Fixed(leading)), content]
            .align_y(Alignment::Center)
            .width(Length::Fixed(graph_metrics.width))
            .height(Length::Fixed(ROW_HEIGHT)),
    )
    .width(Length::Fixed(graph_metrics.width))
    .height(Length::Fixed(ROW_HEIGHT))
    .into()
}

fn avatar_fill_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_3)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

fn avatar_border_overlay_style(border_color: Color) -> container::Style {
    container::Style {
        background: None,
        border: Border {
            color: border_color,
            width: AVATAR_BORDER_WIDTH,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

fn author_initials(name: &str) -> String {
    let mut chars = name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .flat_map(char::to_uppercase);

    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) => format!("{first}{second}"),
        (Some(first), None) => first.to_string(),
        _ => "?".into(),
    }
}

fn graph_metrics(visible_indices: &[usize], graph_layout: &GraphLayout) -> GraphMetrics {
    let max_lanes = visible_indices
        .iter()
        .filter_map(|&index| graph_layout.rows.get(index))
        .map(|row| row.total_lanes.max(1))
        .max()
        .unwrap_or(1);
    let lane_gap = graph_lane_gap(max_lanes);
    let lane_width = graph_lane_span_width(max_lanes, lane_gap);

    GraphMetrics {
        width: lane_width.max(GRAPH_MIN_WIDTH),
        lane_gap,
    }
}

fn commit_list_layout(
    _commits: &[CommitSummary],
    visible_indices: &[usize],
    graph_layout: &GraphLayout,
    _refs: &Refs,
    author_preference: bool,
    list_width: f32,
) -> CommitListLayout {
    let graph = graph_metrics(visible_indices, graph_layout);
    let chrome_width = 2.0 * theme::SP_MD as f32 + SELECTION_BAR_WIDTH;
    let base_width = chrome_width
        + graph.width
        + SHA_COLUMN_WIDTH
        + 3.0 * theme::SP_MD as f32
        + MIN_SUBJECT_COLUMN_WIDTH;
    let when_width = WHEN_COLUMN_WIDTH + theme::SP_MD as f32;
    let author_width = AUTHOR_COLUMN_WIDTH + theme::SP_MD as f32;
    let show_when = list_width >= base_width + when_width;
    let subject_width = if show_when {
        MIN_SUBJECT_WITH_WHEN_WIDTH
    } else {
        MIN_SUBJECT_COLUMN_WIDTH
    };
    let author_base_width = chrome_width
        + graph.width
        + SHA_COLUMN_WIDTH
        + 4.0 * theme::SP_MD as f32
        + subject_width
        + when_width;
    let show_author =
        author_preference && show_when && list_width >= author_base_width + author_width;

    CommitListLayout {
        graph,
        show_author,
        show_when,
    }
}

fn graph_lane_gap(max_lanes: u8) -> f32 {
    if max_lanes <= 1 {
        return GRAPH_LANE_GAP;
    }

    let available = (GRAPH_LANE_SPAN_MAX - GRAPH_LANE_LEFT - AVATAR_VISUAL_SIZE)
        / (max_lanes.saturating_sub(1) as f32);
    snap(available.clamp(GRAPH_MIN_LANE_GAP, GRAPH_LANE_GAP))
}

fn graph_lane_span_width(max_lanes: u8, lane_gap: f32) -> f32 {
    snap(GRAPH_LANE_LEFT + max_lanes.saturating_sub(1) as f32 * lane_gap + AVATAR_HALF)
}

#[derive(Debug, Clone)]
struct GraphCanvas {
    row: GraphRow,
    lane_gap: f32,
}

pub(crate) fn snap(value: f32) -> f32 {
    value.round()
}

pub(crate) fn lane_x(lane: u8, lane_gap: f32) -> f32 {
    snap(GRAPH_LANE_LEFT + lane as f32 * lane_gap)
}

pub(crate) fn row_y(value: f32, height: f32) -> f32 {
    snap(value.clamp(0.0, height))
}

pub(crate) fn graph_fill(color: Color) -> Color {
    color::with_alpha(color, 0.90)
}

pub(crate) fn graph_stroke(color: Color) -> canvas::Stroke<'static> {
    canvas::Stroke::default()
        .with_color(graph_fill(color))
        .with_width(GRAPH_STROKE_WIDTH)
}

pub(crate) fn fill_vertical_segment(
    frame: &mut canvas::Frame,
    x: f32,
    y_start: f32,
    y_end: f32,
    color: Color,
) {
    let top = snap(y_start.min(y_end));
    let bottom = snap(y_start.max(y_end));
    let height = bottom - top;
    if height <= 0.0 {
        return;
    }

    frame.fill_rectangle(
        Point::new(snap(x - GRAPH_STROKE_WIDTH / 2.0), top),
        Size::new(GRAPH_STROKE_WIDTH, height),
        graph_fill(color),
    );
}

pub(crate) fn fill_horizontal_segment(
    frame: &mut canvas::Frame,
    x_start: f32,
    x_end: f32,
    y: f32,
    color: Color,
) {
    let left = snap(x_start.min(x_end));
    let right = snap(x_start.max(x_end));
    let width = right - left;
    if width <= 0.0 {
        return;
    }

    frame.fill_rectangle(
        Point::new(left, snap(y - GRAPH_STROKE_WIDTH / 2.0)),
        Size::new(width, GRAPH_STROKE_WIDTH),
        graph_fill(color),
    );
}

impl canvas::Program<Message> for GraphCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let lane_color = |lane: u8| color::LANES[lane as usize % color::LANES.len()];

        let y_mid = row_y(bounds.height / 2.0, ROW_HEIGHT);
        let y_bottom = row_y(bounds.height, ROW_HEIGHT);
        let y_avatar_top = row_y(y_mid - AVATAR_HALF, ROW_HEIGHT);
        let y_avatar_bottom = row_y(y_mid + AVATAR_HALF, ROW_HEIGHT);
        let commit_x = lane_x(self.row.lane, self.lane_gap);

        // Lanes arriving from above. Draw straight verticals: for the commit's
        // own lane stop at the avatar's top edge so the line doesn't peek out
        // as a stub above the avatar; pass-through lanes go all the way down.
        for &incoming in &self.row.lanes_in {
            let x = lane_x(incoming, self.lane_gap);
            let y_end = if incoming == self.row.lane {
                y_avatar_top
            } else if self.row.lanes_out.contains(&incoming) {
                y_bottom
            } else {
                // Lane terminated here without being this commit — rare, but
                // draw it down to the row mid so it doesn't dangle in mid-air.
                y_mid
            };
            fill_vertical_segment(&mut frame, x, 0.0, y_end, lane_color(incoming));
        }

        // Outgoing strokes: from the dot down to each parent's lane at the
        // bottom edge. A first parent that inherits the commit's lane gets a
        // straight vertical; anything else gets a rounded elbow instead of a
        // direct hook from the node.
        for &parent in &self.row.parent_lanes {
            let target_x = lane_x(parent, self.lane_gap);
            let stroke_color = lane_color(parent);
            if parent == self.row.lane {
                // Outgoing strokes start at the avatar's bottom edge so the
                // line doesn't peek out as a stub below the avatar.
                fill_vertical_segment(
                    &mut frame,
                    commit_x,
                    y_avatar_bottom,
                    y_bottom,
                    stroke_color,
                );
            } else {
                let direction = if target_x >= commit_x { 1.0 } else { -1.0 };
                let radius = GRAPH_CORNER_RADIUS.min((target_x - commit_x).abs() / 2.0);
                let elbow_y = row_y((bounds.height - 6.0).max(y_mid + 6.0), ROW_HEIGHT);
                let first_corner_start = Point::new(commit_x, snap(elbow_y - radius));
                let first_corner_control = Point::new(commit_x, elbow_y);
                let first_corner_end = Point::new(snap(commit_x + direction * radius), elbow_y);
                let second_corner_start = Point::new(snap(target_x - direction * radius), elbow_y);
                let second_corner_control = Point::new(target_x, elbow_y);
                let second_corner_end = Point::new(target_x, snap(elbow_y + radius));

                fill_vertical_segment(
                    &mut frame,
                    commit_x,
                    y_avatar_bottom,
                    first_corner_start.y,
                    stroke_color,
                );
                let first_corner = canvas::Path::new(|p| {
                    p.move_to(first_corner_start);
                    p.quadratic_curve_to(first_corner_control, first_corner_end);
                });
                frame.stroke(&first_corner, graph_stroke(stroke_color));

                fill_horizontal_segment(
                    &mut frame,
                    first_corner_end.x,
                    second_corner_start.x,
                    elbow_y,
                    stroke_color,
                );
                let second_corner = canvas::Path::new(|p| {
                    p.move_to(second_corner_start);
                    p.quadratic_curve_to(second_corner_control, second_corner_end);
                });
                frame.stroke(&second_corner, graph_stroke(stroke_color));

                fill_vertical_segment(
                    &mut frame,
                    target_x,
                    second_corner_end.y,
                    y_bottom,
                    stroke_color,
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(kind: RefKind, short_name: &str, target_short_id: &str, is_head: bool) -> RefSummary {
        let prefix = match kind {
            RefKind::LocalBranch => "refs/heads",
            RefKind::RemoteBranch => "refs/remotes",
            RefKind::Tag => "refs/tags",
        };

        RefSummary {
            kind,
            short_name: short_name.into(),
            full_name: format!("{prefix}/{short_name}"),
            target_short_id: target_short_id.into(),
            is_head,
            sync_status: None,
        }
    }

    fn commit(short_id: &str) -> CommitSummary {
        CommitSummary {
            id: format!("{short_id}000000000000"),
            short_id: short_id.into(),
            summary: "Commit".into(),
            author_name: "Author".into(),
            author_email: "author@example.com".into(),
            author_avatar_url: None,
            time_seconds: 0,
            parent_ids: Vec::new(),
        }
    }

    fn graph_layout() -> GraphLayout {
        GraphLayout {
            rows: vec![GraphRow {
                lane: 0,
                lanes_in: Vec::new(),
                lanes_out: Vec::new(),
                parent_lanes: Vec::new(),
                total_lanes: 1,
            }],
        }
    }

    #[test]
    fn graph_lanes_snap_to_integer_pixels() {
        // Lane 0 sits at AVATAR_HALF (11.0) so the avatar's left edge meets
        // the canvas's x=0. Subsequent lanes step by GRAPH_LANE_GAP (22.0).
        assert_eq!(lane_x(0, GRAPH_LANE_GAP), 11.0);
        assert_eq!(lane_x(1, GRAPH_LANE_GAP), 33.0);
        assert_eq!(row_y(ROW_HEIGHT / 2.0, ROW_HEIGHT), 16.0);
    }

    #[test]
    fn avatar_half_matches_visual_size() {
        assert_eq!(AVATAR_HALF * 2.0, AVATAR_VISUAL_SIZE);
    }

    #[test]
    fn current_branch_ref_label_is_flagged_for_distinct_pill_styling() {
        let ref_summary = branch(RefKind::LocalBranch, "main", "abc1234", true);
        let label = graph_ref_label(&ref_summary);

        // HEAD is conveyed via the filled accent pill style, not a text prefix.
        assert_eq!(label.kind, RefKind::LocalBranch);
        assert_eq!(label.text, "main");
        assert_eq!(label.full_text, "main");
        assert!(label.is_head);
    }

    #[test]
    fn split_always_includes_head_label_even_when_long_lived_branch_wins_primary_slot() {
        let commit = commit("abc1234");
        let refs = Refs {
            local: vec![
                branch(RefKind::LocalBranch, "feature/curitiba", "abc1234", true),
                branch(RefKind::LocalBranch, "main", "abc1234", false),
            ],
            remote: Vec::new(),
            tags: Vec::new(),
        };

        let labels = graph_ref_labels(&commit, &refs);
        let (visible, hidden) = split_visible_graph_ref_labels(&labels);

        let visible_names: Vec<&str> = visible.iter().map(|l| l.full_text.as_str()).collect();
        assert_eq!(visible_names, vec!["main", "feature/curitiba"]);
        assert!(visible
            .iter()
            .any(|l| l.is_head && l.full_text == "feature/curitiba"));
        assert_eq!(hidden, 0);
    }

    #[test]
    fn graph_ref_split_keeps_tags_in_hover_overflow_with_extra_refs() {
        let commit = commit("abc1234");
        let refs = Refs {
            local: vec![branch(RefKind::LocalBranch, "main", "abc1234", false)],
            remote: vec![branch(
                RefKind::RemoteBranch,
                "origin/main",
                "abc1234",
                false,
            )],
            tags: vec![
                branch(RefKind::Tag, "v1.0.0", "abc1234", false),
                branch(RefKind::Tag, "v1.0.1", "abc1234", false),
            ],
        };

        let labels = graph_ref_labels(&commit, &refs);
        let (visible, hidden) = split_visible_graph_ref_labels(&labels);

        assert!(visible
            .iter()
            .any(|label| label.kind == RefKind::LocalBranch && label.full_text == "main"));
        assert!(!visible
            .iter()
            .any(|label| label.kind == RefKind::Tag && label.full_text == "v1.0.0"));
        assert_eq!(hidden, 3);
    }

    #[test]
    fn graph_ref_split_keeps_first_tag_visible_when_only_tags_match() {
        let commit = commit("abc1234");
        let refs = Refs {
            local: Vec::new(),
            remote: Vec::new(),
            tags: vec![
                branch(RefKind::Tag, "v1.0.0", "abc1234", false),
                branch(RefKind::Tag, "v1.0.1", "abc1234", false),
            ],
        };

        let labels = graph_ref_labels(&commit, &refs);
        let (visible, hidden) = split_visible_graph_ref_labels(&labels);

        assert_eq!(
            visible
                .iter()
                .map(|label| (label.kind, label.full_text.as_str()))
                .collect::<Vec<_>>(),
            vec![(RefKind::Tag, "v1.0.0")]
        );
        assert_eq!(hidden, 1);
    }

    #[test]
    fn long_graph_ref_label_preserves_full_text_for_tooltip() {
        let ref_summary = branch(
            RefKind::LocalBranch,
            "feature/some-extremely-long-branch-name",
            "abc1234",
            false,
        );
        let label = graph_ref_label(&ref_summary);

        assert_ne!(label.text, label.full_text);
        assert!(label.text.contains("..."));
        assert_eq!(label.full_text, "feature/some-extremely-long-branch-name");
    }

    #[test]
    fn long_graph_ref_tag_label_preserves_full_text_for_tooltip() {
        let ref_summary = branch(
            RefKind::Tag,
            "release/some-extremely-long-tag-name",
            "abc1234",
            false,
        );
        let label = graph_ref_label(&ref_summary);

        assert_eq!(label.kind, RefKind::Tag);
        assert_ne!(label.text, label.full_text);
        assert!(label.text.contains("..."));
        assert_eq!(label.full_text, "release/some-extremely-long-tag-name");
        assert!(graph_ref_label_needs_full_text_tooltip(&label));
    }

    #[test]
    fn short_graph_ref_tag_label_still_needs_full_text_tooltip() {
        let ref_summary = branch(RefKind::Tag, "v1.0.0", "abc1234", false);
        let label = graph_ref_label(&ref_summary);

        assert_eq!(label.text, "v1.0.0");
        assert!(graph_ref_label_needs_full_text_tooltip(&label));
    }

    #[test]
    fn narrow_commit_list_hides_metadata_columns() {
        let commits = vec![commit("abc1234")];
        let layout = commit_list_layout(
            &commits,
            &[0],
            &graph_layout(),
            &Refs::default(),
            true,
            420.0,
        );

        assert!(!layout.show_author);
        assert!(!layout.show_when);
    }

    #[test]
    fn wide_commit_list_keeps_metadata_columns() {
        let commits = vec![commit("abc1234")];
        let layout = commit_list_layout(
            &commits,
            &[0],
            &graph_layout(),
            &Refs::default(),
            true,
            900.0,
        );

        assert!(layout.show_author);
        assert!(layout.show_when);
    }

    #[test]
    fn graph_ref_label_refs_include_tags_and_skip_remote_head_symbolic_refs() {
        assert!(is_graph_label_ref(&branch(
            RefKind::LocalBranch,
            "feature/demo",
            "abc1234",
            false
        )));
        assert!(is_graph_label_ref(&branch(
            RefKind::RemoteBranch,
            "origin/feature/demo",
            "abc1234",
            false
        )));
        assert!(is_graph_label_ref(&branch(
            RefKind::Tag,
            "v1.0.0",
            "abc1234",
            false
        )));
        assert!(!is_graph_label_ref(&branch(
            RefKind::RemoteBranch,
            "origin/HEAD",
            "abc1234",
            false
        )));
        assert!(!is_graph_label_ref(&branch(
            RefKind::Tag,
            "dangling",
            "",
            false
        )));
    }

    #[test]
    fn graph_ref_labels_prioritize_long_lived_branches() {
        let commit = commit("abc1234");
        let refs = Refs {
            local: vec![
                branch(RefKind::LocalBranch, "feature/demo", "abc1234", true),
                branch(RefKind::LocalBranch, "dev", "abc1234", false),
                branch(RefKind::LocalBranch, "staging", "abc1234", false),
                branch(RefKind::LocalBranch, "main", "abc1234", false),
            ],
            remote: Vec::new(),
            tags: Vec::new(),
        };

        let labels = graph_ref_labels(&commit, &refs);

        assert_eq!(
            labels
                .iter()
                .map(|label| label.full_text.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "staging", "dev", "feature/demo"]
        );
    }

    #[test]
    fn graph_ref_labels_prefer_local_long_lived_branch_over_remote_peer() {
        let commit = commit("abc1234");
        let refs = Refs {
            local: vec![branch(RefKind::LocalBranch, "main", "abc1234", false)],
            remote: vec![
                branch(RefKind::RemoteBranch, "origin/main", "abc1234", false),
                branch(RefKind::RemoteBranch, "origin/dev", "abc1234", false),
            ],
            tags: Vec::new(),
        };

        let labels = graph_ref_labels(&commit, &refs);

        assert_eq!(
            labels
                .iter()
                .map(|label| label.full_text.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "origin/main", "origin/dev"]
        );
    }

    #[test]
    fn graph_ref_labels_keep_branches_before_tags() {
        let commit = commit("abc1234");
        let refs = Refs {
            local: vec![branch(
                RefKind::LocalBranch,
                "feature/demo",
                "abc1234",
                false,
            )],
            remote: vec![branch(
                RefKind::RemoteBranch,
                "origin/dev",
                "abc1234",
                false,
            )],
            tags: vec![branch(RefKind::Tag, "v1.0.0", "abc1234", false)],
        };

        let labels = graph_ref_labels(&commit, &refs);

        assert_eq!(
            labels
                .iter()
                .map(|label| (label.kind, label.full_text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (RefKind::RemoteBranch, "origin/dev"),
                (RefKind::LocalBranch, "feature/demo"),
                (RefKind::Tag, "v1.0.0")
            ]
        );
    }

    #[test]
    fn graph_refs_match_variable_length_commit_abbreviations() {
        let ref_summary = branch(RefKind::LocalBranch, "main", "abc1234", true);
        let commit = commit("abc123456");

        assert!(ref_points_to_commit(&ref_summary, &commit));
    }

    #[test]
    fn long_graph_ref_labels_are_compacted() {
        let label = compact_label("feature/some-extremely-long-branch-name", 24);

        assert!(label.len() <= 24);
        assert!(label.contains("..."));
    }
}
