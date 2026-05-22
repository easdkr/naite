//! Zsh shell integration wrapper.
//!
//! Prepares a sandboxed `ZDOTDIR` that sources the user's real `.zshrc` and
//! then installs naite's integration script so the shell emits OSC 777
//! events. Does not modify the user's dotfiles.
//!
//! # Caller environment requirements
//!
//! The caller (terminal runtime) must set these env vars on the spawned zsh process:
//!
//! - `ZDOTDIR=<launch.zdotdir>` — points zsh at our wrapper dotdir
//! - `NAITE_USER_ZDOTDIR=<original ZDOTDIR, or unset if not present>` — lets the
//!   wrapper `.zshrc` find and source the user's real `.zshrc`
//! - `NAITE_INTEGRATION_SCRIPT=<launch.zdotdir>/naite-integration.zsh` — path
//!   to the embedded integration script written into the temp dir

// Wave 2 wires the wrapper into runtime spawn; until then most items look unused.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Zsh,
    Unsupported,
}

#[allow(clippy::derivable_impls)]
impl Default for ShellKind {
    fn default() -> Self {
        Self::Unsupported
    }
}

/// Result of preparing a zsh integration wrapper. Dropping this cleans up the temp dir.
#[allow(dead_code)]
pub struct IntegrationLaunch {
    pub zdotdir: PathBuf,
    _cleanup: TempDirGuard,
}

#[allow(dead_code)]
struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Detect shell kind from a shell binary path. Accepts both full paths
/// and bare names. Empty input → Unsupported.
pub fn detect_shell_kind(shell_path: &str) -> ShellKind {
    if shell_path.is_empty() {
        return ShellKind::Unsupported;
    }
    // std::path::Path::file_name returns None for paths ending with '/'
    let file_name = std::path::Path::new(shell_path)
        .file_name()
        .and_then(|n| n.to_str());
    match file_name {
        Some("zsh") => ShellKind::Zsh,
        _ => ShellKind::Unsupported,
    }
}

/// Build a wrapper ZDOTDIR for a zsh session.
#[allow(dead_code)]
pub fn prepare_zsh_integration() -> Result<IntegrationLaunch, String> {
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("naite-zsh-{}-{}", std::process::id(), counter));

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create temp dir {}: {}", dir.display(), e))?;

    let write_file = |name: &str, content: &str| -> Result<(), String> {
        std::fs::write(dir.join(name), content)
            .map_err(|e| format!("failed to write {}: {}", name, e))
    };

    let result = (|| -> Result<(), String> {
        write_file(".zshenv", "")?;

        write_file(
            ".zshrc",
            "# naite zsh integration wrapper. Sources user's real .zshrc first.\n\
             if [[ -n \"$NAITE_USER_ZDOTDIR\" ]]; then\n\
             \x20   _naite_user_zdotdir=\"$NAITE_USER_ZDOTDIR\"\n\
             else\n\
             \x20   _naite_user_zdotdir=\"$HOME\"\n\
             fi\n\
             if [[ -r \"$_naite_user_zdotdir/.zshrc\" ]]; then\n\
             \x20   ZDOTDIR=\"$_naite_user_zdotdir\" source \"$_naite_user_zdotdir/.zshrc\"\n\
             fi\n\
             unset _naite_user_zdotdir\n\
             if [[ -r \"$NAITE_INTEGRATION_SCRIPT\" ]]; then\n\
             \x20   source \"$NAITE_INTEGRATION_SCRIPT\"\n\
             fi\n",
        )?;

        write_file(
            "naite-integration.zsh",
            include_str!("naite-zsh-integration.zsh"),
        )?;

        Ok(())
    })();

    match result {
        Ok(()) => Ok(IntegrationLaunch {
            zdotdir: dir.clone(),
            _cleanup: TempDirGuard { path: dir },
        }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bin_zsh() {
        assert_eq!(detect_shell_kind("/bin/zsh"), ShellKind::Zsh);
    }

    #[test]
    fn detect_homebrew_zsh() {
        assert_eq!(detect_shell_kind("/opt/homebrew/bin/zsh"), ShellKind::Zsh);
    }

    #[test]
    fn detect_local_zsh() {
        assert_eq!(detect_shell_kind("/usr/local/bin/zsh"), ShellKind::Zsh);
    }

    #[test]
    fn detect_bare_zsh() {
        assert_eq!(detect_shell_kind("zsh"), ShellKind::Zsh);
    }

    #[test]
    fn detect_bash_unsupported() {
        assert_eq!(detect_shell_kind("/bin/bash"), ShellKind::Unsupported);
    }

    #[test]
    fn detect_fish_unsupported() {
        assert_eq!(detect_shell_kind("/bin/fish"), ShellKind::Unsupported);
    }

    #[test]
    fn detect_empty_unsupported() {
        assert_eq!(detect_shell_kind(""), ShellKind::Unsupported);
    }

    #[test]
    fn detect_trailing_slash_unsupported() {
        assert_eq!(detect_shell_kind("/bin/"), ShellKind::Unsupported);
    }

    #[test]
    fn prepare_returns_ok_and_dir_exists() {
        let launch = prepare_zsh_integration().expect("prepare_zsh_integration failed");
        assert!(launch.zdotdir.exists(), "zdotdir should exist");
        assert!(launch.zdotdir.join(".zshrc").exists(), ".zshrc missing");
        assert!(launch.zdotdir.join(".zshenv").exists(), ".zshenv missing");
        assert!(
            launch.zdotdir.join("naite-integration.zsh").exists(),
            "naite-integration.zsh missing"
        );
    }

    #[test]
    fn zshrc_contains_required_strings() {
        let launch = prepare_zsh_integration().expect("prepare_zsh_integration failed");
        let content = std::fs::read_to_string(launch.zdotdir.join(".zshrc")).expect("read .zshrc");
        assert!(
            content.contains("source \"$NAITE_INTEGRATION_SCRIPT\""),
            ".zshrc missing NAITE_INTEGRATION_SCRIPT source"
        );
        assert!(
            content.contains("NAITE_USER_ZDOTDIR"),
            ".zshrc missing NAITE_USER_ZDOTDIR"
        );
    }

    #[test]
    fn integration_script_matches_embedded() {
        let launch = prepare_zsh_integration().expect("prepare_zsh_integration failed");
        let on_disk = std::fs::read(launch.zdotdir.join("naite-integration.zsh"))
            .expect("read naite-integration.zsh");
        let embedded = include_str!("naite-zsh-integration.zsh").as_bytes();
        assert_eq!(
            on_disk.len(),
            embedded.len(),
            "naite-integration.zsh size mismatch"
        );
    }

    #[test]
    fn drop_removes_directory() {
        let launch = prepare_zsh_integration().expect("prepare_zsh_integration failed");
        let path = launch.zdotdir.clone();
        assert!(path.exists(), "dir should exist before drop");
        drop(launch);
        assert!(!path.exists(), "dir should be removed after drop");
    }

    #[test]
    fn two_calls_produce_different_dirs() {
        let a = prepare_zsh_integration().expect("first call failed");
        let b = prepare_zsh_integration().expect("second call failed");
        assert_ne!(
            a.zdotdir, b.zdotdir,
            "consecutive calls must return different directories"
        );
    }
}
