//! Embedded interactive terminal scoped to a repo or worktree directory.

use std::path::Path;

use iced::widget::text::Span;
use iced::widget::{
    button, column, container, mouse_area, rich_text, row, scrollable, text, text::Wrapping, Space,
};
use iced::{mouse, Alignment, Element, Length, Padding};
use naite_core::WorktreeSummary;

use crate::features::terminal::{self, SessionSelection};
use crate::icons::{self, IconName};
use crate::state::{
    TerminalImePreedit, TerminalLine, TerminalSession, TerminalState, TerminalStatus,
};
use crate::styles;
use crate::theme::{self, color};
use crate::Message;

/// Total height reserved for the bottom terminal pane when expanded.
pub const TERMINAL_PANEL_HEIGHT: f32 = 320.0;
/// Height reserved for the bottom terminal pane when minimized.
pub const TERMINAL_PANEL_HEIGHT_MINIMIZED: f32 = 96.0;
/// Approximate vertical room consumed by the panel chrome (header + tab strip
/// + paddings). Used by the dimension calculation to derive shell row count.
pub const TERMINAL_PANEL_CHROME: f32 = 110.0;
/// Approximate monospace cell width used for terminal hit-testing.
pub const TERMINAL_CHAR_WIDTH: f32 = 7.6;
/// Fixed terminal row height used by rendering and mouse hit-testing.
pub const TERMINAL_LINE_HEIGHT: f32 = 15.0;

const PATH_LABEL_MAX_CHARS: usize = 64;
const TAB_LABEL_MAX_CHARS: usize = 18;

pub fn terminal_panel<'a>(
    terminal_state: &'a TerminalState,
    repo_path: Option<&'a Path>,
    _head_branch: Option<&'a str>,
    _worktrees: &'a [WorktreeSummary],
    _input_id: &'a iced::widget::text_input::Id,
) -> Element<'a, Message> {
    let Some(session) = terminal_state.active_session() else {
        return empty_terminal(repo_path);
    };
    let session_cwd = session
        .shell_cwd
        .as_deref()
        .unwrap_or(session.target.cwd.as_path());

    let title_label = column![
        text("TERMINAL")
            .size(theme::FS_XS)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
        text(truncate_middle(
            &session_cwd.display().to_string(),
            PATH_LABEL_MAX_CHARS,
        ))
        .size(theme::FS_SM)
        .font(theme::font_regular())
        .wrapping(Wrapping::None)
        .width(Length::Fill)
        .color(color::TEXT_MUTED),
    ]
    .width(Length::Fill)
    .spacing(2);

    let mut header = row![title_label]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center);
    if let Some(status_chip) = status_label(session) {
        header = header.push(status_chip);
    }
    header = header.push(icon_action_button(
        if session.minimized {
            IconName::ChevronUp
        } else {
            IconName::ChevronDown
        },
        terminal::Message::ToggleMinimized,
    ));
    header = header.push(icon_action_button(
        IconName::Close,
        terminal::Message::CloseRequested,
    ));

    let divider = container(Space::with_height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(styles::hairline_divider);

    let tabs = tab_strip(terminal_state, repo_path);
    let controls = row![tabs, Space::with_width(Length::Fill)]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center);

    let mut content = column![header, divider, controls].spacing(theme::SP_SM);
    if !session.minimized {
        content = content.push(terminal_viewport(session));
    }

    container(content.padding(theme::SP_MD))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(styles::floating_panel)
        .into()
}

fn tab_strip<'a>(
    terminal_state: &'a TerminalState,
    repo_path: Option<&'a Path>,
) -> Element<'a, Message> {
    let mut row_widget = row![].spacing(4).align_y(Alignment::Center);
    for session in &terminal_state.sessions {
        let active = terminal_state.active == Some(session.id);
        row_widget = row_widget.push(tab_chip(session, active));
    }
    row_widget = row_widget.push(new_tab_button(repo_path, terminal_state));
    row_widget.into()
}

