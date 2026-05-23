use std::path::PathBuf;

use iced::Point;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TerminalSessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationConfig {
    Disabled,
    Zsh,
}

impl std::fmt::Display for TerminalSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "terminal-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTarget {
    pub cwd: PathBuf,
    pub repo_tab: Option<PathBuf>,
    pub worktree_hint: Option<String>,
}

impl TerminalTarget {
    pub fn new(cwd: PathBuf, repo_tab: Option<PathBuf>, worktree_hint: Option<String>) -> Self {
        Self {
            cwd,
            repo_tab,
            worktree_hint,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSelection {
    Existing(TerminalSessionId),
    Target {
        cwd: PathBuf,
        label: String,
        worktree_hint: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenRequested,
    SessionSelected(SessionSelection),
    NewSessionRequested,
    StartRequested,
    RestartRequested,
    InterruptRequested,
    KillRequested,
    CloseRequested,
    CloseSession(TerminalSessionId),
    ToggleMinimized,
    PointerMoved(Point),
    SelectionStarted,
    SelectionEnded,
    CopySelectionRequested,
    PasteRequested,
    Input(TerminalInput),
    RuntimeReady,
    RuntimeEvent(TerminalEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInput {
    Bytes(Vec<u8>),
    Paste(String),
    MaybeAcceptSuggestion { fallback: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalCommand {
    Create {
        id: TerminalSessionId,
        target: TerminalTarget,
        shell: String,
        cols: u16,
        rows: u16,
        integration: IntegrationConfig,
    },
    Input {
        id: TerminalSessionId,
        bytes: Vec<u8>,
    },
    Resize {
        id: TerminalSessionId,
        cols: u16,
        rows: u16,
    },
    Interrupt {
        id: TerminalSessionId,
    },
    Kill {
        id: TerminalSessionId,
    },
    Close {
        id: TerminalSessionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Started {
        id: TerminalSessionId,
    },
    ScreenUpdated {
        id: TerminalSessionId,
        screen: crate::state::TerminalScreen,
    },
    Exited {
        id: TerminalSessionId,
        status: Option<i32>,
    },
    Error {
        id: TerminalSessionId,
        message: String,
    },
    Bell {
        id: TerminalSessionId,
    },
    TitleChanged {
        id: TerminalSessionId,
        title: String,
    },
    ShellIntegration {
        id: TerminalSessionId,
        event: crate::features::terminal::osc::NaiteOscEvent,
    },
}
