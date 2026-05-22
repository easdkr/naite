//! Python syntax lexer.

use crate::highlight::{
    lexer::{eat_ident, eat_line_comment, eat_number, eat_string, match_keyword, push_span},
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[
    "def", "class", "if", "elif", "else", "for", "while", "return", "import", "from", "as", "in",
    "not", "and", "or", "is", "None", "True", "False", "lambda", "yield", "async", "await", "try",
    "except", "finally", "raise", "with", "pass", "break", "continue", "global", "nonlocal", "del",
    "assert",
];

pub const TYPES: &[&str] = &[
    "int",
    "float",
    "str",
    "bool",
    "list",
    "dict",
    "tuple",
    "set",
    "frozenset",
    "bytes",
    "bytearray",
    "Any",
    "List",
    "Dict",
    "Tuple",
    "Set",
    "Optional",
    "Union",
    "Callable",
];

/// Consume a triple-quoted string starting at `i`.
/// Returns `(end, closed)` where `end` is the new cursor and `closed` is true
/// if the triple was closed within this line.
fn eat_triple_string(src: &[u8], mut i: usize, quote: u8) -> (usize, bool) {
    while i + 2 < src.len() {
        if src[i] == quote && src[i + 1] == quote && src[i + 2] == quote {
            return (i + 3, true);
        }
        if src[i] == b'\\' && i + 1 < src.len() {
            i += 2;
            continue;
        }
        i += 1;
    }
    // check if exactly at the boundary
    if i + 2 == src.len() && src[i] == quote && src[i + 1] == quote && i + 2 < src.len() {
        return (i + 3, true);
    }
    (src.len(), false)
}

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let src = src.as_bytes();
    let mut out: Vec<TokenSpan> = Vec::new();
    let mut i = 0;

    // Resume triple-quoted string from previous line
    if state.in_triple_string {
        // raw_string_hashes: Some(0) = double-quote, Some(1) = single-quote
        let quote = match state.raw_string_hashes {
            Some(1) => b'\'',
            _ => b'"',
        };
        let (end, closed) = eat_triple_string(src, 0, quote);
        push_span(&mut out, 0, end, TokenKind::String);
        if closed {
            state.in_triple_string = false;
            state.raw_string_hashes = None;
        }
        i = end;
    }

    while i < src.len() {
        let c = src[i];

        // Skip whitespace
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }

        // Line comment
        if c == b'#' {
            let end = eat_line_comment(src, i);
            push_span(&mut out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }

        // String prefixes: f, F, r, R, b, B, u, U (1-2 chars before quote)
        let prefix_end = {
            let mut p = i;
            if p < src.len()
                && matches!(
                    src[p],
                    b'f' | b'F' | b'r' | b'R' | b'b' | b'B' | b'u' | b'U'
                )
            {
                p += 1;
                if p < src.len()
                    && matches!(
                        src[p],
                        b'f' | b'F' | b'r' | b'R' | b'b' | b'B' | b'u' | b'U'
                    )
                {
                    p += 1;
                }
            }
            if p < src.len() && (src[p] == b'"' || src[p] == b'\'') {
                p
            } else {
                i // no prefix
            }
        };

        let quote_start = if prefix_end > i { prefix_end } else { i };

        if quote_start < src.len() && (src[quote_start] == b'"' || src[quote_start] == b'\'') {
            let quote = src[quote_start];
            // Check for triple quote
            if quote_start + 2 < src.len()
                && src[quote_start + 1] == quote
                && src[quote_start + 2] == quote
            {
                let body_start = quote_start + 3;
                let (end, closed) = eat_triple_string(src, body_start, quote);
                push_span(&mut out, i, end, TokenKind::String);
                if !closed {
                    state.in_triple_string = true;
                    state.raw_string_hashes = if quote == b'\'' { Some(1) } else { Some(0) };
                }
                i = end;
                continue;
            } else {
                // single-quoted string
                let end = eat_string(src, quote_start, quote);
                push_span(&mut out, i, end, TokenKind::String);
                i = end;
                continue;
            }
        }

        // Number
        if c.is_ascii_digit() {
            let end = eat_number(src, i);
            if end > i {
                push_span(&mut out, i, end, TokenKind::Number);
                i = end;
                continue;
            }
        }

        // Identifier / keyword / type / function
        if c.is_ascii_alphabetic() || c == b'_' {
            let id_end = eat_ident(src, i);
            if id_end > i {
                if let Some(kw_end) = match_keyword(src, i, KEYWORDS) {
                    push_span(&mut out, i, kw_end, TokenKind::Keyword);
                    i = kw_end;
                } else if let Some(ty_end) = match_keyword(src, i, TYPES) {
                    push_span(&mut out, i, ty_end, TokenKind::Type);
                    i = ty_end;
                } else {
                    // Check if followed by '(' → Function
                    let mut j = id_end;
                    while j < src.len() && (src[j] == b' ' || src[j] == b'\t') {
                        j += 1;
                    }
                    if j < src.len() && src[j] == b'(' {
                        push_span(&mut out, i, id_end, TokenKind::Function);
                    }
                    i = id_end;
                }
                continue;
            }
        }

        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;

    fn kinds(src: &str) -> Vec<(TokenKind, &str)> {
        let mut state = LineState::default();
        let spans = lex_line(&mut state, src);
        spans
            .iter()
            .map(|s| (s.kind, &src[s.start as usize..s.end as usize]))
            .collect()
    }

    #[test]
    fn test_keywords() {
        let result = kinds("def foo():");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Keyword && *t == "def"));
    }

    #[test]
    fn test_comment() {
        let result = kinds("x = 1  # this is a comment");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Comment && t.starts_with('#')));
    }

    #[test]
    fn test_string_double() {
        let result = kinds(r#"x = "hello""#);
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::String && *t == "\"hello\""));
    }

    #[test]
    fn test_string_single() {
        let result = kinds("x = 'world'");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::String && *t == "'world'"));
    }

    #[test]
    fn test_triple_string_single_line() {
        let result = kinds(r#"x = """triple""""#);
        assert!(result.iter().any(|(k, _)| *k == TokenKind::String));
    }

    #[test]
    fn test_triple_string_multiline() {
        let mut state = LineState::default();
        let spans1 = lex_line(&mut state, r#"x = """start"#);
        assert!(state.in_triple_string);
        assert!(spans1.iter().any(|s| s.kind == TokenKind::String));
        let spans2 = lex_line(&mut state, r#"end""""#);
        assert!(!state.in_triple_string);
        assert!(spans2.iter().any(|s| s.kind == TokenKind::String));
    }

    #[test]
    fn test_type_annotation() {
        let result = kinds("x: int = 0");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Type && *t == "int"));
    }

    #[test]
    fn test_number() {
        let result = kinds("x = 42");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Number && *t == "42"));
    }

    #[test]
    fn test_function_call() {
        let result = kinds("print(x)");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Function && *t == "print"));
    }
}