fn tab_chip<'a>(session: &'a TerminalSession, active: bool) -> Element<'a, Message> {
    let label_text = text(truncate_end(&session.label, TAB_LABEL_MAX_CHARS))
        .size(theme::FS_SM)
        .font(if active {
            theme::font_semibold()
        } else {
            theme::font_regular()
        })
        .wrapping(Wrapping::None)
        .color(if active {
            color::TEXT
        } else {
            color::TEXT_MUTED
        });

    let activate = button(label_text)
        .padding(Padding::from([4, 8]))
        .style(styles::tab_strip_button(active))
        .on_press(Message::from(terminal::Message::SessionSelected(
            SessionSelection::Existing(session.id),
        )));

    let close = button(
        text("×")
            .size(theme::FS_SM)
            .font(theme::font_semibold())
            .wrapping(Wrapping::None)
            .color(color::TEXT_SUBTLE),
    )
    .padding(Padding::from([2, 6]))
    .style(styles::tab_strip_button(false))
    .on_press(Message::from(terminal::Message::CloseSession(session.id)));

    row![activate, close]
        .spacing(2)
        .align_y(Alignment::Center)
        .into()
}

fn new_tab_button<'a>(
    repo_path: Option<&'a Path>,
    terminal_state: &'a TerminalState,
) -> Element<'a, Message> {
    let label = text("+")
        .size(theme::FS_MD)
        .font(theme::font_semibold())
        .wrapping(Wrapping::None)
        .color(color::TEXT_SUBTLE);

    let mut btn = button(label)
        .padding(Padding::from([2, 8]))
        .style(styles::tab_strip_button(false));
    if terminal_state.active_session().is_some() || repo_path.is_some() {
        btn = btn.on_press(Message::from(terminal::Message::NewSessionRequested));
    }
    btn.into()
}

fn terminal_viewport<'a>(session: &'a TerminalSession) -> Element<'a, Message> {
    let body: Element<'a, Message> = if session.screen.lines.is_empty() {
        let label = match session.status {
            TerminalStatus::Idle => "Shell is idle.".to_string(),
            TerminalStatus::Starting => "Starting shell...".to_string(),
            TerminalStatus::Running => String::new(),
            TerminalStatus::Exited => "[session exited]".to_string(),
            TerminalStatus::Error => session
                .error
                .clone()
                .unwrap_or_else(|| "[terminal error]".into()),
        };
        column![plain_terminal_line(label)].into()
    } else {
        let cursor = session.screen.cursor;
        let mut lines = column![].spacing(0);
        for (row_idx, line) in session.screen.lines.iter().enumerate() {
            let cursor_col = cursor.filter(|c| c.row == row_idx).map(|c| c.col);
            let suggestion_suffix = if cursor_col.is_some() {
                session
                    .active_suggestion
                    .as_ref()
                    .map(|s| s.suffix.as_str())
            } else {
                None
            };
            let ime_preedit = if cursor_col.is_some() {
                session.ime_preedit.as_ref()
            } else {
                None
            };
            let selection = terminal_selection_cols(session.selection, row_idx, line);
            lines = lines.push(
                container(terminal_line_view(
                    line,
                    cursor_col,
                    suggestion_suffix,
                    ime_preedit,
                    selection,
                ))
                .height(Length::Fixed(TERMINAL_LINE_HEIGHT))
                .width(Length::Fill),
            );
        }
        if let Some(error) = &session.error {
            lines = lines.push(plain_terminal_line(format!("[error] {error}")));
        }
        lines.into()
    };

    let scrollable_body = scrollable(body)
        .direction(styles::thin_scrollbar_dir())
        .anchor_bottom()
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::thin_scrollbar);

    container(
        mouse_area(scrollable_body)
            .on_move(|point| Message::from(terminal::Message::PointerMoved(point)))
            .on_press(Message::from(terminal::Message::SelectionStarted))
            .on_release(Message::from(terminal::Message::SelectionEnded))
            .interaction(mouse::Interaction::Text),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(theme::SP_SM)
    .clip(true)
    .into()
}

