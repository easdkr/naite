//! YAML lexer.

use crate::highlight::{
    lexer::{eat_ident, eat_number, eat_string, push_span},
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[
    "true", "false", "null", "yes", "no", "on", "off", "Yes", "No", "On", "Off", "TRUE", "FALSE",
    "NULL", "~",
];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let _ = state; // YAML has no multi-line state beyond triple strings (not used here)
    let mut spans: Vec<TokenSpan> = Vec::new();
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Skip leading whitespace to find line-start tokens.
    let line_start = {
        let mut s = 0;
        while s < len && (bytes[s] == b' ' || bytes[s] == b'\t') {
            s += 1;
        }
        s
    };

    // Check for a key: pattern at line start (after whitespace).
    // An identifier (alphanum/_/-) followed immediately by ':' (with no ident char after).
    if line_start < len {
        let b = bytes[line_start];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            // Eat an identifier-like sequence allowing hyphens.
            let mut key_end = line_start;
            while key_end < len {
                let c = bytes[key_end];
                if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' {
                    key_end += 1;
                } else {
                    break;
                }
            }
            // Next non-space character must be ':' and the char after that must not be ':'
            // (to avoid tagging `http://` as a key).
            let mut j = key_end;
            while j < len && bytes[j] == b' ' {
                j += 1;
            }
            if j < len && bytes[j] == b':' && (j + 1 >= len || bytes[j + 1] != b':') {
                push_span(&mut spans, line_start, key_end, TokenKind::Keyword);
            }
        }
    }

    // Walk the full line for comments, strings, numbers, and keyword values.
    while i < len {
        let b = bytes[i];

        // Line comment '#'
        if b == b'#' {
            push_span(&mut spans, i, len, TokenKind::Comment);
            return spans;
        }

        // Double-quoted string
        if b == b'"' {
            let end = eat_string(bytes, i, b'"');
            push_span(&mut spans, i, end, TokenKind::String);
            i = end;
            continue;
        }

        // Single-quoted string
        if b == b'\'' {
            let end = eat_string(bytes, i, b'\'');
            push_span(&mut spans, i, end, TokenKind::String);
            i = end;
            continue;
        }

        // Number
        if b.is_ascii_digit() {
            let end = eat_number(bytes, i);
            push_span(&mut spans, i, end, TokenKind::Number);
            i = end;
            continue;
        }

        // Identifier: keyword value (true/false/null/yes/no/on/off/~)
        if b.is_ascii_alphabetic() || b == b'_' || b == b'~' {
            let ident_end = if b == b'~' {
                i + 1
            } else {
                eat_ident(bytes, i)
            };
            if ident_end > i {
                let ident = &bytes[i..ident_end];
                let is_kw = KEYWORDS.iter().any(|kw| kw.as_bytes() == ident);
                if is_kw {
                    push_span(&mut spans, i, ident_end, TokenKind::Keyword);
                }
                i = ident_end;
                continue;
            }
        }

        i += 1;
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;

    #[test]
    fn yaml_key_value() {
        let mut s = LineState::default();
        let src = "name: Alice";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Keyword
                && &src[t.start as usize..t.end as usize] == "name"),
            "expected 'name' as Keyword, got: {:?}",
            spans
        );
    }

    #[test]
    fn yaml_comment() {
        let mut s = LineState::default();
        let src = "# this is a comment";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Comment),
            "expected Comment span, got: {:?}",
            spans
        );
    }

    #[test]
    fn yaml_string() {
        let mut s = LineState::default();
        let src = r#"greeting: "hello world""#;
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::String
                && &src[t.start as usize..t.end as usize] == "\"hello world\""),
            "expected String span, got: {:?}",
            spans
        );
    }
}
