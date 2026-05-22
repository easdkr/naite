//! Markdown lexer.

use crate::highlight::lexer::push_span;
use crate::highlight::{LineState, TokenSpan};

#[allow(dead_code)]
pub const KEYWORDS: &[&str] = &[];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut out: Vec<TokenSpan> = Vec::new();

    // Handle fenced code block exit: line starting with ```
    if state.in_triple_string {
        // Check if this line starts with ``` (exit fence)
        let mut i = 0;
        while i < len && bytes[i] == b' ' {
            i += 1;
        }
        if i + 2 < len && &bytes[i..i + 3] == b"```" {
            state.in_triple_string = false;
        }
        // Inside code block: no spans
        return out;
    }

    // Check for fenced code block entry: line starting with ```
    {
        let mut i = 0;
        while i < len && bytes[i] == b' ' {
            i += 1;
        }
        if i + 2 < len && &bytes[i..i + 3] == b"```" {
            state.in_triple_string = true;
            // The fence line itself: no spans (treat as plain)
            return out;
        }
    }

    // Skip leading whitespace for structural checks
    let mut ws_end = 0;
    while ws_end < len && (bytes[ws_end] == b' ' || bytes[ws_end] == b'\t') {
        ws_end += 1;
    }

    // Heading: (optional whitespace) + 1-6 `#` + space
    if ws_end < len && bytes[ws_end] == b'#' {
        let mut hashes = ws_end;
        while hashes < len && bytes[hashes] == b'#' {
            hashes += 1;
        }
        let count = hashes - ws_end;
        if count <= 6 && hashes < len && bytes[hashes] == b' ' {
            push_span(&mut out, 0, len, crate::highlight::TokenKind::Keyword);
            return out;
        }
    }

    // Blockquote: line starts (after whitespace) with `>`
    if ws_end < len && bytes[ws_end] == b'>' {
        push_span(&mut out, 0, len, crate::highlight::TokenKind::Comment);
        return out;
    }

    // List bullet: `-`, `*`, or `+` followed by a space
    if ws_end < len
        && (bytes[ws_end] == b'-' || bytes[ws_end] == b'*' || bytes[ws_end] == b'+')
        && ws_end + 1 < len
        && bytes[ws_end + 1] == b' '
    {
        push_span(
            &mut out,
            ws_end,
            ws_end + 1,
            crate::highlight::TokenKind::Number,
        );
        // Fall through to scan rest of line for inline elements
        lex_inline(bytes, ws_end + 2, len, &mut out);
        return out;
    }

    // Scan line for inline elements
    lex_inline(bytes, 0, len, &mut out);
    out
}

/// Scan `bytes[start..end]` for inline markdown tokens (backtick code, links).
fn lex_inline(bytes: &[u8], start: usize, end: usize, out: &mut Vec<TokenSpan>) {
    let mut i = start;
    while i < end {
        match bytes[i] {
            // Inline code: `...`
            b'`' => {
                let tick_start = i;
                i += 1;
                while i < end && bytes[i] != b'`' {
                    i += 1;
                }
                if i < end {
                    i += 1; // closing backtick
                    push_span(out, tick_start, i, crate::highlight::TokenKind::String);
                }
            }
            // Link: [text](url)
            b'[' => {
                let bracket_open = i;
                i += 1;
                // find closing ]
                while i < end && bytes[i] != b']' {
                    i += 1;
                }
                if i < end && bytes[i] == b']' {
                    let bracket_close = i + 1;
                    // emit [text] as String (bracket_open..bracket_close)
                    push_span(
                        out,
                        bracket_open,
                        bracket_close,
                        crate::highlight::TokenKind::String,
                    );
                    i = bracket_close;
                    // check for (url)
                    if i < end && bytes[i] == b'(' {
                        let paren_open = i;
                        i += 1;
                        while i < end && bytes[i] != b')' {
                            i += 1;
                        }
                        if i < end && bytes[i] == b')' {
                            i += 1;
                            // emit (url) span: paren_open+1 .. i-1 (just the url)
                            push_span(
                                out,
                                paren_open + 1,
                                i - 1,
                                crate::highlight::TokenKind::Function,
                            );
                        }
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::TokenKind;

    fn default_state() -> LineState {
        LineState::default()
    }

    #[test]
    fn heading_whole_line_keyword() {
        let mut s = default_state();
        let spans = lex_line(&mut s, "## Hello World");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, TokenKind::Keyword);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].end, "## Hello World".len() as u16);
    }

    #[test]
    fn inline_code_string() {
        let mut s = default_state();
        let spans = lex_line(&mut s, "Use `foo` here");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, TokenKind::String);
        // span covers `foo` including backticks
        let src = "Use `foo` here";
        let start = src.find('`').unwrap();
        assert_eq!(spans[0].start as usize, start);
        assert_eq!(spans[0].end as usize, start + "`foo`".len());
    }

    #[test]
    fn link_text_and_url() {
        let mut s = default_state();
        let src = "[click](https://example.com)";
        let spans = lex_line(&mut s, src);
        // Expect two spans: [click] as String, https://example.com as Function
        assert!(spans.iter().any(|sp| sp.kind == TokenKind::String));
        assert!(spans.iter().any(|sp| sp.kind == TokenKind::Function));
        let string_span = spans
            .iter()
            .find(|sp| sp.kind == TokenKind::String)
            .unwrap();
        assert_eq!(
            &src[string_span.start as usize..string_span.end as usize],
            "[click]"
        );
        let func_span = spans
            .iter()
            .find(|sp| sp.kind == TokenKind::Function)
            .unwrap();
        assert_eq!(
            &src[func_span.start as usize..func_span.end as usize],
            "https://example.com"
        );
    }

    #[test]
    fn blockquote_comment() {
        let mut s = default_state();
        let spans = lex_line(&mut s, "> some quoted text");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, TokenKind::Comment);
    }

    #[test]
    fn list_bullet_number() {
        let mut s = default_state();
        let spans = lex_line(&mut s, "- item one");
        assert!(spans.iter().any(|sp| sp.kind == TokenKind::Number));
        let bullet = spans
            .iter()
            .find(|sp| sp.kind == TokenKind::Number)
            .unwrap();
        assert_eq!(bullet.start, 0);
        assert_eq!(bullet.end, 1);
    }

    #[test]
    fn fenced_code_block_state() {
        let mut s = default_state();
        // Opening fence
        let spans = lex_line(&mut s, "```rust");
        assert!(spans.is_empty());
        assert!(s.in_triple_string);
        // Inside: no spans
        let spans2 = lex_line(&mut s, "let x = 1;");
        assert!(spans2.is_empty());
        // Closing fence
        let spans3 = lex_line(&mut s, "```");
        assert!(spans3.is_empty());
        assert!(!s.in_triple_string);
    }
}
