use iced::widget::{
    button, canvas, column, container, mouse_area, pick_list, row, scrollable, stack, text,
    text::Wrapping, text_input, Space,
};
use iced::{mouse, Alignment, Color, Element, Length, Padding, Point, Rectangle, Renderer, Theme};
use naite_core::{
    build_rebase_gutter, pick_inherits_reword, CommitDiff, GraphRow, RebaseAction,
    WorktreeDiffTarget,
};

use crate::features::rebase::{
    self, DragState, InteractiveRebaseSession, RebaseApplyMode, RebasePlanPreset, RebasePlanRow,
};
use crate::icons::{self, IconName};
use crate::state::{DiffViewMode, FileInsightState};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::commit_list::{
    fill_horizontal_segment, fill_vertical_segment, graph_fill, graph_stroke, lane_x, snap,
    GRAPH_CORNER_RADIUS, GRAPH_LANE_GAP,
};
use super::detail_pane::{diff_content, DiffContentProps};
use super::ROW_HEIGHT;

const MOVE_BUTTON_SIZE: f32 = 22.0;
const ACTION_COLUMN_WIDTH: f32 = 84.0;
const SHA_COLUMN_WIDTH: f32 = 68.0;
const INSERTION_LINE_HEIGHT: f32 = 2.0;
const GHOST_HORIZONTAL_INSET: f32 = 8.0;
const REBASE_TOOLBAR_HEIGHT: f32 = 56.0;
/// Width of the per-row graph gutter that visualizes squash/fixup grouping.
/// Sized to fit two lanes (`GRAPH_LANE_LEFT=11 + GRAPH_LANE_GAP=22 = 33`) plus
/// the node radius (~5) and a small right pad.
const REBASE_GUTTER_WIDTH: f32 = 42.0;
/// Radius of the dot drawn at each row's lane in the gutter. Smaller than the
/// commit-list avatar (which doubles as the node there) because the rebase
/// gutter has no author info — the dot is purely a topological marker.
const REBASE_NODE_RADIUS: f32 = 4.5;

pub fn rebase_editor<'a>(
    session: &'a InteractiveRebaseSession,
    cursor: Option<Point>,
    release_promotion_active: bool,
) -> Element<'a, Message> {
    let active_gap = active_insertion_gap(session);
    let actions: Vec<RebaseAction> = session.plan.iter().map(|row| row.action).collect();
    let gutter_rows = build_rebase_gutter(&actions);
    let mut body = column![toolbar(session, release_promotion_active), header()].spacing(0);
    body = body.push(insertion_gap(active_gap == Some(0)));
    for (index, row) in session.plan.iter().enumerate() {
        let gutter = gutter_rows.get(index).cloned().unwrap_or_else(|| GraphRow {
            lane: 0,
            lanes_in: Vec::new(),
            lanes_out: Vec::new(),
            parent_lanes: Vec::new(),
            total_lanes: 1,
        });
        let inherits_reword = pick_inherits_reword(&actions, index);
        body = body.push(plan_row(session, row, index, gutter, inherits_reword));
        body = body.push(insertion_gap(active_gap == Some(index + 1)));
    }

    let panel: Element<'a, Message> = container(
        scrollable(body)
            .direction(styles::thin_scrollbar_dir())
            .style(styles::thin_scrollbar)
            .on_scroll(|viewport| {
                Message::from(rebase::Message::Scrolled(viewport.absolute_offset().y))
            })
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(styles::bg_panel)
    .into();

    match session.drag.as_ref().filter(|drag| drag.started) {
        Some(drag) => stack![panel, ghost_overlay(session, drag, cursor)].into(),
        None => panel,
    }
}

fn active_insertion_gap(session: &InteractiveRebaseSession) -> Option<usize> {
    let drag = session.drag.as_ref()?;
    if !drag.started || drag.source_index == drag.hover_index {
        return None;
    }
    if drag.hover_index > drag.source_index {
        Some(drag.hover_index + 1)
    } else {
        Some(drag.hover_index)
    }
}

fn insertion_gap(active: bool) -> Element<'static, Message> {
    let bar_color = if active {
        color::ACCENT
    } else {
        Color::TRANSPARENT
    };
    container(Space::new(
        Length::Fill,
        Length::Fixed(INSERTION_LINE_HEIGHT),
    ))
    .width(Length::Fill)
    .height(Length::Fixed(INSERTION_LINE_HEIGHT))
    .style(styles::solid_bar(bar_color))
    .into()
}

