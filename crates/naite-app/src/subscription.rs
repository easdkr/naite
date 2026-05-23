use std::time::Duration;

use iced::keyboard::{
    self,
    key::{Code, Key, Named, Physical},
    Modifiers,
};
use iced::{event, time, window, Event, Subscription};
use naite_core::RebaseAction;

use crate::features::rebase;
use crate::features::terminal;
use crate::message::{KeyAction, Message};
use crate::state::{ReleasePrepPhase, RELEASE_PREP_MODAL_ANIMATION_FRAMES};
use crate::App;

const TRANSIENT_STATUS_TICK: Duration = Duration::from_millis(250);
const RELEASE_PREP_TICK: Duration = Duration::from_millis(80);
const AUTO_FETCH_INTERVAL: Duration = Duration::from_secs(60);

impl App {
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        let event_subscription = if self.rebase.is_some() {
            event::listen_with(rebase_app_event)
        } else if self.terminal.captures_keyboard() {
            event::listen_with(terminal_app_event)
        } else {
            event::listen_with(app_event)
        };
        let events = Subscription::batch([
            event_subscription,
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
        ]);
        let mut subscriptions = vec![events];
        subscriptions.push(terminal::runtime::subscription().map(Message::from));
        if self.operation.transient_status.is_some() {
            subscriptions
                .push(time::every(TRANSIENT_STATUS_TICK).map(|_| Message::TransientStatusTick));
        }
        let release_prep_loading = matches!(
            self.release_prep.phase,
            ReleasePrepPhase::Preparing | ReleasePrepPhase::RunningAction
        );
        let release_prep_entering = self.release_prep.phase != ReleasePrepPhase::Idle
            && self.release_prep.animation_frame < RELEASE_PREP_MODAL_ANIMATION_FRAMES;
        if release_prep_loading || release_prep_entering {
            subscriptions.push(time::every(RELEASE_PREP_TICK).map(|_| Message::ReleasePrepTick));
        }
        if self.repo.path.is_some() && self.repo.sync_status.upstream.is_some() {
            subscriptions.push(time::every(AUTO_FETCH_INTERVAL).map(|_| Message::AutoFetchTick));
        }

        Subscription::batch(subscriptions)
    }
}

pub(crate) fn terminal_app_event(
    event: Event,
    status: event::Status,
    _window: window::Id,
) -> Option<Message> {
    match event {
        Event::Ime(ime) => Some(Message::from(terminal_ime_event(ime))),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key,
            modifiers,
            text,
            ..
        }) => {
            if let Some(message) = terminal_global_shortcut(&key, physical_key, modifiers) {
                return Some(message);
            }
            if matches!(status, event::Status::Captured) {
                if modifiers.command() {
                    if let Some(message) = terminal_key_input(
                        key.clone(),
                        modified_key,
                        physical_key,
                        modifiers,
                        text.as_deref(),
                    ) {
                        return Some(Message::from(message));
                    }
                }
                return keyboard_shortcut(key, physical_key, modifiers, status);
            }
            terminal_key_input(key, modified_key, physical_key, modifiers, text.as_deref())
                .map(Message::from)
        }
        Event::Keyboard(keyboard::Event::KeyReleased { key, modifiers, .. }) => {
            Some(Message::from(terminal::Message::KeyReleased {
                key,
                modifiers,
            }))
        }
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => Some(Message::from(
            terminal::Message::ModifiersChanged(modifiers),
        )),
        Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(position))
        }
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        _ => None,
    }
}

fn terminal_ime_event(ime: iced_core::event::Ime) -> terminal::Message {
    terminal::Message::Ime(match ime {
        iced_core::event::Ime::Enabled => terminal::TerminalIme::Enabled,
        iced_core::event::Ime::Preedit(text, cursor) => {
            terminal::TerminalIme::Preedit { text, cursor }
        }
        iced_core::event::Ime::Commit(text) => terminal::TerminalIme::Commit(text),
        iced_core::event::Ime::Disabled => terminal::TerminalIme::Disabled,
    })
}

fn rebase_app_event(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        }) => rebase_keyboard_shortcut(key, physical_key, modifiers, status),
        Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(position))
        }
        Event::Mouse(iced::mouse::Event::ButtonReleased(_)) => {
            Some(Message::from(rebase::Message::DragEnded))
        }
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        _ => None,
    }
}

