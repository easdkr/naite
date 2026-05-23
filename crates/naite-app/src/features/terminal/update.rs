use iced::Task;

use crate::features::terminal::{
    self, SessionSelection, TerminalCommand, TerminalEvent, TerminalInput, TerminalTarget,
};
use crate::state::{
    default_terminal_shell, IntegrationStatus, TerminalGridPoint, TerminalScreen,
    TerminalSelection, TerminalStatus,
};
use crate::{App, Message};

impl App {
    pub(crate) fn update_terminal(&mut self, message: terminal::Message) -> Task<Message> {
        match message {
            terminal::Message::OpenRequested => {
                self.manager.new_repo_menu_open = false;
                self.open_terminal()
            }
            terminal::Message::SessionSelected(selection) => {
                match selection {
                    SessionSelection::Existing(id) => {
                        self.terminal.active = Some(id);
                    }
                    SessionSelection::Target {
                        cwd,
                        label,
                        worktree_hint,
                    } => {
                        let target =
                            TerminalTarget::new(cwd, self.repo.path.clone(), worktree_hint);
                        let (cols, rows) = self.terminal_dimensions();
                        let id = self.terminal.create_session(
                            target,
                            label,
                            default_terminal_shell(),
                            cols,
                            rows,
                        );
                        return self.start_terminal_session(id);
                    }
                }
                self.terminal.open = true;
                Task::none()
            }
            terminal::Message::NewSessionRequested => {
                let Some((target, label)) = self.new_terminal_session_target() else {
                    return Task::none();
                };
                let (cols, rows) = self.terminal_dimensions();
                let id = self.terminal.create_session(
                    target,
                    label,
                    default_terminal_shell(),
                    cols,
                    rows,
                );
                self.start_terminal_session(id)
            }
            terminal::Message::StartRequested => {
                let Some(id) = self.terminal.active else {
                    return Task::none();
                };
                self.start_terminal_session(id)
            }
            terminal::Message::RestartRequested => {
                let Some(id) = self.terminal.active else {
                    return Task::none();
                };
                let _ = terminal::runtime::send(TerminalCommand::Close { id });
                if let Some(session) = self.terminal.session_mut(id) {
                    session.screen = TerminalScreen {
                        cols: session.cols,
                        rows: session.rows,
                        ..Default::default()
                    };
                    session.status = TerminalStatus::Idle;
                    session.error = None;
                    session.last_exit = None;
                }
                self.start_terminal_session(id)
            }
            terminal::Message::InterruptRequested => {
                if let Some(id) = self.terminal.active {
                    self.send_terminal_command(TerminalCommand::Interrupt { id });
                }
                Task::none()
            }
            terminal::Message::KillRequested => {
                if let Some(id) = self.terminal.active {
                    self.send_terminal_command(TerminalCommand::Kill { id });
                }
                Task::none()
            }
            terminal::Message::CloseRequested => {
                let ids: Vec<_> = self.terminal.sessions.iter().map(|s| s.id).collect();
                for id in ids {
                    self.send_terminal_command(TerminalCommand::Close { id });
                    self.terminal.remove_session(id);
                }
                self.terminal.open = false;
                Task::none()
            }
            terminal::Message::CloseSession(id) => {
                self.send_terminal_command(TerminalCommand::Close { id });
                self.terminal.remove_session(id);
                if self.terminal.sessions.is_empty() {
                    self.terminal.open = false;
                }
                Task::none()
            }
            terminal::Message::ToggleMinimized => {
                if let Some(session) = self.terminal.active_session_mut() {
                    session.minimized = !session.minimized;
                }
                Task::none()
            }
            terminal::Message::PointerMoved(point) => {
                let Some(id) = self.terminal.active else {
                    return Task::none();
                };
                let focus = {
                    let Some(session) = self.terminal.session(id) else {
                        return Task::none();
                    };
                    terminal_grid_point_at(session, point)
                };
                self.terminal.pointer_grid_position = Some(focus);
                let Some(session) = self.terminal.session_mut(id) else {
                    return Task::none();
                };
                let Some(selection) = session.selection else {
                    return Task::none();
                };
                if selection.active {
                    session.selection = Some(TerminalSelection { focus, ..selection });
                }
                Task::none()
            }
            terminal::Message::SelectionStarted => {
                let Some(anchor) = self.terminal.pointer_grid_position else {
                    return Task::none();
                };
                let Some(session) = self.terminal.active_session_mut() else {
                    return Task::none();
                };
                session.selection = Some(TerminalSelection {
                    anchor,
                    focus: anchor,
                    active: true,
                });
                Task::none()
            }
            terminal::Message::SelectionEnded => {
                if let Some(session) = self.terminal.active_session_mut() {
                    if let Some(mut selection) = session.selection {
                        selection.active = false;
                        session.selection = (!selection.is_empty()).then_some(selection);
                    }
                }
                Task::none()
            }
            terminal::Message::CopySelectionRequested => {
                let Some(text) = self
                    .terminal
                    .active_session()
                    .and_then(|session| session.selected_text())
                else {
                    return Task::none();
                };
                iced::clipboard::write(text)
            }
            terminal::Message::PasteRequested => iced::clipboard::read().map(|contents| {
                contents
                    .filter(|text| !text.is_empty())
                    .map(|text| terminal::Message::Input(TerminalInput::Paste(text)).into())
                    .unwrap_or(Message::NoOp)
            }),
            terminal::Message::Input(input) => {
                let Some(id) = self.terminal.active else {
                    return Task::none();
                };
                let bytes = match input {
                    TerminalInput::Bytes(bytes) => bytes,
                    TerminalInput::Paste(text) => bracketed_paste(text).into_bytes(),
                    TerminalInput::MaybeAcceptSuggestion { fallback } => {
                        if let Some(session) = self.terminal.session_mut(id) {
                            if let Some(suggestion) = session.active_suggestion.take() {
                                session.input_buffer.push_str(&suggestion.suffix);
                                session.input_cursor = session.input_buffer.chars().count();
                                suggestion.suffix.into_bytes()
                            } else {
                                fallback
                            }
                        } else {
                            fallback
                        }
                    }
                };
                self.send_terminal_command(TerminalCommand::Input { id, bytes });
                Task::none()
            }
            terminal::Message::RuntimeReady => {
                self.terminal.runtime_ready = true;
                let pending: Vec<_> = self
                    .terminal
                    .sessions
                    .iter()
                    .filter(|session| session.pending_start)
                    .map(|session| session.id)
                    .collect();
                let tasks = pending
                    .into_iter()
                    .map(|id| self.start_terminal_session(id))
                    .collect::<Vec<_>>();
                Task::batch(tasks)
            }
            terminal::Message::RuntimeEvent(event) => {
                self.apply_terminal_event(event);
                Task::none()
            }
        }
    }

