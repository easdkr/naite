//! Terminal autocomplete suggestion engine.
//!
//! Pure ranking + suppression logic. No iced, no IO, no Git.

// Wave 2 wires the suggestion engine into the terminal update loop; until then
// the binary doesn't reference it. Tests cover everything here.
#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionSource {
    ZshHistory,
    SessionHistory,
    PathCompletion,
    GitSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSuggestion {
    pub suffix: String,
    pub source: SuggestionSource,
}

pub struct SuggestionInputs<'a> {
    pub buffer: &'a str,
    pub cursor: usize,
    pub zsh_history: &'a [String],
    pub session_history: &'a [String],
    pub cwd: &'a std::path::Path,
}

pub fn suggest(inputs: SuggestionInputs<'_>) -> Option<ActiveSuggestion> {
    // 1) Suppression
    if inputs.buffer.is_empty() {
        return None;
    }
    let char_count = inputs.buffer.chars().count();
    if inputs.cursor != char_count {
        return None;
    }

    // 2) ZshHistory candidate
    for entry in inputs.zsh_history.iter().rev() {
        if entry.starts_with(inputs.buffer) && entry != inputs.buffer {
            let suffix: String = entry.chars().skip(char_count).collect();
            if !suffix.is_empty() {
                return Some(ActiveSuggestion {
                    suffix,
                    source: SuggestionSource::ZshHistory,
                });
            }
        }
    }

    // 3) SessionHistory candidate
    for entry in inputs.session_history.iter().rev() {
        if entry.starts_with(inputs.buffer) && entry != inputs.buffer {
            let suffix: String = entry.chars().skip(char_count).collect();
            if !suffix.is_empty() {
                return Some(ActiveSuggestion {
                    suffix,
                    source: SuggestionSource::SessionHistory,
                });
            }
        }
    }

    // 4) PathCompletion candidate
    if let Some(suggestion) = suggest_path(inputs.buffer, inputs.cwd) {
        return Some(suggestion);
    }

    // 5) GitSubcommand candidate
    if let Some(suggestion) = suggest_git(inputs.buffer) {
        return Some(suggestion);
    }

    None
}

