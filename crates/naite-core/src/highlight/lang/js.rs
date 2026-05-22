//! JavaScript / TypeScript lexer.

use crate::highlight::{
    lexer::{eat_block_comment, eat_ident, eat_line_comment, eat_number, eat_string, push_span},
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[
    "function",
    "const",
    "let",
    "var",
    "if",
    "else",
    "return",
    "class",
    "extends",
    "import",
    "export",
    "from",
    "default",
    "async",
    "await",
    "yield",
    "new",
    "this",
    "super",
    "typeof",
    "instanceof",
    "in",
    "of",
    "null",
    "true",
    "false",
    "undefined",
    "break",
    "continue",
    "switch",
    "case",
    "do",
    "while",
    "for",
    "try",
    "catch",
    "finally",
    "throw",
    "delete",
    "void",
    "interface",
    "type",
    "enum",
    "as",
    "satisfies",
    "declare",
    "implements",
    "public",
    "private",
    "protected",
    "readonly",
    "static",
    "abstract",
    "namespace",
];

const TYPES: &[&str] = &[
    "string", "number", "boolean", "any", "unknown", "never", "object", "Array", "Promise",
    "Record", "Partial", "Readonly", "Required",
];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let mut spans: Vec<TokenSpan> = Vec::new();
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Continue a block comment from a previous line.
    if state.in_block_comment {
        let end = eat_block_comment(bytes, i, &mut state.in_block_comment);
        push_span(&mut spans, i, end, TokenKind::Comment);
        i = end;
        if state.in_block_comment {
            return spans;
        }
    }

    // Continue a template literal from a previous line.
    if state.in_triple_string {
        // Scan for closing backtick.
        let mut j = i;
        let mut found = false;
        while j < len {
            if bytes[j] == b'`' {
                push_span(&mut spans, 0, j + 1, TokenKind::String);
                state.in_triple_string = false;
                i = j + 1;
                found = true;
                break;
            }
            j += 1;
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

        // Template literal (backtick)
        if b == b'`' {
            let end = eat_string(bytes, i, b'`');
            if end == len && bytes[len - 1] != b'`' {
                // Unterminated — spans multiple lines.
                push_span(&mut spans, i, len, TokenKind::String);
                state.in_triple_string = true;
                return spans;
            }
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

        // Identifier / keyword / type / function
        if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
            // eat_ident doesn't handle `$` as a start — handle manually.
            let ident_end = if b == b'$' {
                let mut e = i + 1;
                while e < len
                    && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_' || bytes[e] == b'$')
                {
                    e += 1;
                }
                e
            } else {
                eat_ident(bytes, i)
            };

            if ident_end > i {
                let ident = &bytes[i..ident_end];
                let mut matched = false;

                // Keyword check
                for kw in KEYWORDS {
                    if ident == kw.as_bytes() {
                        push_span(&mut spans, i, ident_end, TokenKind::Keyword);
                        matched = true;
                        break;
                    }
                }

                if !matched {
                    // Type check
                    for ty in TYPES {
                        if ident == ty.as_bytes() {
                            push_span(&mut spans, i, ident_end, TokenKind::Type);
                            matched = true;
                            break;
                        }
                    }
                }

                if !matched {
                    // Function check: look ahead for `(`
                    let mut j = ident_end;
                    while j < len && bytes[j] == b' ' {
                        j += 1;
                    }
                    if j < len && bytes[j] == b'(' {
                        push_span(&mut spans, i, ident_end, TokenKind::Function);
                    }
                    // Plain — no span.
                }

                i = ident_end;
                continue;
            }
        }

        // Advance one byte.
        i += 1;
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_keyword_function_is_keyword() {
        let mut s = LineState::default();
        let src = "function foo() {}";
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::Keyword
                && &src[t.start as usize..t.end as usize] == "function"),
            "expected 'function' to be Keyword, got: {:?}",
            spans
        );
    }

    #[test]
    fn js_string_double_quote() {
        let mut s = LineState::default();
        let src = r#"const x = "hello";"#;
        let spans = lex_line(&mut s, src);
        assert!(
            spans.iter().any(|t| t.kind == TokenKind::String
                && &src[t.start as usize..t.end as usize] == "\"hello\""),
            "expected string span, got: {:?}",
            spans
        );
    }

    #[test]
    fn js_line_comment_is_comment() {
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
    fn js_block_comment_multiline() {
        let mut s = LineState::default();
        let src = "/* open block";
        let spans = lex_line(&mut s, src);
        assert!(s.in_block_comment, "state should be in block comment");
        assert!(spans.iter().any(|t| t.kind == TokenKind::Comment));
        let src2 = "still comment */";
        let spans2 = lex_line(&mut s, src2);
        assert!(!s.in_block_comment, "block comment should be closed");
        assert!(spans2.iter().any(|t| t.kind == TokenKind::Comment));
    }

    #[test]
    fn js_type_is_type() {
        let mut s = LineState::default();
        let src = "const arr: Array<number> = [];";
        let spans = lex_line(&mut s, src);
        assert!(
            spans
                .iter()
                .any(|t| t.kind == TokenKind::Type
                    && &src[t.start as usize..t.end as usize] == "Array"),
            "expected 'Array' to be Type, got: {:?}",
            spans
        );
    }

    #[test]
    fn js_function_call_is_function() {
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

    #[test]
    fn js_template_literal_multiline() {
        let mut s = LineState::default();
        let src = "`open template";
        let spans = lex_line(&mut s, src);
        assert!(s.in_triple_string, "state should be in template literal");
        assert!(spans.iter().any(|t| t.kind == TokenKind::String));
        let src2 = "closing`";
        let spans2 = lex_line(&mut s, src2);
        assert!(!s.in_triple_string, "template literal should be closed");
        assert!(spans2.iter().any(|t| t.kind == TokenKind::String));
    }
}
