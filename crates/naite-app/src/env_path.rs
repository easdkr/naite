//! Process `PATH` augmentation for GUI launches.
//!
//! macOS GUI apps (launched from Finder or a `.app` bundle) inherit only the
//! minimal launchd `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`). Tools installed by
//! Homebrew (`/opt/homebrew/bin`), MacPorts, or cargo are therefore invisible to
//! the bare `Command::new("gh")` / `Command::new("git")` lookups in
//! `naite-core`, so GitHub integrations silently fail even though they work from
//! a terminal. We repair `PATH` once at startup so every child process — `gh`,
//! `git`, future provider CLIs, and the embedded terminal — can find those
//! binaries.

/// Standard binary locations to ensure are on `PATH`, in priority order.
#[cfg(target_os = "macos")]
const STANDARD_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
    "/opt/local/bin",
    "/opt/local/sbin",
];

/// Augment the process `PATH` so a GUI-launched naite can find CLIs installed
/// outside the minimal launchd environment.
///
/// Call this at the very top of `main()`, before iced/tokio start their worker
/// threads: the `std::env::set_var` below must not race other threads reading
/// the environment. (The short-lived worker thread this spawns via
/// `capture_login_shell_path` only *reads* `PATH` through a child shell and
/// never mutates the environment, so it does not undermine that contract.)
#[cfg(target_os = "macos")]
pub fn augment_process_path() {
    let existing = std::env::var("PATH").unwrap_or_default();

    let mut extra: Vec<String> = Vec::new();

    // Per-user binary directories.
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        extra.push(home.join(".cargo/bin").to_string_lossy().into_owned());
        extra.push(home.join(".local/bin").to_string_lossy().into_owned());
    }

    // Standard system-wide locations (Homebrew on Apple Silicon and Intel,
    // MacPorts).
    extra.extend(STANDARD_DIRS.iter().map(|dir| (*dir).to_string()));

    // Best-effort login-shell `PATH` for non-standard installs
    // (asdf / mise / nvm / custom prefixes). Bounded so a slow or broken shell
    // profile can never hang launch.
    if let Some(login_path) = capture_login_shell_path() {
        extra.extend(login_path.split(':').map(str::to_string));
    }

    // Only keep entries that actually exist on disk so we don't bloat `PATH`
    // with dead directories. `existing` is left untouched.
    extra.retain(|dir| !dir.is_empty() && std::path::Path::new(dir).is_dir());

    let merged = merge_path_entries(&existing, &extra);
    if merged != existing {
        std::env::set_var("PATH", merged);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn augment_process_path() {}

/// Run the user's login shell to capture the `PATH` it would set up, with a
/// hard timeout so a misbehaving profile cannot stall startup. Returns `None`
/// on timeout, non-zero exit, spawn failure, or empty output.
#[cfg(target_os = "macos")]
fn capture_login_shell_path() -> Option<String> {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let output = Command::new(&shell)
            .args(["-lc", "printf %s \"$PATH\""])
            .stdin(Stdio::null())
            .output();
        let _ = tx.send(output);
    });

    match rx.recv_timeout(Duration::from_millis(800)) {
        Ok(Ok(output)) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!path.is_empty()).then_some(path)
        }
        _ => None,
    }
}

/// Merge `extra` path entries ahead of `existing`, de-duplicating while
/// preserving first-seen order. Pure (no env / filesystem access) so it can be
/// unit-tested. Compiled under `test` on every platform so non-macOS CI still
/// exercises the merge logic.
#[cfg(any(target_os = "macos", test))]
fn merge_path_entries(existing: &str, extra: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    for entry in extra.iter().map(String::as_str).chain(existing.split(':')) {
        if entry.is_empty() {
            continue;
        }
        if seen.insert(entry) {
            out.push(entry);
        }
    }
    out.join(":")
}

#[cfg(test)]
mod tests {
    use super::merge_path_entries;

    #[test]
    fn prepends_extra_before_existing() {
        let merged = merge_path_entries("/usr/bin:/bin", &["/opt/homebrew/bin".to_string()]);
        assert_eq!(merged, "/opt/homebrew/bin:/usr/bin:/bin");
    }

    #[test]
    fn empty_existing_yields_extra_only() {
        let merged = merge_path_entries("", &["/opt/homebrew/bin".to_string()]);
        assert_eq!(merged, "/opt/homebrew/bin");
    }

    #[test]
    fn empty_extra_yields_existing_only() {
        let merged = merge_path_entries("/usr/bin:/bin", &[]);
        assert_eq!(merged, "/usr/bin:/bin");
    }

    #[test]
    fn dedups_preserving_first_occurrence() {
        let merged = merge_path_entries(
            "/usr/bin:/opt/homebrew/bin:/bin",
            &[
                "/opt/homebrew/bin".to_string(),
                "/usr/local/bin".to_string(),
            ],
        );
        assert_eq!(merged, "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin");
    }

    #[test]
    fn skips_empty_entries() {
        let merged = merge_path_entries("/usr/bin::/bin", &[String::new()]);
        assert_eq!(merged, "/usr/bin:/bin");
    }
}
