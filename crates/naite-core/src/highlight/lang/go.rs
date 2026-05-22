//! Go syntax lexer.

use crate::highlight::{
    lexer::{
        eat_block_comment, eat_ident, eat_line_comment, eat_number, eat_string, match_keyword,
        push_span,
    },
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[
    "func",
    "var",
    "const",
    "type",
    "struct",
    "interface",
    "package",
    "import",
    "return",
    "if",
    "else",
    "for",
    "range",
    "switch",
    "case",
    "default",
    "go",
    "defer",
    "chan",
    "select",
    "map",
    "nil",
    "true",
    "false",
    "break",
    "continue",
    "fallthrough",
    "goto",
];

pub const TYPES: &[&str] = &[
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    "float32",
    "float64",
    "string",
    "bool",
    "byte",
    "rune",
    "error",
    "any",
    "complex64",
    "complex128",
];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let src = src.as_bytes();
    let mut out: Vec<TokenSpan> = Vec::new();
    let mut i = 0;

    // Resume block comment from previous line
    if state.in_block_comment {
        let end = eat_block_comment(src, 0, &mut state.in_block_comment);
        push_span(&mut out, 0, end, TokenKind::Comment);
        i = end;
    }

    // Resume backtick raw string from previous line (reuse in_triple_string)
    if state.in_triple_string {
        let mut j = i;
        while j < src.len() && src[j] != b'`' {
            j += 1;
        }
        if j < src.len() && src[j] == b'`' {
            j += 1;
            state.in_triple_string = false;
        }
        push_span(&mut out, i, j, TokenKind::String);
        i = j;
    }

    while i < src.len() {
        let c = src[i];

        // Whitespace
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }

        // Line comment
        if i + 1 < src.len() && c == b'/' && src[i + 1] == b'/' {
            let end = eat_line_comment(src, i);
            push_span(&mut out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }

        // Block comment
        if i + 1 < src.len() && c == b'/' && src[i + 1] == b'*' {
            let end = eat_block_comment(src, i, &mut state.in_block_comment);
            push_span(&mut out, i, end, TokenKind::Comment);
            i = end;
            continue;
        }

        // Backtick raw string
        if c == b'`' {
            let mut j = i + 1;
            while j < src.len() && src[j] != b'`' {
                j += 1;
            }
            if j < src.len() && src[j] == b'`' {
                j += 1;
                push_span(&mut out, i, j, TokenKind::String);
            } else {
                // unterminated — extends to next line
                push_span(&mut out, i, j, TokenKind::String);
                state.in_triple_string = true;
            }
            i = j;
            continue;
        }

        // Double-quoted string
        if c == b'"' {
            let end = eat_string(src, i, b'"');
            push_span(&mut out, i, end, TokenKind::String);
            i = end;
            continue;
        }

        // Rune literal: short `'x'` — only treat as String if short (≤ 8 bytes to closing quote)
        if c == b'\'' {
            let mut j = i + 1;
            let limit = (i + 9).min(src.len());
            let mut found = false;
            while j < limit {
                if src[j] == b'\\' && j + 1 < limit {
                    j += 2;
                    continue;
                }
                if src[j] == b'\'' {
                    j += 1;
                    found = true;
                    break;
                }
                j += 1;
            }
            if found {
                push_span(&mut out, i, j, TokenKind::String);
                i = j;
            } else {
                i += 1;
            }
            continue;
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
        let result = kinds("func main() {}");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Keyword && *t == "func"));
    }

    #[test]
    fn test_line_comment() {
        let result = kinds("x := 1 // comment");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Comment && t.starts_with("//")));
    }

    #[test]
    fn test_block_comment() {
        let result = kinds("/* block */");
        assert!(result.iter().any(|(k, _)| *k == TokenKind::Comment));
    }

    #[test]
    fn test_string_double() {
        let result = kinds(r#"s := "hello""#);
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::String && *t == "\"hello\""));
    }

    #[test]
    fn test_backtick_string_single_line() {
        let result = kinds("s := `raw string`");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::String && *t == "`raw string`"));
    }

    #[test]
    fn test_backtick_string_multiline() {
        let mut state = LineState::default();
        let spans1 = lex_line(&mut state, "s := `start");
        assert!(state.in_triple_string);
        assert!(spans1.iter().any(|s| s.kind == TokenKind::String));
        let spans2 = lex_line(&mut state, "end`");
        assert!(!state.in_triple_string);
        assert!(spans2.iter().any(|s| s.kind == TokenKind::String));
    }

    #[test]
    fn test_rune_literal() {
        let result = kinds("c := 'x'");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::String && *t == "'x'"));
    }

    #[test]
    fn test_type() {
        let result = kinds("var x int");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Type && *t == "int"));
    }

    #[test]
    fn test_number() {
        let result = kinds("x := 42");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Number && *t == "42"));
    }

    #[test]
    fn test_function_call() {
        let result = kinds("fmt.Println(x)");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Function && *t == "Println"));
    }
}