fn ghost_overlay<'a>(
    session: &'a InteractiveRebaseSession,
    drag: &DragState,
    cursor: Option<Point>,
) -> Element<'a, Message> {
    let Some(row_data) = session.plan.get(drag.source_index) else {
        return Space::new(Length::Fill, Length::Fill).into();
    };
    let leading_space = ghost_top_y(session, drag, cursor).max(0.0);
    let ghost = container(ghost_row(row_data))
        .padding(Padding::from([0, GHOST_HORIZONTAL_INSET as u16]))
        .width(Length::Fill);
    column![Space::with_height(Length::Fixed(leading_space)), ghost]
        .width(Length::Fill)
        .into()
}

/// Top edge of the ghost row in the rebase panel's local coordinate space.
/// Anchored at the source row's initial position, sliding by the cursor's
/// vertical motion since press so the ghost reads as "the row you grabbed".
fn ghost_top_y(session: &InteractiveRebaseSession, drag: &DragState, cursor: Option<Point>) -> f32 {
    let source_row_top = approx_toolbar_height()
        + approx_header_height()
        + INSERTION_LINE_HEIGHT
        + drag.source_index as f32 * (ROW_HEIGHT + INSERTION_LINE_HEIGHT)
        - session.scroll_offset;
    let delta_y = cursor.map(|c| c.y - drag.press_origin.y).unwrap_or(0.0);
    source_row_top + delta_y
}

fn approx_toolbar_height() -> f32 {
    REBASE_TOOLBAR_HEIGHT
}

fn approx_header_height() -> f32 {
    // header row: vertical padding 8px twice, plus text FS_XS=10.
    theme::FS_XS as f32 + 16.0
}

fn ghost_row<'a>(row_data: &'a RebasePlanRow) -> Element<'a, Message> {
    let sha = text(short_id(&row_data.commit.id))
        .size(theme::FS_SM)
        .font(iced::Font::MONOSPACE)
        .color(color::with_alpha(color::TEXT_MUTED, 0.85));

    let summary = text(row_data.commit.summary.clone())
        .size(theme::FS_SM)
        .font(theme::font_semibold())
        .color(color::with_alpha(color::TEXT, 0.95));

    let action = container(
        text(action_label(row_data.action))
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .color(color::with_alpha(action_color(row_data.action), 0.9)),
    )
    .padding(Padding::from([3, 8]))
    .style(styles::ghost_action_chip);

    let content = row![
        Space::with_width(Length::Fixed(2.0)),
        container(action).width(Length::Fixed(ACTION_COLUMN_WIDTH)),
        Space::with_width(Length::Fixed(theme::SP_MD as f32)),
        container(sha).width(Length::Fixed(SHA_COLUMN_WIDTH)),
        Space::with_width(Length::Fixed(theme::SP_MD as f32)),
        container(summary).width(Length::Fill),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);

    container(content)
        .height(Length::Fixed(ROW_HEIGHT))
        .width(Length::Fill)
        .padding(Padding::from([0, theme::SP_MD]))
        .style(styles::ghost_row)
        .into()
}

/// Per-row gutter canvas for the interactive rebase editor. Renders the
/// squash/fixup grouping topology supplied by
/// [`naite_core::build_rebase_gutter`] — trunk picks on lane 0, squash/fixup
/// children on lane 1, with an elbow drawn on the parent pick's row spawning
/// lane 1 down to the first child below it. Mirrors `GraphCanvas` in
/// `commit_list.rs` but with a smaller node gap because the rebase gutter has
/// no author avatar to fill the dot.
#[derive(Debug, Clone)]
struct RebaseGutterCanvas {
    row: GraphRow,
    muted: bool,
}

impl canvas::Program<Message> for RebaseGutterCanvas {
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
        let lane_color = |lane: u8| {
            let base = color::LANES[lane as usize % color::LANES.len()];
            if self.muted {
                color::with_alpha(base, 0.45)
            } else {
                base
            }
        };

        let y_mid = snap(bounds.height / 2.0);
        let y_bottom = snap(bounds.height);
        let y_node_top = snap(y_mid - REBASE_NODE_RADIUS);
        let y_node_bottom = snap(y_mid + REBASE_NODE_RADIUS);
        let commit_x = lane_x(self.row.lane, GRAPH_LANE_GAP);

