//! TOML lexer.

use crate::highlight::{
    lexer::{eat_ident, eat_number, eat_string, push_span},
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &["true", "false", "inf", "nan"];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let mut spans: Vec<TokenSpan> = Vec::new();
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Continue a triple-quoted string from a previous line.
    if state.in_triple_string {
        // Scan for closing `"""` or `'''` — we don't track which delimiter opened
        // it, so try both. In practice only one will be active at a time.
        let close_dq = find_triple(bytes, 0, b'"');
        let close_sq = find_triple(bytes, 0, b'\'');
        let close = match (close_dq, close_sq) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        if let Some(end) = close {
            push_span(&mut spans, 0, end, TokenKind::String);
            state.in_triple_string = false;
            i = end;
        } else {
            push_span(&mut spans, 0, len, TokenKind::String);
            return spans;
        }
    }

    // Skip leading whitespace to detect line-start tokens.
    let line_start = {
        let mut s = i;
        while s < len && (bytes[s] == b' ' || bytes[s] == b'\t') {
            s += 1;
        }
        s
    };

    // Section header: line starts with '['.
    if line_start < len && bytes[line_start] == b'[' {
        // Find the matching ']', handling '[[...]]'.
        if let Some(close) = bytes[line_start..].iter().position(|&b| b == b']') {
            // Include up to and including the last ']' on the line (for [[arr]]).
            let mut end = line_start + close + 1;
            if end < len && bytes[end] == b']' {
                end += 1;
            }
            push_span(&mut spans, line_start, end, TokenKind::Type);
            // Nothing else to highlight on a section header line.
            i = len;
        }
    } else if line_start < len {
        // Key detection: ident (alphanum/_/-) followed by optional spaces then '='.
        let b = bytes[line_start];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            let mut key_end = line_start;
            while key_end < len {
                let c = bytes[key_end];
                if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.' {
                    key_end += 1;
                } else {
                    break;
                }
            }
            let mut j = key_end;
            while j < len && bytes[j] == b' ' {
                j += 1;
            }
            if j < len && bytes[j] == b'=' {
                push_span(&mut spans, line_start, key_end, TokenKind::Keyword);
            }
        }
    }

    // Walk full line for comments, strings, numbers, keyword values.
    while i < len {
        let b = bytes[i];

        // Line comment '#'
        if b == b'#' {
            push_span(&mut spans, i, len, TokenKind::Comment);
            return spans;
        }

        // Triple double-quoted string `"""..."""`
        if b == b'"' && i + 2 < len && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' {
            let start = i;
            i += 3;
            if let Some(rel) = find_triple(bytes, i, b'"') {
                push_span(&mut spans, start, rel, TokenKind::String);
                i = rel;
            } else {
                push_span(&mut spans, start, len, TokenKind::String);
                state.in_triple_string = true;
                return spans;
            }
            continue;
        }

        // Triple single-quoted string `'''...'''`
        if b == b'\'' && i + 2 < len && bytes[i + 1] == b'\'' && bytes[i + 2] == b'\'' {
            let start = i;
            i += 3;
            if let Some(rel) = find_triple(bytes, i, b'\'') {
                push_span(&mut spans, start, rel, TokenKind::String);
                i = rel;
            } else {
                push_span(&mut spans, start, len, TokenKind::String);
                state.in_triple_string = true;
                return spans;
            }
            continue;
        }

        // Basic string `"..."`
        if b == b'"' {
            let end = eat_string(bytes, i, b'"');
            push_span(&mut spans, i, end, TokenKind::String);
            i = end;
            continue;
        }

        // Literal string `'...'`
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

        // Identifier: keyword (true/false/inf/nan)
        if b.is_ascii_alphabetic() || b == b'_' {
            let ident_end = eat_ident(bytes, i);
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

/// Find the first occurrence of three consecutive `delim` bytes starting at
/// `from`. Returns the offset *past* the closing triple (i.e. `pos + 3`).
fn find_triple(src: &[u8], from: usize, delim: u8) -> Option<usize> {
    let mut i = from;
    while i + 2 < src.len() {
        if src[i] == delim && src[i + 1] == delim && src[i + 2] == delim {
            return Some(i + 3);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;

    #[test]
    fn toml_section_header() {
        let mut s = LineState::default();
        let src = "[package]";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Type
                && &src[t.start as usize..t.end as usize] == "[package]"),
            "expected '[package]' as Type, got: {:?}",
            spans
        );
    }

    #[test]
    fn toml_key_value() {
        let mut s = LineState::default();
        let src = r#"name = "Alice""#;
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Keyword
                && &src[t.start as usize..t.end as usize] == "name"),
            "expected 'name' as Keyword, got: {:?}",
            spans
        );
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::String
                && &src[t.start as usize..t.end as usize] == "\"Alice\""),
            "expected '\"Alice\"' as String, got: {:?}",
            spans
        );
    }

    #[test]
    fn toml_true_false_keywords() {
        let mut s = LineState::default();
        let src = "enabled = true";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Keyword
                && &src[t.start as usize..t.end as usize] == "true"),
            "expected 'true' as Keyword, got: {:?}",
            spans
        );
    }
}