    pub(crate) fn open_terminal(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            self.operation.error = Some("Open a repository first.".into());
            return Task::none();
        };
        let label = self
            .repo
            .head_branch
            .clone()
            .unwrap_or_else(|| "Current repo".into());
        let id = self.terminal.ensure_session(path, label);
        self.terminal.open = true;
        self.start_terminal_session(id)
    }

    pub(crate) fn ensure_repo_terminal_session(
        &mut self,
        path: std::path::PathBuf,
        label: String,
    ) -> Task<Message> {
        let id = self.terminal.ensure_session(path, label);
        if self.terminal.open {
            self.start_terminal_session(id)
        } else {
            Task::none()
        }
    }

    pub(crate) fn resize_active_terminal_to_window(&mut self) {
        let Some(id) = self.terminal.active else {
            return;
        };
        let (cols, rows) = self.terminal_dimensions();
        let Some(session) = self.terminal.session_mut(id) else {
            return;
        };
        if session.cols == cols && session.rows == rows {
            return;
        }
        session.cols = cols;
        session.rows = rows;
        session.screen.cols = cols;
        session.screen.rows = rows;
        let should_send = matches!(
            session.status,
            TerminalStatus::Starting | TerminalStatus::Running
        );
        if should_send {
            self.send_terminal_command(TerminalCommand::Resize { id, cols, rows });
        }
    }

    fn start_terminal_session(&mut self, id: terminal::TerminalSessionId) -> Task<Message> {
        let runtime_ready = self.terminal.runtime_ready;
        let Some(session) = self.terminal.session_mut(id) else {
            return Task::none();
        };
        if session.status == TerminalStatus::Running
            || (session.status == TerminalStatus::Starting && !session.pending_start)
        {
            return Task::none();
        }

        session.status = TerminalStatus::Starting;
        session.pending_start = !runtime_ready;
        session.error = None;
        let target = session.target.clone();
        let shell = session.shell.clone();
        let cols = session.cols;
        let rows = session.rows;
        let shell_kind = session.shell_kind;

        if !runtime_ready {
            return Task::none();
        }

        let integration = match shell_kind {
            crate::features::terminal::zsh_integration::ShellKind::Zsh => {
                crate::features::terminal::message::IntegrationConfig::Zsh
            }
            crate::features::terminal::zsh_integration::ShellKind::Unsupported => {
                crate::features::terminal::message::IntegrationConfig::Disabled
            }
        };
        if let Some(session) = self.terminal.session_mut(id) {
            session.status = TerminalStatus::Starting;
            session.pending_start = false;
            if matches!(
                integration,
                crate::features::terminal::message::IntegrationConfig::Zsh
            ) {
                session.integration_status = IntegrationStatus::Pending;
            }
        }
        self.send_terminal_command(TerminalCommand::Create {
            id,
            target,
            shell,
            cols,
            rows,
            integration,
        });
        Task::none()
    }

    fn apply_terminal_event(&mut self, event: TerminalEvent) {
        match event {
            TerminalEvent::Started { id } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.status = TerminalStatus::Running;
                    session.pending_start = false;
                    session.error = None;
                }
            }
            TerminalEvent::ScreenUpdated { id, screen } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.screen = screen;
                }
            }
            TerminalEvent::Exited { id, status } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.status = TerminalStatus::Exited;
                    session.pending_start = false;
                    session.last_exit = status;
                }
            }
            TerminalEvent::Error { id, message } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.status = TerminalStatus::Error;
                    session.pending_start = false;
                    session.error = Some(message);
                }
            }
            TerminalEvent::Bell { id } => {
                if self.terminal.session(id).is_some() {
                    self.set_transient_status("Terminal bell".into());
                }
            }
            TerminalEvent::TitleChanged { id, title } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.title = (!title.is_empty()).then_some(title);
                }
            }
            TerminalEvent::ShellIntegration { id, event } => {
                self.apply_shell_integration_event(id, event);
            }
        }
    }

    fn apply_shell_integration_event(
        &mut self,
        id: crate::features::terminal::TerminalSessionId,
        event: crate::features::terminal::osc::NaiteOscEvent,
    ) {
        use crate::features::terminal::osc::NaiteOscEvent;
        if self.terminal.session(id).is_none() {
            return;
        }
        let recompute = match event {
            NaiteOscEvent::Ready => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.integration_status = crate::state::IntegrationStatus::Ready;
                }
                false
            }
            NaiteOscEvent::Cwd(path) => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.shell_cwd = Some(path);
                }
                false
            }
            NaiteOscEvent::Input { buffer, cursor } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.input_buffer = buffer;
                    session.input_cursor = cursor;
                }
                true
            }
            NaiteOscEvent::CommandStart { command } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.last_command = Some(command);
                    session.active_suggestion = None;
                }
                false
            }
            NaiteOscEvent::CommandFinish { exit_code } => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.last_exit_code = Some(exit_code);
                    session.active_suggestion = None;
                }
                false
            }
            NaiteOscEvent::History(entries) => {
                if let Some(session) = self.terminal.session_mut(id) {
                    session.push_history_capped(entries);
                }
                false
            }
        };
        if recompute {
            self.recompute_terminal_suggestion(id);
        }
    }

    fn recompute_terminal_suggestion(&mut self, id: crate::features::terminal::TerminalSessionId) {
        let Some(session) = self.terminal.session(id) else {
            return;
        };
        let cwd: std::path::PathBuf = session
            .shell_cwd
            .clone()
            .unwrap_or_else(|| session.target.cwd.clone());
        let buffer = session.input_buffer.clone();
        let cursor = session.input_cursor;
        let zsh_history = session.shell_history.clone();

        let suggestion = crate::features::terminal::suggestion::suggest(
            crate::features::terminal::suggestion::SuggestionInputs {
                buffer: &buffer,
                cursor,
                zsh_history: &zsh_history,
                session_history: &[],
                cwd: &cwd,
            },
        );

        if let Some(session) = self.terminal.session_mut(id) {
            session.active_suggestion = suggestion;
        }
    }

    fn send_terminal_command(&mut self, command: TerminalCommand) {
        if let Err(message) = terminal::runtime::send(command) {
            self.operation.error = Some(message);
        }
    }

    fn new_terminal_session_target(&self) -> Option<(TerminalTarget, String)> {
        if let Some(session) = self.terminal.active_session() {
            let cwd = session
                .shell_cwd
                .clone()
                .unwrap_or_else(|| session.target.cwd.clone());
            let target = TerminalTarget::new(
                cwd,
                session
                    .target
                    .repo_tab
                    .clone()
                    .or_else(|| self.repo.path.clone()),
                session.target.worktree_hint.clone(),
            );
            return Some((target, session.label.clone()));
        }

        self.repo.path.clone().map(|path| {
            let label = self
                .repo
                .head_branch
                .clone()
                .unwrap_or_else(|| "Current repo".into());
            (
                TerminalTarget::new(path, self.repo.path.clone(), None),
                label,
            )
        })
    }

    fn terminal_dimensions(&self) -> (u16, u16) {
        // Panel spans only the commit-list pane (between sidebar and detail),
        // so columns scale with that pane while rows stay anchored to the
        // fixed panel envelope. Character metrics are shared with selection
        // hit-testing so rendered rows and mouse coordinates stay aligned.
        let sidebar_ratio = self.preferences.sidebar_ratio.clamp(0.14, 0.36);
        let detail_ratio = self.preferences.detail_ratio.clamp(0.50, 0.78);
        let panel_width = self.window_width * (1.0 - sidebar_ratio) * detail_ratio;
        let body_height = (crate::widgets::TERMINAL_PANEL_HEIGHT
            - crate::widgets::TERMINAL_PANEL_CHROME)
            .max(60.0);
        let cols =
            ((panel_width - 64.0) / crate::widgets::TERMINAL_CHAR_WIDTH).clamp(40.0, 240.0) as u16;
        let rows = (body_height / crate::widgets::TERMINAL_LINE_HEIGHT).clamp(6.0, 40.0) as u16;
        (cols, rows)
    }
}

fn bracketed_paste(text: String) -> String {
    format!("\x1b[200~{text}\x1b[201~")
}

fn terminal_grid_point_at(
    session: &crate::state::TerminalSession,
    point: iced::Point,
) -> TerminalGridPoint {
    let last_row = session.screen.lines.len().saturating_sub(1);
    let row = (point.y / crate::widgets::TERMINAL_LINE_HEIGHT)
        .floor()
        .max(0.0) as usize;
    let row = row.min(last_row);
    let line_len = session
        .screen
        .lines
        .get(row)
        .map(|line| line.text().chars().count())
        .unwrap_or_default();
    let col = (point.x / crate::widgets::TERMINAL_CHAR_WIDTH)
        .floor()
        .max(0.0) as usize;
    TerminalGridPoint {
        row,
        col: col.min(line_len),
    }
}