        for &incoming in &self.row.lanes_in {
            let x = lane_x(incoming, GRAPH_LANE_GAP);
            let y_end = if incoming == self.row.lane {
                y_node_top
            } else if self.row.lanes_out.contains(&incoming) {
                y_bottom
            } else {
                y_mid
            };
            fill_vertical_segment(&mut frame, x, 0.0, y_end, lane_color(incoming));
        }

        for &parent in &self.row.parent_lanes {
            let target_x = lane_x(parent, GRAPH_LANE_GAP);
            let stroke_color = lane_color(parent);
            if parent == self.row.lane {
                fill_vertical_segment(&mut frame, commit_x, y_node_bottom, y_bottom, stroke_color);
            } else {
                let direction = if target_x >= commit_x { 1.0 } else { -1.0 };
                let radius = GRAPH_CORNER_RADIUS.min((target_x - commit_x).abs() / 2.0);
                let elbow_y = snap((bounds.height - 6.0).max(y_mid + 6.0));
                let first_corner_start = Point::new(commit_x, snap(elbow_y - radius));
                let first_corner_control = Point::new(commit_x, elbow_y);
                let first_corner_end = Point::new(snap(commit_x + direction * radius), elbow_y);
                let second_corner_start = Point::new(snap(target_x - direction * radius), elbow_y);
                let second_corner_control = Point::new(target_x, elbow_y);
                let second_corner_end = Point::new(target_x, snap(elbow_y + radius));

                fill_vertical_segment(
                    &mut frame,
                    commit_x,
                    y_node_bottom,
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

        let node_color = lane_color(self.row.lane);
        let node = canvas::Path::circle(Point::new(commit_x, y_mid), REBASE_NODE_RADIUS);
        frame.fill(&node, graph_fill(node_color));

        vec![frame.into_geometry()]
    }
}

pub struct RebaseDetailProps<'a> {
    pub session: &'a InteractiveRebaseSession,
    pub diff: Option<&'a CommitDiff>,
    pub diff_loading: bool,
    pub diff_error: Option<&'a str>,
    pub selected_file: Option<usize>,
    pub selected_hunk: Option<usize>,
    pub diff_view_mode: DiffViewMode,
    pub selected_wip_file: Option<&'a WorktreeDiffTarget>,
    pub file_insight: &'a FileInsightState,
}

pub fn rebase_detail<'a>(props: RebaseDetailProps<'a>) -> Element<'a, Message> {
    let RebaseDetailProps {
        session,
        diff,
        diff_loading,
        diff_error,
        selected_file,
        selected_hunk,
        diff_view_mode,
        selected_wip_file,
        file_insight,
    } = props;

    let inner: Element<'_, Message> = match session.selected_row() {
        Some(row) => scrollable(
            container(
                column![
                    rebase_summary(session),
                    Space::with_height(theme::SP_MD),
                    selected_operation(session, row),
                    Space::with_height(theme::SP_MD),
                    diff_content(DiffContentProps {
                        diff,
                        diff_highlight: None,
                        loading: diff_loading,
                        error: diff_error,
                        selected_file,
                        selected_hunk,
                        diff_view_mode,
                        selected_wip_file,
                        actions_disabled: session.applying,
                        file_insight,
                        show_file_inspection: false,
                    }),
                ]
                .width(Length::Fill)
                .spacing(theme::SP_SM),
            )
            .width(Length::Fill)
            .padding(theme::SP_LG),
        )
        .direction(styles::thin_scrollbar_dir())
        .style(styles::thin_scrollbar)
        .height(Length::Fill)
        .into(),
        None => container(
            text("Select a rebase operation to inspect.")
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_SUBTLE),
        )
        .padding(theme::SP_LG)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into(),
    };

    container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::surface_panel)
        .into()
}

