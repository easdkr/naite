use super::*;
use crate::diff::{ChangeStatus, CommitDiff, DiffLine, FileChange, Hunk};
use std::collections::HashMap;

#[test]
fn detect_language_known_extensions() {
    assert_eq!(detect_language("src/main.rs"), Some(Language::Rust));
    assert_eq!(detect_language("a.ts"), Some(Language::JavaScript));
    assert_eq!(detect_language("a.tsx"), Some(Language::JavaScript));
    assert_eq!(detect_language("script.py"), Some(Language::Python));
    assert_eq!(detect_language("README.md"), Some(Language::Markdown));
    assert_eq!(detect_language("Cargo.toml"), Some(Language::Toml));
    assert_eq!(detect_language("config.yaml"), Some(Language::Yaml));
    assert_eq!(detect_language("install.sh"), Some(Language::Shell));
}

#[test]
fn detect_language_unknown_extension_is_none() {
    assert_eq!(detect_language("Cargo.lock"), None);
    assert_eq!(detect_language("LICENSE"), None);
    assert_eq!(detect_language(""), None);
}

#[test]
fn highlight_diff_unknown_extension_returns_empty_spans() {
    let files = vec![FileChange {
        path: "Cargo.lock".into(),
        status: ChangeStatus::Modified,
        old_path: None,
        is_binary: false,
        is_truncated: false,
    }];
    let mut hunks_by_file = HashMap::new();
    hunks_by_file.insert(
        "Cargo.lock".to_string(),
        vec![Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            header: "@@".into(),
            lines: vec![DiffLine::Ctx("name = \"foo\"".into())],
        }],
    );
    let diff = CommitDiff {
        files,
        hunks_by_file,
    };
    let hl = highlight_diff(&diff);
    let file = &hl.by_file["Cargo.lock"];
    assert_eq!(file.len(), 1);
    assert_eq!(file[0].lines.len(), 1);
    assert!(file[0].lines[0].spans.is_empty());
}

#[test]
fn highlight_diff_long_line_returns_empty_spans() {
    // Even with a known language, lines over MAX_LINE_BYTES collapse to empty.
    let long = "a".repeat(MAX_LINE_BYTES + 10);
    let files = vec![FileChange {
        path: "x.rs".into(),
        status: ChangeStatus::Modified,
        old_path: None,
        is_binary: false,
        is_truncated: false,
    }];
    let mut hunks_by_file = HashMap::new();
    hunks_by_file.insert(
        "x.rs".to_string(),
        vec![Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            header: "@@".into(),
            lines: vec![DiffLine::Add(long)],
        }],
    );
    let diff = CommitDiff {
        files,
        hunks_by_file,
    };
    let hl = highlight_diff(&diff);
    assert!(hl.by_file["x.rs"][0].lines[0].spans.is_empty());
}
