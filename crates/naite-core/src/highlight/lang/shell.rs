//! Shell (bash/zsh) syntax lexer.

use crate::highlight::{
    lexer::{eat_ident, eat_number, eat_string, match_keyword, push_span},
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "case", "esac", "for", "in", "while", "do", "done",
    "function", "return", "exit", "local", "export", "readonly", "declare", "typeset", "source",
    "set", "shift", "trap", "unset", "alias",
];

pub const BUILTINS: &[&str] = &[
    "echo", "printf", "read", "cd", "pwd", "ls", "cat", "grep", "sed", "awk", "find", "xargs",
    "cp", "mv", "rm", "mkdir", "rmdir", "touch", "chmod", "chown", "which", "test", "true",
    "false",
];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let _ = state; // no multi-line state for shell
    let src_bytes = src.as_bytes();
    let mut out: Vec<TokenSpan> = Vec::new();
    let mut i = 0;

    // Shebang: if line starts with #! treat whole line as comment
    if src_bytes.starts_with(b"#!") {
        push_span(&mut out, 0, src_bytes.len(), TokenKind::Comment);
        return out;
    }

    while i < src_bytes.len() {
        let c = src_bytes[i];

        // Whitespace
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }

        // Line comment
        if c == b'#' {
            push_span(&mut out, i, src_bytes.len(), TokenKind::Comment);
            break;
        }

        // Strings
        if c == b'"' || c == b'\'' {
            let end = eat_string(src_bytes, i, c);
            push_span(&mut out, i, end, TokenKind::String);
            i = end;
            continue;
        }

        // Variable expansion: ${...} or $VAR
        if c == b'$' {
            if i + 1 < src_bytes.len() && src_bytes[i + 1] == b'{' {
                // ${...}: find closing }
                let start = i;
                i += 2;
                while i < src_bytes.len() && src_bytes[i] != b'}' {
                    i += 1;
                }
                if i < src_bytes.len() {
                    i += 1; // consume '}'
                }
                push_span(&mut out, start, i, TokenKind::Function);
                continue;
            } else if i + 1 < src_bytes.len()
                && (src_bytes[i + 1].is_ascii_alphabetic() || src_bytes[i + 1] == b'_')
            {
                let start = i;
                i += 1; // skip '$'
                let id_end = eat_ident(src_bytes, i);
                push_span(&mut out, start, id_end, TokenKind::Function);
                i = id_end;
                continue;
            } else {
                i += 1;
                continue;
            }
        }

        // Flags: --long or -x
        if c == b'-'
            && i + 1 < src_bytes.len()
            && (src_bytes[i + 1].is_ascii_alphabetic() || src_bytes[i + 1] == b'-')
        {
            let start = i;
            i += 1;
            if i < src_bytes.len() && src_bytes[i] == b'-' {
                i += 1; // second dash
            }
            let id_end = eat_ident(src_bytes, i);
            if id_end > i {
                push_span(&mut out, start, id_end, TokenKind::Number);
                i = id_end;
            } else {
                i = start + 1;
            }
            continue;
        }

        // Number
        if c.is_ascii_digit() {
            let end = eat_number(src_bytes, i);
            if end > i {
                push_span(&mut out, i, end, TokenKind::Number);
                i = end;
                continue;
            }
        }

        // Identifier: keyword, builtin, or plain
        if c.is_ascii_alphabetic() || c == b'_' {
            if let Some(kw_end) = match_keyword(src_bytes, i, KEYWORDS) {
                push_span(&mut out, i, kw_end, TokenKind::Keyword);
                i = kw_end;
                continue;
            }
            if let Some(bi_end) = match_keyword(src_bytes, i, BUILTINS) {
                push_span(&mut out, i, bi_end, TokenKind::Type);
                i = bi_end;
                continue;
            }
            let id_end = eat_ident(src_bytes, i);
            i = id_end;
            continue;
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
    fn test_keyword() {
        let result = kinds("if [ $x ]; then");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Keyword && *t == "if"));
    }

    #[test]
    fn test_comment() {
        let result = kinds("echo hello # this is a comment");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Comment && t.starts_with('#')));
    }

    #[test]
    fn test_variable() {
        let result = kinds("echo $VAR");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Function && *t == "$VAR"));
    }
}