fn toolbar(
    session: &InteractiveRebaseSession,
    release_promotion_active: bool,
) -> Element<'_, Message> {
    let dropped = session
        .plan
        .iter()
        .filter(|row| row.action == RebaseAction::Drop)
        .count();
    let replayed = session.plan.len().saturating_sub(dropped);
    let current_branch = compact_toolbar_label(&session.current_branch.short_name);
    let target_branch = compact_toolbar_label(&session.target.short_name);
    let status = if dropped == 0 {
        format!("{current_branch} onto {target_branch} | {replayed} replay")
    } else {
        format!("{current_branch} onto {target_branch} | {replayed} replay, {dropped} drop")
    };
    let status = match own_commit_count(session) {
        Some(own) => format!("{status}, {own} own"),
        None => status,
    };

    let apply_enabled = can_apply(session);
    let pick_mine_enabled = can_pick_mine(session);
    let preset_enabled = !session.applying && !drag_in_progress(session);

    let mut actions = row![
        rebase_toolbar_button(
            IconName::GitCommit,
            "Keep",
            RebaseToolbarTone::Neutral,
            pick_mine_enabled,
            Message::from(rebase::Message::PresetRequested(RebasePlanPreset::KeepMine,)),
        ),
        rebase_toolbar_button(
            IconName::GitMerge,
            "Squash Mine",
            RebaseToolbarTone::Neutral,
            pick_mine_enabled,
            Message::from(rebase::Message::PresetRequested(
                RebasePlanPreset::SquashMine,
            )),
        ),
        rebase_toolbar_button(
            IconName::GitMerge,
            "Squash All",
            RebaseToolbarTone::Neutral,
            preset_enabled,
            Message::from(rebase::Message::PresetRequested(
                RebasePlanPreset::SquashAll,
            )),
        ),
        rebase_toolbar_button(
            IconName::GitBranch,
            "Apply",
            RebaseToolbarTone::Primary,
            apply_enabled,
            Message::from(rebase::Message::ApplyRequested(RebaseApplyMode::RebaseOnly,)),
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    if release_promotion_active {
        actions = actions.push(rebase_toolbar_button(
            IconName::Cloud,
            "Promote",
            RebaseToolbarTone::Danger,
            apply_enabled,
            Message::from(rebase::Message::ApplyRequested(
                RebaseApplyMode::ReleasePromotionAuto,
            )),
        ));
    } else {
        actions = actions.push(rebase_toolbar_button(
            IconName::Cloud,
            "Push",
            RebaseToolbarTone::Danger,
            apply_enabled,
            Message::from(rebase::Message::ApplyRequested(
                RebaseApplyMode::RebaseThenForcePush,
            )),
        ));
    }

    actions = actions.push(rebase_toolbar_button(
        IconName::Close,
        "Cancel",
        RebaseToolbarTone::Neutral,
        true,
        Message::from(rebase::Message::Cancelled),
    ));

    container(
        row![
            column![
                text("Interactive Rebase")
                    .size(theme::FS_BASE)
                    .font(theme::font_semibold())
                    .color(color::TEXT)
                    .wrapping(Wrapping::None),
                text(status)
                    .size(theme::FS_SM)
                    .font(theme::font_regular())
                    .color(color::TEXT_MUTED)
                    .wrapping(Wrapping::None),
            ]
            .spacing(2)
            .width(Length::Fill),
            actions.height(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD)
        .height(Length::Fill),
    )
    .padding(Padding::from([theme::SP_SM, theme::SP_LG]))
    .width(Length::Fill)
    .height(Length::Fixed(REBASE_TOOLBAR_HEIGHT))
    .style(styles::commit_list_header)
    .into()
}

fn compact_toolbar_label(label: &str) -> String {
    const MAX_CHARS: usize = 22;
    let mut chars = label.chars();
    let prefix = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[derive(Clone, Copy)]
enum RebaseToolbarTone {
    Neutral,
    Primary,
    Danger,
}

fn rebase_toolbar_button<'a>(
    icon: IconName,
    label: &'static str,
    tone: RebaseToolbarTone,
    enabled: bool,
    message: Message,
) -> Element<'a, Message> {
    let tint = if !enabled {
        color::TEXT_SUBTLE
    } else {
        match tone {
            RebaseToolbarTone::Neutral => color::TEXT_MUTED,
            RebaseToolbarTone::Primary => color::ACCENT,
            RebaseToolbarTone::Danger => color::DANGER,
        }
    };
    let text_color = if enabled {
        color::TEXT
    } else {
        color::TEXT_SUBTLE
    };
    let content = row![
        icons::icon(icon, 14, tint),
        text(label)
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(text_color),
    ]
    .align_y(Alignment::Center)
    .spacing(6);

    button(content)
        .padding(Padding::from([4, 8]))
        .style(styles::toolbar_button)
        .on_press_maybe(enabled.then_some(message))
        .into()
}

fn header<'a>() -> Element<'a, Message> {
    // Pre-gutter this spacer was 92.0 (matching plan_row's leading columns
    // plus a small fudge for container-padding diffs). The gutter adds one new
    // child (REBASE_GUTTER_WIDTH) and one new SP_SM spacing slot between it
    // and its neighbours, so we shift the header right by the same amount.
    const HEADER_LEADING_SPACER: f32 = 92.0 + REBASE_GUTTER_WIDTH + theme::SP_SM as f32;
    container(
        row![
            Space::with_width(Length::Fixed(HEADER_LEADING_SPACER)),
            container(
                text("ACTION")
                    .size(theme::FS_XS)
                    .font(theme::font_semibold())
                    .color(color::TEXT_SUBTLE),
            )
            .width(Length::Fixed(ACTION_COLUMN_WIDTH)),
            Space::with_width(Length::Fixed(theme::SP_MD as f32)),
            container(
                text("SHA")
                    .size(theme::FS_XS)
                    .font(theme::font_semibold())
                    .color(color::TEXT_SUBTLE),
            )
            .width(Length::Fixed(SHA_COLUMN_WIDTH)),
            Space::with_width(Length::Fixed(theme::SP_MD as f32)),
            text("MESSAGE")
                .size(theme::FS_XS)
                .font(theme::font_semibold())
                .color(color::TEXT_SUBTLE),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([8, theme::SP_LG]))
    .width(Length::Fill)
    .style(styles::commit_list_header)
    .into()
}

fn plan_row<'a>(
    session: &'a InteractiveRebaseSession,
    row_data: &'a RebasePlanRow,
    index: usize,
    gutter_row: GraphRow,
    inherits_reword: bool,
) -> Element<'a, Message> {
    let selected = session.selected == index;
    let drag_active = session.drag.as_ref().is_some_and(|drag| drag.started);
    let dragging_source = session
        .drag
        .as_ref()
        .is_some_and(|drag| drag.started && drag.source_index == index);
    let bar_color = if selected {
        color::with_alpha(color::ACCENT, 0.65)
    } else {
        Color::TRANSPARENT
    };
    let bar = container(Space::new(Length::Fixed(2.0), Length::Fixed(ROW_HEIGHT)))
        .style(styles::solid_bar(bar_color));

    let gutter = canvas(RebaseGutterCanvas {
        row: gutter_row,
        muted: row_data.action == RebaseAction::Drop,
    })
    .width(Length::Fixed(REBASE_GUTTER_WIDTH))
    .height(Length::Fixed(ROW_HEIGHT));

    let up = move_button(
        IconName::ChevronUp,
        index > 0,
        Message::from(rebase::Message::MoveUp(index)),
        selected,
    );
    let down = move_button(
        IconName::ChevronDown,
        index + 1 < session.plan.len(),
        Message::from(rebase::Message::MoveDown(index)),
        selected,
    );

    // A pick whose group contains a squash gets git's combined-message editor
    // at rebase time, which effectively rewords it — flag that here by
    // tinting the chip ACCENT (matching the reword action's own colour).
    let chip_color = if inherits_reword {
        color::ACCENT
    } else {
        action_color(row_data.action)
    };
    let action: Element<'a, Message> = pick_list(
        allowed_actions(index),
        Some(ActionOption(row_data.action)),
        move |opt| Message::from(rebase::Message::ActionSet(index, opt.0)),
    )
    .padding(Padding::from([2, 6]))
    .text_size(theme::FS_SM)
    .style(styles::rebase_action_pick_list(chip_color))
    .menu_style(styles::release_prep_pick_list_menu)
    .width(Length::Fill)
    .into();

    let sha = text(short_id(&row_data.commit.id))
        .size(theme::FS_SM)
        .font(iced::Font::MONOSPACE)
        .color(color::TEXT_MUTED);

    let summary: Element<'a, Message> = if row_data.action == RebaseAction::Reword {
        let value = session
            .reword_drafts
            .get(&row_data.commit.id)
            .map(String::as_str)
            .unwrap_or(row_data.commit.summary.as_str());
        text_input("Commit message", value)
            .on_input(move |value| Message::from(rebase::Message::RewordChanged(index, value)))
            .on_submit(Message::from(rebase::Message::RewordCommitted(index)))
            .padding(Padding::from([4, 8]))
            .size(theme::FS_SM)
            .into()
    } else {
        text(row_data.commit.summary.clone())
            .size(theme::FS_SM)
            .font(if selected {
                theme::font_semibold()
            } else {
                theme::font_regular()
            })
            .color(if row_data.action == RebaseAction::Drop {
                color::DANGER
            } else {
                color::TEXT
            })
            .into()
    };

    let content = row![
        bar,
        gutter,
        up,
        down,
        container(action).width(Length::Fixed(ACTION_COLUMN_WIDTH)),
        Space::with_width(Length::Fixed(theme::SP_MD as f32)),
        container(sha).width(Length::Fixed(SHA_COLUMN_WIDTH)),
        Space::with_width(Length::Fixed(theme::SP_MD as f32)),
        container(summary).width(Length::Fill),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);

    let content = container(content)
        .height(Length::Fixed(ROW_HEIGHT))
        .width(Length::Fill)
        .padding(Padding::from([0, theme::SP_MD]))
        .style(styles::rebase_row_container(selected, dragging_source));

    let interaction = if dragging_source || drag_active {
        mouse::Interaction::Grabbing
    } else {
        mouse::Interaction::Grab
    };

    mouse_area(content)
        .on_press(Message::from(rebase::Message::DragPressed(index)))
        .on_release(Message::from(rebase::Message::DragEnded))
        .interaction(interaction)
        .into()
}

