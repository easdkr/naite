use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text::Wrapping,
    text_input, Space,
};
use iced::{mouse, Alignment, Color, Element, Length, Padding, Point};
use naite_core::{CommitDiff, RebaseAction, WorktreeDiffTarget};

use crate::features::rebase::{
    self, DragState, InteractiveRebaseSession, RebaseApplyMode, RebasePlanPreset, RebasePlanRow,
};
use crate::icons::{self, IconName};
use crate::state::{DiffViewMode, FileInsightState};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

use super::detail_pane::{diff_content, DiffContentProps};
use super::ROW_HEIGHT;

const MOVE_BUTTON_SIZE: f32 = 22.0;
const ACTION_COLUMN_WIDTH: f32 = 74.0;
const SHA_COLUMN_WIDTH: f32 = 68.0;
const INSERTION_LINE_HEIGHT: f32 = 2.0;
const GHOST_HORIZONTAL_INSET: f32 = 8.0;
const REBASE_TOOLBAR_HEIGHT: f32 = 52.0;

pub fn rebase_editor<'a>(
    session: &'a InteractiveRebaseSession,
    cursor: Option<Point>,
) -> Element<'a, Message> {
    let active_gap = active_insertion_gap(session);
    let mut body = column![toolbar(session), header()].spacing(0);
    body = body.push(insertion_gap(active_gap == Some(0)));
    for (index, row) in session.plan.iter().enumerate() {
        body = body.push(plan_row(session, row, index));
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
                    Space::with_height(theme::SP_MD),
                    action_picker(session.selected, row.action, session.applying),
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

fn toolbar(session: &InteractiveRebaseSession) -> Element<'_, Message> {
    let dropped = session
        .plan
        .iter()
        .filter(|row| row.action == RebaseAction::Drop)
        .count();
    let replayed = session.plan.len().saturating_sub(dropped);
    let target_branch = compact_toolbar_label(&session.target.short_name);
    let status = if dropped == 0 {
        format!("{} commits will replay onto {}", replayed, target_branch)
    } else {
        format!(
            "{} commits will replay onto {}, {} dropped",
            replayed, target_branch, dropped
        )
    };
    let status = match own_commit_count(session) {
        Some(own) => format!("{status}, {own} authored"),
        None => status,
    };

    let apply_enabled = can_apply(session);
    let pick_mine_enabled = can_pick_mine(session);
    let preset_enabled = !session.applying && !drag_in_progress(session);

    let current_branch = compact_toolbar_label(&session.current_branch.short_name);

    container(
        row![
            column![
                text(format!(
                    "Interactive Rebase: {} onto {}",
                    current_branch, target_branch
                ))
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
            button(text("Keep Mine").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press_maybe(pick_mine_enabled.then_some(Message::from(
                    rebase::Message::PresetRequested(RebasePlanPreset::KeepMine,)
                ))),
            button(text("Squash Mine").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press_maybe(pick_mine_enabled.then_some(Message::from(
                    rebase::Message::PresetRequested(RebasePlanPreset::SquashMine,)
                ))),
            button(text("Squash All").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press_maybe(preset_enabled.then_some(Message::from(
                    rebase::Message::PresetRequested(RebasePlanPreset::SquashAll,)
                ))),
            button(text("Apply").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::primary_button)
                .on_press_maybe(apply_enabled.then_some(Message::from(
                    rebase::Message::ApplyRequested(RebaseApplyMode::RebaseOnly,)
                ))),
            button(text("Apply + Push").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::danger_button)
                .on_press_maybe(apply_enabled.then_some(Message::from(
                    rebase::Message::ApplyRequested(RebaseApplyMode::RebaseThenForcePush,)
                ))),
            button(text("Cancel").size(theme::FS_SM))
                .padding(Padding::from([5, 10]))
                .style(styles::subtle_button)
                .on_press(Message::from(rebase::Message::Cancelled)),
        ]
        .align_y(Alignment::Center)
        .spacing(theme::SP_MD)
        .height(Length::Fill),
    )
    .padding(Padding::from([theme::SP_MD, theme::SP_LG]))
    .width(Length::Fill)
    .height(Length::Fixed(REBASE_TOOLBAR_HEIGHT))
    .style(styles::commit_list_header)
    .into()
}

fn compact_toolbar_label(label: &str) -> String {
    const MAX_CHARS: usize = 28;
    let mut chars = label.chars();
    let prefix = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn header<'a>() -> Element<'a, Message> {
    container(
        row![
            Space::with_width(Length::Fixed(92.0)),
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

    let action = button(
        text(action_label(row_data.action))
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .color(action_color(row_data.action)),
    )
    .padding(Padding::from([3, 8]))
    .style(if row_data.action == RebaseAction::Drop {
        styles::danger_button
    } else {
        styles::subtle_button
    })
    .on_press(Message::from(rebase::Message::ActionSet(
        index,
        next_action(row_data.action, index),
    )));

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
    column![
        detail_label("SELECTED OPERATION"),
        detail_value(
            "Action",
            action_label(row.action),
            action_color(row.action),
            false
        ),
        detail_value("Commit", &row.commit.id, color::TEXT, true),
        detail_value("Message", &row.commit.summary, color::TEXT, false),
        detail_value(
            "Effect",
            action_description(row.action),
            color::TEXT_MUTED,
            false,
        ),
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

fn action_picker(
    index: usize,
    selected: RebaseAction,
    applying: bool,
) -> Element<'static, Message> {
    let mut rows = column![].spacing(theme::SP_SM);
    for group in [
        [RebaseAction::Pick, RebaseAction::Reword, RebaseAction::Edit],
        [
            RebaseAction::Squash,
            RebaseAction::Fixup,
            RebaseAction::Drop,
        ],
    ] {
        let mut action_row = row![].spacing(theme::SP_SM).align_y(Alignment::Center);
        for action in group {
            let enabled =
                !applying && action != selected && !(index == 0 && is_merge_action(action));
            action_row = action_row.push(action_choice(index, action, enabled));
        }
        rows = rows.push(action_row);
    }

    column![detail_label("CHANGE ACTION"), rows]
        .spacing(theme::SP_SM)
        .into()
}

fn action_choice(index: usize, action: RebaseAction, enabled: bool) -> Element<'static, Message> {
    button(
        text(action_label(action))
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .color(if action == RebaseAction::Drop {
                Color::WHITE
            } else {
                action_color(action)
            }),
    )
    .padding(Padding::from([4, 8]))
    .style(if action == RebaseAction::Drop {
        styles::danger_button
    } else {
        styles::subtle_button
    })
    .on_press_maybe(enabled.then_some(Message::from(rebase::Message::ActionSet(index, action))))
    .into()
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

fn detail_value<'a>(
    label: &'static str,
    value: &'a str,
    value_color: Color,
    mono: bool,
) -> Element<'a, Message> {
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

fn next_action(action: RebaseAction, index: usize) -> RebaseAction {
    match action {
        RebaseAction::Pick => RebaseAction::Reword,
        RebaseAction::Reword => RebaseAction::Edit,
        RebaseAction::Edit if index == 0 => RebaseAction::Drop,
        RebaseAction::Edit => RebaseAction::Squash,
        RebaseAction::Squash => RebaseAction::Fixup,
        RebaseAction::Fixup => RebaseAction::Drop,
        RebaseAction::Drop => RebaseAction::Pick,
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
