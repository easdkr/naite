//! HTML/XML syntax lexer.

use crate::highlight::{
    lexer::{eat_ident, eat_string, push_span},
    LineState, TokenKind, TokenSpan,
};

pub const KEYWORDS: &[&str] = &[];

pub fn lex_line(state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    let src_bytes = src.as_bytes();
    let mut out: Vec<TokenSpan> = Vec::new();
    let mut i = 0;

    // Resume block comment (HTML <!-- -->) from previous line
    if state.in_block_comment {
        // Search for --> on this line
        let mut found = false;
        while i + 2 < src_bytes.len() {
            if &src_bytes[i..i + 3] == b"-->" {
                i += 3;
                push_span(&mut out, 0, i, TokenKind::Comment);
                state.in_block_comment = false;
                found = true;
                break;
            }
            i += 1;
        }
        if !found {
            // whole line is comment
            push_span(&mut out, 0, src_bytes.len(), TokenKind::Comment);
            return out;
        }
    }

    while i < src_bytes.len() {
        let c = src_bytes[i];

        // Whitespace
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }

        // HTML comment: <!-- ... -->
        if i + 3 < src_bytes.len() && &src_bytes[i..i + 4] == b"<!--" {
            let start = i;
            i += 4;
            let mut closed = false;
            while i + 2 < src_bytes.len() {
                if &src_bytes[i..i + 3] == b"-->" {
                    i += 3;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                // check exact boundary
                if i + 2 == src_bytes.len() && &src_bytes[i..i + 3] == b"-->" {
                    i += 3;
                    closed = true;
                }
            }
            if closed {
                push_span(&mut out, start, i, TokenKind::Comment);
            } else {
                push_span(&mut out, start, src_bytes.len(), TokenKind::Comment);
                state.in_block_comment = true;
                return out;
            }
            continue;
        }

        // Tag: < /? ident ... >
        if c == b'<' {
            i += 1;
            // optional closing slash
            let mut _is_close = false;
            if i < src_bytes.len() && src_bytes[i] == b'/' {
                _is_close = true;
                i += 1;
            }
            // tag name
            let name_start = i;
            let name_end = eat_ident(src_bytes, i);
            if name_end > name_start {
                push_span(&mut out, name_start, name_end, TokenKind::Keyword);
                i = name_end;
                // parse attributes until > or />
                loop {
                    // skip whitespace
                    while i < src_bytes.len()
                        && (src_bytes[i] == b' '
                            || src_bytes[i] == b'\t'
                            || src_bytes[i] == b'\r'
                            || src_bytes[i] == b'\n')
                    {
                        i += 1;
                    }
                    if i >= src_bytes.len() {
                        break;
                    }
                    let ch = src_bytes[i];
                    // end of tag
                    if ch == b'>' {
                        i += 1;
                        break;
                    }
                    if ch == b'/' && i + 1 < src_bytes.len() && src_bytes[i + 1] == b'>' {
                        i += 2;
                        break;
                    }
                    // attribute name
                    let attr_start = i;
                    let attr_end = eat_ident(src_bytes, i);
                    if attr_end > attr_start {
                        i = attr_end;
                        // skip whitespace then check for =
                        let mut j = i;
                        while j < src_bytes.len() && (src_bytes[j] == b' ' || src_bytes[j] == b'\t')
                        {
                            j += 1;
                        }
                        if j < src_bytes.len() && src_bytes[j] == b'=' {
                            // attribute name
                            push_span(&mut out, attr_start, attr_end, TokenKind::Type);
                            i = j + 1; // skip '='
                                       // skip whitespace
                            while i < src_bytes.len()
                                && (src_bytes[i] == b' ' || src_bytes[i] == b'\t')
                            {
                                i += 1;
                            }
                            // attribute value string
                            if i < src_bytes.len()
                                && (src_bytes[i] == b'"' || src_bytes[i] == b'\'')
                            {
                                let q = src_bytes[i];
                                let end = eat_string(src_bytes, i, q);
                                push_span(&mut out, i, end, TokenKind::String);
                                i = end;
                            }
                        } else {
                            // boolean attribute — no value, skip it
                            i = attr_end;
                        }
                    } else {
                        // unrecognized char inside tag, skip
                        i += 1;
                    }
                }
            }
            continue;
        }

        // HTML entity: &name;
        if c == b'&' {
            let start = i;
            i += 1;
            let id_end = eat_ident(src_bytes, i);
            if id_end > i && id_end < src_bytes.len() && src_bytes[id_end] == b';' {
                push_span(&mut out, start, id_end + 1, TokenKind::Number);
                i = id_end + 1;
                continue;
            }
            // not a valid entity, keep going
            i = start + 1;
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
    fn test_tag() {
        let result = kinds("<div>");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Keyword && *t == "div"));
    }

    #[test]
    fn test_attribute() {
        let result = kinds(r#"<a href="https://example.com">"#);
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Type && *t == "href"));
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::String && t.contains("example.com")));
    }

    #[test]
    fn test_comment() {
        let result = kinds("<!-- this is a comment -->");
        assert!(result
            .iter()
            .any(|(k, t)| *k == TokenKind::Comment && t.starts_with("<!--")));
    }
}