fn terminal_line_view<'a>(
    line: &TerminalLine,
    cursor_col: Option<usize>,
    suggestion_suffix: Option<&str>,
    ime_preedit: Option<&TerminalImePreedit>,
    selection_cols: Option<(usize, usize)>,
) -> Element<'a, Message> {
    let Some(col) = cursor_col else {
        return terminal_rich_line(line.text(), selection_cols);
    };

    let mut chars = line.visible_chars();
    let idx = line.cell_col_to_char_idx(col).min(chars.len());
    while chars.len() <= idx {
        chars.push(' ');
    }

    let before: String = chars.iter().take(idx).collect();
    let cursor_char = if ime_preedit.is_some() {
        " ".to_string()
    } else {
        chars[idx].to_string()
    };
    let mut after: String = chars.iter().skip(idx + 1).collect();
    while after.ends_with(' ') {
        after.pop();
    }

    let mut spans = terminal_spans_segment(before.clone(), color::TEXT, selection_cols, 0);
    let mut preedit_after_cursor = "";
    if let Some(preedit) = ime_preedit {
        let (before_cursor, after_cursor) = split_ime_preedit_at_cursor(preedit);
        if !before_cursor.is_empty() {
            spans.extend(terminal_spans(before_cursor.to_string(), color::ACCENT));
        }
        preedit_after_cursor = after_cursor;
    }

    spans.push(terminal_span(cursor_char, color::BG).background(color::ACCENT));

    if !preedit_after_cursor.is_empty() {
        spans.extend(terminal_spans(
            preedit_after_cursor.to_string(),
            color::ACCENT,
        ));
    }
    spans.extend(terminal_spans_segment(
        after.clone(),
        color::TEXT,
        selection_cols,
        before.chars().count() + 1,
    ));
    if after.is_empty() && ime_preedit.is_none() {
        if let Some(suffix) = suggestion_suffix.filter(|suffix| !suffix.is_empty()) {
            spans.extend(terminal_spans(suffix.to_string(), color::TEXT_SUBTLE));
        }
    }

    rich_text(spans)
        .size(theme::FS_SM)
        .font(theme::font_code())
        .into()
}

pub(crate) fn split_ime_preedit_at_cursor(preedit: &TerminalImePreedit) -> (&str, &str) {
    let cursor = preedit
        .cursor
        .map(|(_, end)| end)
        .unwrap_or_else(|| preedit.text.len());
    let cursor = nearest_char_boundary(&preedit.text, cursor.min(preedit.text.len()));
    preedit.text.split_at(cursor)
}

fn nearest_char_boundary(text: &str, mut idx: usize) -> usize {
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn plain_terminal_line<'a>(line: String) -> Element<'a, Message> {
    terminal_rich_line(line, None)
}

fn terminal_rich_line<'a>(
    line: String,
    selection_cols: Option<(usize, usize)>,
) -> Element<'a, Message> {
    let line = if line.is_empty() {
        " ".to_string()
    } else {
        line
    };
    rich_text(terminal_spans_segment(line, color::TEXT, selection_cols, 0))
        .size(theme::FS_SM)
        .font(theme::font_code())
        .into()
}

fn terminal_spans(text: String, color: iced::Color) -> Vec<Span<'static, Message>> {
    terminal_spans_segment(text, color, None, 0)
}

fn terminal_spans_segment(
    text: String,
    color: iced::Color,
    selection_cols: Option<(usize, usize)>,
    start_col: usize,
) -> Vec<Span<'static, Message>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_fallback = None;
    let mut current_selected = None;

    for (idx, ch) in text.chars().enumerate() {
        let fallback = uses_ui_font_fallback(ch);
        let selected = selection_cols.is_some_and(|(start, end)| {
            let col = start_col + idx;
            col >= start && col < end
        });
        if current_fallback == Some(fallback) && current_selected == Some(selected) {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            spans.push(terminal_span_with_fallback_and_background(
                std::mem::take(&mut current),
                color,
                current_fallback.unwrap_or(false),
                current_selected.unwrap_or(false),
            ));
        }
        current.push(ch);
        current_fallback = Some(fallback);
        current_selected = Some(selected);
    }

    if !current.is_empty() {
        spans.push(terminal_span_with_fallback_and_background(
            current,
            color,
            current_fallback.unwrap_or(false),
            current_selected.unwrap_or(false),
        ));
    }

    spans
}

