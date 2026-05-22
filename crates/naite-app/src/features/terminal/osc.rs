//! naite-specific OSC sequence parsing.
//!
//! Shell integration scripts emit `OSC 777 ; naite ; <event> ; <field>... BEL`
//! to push state into the app without rendering to the terminal grid.

// Wave 2 wires this module into the runtime; until then the types look unused
// from the binary entrypoint. Tests exercise everything here.
#![allow(dead_code)]

use std::path::PathBuf;

use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NaiteOscEvent {
    Ready,
    Cwd(PathBuf),
    Input { buffer: String, cursor: usize },
    CommandStart { command: String },
    CommandFinish { exit_code: i32 },
    History(Vec<String>),
}

pub struct NaiteOscSink {
    tx: UnboundedSender<NaiteOscEvent>,
}

impl NaiteOscSink {
    pub fn new(tx: UnboundedSender<NaiteOscEvent>) -> Self {
        Self { tx }
    }
}

impl alacritty_terminal::vte::Perform for NaiteOscSink {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() < 2 {
            return;
        }
        if params[0] != b"777" || params[1] != b"naite" {
            return;
        }
        if params.len() < 3 {
            return;
        }

        let event_name = params[2];
        let fields = &params[3..];

        let event = match event_name {
            b"ready" => {
                if !fields.is_empty() {
                    return;
                }
                NaiteOscEvent::Ready
            }
            b"cwd" => {
                if fields.len() != 1 {
                    return;
                }
                let decoded = pct_decode(fields[0]);
                let Ok(s) = std::str::from_utf8(&decoded) else {
                    return;
                };
                NaiteOscEvent::Cwd(PathBuf::from(s))
            }
            b"input" => {
                if fields.len() != 2 {
                    return;
                }
                let buf_decoded = pct_decode(fields[0]);
                let Ok(buffer) = std::str::from_utf8(&buf_decoded) else {
                    return;
                };
                let cur_decoded = pct_decode(fields[1]);
                let Ok(cur_str) = std::str::from_utf8(&cur_decoded) else {
                    return;
                };
                let Ok(cursor) = cur_str.parse::<usize>() else {
                    return;
                };
                NaiteOscEvent::Input {
                    buffer: buffer.to_owned(),
                    cursor,
                }
            }
            b"command_start" => {
                if fields.len() != 1 {
                    return;
                }
                let decoded = pct_decode(fields[0]);
                let Ok(s) = std::str::from_utf8(&decoded) else {
                    return;
                };
                NaiteOscEvent::CommandStart {
                    command: s.to_owned(),
                }
            }
            b"command_finish" => {
                if fields.len() != 1 {
                    return;
                }
                let decoded = pct_decode(fields[0]);
                let Ok(s) = std::str::from_utf8(&decoded) else {
                    return;
                };
                let Ok(exit_code) = s.parse::<i32>() else {
                    return;
                };
                NaiteOscEvent::CommandFinish { exit_code }
            }
            b"history" => {
                let mut entries = Vec::with_capacity(fields.len());
                for field in fields {
                    let decoded = pct_decode(field);
                    let Ok(s) = std::str::from_utf8(&decoded) else {
                        return;
                    };
                    entries.push(s.to_owned());
                }
                NaiteOscEvent::History(entries)
            }
            _ => return,
        };

        // Ignore send errors — receiver may have dropped.
        let _ = self.tx.send(event);
    }
}

fn pct_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            let hi = hex_val(input[i + 1]);
            let lo = hex_val(input[i + 2]);
            match (hi, lo) {
                (Some(h), Some(l)) => {
                    out.push((h << 4) | l);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            }
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::Parser;
    use tokio::sync::mpsc;

    fn parse_bytes(bytes: &[u8]) -> Vec<NaiteOscEvent> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, bytes);
        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        events
    }

    #[test]
    fn ready() {
        let events = parse_bytes(b"\x1b]777;naite;ready\x07");
        assert_eq!(events, vec![NaiteOscEvent::Ready]);
    }

    #[test]
    fn cwd_plain() {
        let events = parse_bytes(b"\x1b]777;naite;cwd;/tmp/foo\x07");
        assert_eq!(events, vec![NaiteOscEvent::Cwd(PathBuf::from("/tmp/foo"))]);
    }

    #[test]
    fn cwd_pct_encoded() {
        let events = parse_bytes(b"\x1b]777;naite;cwd;%2Ftmp%2Ffoo%20bar\x07");
        assert_eq!(
            events,
            vec![NaiteOscEvent::Cwd(PathBuf::from("/tmp/foo bar"))]
        );
    }

    #[test]
    fn input() {
        let events = parse_bytes(b"\x1b]777;naite;input;hello%20world;5\x07");
        assert_eq!(
            events,
            vec![NaiteOscEvent::Input {
                buffer: "hello world".to_owned(),
                cursor: 5,
            }]
        );
    }

    #[test]
    fn command_start_with_special_chars() {
        let events =
            parse_bytes(b"\x1b]777;naite;command_start;git%20commit%20-m%20%22hi%3Bthere%22\x07");
        assert_eq!(
            events,
            vec![NaiteOscEvent::CommandStart {
                command: r#"git commit -m "hi;there""#.to_owned(),
            }]
        );
    }

    #[test]
    fn command_finish_negative() {
        let events = parse_bytes(b"\x1b]777;naite;command_finish;-1\x07");
        assert_eq!(events, vec![NaiteOscEvent::CommandFinish { exit_code: -1 }]);
    }

    #[test]
    fn history_multiple() {
        let events = parse_bytes(b"\x1b]777;naite;history;one;two;three\x07");
        assert_eq!(
            events,
            vec![NaiteOscEvent::History(vec![
                "one".to_owned(),
                "two".to_owned(),
                "three".to_owned(),
            ])]
        );
    }

    #[test]
    fn history_empty() {
        let events = parse_bytes(b"\x1b]777;naite;history\x07");
        assert_eq!(events, vec![NaiteOscEvent::History(vec![])]);
    }

    #[test]
    fn split_chunks() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, b"\x1b]777;naite;ready");
        parser.advance(&mut sink, b"\x07");
        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        assert_eq!(events, vec![NaiteOscEvent::Ready]);
    }

    #[test]
    fn non_naite_osc_ignored() {
        let (tx, mut rx) = mpsc::unbounded_channel::<NaiteOscEvent>();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, b"\x1b]0;some title\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn different_vendor_ignored() {
        let (tx, mut rx) = mpsc::unbounded_channel::<NaiteOscEvent>();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, b"\x1b]777;otherapp;foo\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn input_bad_cursor_dropped() {
        let (tx, mut rx) = mpsc::unbounded_channel::<NaiteOscEvent>();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, b"\x1b]777;naite;input;hi;not_a_num\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn cwd_missing_path_dropped() {
        let (tx, mut rx) = mpsc::unbounded_channel::<NaiteOscEvent>();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, b"\x1b]777;naite;cwd\x07");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn unknown_event_dropped() {
        let (tx, mut rx) = mpsc::unbounded_channel::<NaiteOscEvent>();
        let mut sink = NaiteOscSink::new(tx);
        let mut parser = Parser::new();
        parser.advance(&mut sink, b"\x1b]777;naite;unknownevent;x\x07");
        assert!(rx.try_recv().is_err());
    }
}