fn move_button(
    icon: IconName,
    enabled: bool,
    message: Message,
    selected: bool,
) -> Element<'static, Message> {
    let tint = if enabled {
        color::TEXT_SUBTLE
    } else {
        color::with_alpha(color::TEXT_SUBTLE, 0.45)
    };
    let content = container(icons::icon(icon, 11, tint))
        .center_x(Length::Fixed(MOVE_BUTTON_SIZE))
        .center_y(Length::Fixed(MOVE_BUTTON_SIZE));

    button(content)
        .padding(Padding::from([0, 0]))
        .style(styles::commit_row_button(selected))
        .on_press_maybe(enabled.then_some(message))
        .into()
}

fn can_apply(session: &InteractiveRebaseSession) -> bool {
    !session.plan.is_empty()
        && !drag_in_progress(session)
        && !session.applying
        && !matches!(
            session.plan.first().map(|row| row.action),
            Some(RebaseAction::Squash | RebaseAction::Fixup)
        )
        && session
            .plan
            .iter()
            .filter(|row| row.action == RebaseAction::Reword)
            .all(|row| {
                session
                    .reword_drafts
                    .get(&row.commit.id)
                    .is_some_and(|message| !message.trim().is_empty())
            })
}

fn can_pick_mine(session: &InteractiveRebaseSession) -> bool {
    !session.applying
        && !drag_in_progress(session)
        && own_commit_count(session).is_some_and(|count| count > 0)
}