pub(crate) fn app_event(
    event: Event,
    status: event::Status,
    _window: window::Id,
) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        }) => keyboard_shortcut(key, physical_key, modifiers, status),
        Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(position))
        }
        Event::Window(window::Event::Focused) => Some(Message::WindowFocused),
        _ => None,
    }
}

pub(crate) fn keyboard_shortcut(
    key: Key,
    physical_key: Physical,
    modifiers: Modifiers,
    status: event::Status,
) -> Option<Message> {
    let command = modifiers.command() || modifiers.control();
    let captured = matches!(status, event::Status::Captured);

    if is_key(&key, physical_key, Code::KeyK, "k") && command {
        return Some(Message::Keyboard(KeyAction::OpenCommandPalette));
    }
    if terminal_shortcut(&key, physical_key, modifiers) {
        return Some(Message::Keyboard(KeyAction::OpenTerminal));
    }
    if matches!(key.as_ref(), Key::Character(value) if value == "?") {
        return Some(Message::Keyboard(KeyAction::ToggleShortcutOverlay));
    }
    if is_key(&key, physical_key, Code::KeyP, "p") && command && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::Push));
    }
    if is_key(&key, physical_key, Code::KeyR, "r") && command && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::ReleasePromotion));
    }
    if is_key(&key, physical_key, Code::KeyT, "t") && command && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::CreateAndPushTag));
    }

    if captured {
        return match key.as_ref() {
            Key::Named(Named::Escape) => Some(Message::Keyboard(KeyAction::Escape)),
            Key::Named(Named::Enter) => Some(Message::Keyboard(KeyAction::CommandPaletteRun)),
            Key::Named(Named::ArrowDown) => Some(Message::Keyboard(KeyAction::CommandPaletteNext)),
            Key::Named(Named::ArrowUp) => {
                Some(Message::Keyboard(KeyAction::CommandPalettePrevious))
            }
            _ => None,
        };
    }

    match key.as_ref() {
        _ if is_key(&key, physical_key, Code::KeyR, "r") && !command => {
            Some(Message::Keyboard(KeyAction::RewordSelectedCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyS, "s") && !command => {
            Some(Message::Keyboard(KeyAction::SquashSelectedCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyF, "f") && !command => {
            Some(Message::Keyboard(KeyAction::FixupSelectedCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyE, "e") && !command => {
            Some(Message::Keyboard(KeyAction::EditSelectedCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyD, "d") && !command => {
            Some(Message::Keyboard(KeyAction::DropSelectedCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyT, "t") && !command => {
            Some(Message::Keyboard(KeyAction::TagSelectedCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyY, "y") && !command => {
            Some(Message::Keyboard(KeyAction::CopySelectedCommitHash))
        }
        _ if is_key(&key, physical_key, Code::KeyJ, "j") && !command => {
            Some(Message::Keyboard(KeyAction::NextCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyK, "k") && !command => {
            Some(Message::Keyboard(KeyAction::PreviousCommit))
        }
        _ if is_key(&key, physical_key, Code::KeyO, "o") && command => {
            Some(Message::Keyboard(KeyAction::OpenRepository))
        }
        _ if is_key(&key, physical_key, Code::KeyF, "f") && command => {
            Some(Message::Keyboard(KeyAction::FocusSearch))
        }
        _ if is_key(&key, physical_key, Code::BracketRight, "]") && !command => {
            Some(Message::Keyboard(KeyAction::NextHunk))
        }
        _ if is_key(&key, physical_key, Code::BracketLeft, "[") && !command => {
            Some(Message::Keyboard(KeyAction::PreviousHunk))
        }
        Key::Named(Named::ArrowDown) => Some(Message::Keyboard(KeyAction::NextCommit)),
        Key::Named(Named::ArrowUp) => Some(Message::Keyboard(KeyAction::PreviousCommit)),
        Key::Named(Named::Enter) => Some(Message::Keyboard(KeyAction::Enter)),
        Key::Named(Named::Escape) => Some(Message::Keyboard(KeyAction::Escape)),
        _ => None,
    }
}

fn rebase_keyboard_shortcut(
    key: Key,
    physical_key: Physical,
    modifiers: Modifiers,
    status: event::Status,
) -> Option<Message> {
    let command = modifiers.command() || modifiers.control();
    let captured = matches!(status, event::Status::Captured);

    if is_key(&key, physical_key, Code::KeyK, "k") && command {
        return Some(Message::Keyboard(KeyAction::OpenCommandPalette));
    }
    if terminal_shortcut(&key, physical_key, modifiers) {
        return Some(Message::Keyboard(KeyAction::OpenTerminal));
    }
    if matches!(key.as_ref(), Key::Character(value) if value == "?") {
        return Some(Message::Keyboard(KeyAction::ToggleShortcutOverlay));
    }
    if is_key(&key, physical_key, Code::KeyR, "r") && command && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::ReleasePromotion));
    }
    if is_key(&key, physical_key, Code::KeyT, "t") && command && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::CreateAndPushTag));
    }

    if captured {
        return match key.as_ref() {
            Key::Named(Named::Escape) => Some(Message::from(rebase::Message::EscapePressed)),
            _ => None,
        };
    }

    match key.as_ref() {
        _ if is_key(&key, physical_key, Code::KeyP, "p") && !command => Some(Message::from(
            rebase::Message::ActionSetSelected(RebaseAction::Pick),
        )),
        _ if is_key(&key, physical_key, Code::KeyR, "r") && !command => Some(Message::from(
            rebase::Message::ActionSetSelected(RebaseAction::Reword),
        )),
        _ if is_key(&key, physical_key, Code::KeyD, "d") && !command => Some(Message::from(
            rebase::Message::ActionSetSelected(RebaseAction::Drop),
        )),
        _ if is_key(&key, physical_key, Code::KeyS, "s") && !command => Some(Message::from(
            rebase::Message::ActionSetSelected(RebaseAction::Squash),
        )),
        _ if is_key(&key, physical_key, Code::KeyF, "f") && !command => Some(Message::from(
            rebase::Message::ActionSetSelected(RebaseAction::Fixup),
        )),
        _ if is_key(&key, physical_key, Code::KeyE, "e") && !command => Some(Message::from(
            rebase::Message::ActionSetSelected(RebaseAction::Edit),
        )),
        _ if is_key(&key, physical_key, Code::KeyJ, "j") && modifiers.shift() => {
            Some(Message::from(rebase::Message::MoveSelected(1)))
        }
        _ if is_key(&key, physical_key, Code::KeyK, "k") && modifiers.shift() => {
            Some(Message::from(rebase::Message::MoveSelected(-1)))
        }
        _ if is_key(&key, physical_key, Code::KeyJ, "j") && !command => {
            Some(Message::from(rebase::Message::RowSelectedRelative(1)))
        }
        _ if is_key(&key, physical_key, Code::KeyK, "k") && !command => {
            Some(Message::from(rebase::Message::RowSelectedRelative(-1)))
        }
        Key::Named(Named::ArrowDown) if modifiers.alt() || command => {
            Some(Message::from(rebase::Message::MoveSelected(1)))
        }
        Key::Named(Named::ArrowUp) if modifiers.alt() || command => {
            Some(Message::from(rebase::Message::MoveSelected(-1)))
        }
        Key::Named(Named::ArrowDown) => {
            Some(Message::from(rebase::Message::RowSelectedRelative(1)))
        }
        Key::Named(Named::ArrowUp) => Some(Message::from(rebase::Message::RowSelectedRelative(-1))),
        Key::Named(Named::Enter) => Some(Message::from(rebase::Message::ApplyRequested(
            rebase::RebaseApplyMode::RebaseOnly,
        ))),
        Key::Named(Named::Escape) => Some(Message::from(rebase::Message::EscapePressed)),
        _ => None,
    }
}

fn is_key(key: &Key, physical_key: Physical, code: Code, character: &str) -> bool {
    physical_key == code
        || matches!(key.as_ref(), Key::Character(value) if value.eq_ignore_ascii_case(character))
}

fn terminal_shortcut(key: &Key, physical_key: Physical, modifiers: Modifiers) -> bool {
    (modifiers.command() || modifiers.control()) && is_key(key, physical_key, Code::Backquote, "`")
}

fn terminal_global_shortcut(
    key: &Key,
    physical_key: Physical,
    modifiers: Modifiers,
) -> Option<Message> {
    if terminal_shortcut(key, physical_key, modifiers) {
        return Some(Message::Keyboard(KeyAction::OpenTerminal));
    }

    if !modifiers.command() {
        return None;
    }

    if is_key(key, physical_key, Code::KeyK, "k") && !modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::OpenCommandPalette));
    }
    if is_key(key, physical_key, Code::KeyO, "o") && !modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::OpenRepository));
    }
    if is_key(key, physical_key, Code::KeyF, "f") && !modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::FocusSearch));
    }
    if is_key(key, physical_key, Code::KeyP, "p") && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::Push));
    }
    if is_key(key, physical_key, Code::KeyR, "r") && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::ReleasePromotion));
    }
    if is_key(key, physical_key, Code::KeyT, "t") && modifiers.shift() {
        return Some(Message::Keyboard(KeyAction::CreateAndPushTag));
    }

    None
}