fn suggest_path(buffer: &str, cwd: &std::path::Path) -> Option<ActiveSuggestion> {
    // Extract current token (after last ASCII whitespace)
    let token = match buffer.rfind(|c: char| c.is_ascii_whitespace()) {
        Some(pos) => &buffer[pos + 1..],
        None => buffer,
    };

    // Only activate when token contains '/'
    if !token.contains('/') {
        return None;
    }

    // Split into dir_part and basename
    let slash_pos = token.rfind('/').unwrap();
    let dir_part = &token[..=slash_pos];
    let basename = &token[slash_pos + 1..];

    // Resolve dir_part to a filesystem path
    let resolved_dir = if dir_part.starts_with("~/") || dir_part == "~/" {
        let home = std::env::var("HOME").ok()?;
        std::path::PathBuf::from(home).join(&dir_part[2..])
    } else if dir_part.starts_with('/') {
        std::path::PathBuf::from(dir_part)
    } else {
        cwd.join(dir_part)
    };

    // Read and sort directory entries
    let mut entries: Vec<String> = std::fs::read_dir(&resolved_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();

    // Find first child whose name starts_with basename and != basename
    for name in &entries {
        if name.starts_with(basename) && name != basename {
            let suffix = name[basename.len()..].to_owned();
            if !suffix.is_empty() {
                return Some(ActiveSuggestion {
                    suffix,
                    source: SuggestionSource::PathCompletion,
                });
            }
        }
    }

    None
}

static GIT_SUBCOMMANDS: &[&str] = &[
    "status",
    "stash",
    "switch",
    "show",
    "push",
    "pull",
    "fetch",
    "commit",
    "checkout",
    "branch",
    "rebase",
    "merge",
    "log",
    "diff",
    "reset",
    "restore",
    "add",
    "cherry-pick",
    "revert",
    "tag",
    "blame",
    "bisect",
    "clean",
    "remote",
    "worktree",
];

fn suggest_git(buffer: &str) -> Option<ActiveSuggestion> {
    // Check structure: "git" followed by optional whitespace and optional subcommand token,
    // with no additional whitespace segments after the subcommand token.
    // Valid forms: "git ", "git <token>" (no space after token).
    // Invalid: "git status ", "git status -v", etc.
    let parts: Vec<&str> = buffer.split_whitespace().collect();

    if parts.is_empty() || parts[0] != "git" {
        return None;
    }

    // Must have exactly 1 or 2 whitespace-split segments
    if parts.len() > 2 {
        return None;
    }

    let subcmd_token = if parts.len() == 2 {
        // If there's trailing whitespace after the second token, it means
        // the user typed "git status " — that has a trailing space so we'd
        // have a third segment if there was more; but with parts.len() == 2
        // and buffer ending in whitespace, that means "git <token> " which
        // has trailing space → more than one segment after git → skip.
        if buffer.ends_with(|c: char| c.is_whitespace()) {
            // e.g. "git status " — trailing space after token, not a valid 2-token form
            return None;
        }
        parts[1]
    } else {
        // parts.len() == 1: just "git" or "git " (with trailing whitespace)
        if !buffer.ends_with(|c: char| c.is_whitespace()) {
            // Just "git" with no space — don't trigger
            return None;
        }
        // "git " with trailing whitespace — token is ""
        ""
    };

    for &cmd in GIT_SUBCOMMANDS {
        if cmd.starts_with(subcmd_token) && cmd != subcmd_token {
            let suffix = cmd[subcmd_token.len()..].to_owned();
            if !suffix.is_empty() {
                return Some(ActiveSuggestion {
                    suffix,
                    source: SuggestionSource::GitSubcommand,
                });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn inputs<'a>(
        buffer: &'a str,
        zsh: &'a [String],
        session: &'a [String],
        cwd: &'a Path,
    ) -> SuggestionInputs<'a> {
        SuggestionInputs {
            buffer,
            cursor: buffer.chars().count(),
            zsh_history: zsh,
            session_history: session,
            cwd,
        }
    }

    fn empty_strs() -> Vec<String> {
        vec![]
    }

    #[test]
    fn empty_buffer_returns_none() {
        let result = suggest(SuggestionInputs {
            buffer: "",
            cursor: 0,
            zsh_history: &[],
            session_history: &[],
            cwd: Path::new("/tmp"),
        });
        assert_eq!(result, None);
    }

    #[test]
    fn cursor_mid_buffer_returns_none() {
        let zsh = empty_strs();
        let session = empty_strs();
        let result = suggest(SuggestionInputs {
            buffer: "hello",
            cursor: 2,
            zsh_history: &zsh,
            session_history: &session,
            cwd: Path::new("/tmp"),
        });
        assert_eq!(result, None);
    }

    #[test]
    fn zsh_history_beats_session_history() {
        let zsh = vec!["git status".to_string()];
        let session = vec!["git stash".to_string()];
        let cwd = Path::new("/tmp");
        let result = suggest(inputs("git s", &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "tatus".to_string(),
                source: SuggestionSource::ZshHistory,
            })
        );
    }

    #[test]
    fn exact_match_in_history_is_skipped() {
        let zsh = vec!["git status".to_string(), "git s".to_string()];
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        // "git s" is exact match → skip; "git status" also matches but is older.
        // iter().rev() gives "git s" first (newest) → skip exact; then "git status" → suffix "tatus"
        let result = suggest(inputs("git s", &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "tatus".to_string(),
                source: SuggestionSource::ZshHistory,
            })
        );
    }

    #[test]
    fn zsh_history_most_recent_wins() {
        // "git stash" is newer (last element)
        let zsh = vec!["git status".to_string(), "git stash".to_string()];
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        let result = suggest(inputs("git s", &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "tash".to_string(),
                source: SuggestionSource::ZshHistory,
            })
        );
    }

    #[test]
    fn git_static_st_gives_status() {
        let zsh = empty_strs();
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        let result = suggest(inputs("git st", &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "atus".to_string(),
                source: SuggestionSource::GitSubcommand,
            })
        );
    }

    #[test]
    fn git_static_trailing_space_gives_status() {
        let zsh = empty_strs();
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        let result = suggest(inputs("git ", &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "status".to_string(),
                source: SuggestionSource::GitSubcommand,
            })
        );
    }

    #[test]
    fn git_static_not_triggered_for_extra_tokens() {
        let zsh = empty_strs();
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        // "git status -" has 3 whitespace segments → not triggered
        let result = suggest(inputs("git status -", &zsh, &session, cwd));
        assert_eq!(result, None);
    }

    #[test]
    fn git_static_not_triggered_trailing_space_after_subcommand() {
        let zsh = empty_strs();
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        // "git status " → parts = ["git", "status"] but ends with space → None
        let result = suggest(inputs("git status ", &zsh, &session, cwd));
        assert_eq!(result, None);
    }

    #[test]
    fn multibyte_buffer_suggestion() {
        let zsh = vec!["\u{C548}\u{B155}\u{D558}\u{C138}\u{C694} world".to_string()];
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        let buffer = "\u{C548}\u{B155}\u{D558}\u{C138}\u{C694} wo";
        let result = suggest(inputs(buffer, &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "rld".to_string(),
                source: SuggestionSource::ZshHistory,
            })
        );
    }

    #[test]
    fn path_completion_with_slash() {
        let dir = std::env::temp_dir().join(format!("naite_suggest_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("alpha"), b"").unwrap();
        std::fs::write(dir.join("alphabet"), b"").unwrap();
        std::fs::write(dir.join("beta"), b"").unwrap();

        let dir_str = dir.to_string_lossy();
        let buffer = format!("cat {}/alph", dir_str);
        let zsh = empty_strs();
        let session = empty_strs();
        let result = suggest(inputs(&buffer, &zsh, &session, &dir));

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "a".to_string(),
                source: SuggestionSource::PathCompletion,
            })
        );
    }

    #[test]
    fn path_completion_not_triggered_without_slash() {
        let zsh = empty_strs();
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        // "alpha" has no '/' → no path completion, no history → None
        let result = suggest(inputs("alpha", &zsh, &session, cwd));
        assert_eq!(result, None);
    }

    #[test]
    fn exact_history_only_entry_returns_none() {
        // buffer == only zsh entry → skipped → None
        let zsh = vec!["git".to_string()];
        let session = empty_strs();
        let cwd = Path::new("/tmp");
        let result = suggest(inputs("git", &zsh, &session, cwd));
        // "git" alone with no trailing space also won't trigger git static
        assert_eq!(result, None);
    }

    #[test]
    fn session_history_used_when_zsh_empty() {
        let zsh = empty_strs();
        let session = vec!["cargo build --release".to_string()];
        let cwd = Path::new("/tmp");
        let result = suggest(inputs("cargo b", &zsh, &session, cwd));
        assert_eq!(
            result,
            Some(ActiveSuggestion {
                suffix: "uild --release".to_string(),
                source: SuggestionSource::SessionHistory,
            })
        );
    }
}
