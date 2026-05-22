//! Syntax highlighting layer — plain-data tokens consumed by the UI.
//!
//! Highlights are computed eagerly alongside diff parsing and stored in a
//! parallel `HighlightedDiff` cache keyed by file path / hunk index / line
//! index. The lexer is a small hand-rolled cursor (no regex, no deps).
//!
//! Multi-line state (block comments, triple-quoted strings) is carried
//! within a hunk only. A construct opened *before* the visible hunk window
//! may be mis-lexed on its first lines — accepted behavior, matches GitHub.

use std::collections::HashMap;

use crate::diff::{CommitDiff, DiffLine};

#[allow(dead_code)]
pub mod lang;
pub mod lexer;

#[cfg(test)]
mod tests;

/// Lines longer than this return empty spans (prevents pathological slowdown
/// on minified files).
pub const MAX_LINE_BYTES: usize = 4096;

/// Hard cap on spans per line. Trailing tokens collapse into one Plain span
/// so iced's widget tree stays shallow.
pub const MAX_SPANS_PER_LINE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    JavaScript,
    Python,
    Go,
    Markdown,
    Json,
    Yaml,
    Toml,
    Shell,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Function,
    Punct,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan {
    pub start: u16,
    pub end: u16,
    pub kind: TokenKind,
}

/// State carried across lines within a single hunk.
#[derive(Debug, Clone, Default)]
pub struct LineState {
    pub in_block_comment: bool,
    pub raw_string_hashes: Option<u8>,
    pub in_triple_string: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighlightedLine {
    pub spans: Vec<TokenSpan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighlightedHunk {
    pub lines: Vec<HighlightedLine>,
}

#[derive(Debug, Clone, Default)]
pub struct HighlightedDiff {
    pub by_file: HashMap<String, Vec<HighlightedHunk>>,
}

/// Match the *final* extension on the path to a known language.
pub fn detect_language(path: &str) -> Option<Language> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => Language::Rust,
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => Language::JavaScript,
        "py" | "pyi" => Language::Python,
        "go" => Language::Go,
        "md" | "markdown" => Language::Markdown,
        "json" | "jsonc" => Language::Json,
        "yaml" | "yml" => Language::Yaml,
        "toml" => Language::Toml,
        "sh" | "bash" | "zsh" => Language::Shell,
        "html" | "htm" | "xml" | "svg" => Language::Html,
        _ => return None,
    })
}

fn lex_line(lang: Language, state: &mut LineState, src: &str) -> Vec<TokenSpan> {
    if src.len() > MAX_LINE_BYTES {
        return Vec::new();
    }
    let mut spans = match lang {
        Language::Rust => lang::rust::lex_line(state, src),
        Language::JavaScript => lang::js::lex_line(state, src),
        Language::Python => lang::python::lex_line(state, src),
        Language::Go => lang::go::lex_line(state, src),
        Language::Markdown => lang::markdown::lex_line(state, src),
        Language::Json => lang::json::lex_line(state, src),
        Language::Yaml => lang::yaml::lex_line(state, src),
        Language::Toml => lang::toml::lex_line(state, src),
        Language::Shell => lang::shell::lex_line(state, src),
        Language::Html => lang::html::lex_line(state, src),
    };
    if spans.len() > MAX_SPANS_PER_LINE {
        spans.truncate(MAX_SPANS_PER_LINE);
    }
    spans
}

/// Compute highlights for every line of every hunk in the diff.
pub fn highlight_diff(diff: &CommitDiff) -> HighlightedDiff {
    let mut by_file: HashMap<String, Vec<HighlightedHunk>> = HashMap::new();
    for file in &diff.files {
        let Some(hunks) = diff.hunks_by_file.get(&file.path) else {
            continue;
        };
        let lang = detect_language(&file.path);
        let mut file_hunks = Vec::with_capacity(hunks.len());
        for hunk in hunks {
            let mut state = LineState::default();
            let mut hl_lines = Vec::with_capacity(hunk.lines.len());
            for line in &hunk.lines {
                let body = match line {
                    DiffLine::Ctx(s) | DiffLine::Add(s) | DiffLine::Del(s) => s.as_str(),
                };
                let spans = match lang {
                    Some(l) => lex_line(l, &mut state, body),
                    None => Vec::new(),
                };
                hl_lines.push(HighlightedLine { spans });
            }
            file_hunks.push(HighlightedHunk { lines: hl_lines });
        }
        by_file.insert(file.path.clone(), file_hunks);
    }
    HighlightedDiff { by_file }
}
