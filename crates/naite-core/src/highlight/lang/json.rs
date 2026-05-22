//! JSON lexer.

use crate::highlight::lexer::{eat_number, eat_string, match_keyword, push_span};
use crate::highlight::{LineState, TokenKind, TokenSpan};

pub const KEYWORDS: &[&str] = &["true", "false", "null"];

pub fn lex_line(_state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut out: Vec<TokenSpan> = Vec::new();
    let mut i = 0;

    while i < len {
        let c = bytes[i];

        // Skip whitespace
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }

        // String
        if c == b'"' {
            let str_start = i;
            let str_end = eat_string(bytes, i, b'"');
            // Peek past the string for `:` to determine if it's a key (Type) or value (String)
            let mut j = str_end;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let kind = if j < len && bytes[j] == b':' {
                TokenKind::Type
            } else {
                TokenKind::String
            };
            push_span(&mut out, str_start, str_end, kind);
            i = str_end;
            continue;
        }

        // Number (including negative)
        if c.is_ascii_digit() || (c == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let num_start = i;
            // eat_number doesn't handle leading `-`, advance past it
            let parse_start = if c == b'-' { i + 1 } else { i };
            let num_end = eat_number(bytes, parse_start);
            if num_end > parse_start {
                push_span(&mut out, num_start, num_end, TokenKind::Number);
                i = num_end;
            } else {
                i += 1;
            }
            continue;
        }

        // Keywords: true, false, null
        if c.is_ascii_alphabetic() {
            if let Some(kw_end) = match_keyword(bytes, i, KEYWORDS) {
                push_span(&mut out, i, kw_end, TokenKind::Keyword);
                i = kw_end;
                continue;
            }
        }

        // Everything else (punctuation: { } [ ] , :) — plain, skip
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;

    fn default_state() -> LineState {
        crate::highlight::LineState::default()
    }

    #[test]
    fn key_string_is_type() {
        let mut s = default_state();
        let src = r#"  "name": "Alice""#;
        let spans = lex_line(&mut s, src);
        // First string ("name") should be Type, second ("Alice") should be String
        let key_span = spans.iter().find(|sp| sp.kind == TokenKind::Type).unwrap();
        assert_eq!(
            &src[key_span.start as usize..key_span.end as usize],
            r#""name""#
        );
        let val_span = spans
            .iter()
            .find(|sp| sp.kind == TokenKind::String)
            .unwrap();
        assert_eq!(
            &src[val_span.start as usize..val_span.end as usize],
            r#""Alice""#
        );
    }

    #[test]
    fn number_span() {
        let mut s = default_state();
        let src = "  42";
        let spans = lex_line(&mut s, src);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, TokenKind::Number);
        assert_eq!(&src[spans[0].start as usize..spans[0].end as usize], "42");
    }

    #[test]
    fn keywords_true_false_null() {
        let mut s = default_state();
        let src = "true false null";
        let spans = lex_line(&mut s, src);
        assert_eq!(spans.len(), 3);
        for sp in &spans {
            assert_eq!(sp.kind, TokenKind::Keyword);
        }
        assert_eq!(&src[spans[0].start as usize..spans[0].end as usize], "true");
        assert_eq!(
            &src[spans[1].start as usize..spans[1].end as usize],
            "false"
        );
        assert_eq!(&src[spans[2].start as usize..spans[2].end as usize], "null");
    }

    #[test]
    fn negative_number() {
        let mut s = default_state();
        let src = "-3.14";
        let spans = lex_line(&mut s, src);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, TokenKind::Number);
        assert_eq!(
            &src[spans[0].start as usize..spans[0].end as usize],
            "-3.14"
        );
    }

    #[test]
    fn punctuation_ignored() {
        let mut s = default_state();
        let src = "{ }";
        let spans = lex_line(&mut s, src);
        assert!(spans.is_empty());
    }
}