fn terminal_span(text: String, color: iced::Color) -> Span<'static, Message> {
    let fallback = text.chars().any(uses_ui_font_fallback);
    terminal_span_with_fallback(text, color, fallback)
}

fn terminal_span_with_fallback(
    text: String,
    color: iced::Color,
    fallback: bool,
) -> Span<'static, Message> {
    terminal_span_with_fallback_and_background(text, color, fallback, false)
}

fn terminal_span_with_fallback_and_background(
    text: String,
    color: iced::Color,
    fallback: bool,
    selected: bool,
) -> Span<'static, Message> {
    let mut span = Span::new(text).color(color);
    if selected {
        span = span.background(crate::theme::color::with_alpha(
            crate::theme::color::ACCENT,
            0.45,
        ));
    }
    if fallback {
        span.font(theme::font_regular())
    } else {
        span
    }
}

fn terminal_selection_cols(
    selection: Option<crate::state::TerminalSelection>,
    row: usize,
    line: &TerminalLine,
) -> Option<(usize, usize)> {
    let selection = selection?;
    let (start, end) = selection.normalized();
    if row < start.row || row > end.row || start == end {
        return None;
    }
    let line_len = line.text().chars().count();
    let row_start = if row == start.row { start.col } else { 0 };
    let row_end = if row == end.row { end.col } else { line_len };
    let row_start = row_start.min(line_len);
    let row_end = row_end.min(line_len);
    (row_start < row_end).then_some((row_start, row_end))
}

fn uses_ui_font_fallback(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x11ff
            | 0x3130..=0x318f
            | 0xac00..=0xd7af
            | 0x3040..=0x30ff
            | 0x3400..=0x9fff
    )
}

fn empty_terminal<'a>(repo_path: Option<&'a Path>) -> Element<'a, Message> {
    let label = repo_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Open a repository first".into());
    container(
        column![
            text("Terminal")
                .size(theme::FS_BASE)
                .font(theme::font_semibold())
                .color(color::TEXT),
            text(label)
                .size(theme::FS_SM)
                .font(theme::font_regular())
                .color(color::TEXT_SUBTLE),
        ]
        .align_x(Alignment::Center)
        .spacing(theme::SP_SM),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(styles::surface_panel)
    .into()
}

fn status_label<'a>(session: &TerminalSession) -> Option<Element<'a, Message>> {
    // Running shells advertise their title via OSC escapes (typically
    // user@host:cwd), which duplicates the cwd shown in the panel title.
    // Skip the chip for the steady-state Running case so the header stays
    // compact when the panel is narrow.
    let (label, text_color) = match session.status {
        TerminalStatus::Idle => ("idle".to_string(), color::TEXT_MUTED),
        TerminalStatus::Starting => ("starting".to_string(), color::WARNING),
        TerminalStatus::Running => return None,
        TerminalStatus::Exited => (
            match session.last_exit {
                Some(code) => format!("exit {code}"),
                None => "exited".into(),
            },
            color::TEXT_MUTED,
        ),
        TerminalStatus::Error => ("error".to_string(), color::DANGER),
    };
    Some(
        container(
            text(label)
                .size(theme::FS_XS)
                .font(theme::font_regular())
                .wrapping(Wrapping::None)
                .color(text_color),
        )
        .padding(Padding::from([2, 8]))
        .style(styles::header_chip)
        .into(),
    )
}

fn icon_action_button<'a>(icon: IconName, message: terminal::Message) -> Element<'a, Message> {
    button(icons::icon(icon, 14, color::TEXT_MUTED))
        .padding(Padding::from([4, 6]))
        .style(styles::ghost_icon_button)
        .on_press(Message::from(message))
        .into()
}

/// Shorten a string that is too long for inline display. Keeps the trailing
/// segment intact since the most meaningful part of paths and titles is
/// usually on the right.
fn truncate_middle(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let tail: String = text.chars().skip(count - keep).collect();
    format!("…{tail}")
}

fn truncate_end(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let head: String = text.chars().take(keep).collect();
    format!("{head}…")
}
