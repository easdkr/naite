use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Mutex, OnceLock};

use alacritty_terminal::event::{Event as AlacrittyEvent, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi;
use iced::futures::SinkExt;
use iced::{stream, Subscription};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;

use crate::features::terminal::message::IntegrationConfig;
use crate::features::terminal::{
    Message, TerminalCommand, TerminalEvent, TerminalSessionId, TerminalTarget,
};
use crate::state::{TerminalCell, TerminalCursor, TerminalLine, TerminalScreen};

static COMMAND_TX: OnceLock<Mutex<Option<mpsc::UnboundedSender<TerminalCommand>>>> =
    OnceLock::new();

pub(crate) fn subscription() -> Subscription<Message> {
    Subscription::run(worker_stream)
}

pub(crate) fn send(command: TerminalCommand) -> Result<(), String> {
    let Some(sender) = COMMAND_TX
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "terminal runtime lock is poisoned".to_string())?
        .clone()
    else {
        return Err("terminal runtime is not ready yet".into());
    };
    sender
        .send(command)
        .map_err(|_| "terminal runtime is no longer receiving commands".into())
}

fn worker_stream() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(100, |mut output| async move {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        let (internal_tx, mut internal_rx) = mpsc::unbounded_channel();

        if let Ok(mut slot) = COMMAND_TX.get_or_init(|| Mutex::new(None)).lock() {
            *slot = Some(command_tx);
        }
        let _ = output.send(Message::RuntimeReady).await;

        let mut sessions: HashMap<TerminalSessionId, RuntimeSession> = HashMap::new();

        loop {
            tokio::select! {
                Some(command) = command_rx.recv() => {
                    handle_command(command, &internal_tx, &mut sessions, &mut output).await;
                }
                Some(event) = internal_rx.recv() => {
                    handle_internal(event, &mut sessions, &mut output).await;
                }
                else => break,
            }
        }
    })
}

async fn handle_command(
    command: TerminalCommand,
    internal_tx: &mpsc::UnboundedSender<RuntimeInternal>,
    sessions: &mut HashMap<TerminalSessionId, RuntimeSession>,
    output: &mut iced::futures::channel::mpsc::Sender<Message>,
) {
    match command {
        TerminalCommand::Create {
            id,
            target,
            shell,
            cols,
            rows,
            integration,
        } => match RuntimeSession::spawn(id, target, shell, cols, rows, integration, internal_tx) {
            Ok(session) => {
                sessions.insert(id, session);
                let _ = output
                    .send(Message::RuntimeEvent(TerminalEvent::Started { id }))
                    .await;
            }
            Err(message) => {
                let _ = output
                    .send(Message::RuntimeEvent(TerminalEvent::Error { id, message }))
                    .await;
            }
        },
        TerminalCommand::Input { id, bytes } => {
            if let Some(session) = sessions.get_mut(&id) {
                if let Err(err) = session
                    .writer
                    .write_all(&bytes)
                    .and_then(|_| session.writer.flush())
                {
                    let _ = output
                        .send(Message::RuntimeEvent(TerminalEvent::Error {
                            id,
                            message: format!("failed to write terminal input: {err}"),
                        }))
                        .await;
                }
            }
        }
        TerminalCommand::Resize { id, cols, rows } => {
            if let Some(session) = sessions.get_mut(&id) {
                session.resize(cols, rows, output).await;
            }
        }
        TerminalCommand::Interrupt { id } => {
            if let Some(session) = sessions.get_mut(&id) {
                let _ = session
                    .writer
                    .write_all(&[0x03])
                    .and_then(|_| session.writer.flush());
            }
        }
        TerminalCommand::Kill { id } => {
            if let Some(session) = sessions.get_mut(&id) {
                if let Err(err) = session.killer.kill() {
                    let _ = output
                        .send(Message::RuntimeEvent(TerminalEvent::Error {
                            id,
                            message: format!("failed to kill terminal session: {err}"),
                        }))
                        .await;
                }
            }
        }
        TerminalCommand::Close { id } => {
            if let Some(mut session) = sessions.remove(&id) {
                let _ = session.killer.kill();
            }
        }
    }
}

async fn handle_internal(
    event: RuntimeInternal,
    sessions: &mut HashMap<TerminalSessionId, RuntimeSession>,
    output: &mut iced::futures::channel::mpsc::Sender<Message>,
) {
    match event {
        RuntimeInternal::Bytes { id, bytes } => {
            let Some(session) = sessions.get_mut(&id) else {
                return;
            };
            session.parser.advance(&mut session.term, &bytes);
            session
                .naite_parser
                .advance(&mut session.naite_sink, &bytes);
            while let Ok(event) = session.naite_rx.try_recv() {
                let _ = output
                    .send(Message::RuntimeEvent(TerminalEvent::ShellIntegration {
                        id,
                        event,
                    }))
                    .await;
            }
            let screen = session.snapshot();
            let _ = output
                .send(Message::RuntimeEvent(TerminalEvent::ScreenUpdated {
                    id,
                    screen,
                }))
                .await;
        }
        RuntimeInternal::Exited { id, status } => {
            sessions.remove(&id);
            let _ = output
                .send(Message::RuntimeEvent(TerminalEvent::Exited { id, status }))
                .await;
        }
        RuntimeInternal::ReadError { id, message } => {
            if sessions.contains_key(&id) {
                let _ = output
                    .send(Message::RuntimeEvent(TerminalEvent::Error { id, message }))
                    .await;
            }
        }
        RuntimeInternal::Emulator { id, event } => {
            handle_emulator_event(id, event, sessions, output).await;
        }
    }
}

