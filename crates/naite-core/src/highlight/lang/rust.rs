//! Rust lexer.

use crate::highlight::{
    lexer::{
        eat_block_comment, eat_ident, eat_line_comment, eat_number, eat_string, match_keyword,
        push_span,
    },
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "match", "if", "else",
    "loop", "while", "for", "return", "self", "Self", "where", "move", "async", "await", "dyn",
    "const", "static", "ref", "break", "continue", "trait", "type", "as", "in", "unsafe", "extern",
    "crate", "true", "false",
];

const TYPES: &[&str] = &[
    "Box", "Option", "Result", "Vec", "String", "bool", "i8", "i16", "i32", "i64", "i128", "isize",
    "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "char", "str",
];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let mut spans: Vec<TokenSpan> = Vec::new();
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Continue a block comment that started on a previous line.
    if state.in_block_comment {
        let end = eat_block_comment(bytes, i, &mut state.in_block_comment);
        push_span(&mut spans, i, end, TokenKind::Comment);
        i = end;
        if state.in_block_comment {
            // Still inside — whole line consumed.
            return spans;
        }
    }

    // Continue a raw string that started on a previous line.
    if let Some(hashes) = state.raw_string_hashes {
        // Look for closing quote + hashes.
        let h = hashes as usize;
        let mut found = false;
        while i < len {
            if bytes[i] == b'"' {
                let after_quote = i + 1;
                let hash_end = after_quote + h;
                if hash_end <= len && bytes[after_quote..hash_end].iter().all(|&b| b == b'#') {
                    push_span(&mut spans, 0, hash_end, TokenKind::String);
                    state.raw_string_hashes = None;
                    i = hash_end;
                    found = true;
                    break;
                }
            }
            i += 1;
        }
        if !found {
            push_span(&mut spans, 0, len, TokenKind::String);
            return spans;
        }
    }

    while i < len {
        let b = bytes[i];

        // Line comment `//`
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            let end = eat_line_comment(bytes, i);
            push_span(&mut spans, i, end, TokenKind::Comment);
            i = end;
            continue;
        }

        // Block comment `/* ... */`
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let end = eat_block_comment(bytes, i, &mut state.in_block_comment);
            push_span(&mut spans, i, end, TokenKind::Comment);
            i = end;
            if state.in_block_comment {
                return spans;
            }
            continue;
        }

        // Raw string r#"..."# or r"..."
        if b == b'r' {
            let mut j = i + 1;
            let mut hash_count: u8 = 0;
            while j < len && bytes[j] == b'#' {
                hash_count += 1;
                j += 1;
            }
            if j < len && bytes[j] == b'"' {
                // Opening found — scan for closing `"` + hash_count `#`.
                j += 1; // past opening quote
                let h = hash_count as usize;
                let mut closed = false;
                while j < len {
                    if bytes[j] == b'"' {
                        let after_q = j + 1;
                        let hash_end = after_q + h;
                        if hash_end <= len && bytes[after_q..hash_end].iter().all(|&b2| b2 == b'#')
                        {
                            push_span(&mut spans, i, hash_end, TokenKind::String);
                            i = hash_end;
                            closed = true;
                            break;
                        }
                    }
                    j += 1;
                }
                if !closed {
                    push_span(&mut spans, i, len, TokenKind::String);
                    state.raw_string_hashes = Some(hash_count);
                    return spans;
                }
                continue;
            }
        }

        // String literal `"..."`
        if b == b'"' {
            let end = eat_string(bytes, i, b'"');
            push_span(&mut spans, i, end, TokenKind::String);
            i = end;
            continue;
        }

        // Char literal or lifetime.
        if b == b'\'' {
            // Distinguish lifetime `'a` vs char literal `'x'` or `'\n'` etc.
            // If the next char is alpha/_ and not followed by `'` within short
            // range, treat as lifetime (skip).
            if i + 1 < len && (bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_') {
                // Check for char: look for closing `'` within 4 bytes of opening.
                let is_char = (i + 2 < len && bytes[i + 2] == b'\'')
                    || (i + 3 < len && bytes[i + 1] == b'\\' && bytes[i + 3] == b'\'')
                    || (i + 4 < len && bytes[i + 1] == b'\\' && bytes[i + 4] == b'\'');
                if is_char {
                    let end = eat_string(bytes, i, b'\'');
                    push_span(&mut spans, i, end, TokenKind::String);
                    i = end;
                    continue;
                }
                // Otherwise lifetime — skip the `'` and let ident be handled below.
                i += 1;
                continue;
            } else {
                // Not alpha after quote — treat as char literal (e.g. `' '`, `'\n'`).
                let end = eat_string(bytes, i, b'\'');
                push_span(&mut spans, i, end, TokenKind::String);
                i = end;
                continue;
            }
        }

        // Number
        if b.is_ascii_digit() {
            let end = eat_number(bytes, i);
            push_span(&mut spans, i, end, TokenKind::Number);
            i = end;
            continue;
        }

        // Identifier / keyword / type / function
        if b.is_ascii_alphabetic() || b == b'_' {
            if let Some(kw_end) = match_keyword(bytes, i, KEYWORDS) {
                push_span(&mut spans, i, kw_end, TokenKind::Keyword);
                i = kw_end;
                continue;
            }
            if let Some(ty_end) = match_keyword(bytes, i, TYPES) {
                push_span(&mut spans, i, ty_end, TokenKind::Type);
                i = ty_end;
                continue;
            }
            let ident_end = eat_ident(bytes, i);
            // Look ahead for `(` — function call/definition.
            let mut j = ident_end;
            while j < len && bytes[j] == b' ' {
                j += 1;
            }
            if j < len && bytes[j] == b'(' {
                push_span(&mut spans, i, ident_end, TokenKind::Function);
            }
            // Plain — no span needed (default rendering).
            i = ident_end;
            continue;
        }

        // Advance one byte for everything else (punctuation, whitespace, etc.)
        i += 1;
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_keyword_fn_is_keyword() {
        let mut s = LineState::default();
        let spans = lex_line(&mut s, "fn main() {}");
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Keyword
                && &"fn main() {}"[t.start as usize..t.end as usize] == "fn"),
            "expected 'fn' to be Keyword, got: {:?}",
            spans
        );
    }

    #[test]
    fn rust_string_is_string() {
        let mut s = LineState::default();
        let src = r#"let x = "hello";"#;
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::String
                && &src[t.start as usize..t.end as usize] == "\"hello\""),
            "expected string span, got: {:?}",
            spans
        );
    }

    #[test]
    fn rust_line_comment_is_comment() {
        let mut s = LineState::default();
        let src = "// this is a comment";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Comment),
            "expected Comment span, got: {:?}",
            spans
        );
    }

    #[test]
    fn rust_block_comment_multiline() {
        let mut s = LineState::default();
        // Open without closing — state should persist.
        let src = "/* open block";
        let spans = lex_line(&mut s, src);
        assert!(s.in_block_comment, "state should be in block comment");
        assert!(spans.iter().any(|t| t.kind == TokenKind::Comment));
        // Close on next line.
        let src2 = "still comment */";
        let spans2 = lex_line(&mut s, src2);
        assert!(!s.in_block_comment, "block comment should be closed");
        assert!(spans2.iter().any(|t| t.kind == TokenKind::Comment));
    }

    #[test]
    fn rust_type_is_type() {
        let mut s = LineState::default();
        let src = "let x: Vec<i32> = Vec::new();";
        let spans = lex_line(&mut s, src);
        assert!(
            spans
                .iter()
                .any(|t| t.kind == TokenKind::Type
                    && &src[t.start as usize..t.end as usize] == "Vec"),
            "expected Vec to be Type, got: {:?}",
            spans
        );
    }

    #[test]
    fn rust_function_call_is_function() {
        let mut s = LineState::default();
        let src = "foo(42)";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Function
                && &src[t.start as usize..t.end as usize] == "foo"),
            "expected 'foo' to be Function, got: {:?}",
            spans
        );
    }
}