fn terminal_key_input(
    key: Key,
    modified_key: Key,
    physical_key: Physical,
    modifiers: Modifiers,
    text: Option<&str>,
) -> Option<terminal::Message> {
    if modifiers.command()
        && !modifiers.alt()
        && !modifiers.shift()
        && is_key(&key, physical_key, Code::KeyC, "c")
    {
        return Some(terminal::Message::CopySelectionRequested);
    }
    if modifiers.command()
        && !modifiers.alt()
        && !modifiers.shift()
        && is_key(&key, physical_key, Code::KeyV, "v")
    {
        return Some(terminal::Message::PasteRequested);
    }

    // Suggestion-accept shortcuts. update.rs decides whether to consume the
    // active suggestion or fall through to the standard byte mapping.
    if !modifiers.command() {
        if matches!(key.as_ref(), Key::Named(Named::ArrowRight))
            && !modifiers.control()
            && !modifiers.alt()
            && !modifiers.shift()
        {
            return Some(terminal::Message::Input(
                terminal::TerminalInput::MaybeAcceptSuggestion {
                    fallback: b"\x1b[C".to_vec(),
                },
            ));
        }
        if modifiers.control()
            && !modifiers.alt()
            && !modifiers.shift()
            && is_key(&key, physical_key, Code::KeyF, "f")
        {
            return Some(terminal::Message::Input(
                terminal::TerminalInput::MaybeAcceptSuggestion {
                    fallback: vec![0x06],
                },
            ));
        }
    }

    if modifiers.command() {
        if let Some(bytes) = command_bytes(&key, physical_key) {
            return Some(terminal::Message::Input(terminal::TerminalInput::Bytes(
                bytes,
            )));
        }
        return compatibility_jamo_preedit(text, &modified_key, &key)
            .map(terminal::TerminalIme::FallbackPreedit)
            .map(terminal::Message::Ime);
    }

    let bytes = if modifiers.control() {
        control_bytes(&key, physical_key)?
    } else {
        match key.as_ref() {
            Key::Named(Named::Enter) => b"\r".to_vec(),
            Key::Named(Named::Tab) => b"\t".to_vec(),
            Key::Named(Named::Backspace) => vec![0x7f],
            Key::Named(Named::Escape) => vec![0x1b],
            Key::Named(Named::ArrowUp) => b"\x1b[A".to_vec(),
            Key::Named(Named::ArrowDown) => b"\x1b[B".to_vec(),
            Key::Named(Named::ArrowRight) => b"\x1b[C".to_vec(),
            Key::Named(Named::ArrowLeft) => b"\x1b[D".to_vec(),
            Key::Named(Named::Home) => b"\x1b[H".to_vec(),
            Key::Named(Named::End) => b"\x1b[F".to_vec(),
            Key::Named(Named::PageUp) => b"\x1b[5~".to_vec(),
            Key::Named(Named::PageDown) => b"\x1b[6~".to_vec(),
            Key::Named(Named::Delete) => b"\x1b[3~".to_vec(),
            _ => {
                if let Some(preedit) = compatibility_jamo_preedit(text, &modified_key, &key) {
                    return Some(terminal::Message::Ime(
                        terminal::TerminalIme::FallbackPreedit(preedit),
                    ));
                }
                let text = terminal_text_input(text, &modified_key, &key)?;
                if modifiers.alt() {
                    let mut bytes = Vec::with_capacity(text.len() + 1);
                    bytes.push(0x1b);
                    bytes.extend(naite_core::compose_hangul(&text).into_bytes());
                    return Some(terminal::Message::Input(terminal::TerminalInput::Bytes(
                        bytes,
                    )));
                }
                return Some(terminal::Message::Input(terminal::TerminalInput::Text(
                    text,
                )));
            }
        }
    };

    let bytes = if modifiers.alt() {
        let mut prefixed = Vec::with_capacity(bytes.len() + 1);
        prefixed.push(0x1b);
        prefixed.extend(bytes);
        prefixed
    } else {
        bytes
    };

    Some(terminal::Message::Input(terminal::TerminalInput::Bytes(
        bytes,
    )))
}