async fn handle_emulator_event(
    id: TerminalSessionId,
    event: AlacrittyEvent,
    sessions: &mut HashMap<TerminalSessionId, RuntimeSession>,
    output: &mut iced::futures::channel::mpsc::Sender<Message>,
) {
    match event {
        AlacrittyEvent::Title(title) => {
            let _ = output
                .send(Message::RuntimeEvent(TerminalEvent::TitleChanged {
                    id,
                    title,
                }))
                .await;
        }
        AlacrittyEvent::ResetTitle => {
            let _ = output
                .send(Message::RuntimeEvent(TerminalEvent::TitleChanged {
                    id,
                    title: String::new(),
                }))
                .await;
        }
        AlacrittyEvent::Bell => {
            let _ = output
                .send(Message::RuntimeEvent(TerminalEvent::Bell { id }))
                .await;
        }
        AlacrittyEvent::PtyWrite(text) => {
            if let Some(session) = sessions.get_mut(&id) {
                let _ = session
                    .writer
                    .write_all(text.as_bytes())
                    .and_then(|_| session.writer.flush());
            }
        }
        AlacrittyEvent::Exit => {
            if let Some(session) = sessions.get_mut(&id) {
                let _ = session.killer.kill();
            }
        }
        _ => {}
    }
}

struct RuntimeSession {
    id: TerminalSessionId,
    term: Term<TerminalEventProxy>,
    parser: ansi::Processor,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    cols: u16,
    rows: u16,
    naite_parser: alacritty_terminal::vte::Parser,
    naite_sink: crate::features::terminal::osc::NaiteOscSink,
    naite_rx: tokio::sync::mpsc::UnboundedReceiver<crate::features::terminal::osc::NaiteOscEvent>,
    _integration: Option<crate::features::terminal::zsh_integration::IntegrationLaunch>,
}

impl RuntimeSession {
    fn spawn(
        id: TerminalSessionId,
        target: TerminalTarget,
        shell: String,
        cols: u16,
        rows: u16,
        integration: IntegrationConfig,
        internal_tx: &mpsc::UnboundedSender<RuntimeInternal>,
    ) -> Result<Self, String> {
        let integration_launch = if matches!(integration, IntegrationConfig::Zsh) {
            crate::features::terminal::zsh_integration::prepare_zsh_integration().ok()
        } else {
            None
        };

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(pty_size(cols, rows))
            .map_err(|err| format!("failed to open PTY: {err}"))?;
        let mut command = CommandBuilder::new(&shell);
        command.cwd(target.cwd.as_os_str());
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");

        if let Some(launch) = &integration_launch {
            command.env("ZDOTDIR", launch.zdotdir.as_os_str());
            if let Ok(existing) = std::env::var("ZDOTDIR") {
                command.env("NAITE_USER_ZDOTDIR", existing);
            } else if let Ok(home) = std::env::var("HOME") {
                command.env("NAITE_USER_ZDOTDIR", home);
            }
            command.env(
                "NAITE_INTEGRATION_SCRIPT",
                launch.zdotdir.join("naite-integration.zsh").as_os_str(),
            );
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|err| format!("failed to start shell {shell}: {err}"))?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| format!("failed to clone PTY reader: {err}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("failed to open PTY writer: {err}"))?;
        let killer = child.clone_killer();

        spawn_reader(id, reader, internal_tx.clone());
        spawn_waiter(id, child, internal_tx.clone());

        let size = TerminalSize { cols, rows };
        let proxy = TerminalEventProxy {
            id,
            tx: internal_tx.clone(),
        };
        let term = Term::new(
            Config {
                scrolling_history: 10_000,
                ..Default::default()
            },
            &size,
            proxy,
        );

        let (naite_tx, naite_rx) = tokio::sync::mpsc::unbounded_channel();
        let naite_sink = crate::features::terminal::osc::NaiteOscSink::new(naite_tx);
        let naite_parser = alacritty_terminal::vte::Parser::new();

        Ok(Self {
            id,
            term,
            parser: ansi::Processor::new(),
            master: pair.master,
            writer,
            killer,
            cols,
            rows,
            naite_parser,
            naite_sink,
            naite_rx,
            _integration: integration_launch,
        })
    }

    async fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        output: &mut iced::futures::channel::mpsc::Sender<Message>,
    ) {
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        let size = pty_size(cols, rows);
        if let Err(err) = self.master.resize(size) {
            let _ = output
                .send(Message::RuntimeEvent(TerminalEvent::Error {
                    id: self.id,
                    message: format!("failed to resize PTY: {err}"),
                }))
                .await;
            return;
        }
        self.term.resize(TerminalSize { cols, rows });
        let screen = self.snapshot();
        let _ = output
            .send(Message::RuntimeEvent(TerminalEvent::ScreenUpdated {
                id: self.id,
                screen,
            }))
            .await;
    }