fn drag_in_progress(session: &InteractiveRebaseSession) -> bool {
    session.drag.as_ref().is_some_and(|drag| drag.started)
}

fn own_commit_count(session: &InteractiveRebaseSession) -> Option<usize> {
    let author_email = session
        .current_author_email
        .as_deref()
        .and_then(normalized_email)?;
    Some(
        session
            .plan
            .iter()
            .filter(|row| emails_match(&row.commit.author_email, &author_email))
            .count(),
    )
}

fn normalized_email(email: &str) -> Option<String> {
    let email = email.trim();
    (!email.is_empty()).then(|| email.to_ascii_lowercase())
}

fn emails_match(commit_email: &str, configured_email: &str) -> bool {
    normalized_email(commit_email).as_deref() == Some(configured_email)
}

fn rebase_summary(session: &InteractiveRebaseSession) -> Element<'_, Message> {
    column![
        detail_label("INTERACTIVE REBASE"),
        text(format!(
            "{} onto {}",
            session.current_branch.short_name, session.target.short_name
        ))
        .size(theme::FS_BASE)
        .font(theme::font_semibold())
        .color(color::TEXT)
        .wrapping(Wrapping::Word),
        text(format!(
            "{} of {} operations selected",
            session.selected.saturating_add(1).min(session.plan.len()),
            session.plan.len()
        ))
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .color(color::TEXT_MUTED),
    ]
    .spacing(2)
    .into()
}