fn terminal_text_input(text: Option<&str>, modified_key: &Key, key: &Key) -> Option<String> {
    text.filter(|value| is_printable_text(value) && !is_hangul_compatibility_text(value))
        .map(str::to_string)
        .or_else(|| key_text(modified_key))
        .or_else(|| key_text(key))
}

fn compatibility_jamo_preedit(text: Option<&str>, modified_key: &Key, key: &Key) -> Option<String> {
    if let Some(value) = text {
        return is_hangul_compatibility_text(value).then(|| value.to_string());
    }

    if key_text(modified_key).is_some() {
        return None;
    }

    key_compatibility_jamo(key)
}

fn key_compatibility_jamo(key: &Key) -> Option<String> {
    match key.as_ref() {
        Key::Character(value) if is_hangul_compatibility_text(value) => Some(value.to_string()),
        _ => None,
    }
}

fn key_text(key: &Key) -> Option<String> {
    match key.as_ref() {
        Key::Character(value)
            if is_printable_text(value) && !is_hangul_compatibility_text(value) =>
        {
            Some(value.to_string())
        }
        _ => None,
    }
}

fn is_printable_text(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| !ch.is_control())
}

fn is_hangul_compatibility_text(value: &str) -> bool {
    value.chars().all(naite_core::is_hangul_compatibility_jamo)
}

