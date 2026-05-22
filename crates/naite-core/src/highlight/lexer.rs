//! Shared cursor primitives for hand-rolled language lexers.
//!
//! All eaters operate on byte indices. Callers pass `(src, start)` and get
//! back the next byte offset after the consumed token. UTF-8 awareness: we
//! only inspect ASCII bytes for delimiters, so multi-byte content in
//! strings/comments/identifiers is preserved correctly.

use super::{TokenKind, TokenSpan};

/// Push a span guarding against `start == end` (empty spans pollute the
/// iced widget tree without changing rendering).
pub fn push_span(out: &mut Vec<TokenSpan>, start: usize, end: usize, kind: TokenKind) {
    if end > start && start <= u16::MAX as usize && end <= u16::MAX as usize {
        out.push(TokenSpan {
            start: start as u16,
            end: end as u16,
            kind,
        });
    }
}

/// Match the keyword set against `src[start..]`. Returns the end offset if
/// the identifier at `start` is a whole-word keyword.
pub fn match_keyword(src: &[u8], start: usize, keywords: &[&str]) -> Option<usize> {
    let end = eat_ident(src, start);
    if end == start {
        return None;
    }
    let ident = &src[start..end];
    for kw in keywords {
        if ident == kw.as_bytes() {
            return Some(end);
        }
    }
    None
}

/// Consume an ASCII identifier (`[A-Za-z_][A-Za-z0-9_]*`). Returns the byte
/// after the identifier, or `start` if none.
pub fn eat_ident(src: &[u8], start: usize) -> usize {
    if start >= src.len() {
        return start;
    }
    let first = src[start];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return start;
    }
    let mut end = start + 1;
    while end < src.len() {
        let c = src[end];
        if c.is_ascii_alphanumeric() || c == b'_' {
            end += 1;
        } else {
            break;
        }
    }
    end
}

/// Consume a numeric literal (decimal/hex/oct/bin, with optional underscores
/// and a single trailing dot+digits for floats). Returns end offset.
pub fn eat_number(src: &[u8], start: usize) -> usize {
    if start >= src.len() || !src[start].is_ascii_digit() {
        return start;
    }
    let mut end = start + 1;
    // optional 0x / 0o / 0b prefix
    if start + 1 < src.len() && src[start] == b'0' {
        match src[start + 1] {
            b'x' | b'X' | b'o' | b'O' | b'b' | b'B' => {
                end = start + 2;
                while end < src.len() && (src[end].is_ascii_hexdigit() || src[end] == b'_') {
                    end += 1;
                }
                return end;
            }
            _ => {}
        }
    }
    while end < src.len() && (src[end].is_ascii_digit() || src[end] == b'_') {
        end += 1;
    }
    // optional .frac
    if end + 1 < src.len() && src[end] == b'.' && src[end + 1].is_ascii_digit() {
        end += 1;
        while end < src.len() && (src[end].is_ascii_digit() || src[end] == b'_') {
            end += 1;
        }
    }
    // optional exponent
    if end < src.len() && (src[end] == b'e' || src[end] == b'E') {
        end += 1;
        if end < src.len() && (src[end] == b'+' || src[end] == b'-') {
            end += 1;
        }
        while end < src.len() && src[end].is_ascii_digit() {
            end += 1;
        }
    }
    // optional type suffix (i32, u8, f64, usize, etc.) — consume identifier tail
    if end < src.len() && (src[end].is_ascii_alphabetic() || src[end] == b'_') {
        end = eat_ident(src, end);
    }
    end
}

/// Consume a single-line string starting with `quote` at `start`. Handles
/// `\\` escapes. Returns end offset *past* the closing quote, or `src.len()`
/// if unterminated.
pub fn eat_string(src: &[u8], start: usize, quote: u8) -> usize {
    debug_assert!(start < src.len() && src[start] == quote);
    let mut i = start + 1;
    while i < src.len() {
        let c = src[i];
        if c == b'\\' && i + 1 < src.len() {
            i += 2;
            continue;
        }
        if c == quote {
            return i + 1;
        }
        i += 1;
    }
    src.len()
}

/// Consume a line comment from `start` to end of line.
pub fn eat_line_comment(src: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < src.len() && src[i] != b'\n' {
        i += 1;
    }
    i
}

/// Consume a `/* ... */` block comment starting at `start`. Sets
/// `in_block_comment` if the block extends past end-of-line. Returns end
/// offset (either past `*/` or `src.len()`).
pub fn eat_block_comment(src: &[u8], start: usize, in_block: &mut bool) -> usize {
    let mut i = start;
    // if we entered already-inside, skip the opening check
    if !*in_block {
        if i + 1 >= src.len() || &src[i..i + 2] != b"/*" {
            return start;
        }
        i += 2;
        *in_block = true;
    }
    while i + 1 < src.len() {
        if &src[i..i + 2] == b"*/" {
            *in_block = false;
            return i + 2;
        }
        i += 1;
    }
    src.len()
}