fn selected_operation<'a>(
    session: &'a InteractiveRebaseSession,
    row: &'a RebasePlanRow,
) -> Element<'a, Message> {
    let actions: Vec<RebaseAction> = session.plan.iter().map(|row| row.action).collect();
    let inherits_reword = pick_inherits_reword(&actions, session.selected);
    let action_text_color = if inherits_reword {
        color::ACCENT
    } else {
        action_color(row.action)
    };
    let effect: String = if inherits_reword {
        format!(
            "{} A squash below will reopen git's message editor for this pick, so its message gets rewritten too.",
            action_description(row.action)
        )
    } else {
        action_description(row.action).to_string()
    };
    column![
        detail_label("SELECTED OPERATION"),
        detail_value("Action", action_label(row.action), action_text_color, false),
        detail_value("Commit", &row.commit.id, color::TEXT, true),
        detail_value("Message", &row.commit.summary, color::TEXT, false),
        detail_value("Effect", &effect, color::TEXT_MUTED, false),
        if row.action == RebaseAction::Reword {
            reword_detail(session, row)
        } else {
            Space::with_height(0).into()
        },
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn reword_detail<'a>(
    session: &'a InteractiveRebaseSession,
    row: &'a RebasePlanRow,
) -> Element<'a, Message> {
    let message = session
        .reword_drafts
        .get(&row.commit.id)
        .map(String::as_str)
        .unwrap_or(row.commit.summary.as_str());

    detail_value("New message", message, color::ACCENT, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActionOption(RebaseAction);

impl std::fmt::Display for ActionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(action_label(self.0))
    }
}

/// The full action menu for a rebase row. squash/fixup are filtered out at the
/// first row because they would have no prior operation to merge into.
fn allowed_actions(index: usize) -> Vec<ActionOption> {
    [
        RebaseAction::Pick,
        RebaseAction::Reword,
        RebaseAction::Edit,
        RebaseAction::Squash,
        RebaseAction::Fixup,
        RebaseAction::Drop,
    ]
    .into_iter()
    .filter(|action| !(index == 0 && is_merge_action(*action)))
    .map(ActionOption)
    .collect()
}

fn is_merge_action(action: RebaseAction) -> bool {
    matches!(action, RebaseAction::Squash | RebaseAction::Fixup)
}

fn detail_label(label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(theme::FS_XS)
        .font(theme::font_semibold())
        .color(color::TEXT_SUBTLE)
        .into()
}

fn detail_value(
    label: &'static str,
    value: &str,
    value_color: Color,
    mono: bool,
) -> Element<'static, Message> {
    let value_text = if mono {
        text(value.to_string())
            .size(theme::FS_SM)
            .font(iced::Font::MONOSPACE)
            .color(value_color)
            .wrapping(Wrapping::Word)
    } else {
        text(value.to_string())
            .size(theme::FS_BASE)
            .font(theme::font_regular())
            .color(value_color)
            .wrapping(Wrapping::Word)
    };

    column![detail_label(label), value_text].spacing(2).into()
}

fn action_description(action: RebaseAction) -> &'static str {
    match action {
        RebaseAction::Pick => "Replay this commit unchanged.",
        RebaseAction::Reword => "Replay this commit and replace its commit message.",
        RebaseAction::Edit => {
            "Pause at this commit so staged changes can be amended before continuing."
        }
        RebaseAction::Squash => {
            "Combine this commit into the previous operation and keep both messages."
        }
        RebaseAction::Fixup => {
            "Combine this commit into the previous operation and discard this message."
        }
        RebaseAction::Drop => "Omit this commit from the rewritten branch.",
    }
}

fn action_label(action: RebaseAction) -> &'static str {
    match action {
        RebaseAction::Pick => "pick",
        RebaseAction::Reword => "reword",
        RebaseAction::Edit => "edit",
        RebaseAction::Squash => "squash",
        RebaseAction::Fixup => "fixup",
        RebaseAction::Drop => "drop",
    }
}

fn action_color(action: RebaseAction) -> Color {
    match action {
        RebaseAction::Pick => color::TEXT_MUTED,
        RebaseAction::Reword | RebaseAction::Squash => color::ACCENT,
        RebaseAction::Fixup => color::with_alpha(color::ACCENT, 0.75),
        RebaseAction::Edit => color::WARNING,
        RebaseAction::Drop => Color::WHITE,
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(7).collect()
}