fn control_bytes(key: &Key, physical_key: Physical) -> Option<Vec<u8>> {
    if let Some(byte) = physical_control_byte(physical_key) {
        return Some(vec![byte]);
    }

    match key.as_ref() {
        Key::Named(Named::Enter) => Some(b"\n".to_vec()),
        Key::Named(Named::Tab) => Some(b"\t".to_vec()),
        Key::Named(Named::Backspace) => Some(vec![0x08]),
        Key::Named(Named::Escape) => Some(vec![0x1b]),
        Key::Named(Named::ArrowUp) => Some(b"\x1b[1;5A".to_vec()),
        Key::Named(Named::ArrowDown) => Some(b"\x1b[1;5B".to_vec()),
        Key::Named(Named::ArrowRight) => Some(b"\x1b[1;5C".to_vec()),
        Key::Named(Named::ArrowLeft) => Some(b"\x1b[1;5D".to_vec()),
        Key::Character(value) => {
            let ch = value.chars().next()?.to_ascii_lowercase();
            if ch.is_ascii_lowercase() {
                Some(vec![(ch as u8) - b'a' + 1])
            } else if ch == '[' {
                Some(vec![0x1b])
            } else if ch == '\\' {
                Some(vec![0x1c])
            } else if ch == ']' {
                Some(vec![0x1d])
            } else if ch == '^' {
                Some(vec![0x1e])
            } else if ch == '_' {
                Some(vec![0x1f])
            } else {
                None
            }
        }
        _ => None,
    }
}

fn physical_control_byte(physical_key: Physical) -> Option<u8> {
    let Physical::Code(code) = physical_key else {
        return None;
    };
    let offset = match code {
        Code::KeyA => 0,
        Code::KeyB => 1,
        Code::KeyC => 2,
        Code::KeyD => 3,
        Code::KeyE => 4,
        Code::KeyF => 5,
        Code::KeyG => 6,
        Code::KeyH => 7,
        Code::KeyI => 8,
        Code::KeyJ => 9,
        Code::KeyK => 10,
        Code::KeyL => 11,
        Code::KeyM => 12,
        Code::KeyN => 13,
        Code::KeyO => 14,
        Code::KeyP => 15,
        Code::KeyQ => 16,
        Code::KeyR => 17,
        Code::KeyS => 18,
        Code::KeyT => 19,
        Code::KeyU => 20,
        Code::KeyV => 21,
        Code::KeyW => 22,
        Code::KeyX => 23,
        Code::KeyY => 24,
        Code::KeyZ => 25,
        _ => return None,
    };
    Some(offset + 1)
}

fn command_bytes(key: &Key, physical_key: Physical) -> Option<Vec<u8>> {
    match key.as_ref() {
        Key::Named(Named::Backspace) => Some(vec![0x15]),
        _ if physical_key == Physical::Code(Code::Backspace) => Some(vec![0x15]),
        _ => None,
    }
}