    fn snapshot(&self) -> TerminalScreen {
        let mut lines = Vec::<TerminalLine>::new();
        let mut current_line: Option<Line> = None;

        // Iterate over scrollback history + visible screen so the iced viewport
        // has real overflow to scroll. `display_iter` only walks the visible
        // rows (sized to the panel), which leaves the scrollable with no
        // overflow and therefore no way to surface earlier output.
        let grid = self.term.grid();
        let topmost = grid.topmost_line();
        let last_column = grid.last_column();
        let history_size = grid.history_size();
        let start = Point::new(Line(topmost.0 - 1), last_column);

        for indexed in grid.iter_from(start) {
            if current_line != Some(indexed.point.line) {
                current_line = Some(indexed.point.line);
                lines.push(TerminalLine::default());
            }
            let flags = indexed.cell.flags;
            let spacer =
                flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER);
            lines
                .last_mut()
                .expect("line exists after current line switch")
                .cells
                .push(TerminalCell {
                    ch: indexed.cell.c,
                    bold: flags.intersects(Flags::BOLD | Flags::DIM_BOLD),
                    italic: flags.contains(Flags::ITALIC),
                    underline: flags.intersects(Flags::ALL_UNDERLINES),
                    inverse: flags.contains(Flags::INVERSE),
                    hidden: flags.contains(Flags::HIDDEN),
                    spacer,
                });
        }

        let cursor_point = self.term.grid().cursor.point;
        let cursor_row_idx = if cursor_point.line.0 >= 0 {
            Some(history_size + cursor_point.line.0 as usize)
        } else {
            None
        };

        // Trim trailing blank rows below the cursor so the bottom-anchored
        // scrollable hugs the prompt instead of stacking the prompt above a
        // tail of dead empty rows from the visible screen area.
        let keep_through = cursor_row_idx.map(|row| row + 1).unwrap_or(0);
        while lines.len() > keep_through
            && lines
                .last()
                .map(|line| line.text().is_empty())
                .unwrap_or(false)
        {
            lines.pop();
        }

        // If the only surviving row is an empty cursor row at the very top
        // (i.e. the shell has not produced any visible output yet), let the
        // widget fall back to the status label rather than render a hollow
        // panel.
        let only_cursor_blank = history_size == 0
            && cursor_row_idx == Some(0)
            && lines.len() == 1
            && lines[0].text().is_empty();
        if only_cursor_blank {
            lines.clear();
        }

        let cursor = cursor_row_idx
            .filter(|_| !lines.is_empty())
            .map(|row| TerminalCursor {
                row,
                col: cursor_point.column.0,
            });

        TerminalScreen {
            cols: self.cols,
            rows: self.rows,
            lines,
            cursor,
            scrollback_len: self.term.grid().display_offset(),
        }
    }
}

fn spawn_reader(
    id: TerminalSessionId,
    mut reader: Box<dyn Read + Send>,
    tx: mpsc::UnboundedSender<RuntimeInternal>,
) {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if tx
                        .send(RuntimeInternal::Bytes {
                            id,
                            bytes: buffer[..read].to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx.send(RuntimeInternal::ReadError {
                        id,
                        message: format!("terminal read failed: {err}"),
                    });
                    break;
                }
            }
        }
    });
}

fn spawn_waiter(
    id: TerminalSessionId,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    tx: mpsc::UnboundedSender<RuntimeInternal>,
) {
    std::thread::spawn(move || {
        let status = child.wait().ok().map(|status| status.exit_code() as i32);
        let _ = tx.send(RuntimeInternal::Exited { id, status });
    });
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[derive(Clone)]
struct TerminalEventProxy {
    id: TerminalSessionId,
    tx: mpsc::UnboundedSender<RuntimeInternal>,
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: AlacrittyEvent) {
        let _ = self
            .tx
            .send(RuntimeInternal::Emulator { id: self.id, event });
    }
}

enum RuntimeInternal {
    Bytes {
        id: TerminalSessionId,
        bytes: Vec<u8>,
    },
    Exited {
        id: TerminalSessionId,
        status: Option<i32>,
    },
    ReadError {
        id: TerminalSessionId,
        message: String,
    },
    Emulator {
        id: TerminalSessionId,
        event: AlacrittyEvent,
    },
}

struct TerminalSize {
    cols: u16,
    rows: u16,
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}
