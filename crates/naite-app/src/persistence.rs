use std::fs;
use std::path::{Path, PathBuf};

use naite_core::ReleaseProfile;

use crate::state::{
    DensityPreference, PreferencesState, RepositoryCatalog, RepositoryEntry, ThemePreference,
};

pub fn load_repository_catalog() -> Result<RepositoryCatalog, String> {
    let path = catalog_file_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) => Ok(parse_repository_catalog(&raw)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(RepositoryCatalog::default()),
        Err(err) => Err(format!(
            "failed to read repository catalog at {}: {err}",
            path.display()
        )),
    }
}

pub fn save_repository_catalog(catalog: &RepositoryCatalog) -> Result<(), String> {
    let path = catalog_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create repository catalog directory {}: {err}",
                parent.display()
            )
        })?;
    }

    fs::write(&path, serialize_repository_catalog(catalog)).map_err(|err| {
        format!(
            "failed to write repository catalog at {}: {err}",
            path.display()
        )
    })
}

fn catalog_file_path() -> Result<PathBuf, String> {
    Ok(naite_data_dir()?.join("repositories.tsv"))
}

fn open_tabs_file_path() -> Result<PathBuf, String> {
    Ok(naite_data_dir()?.join("open_tabs.tsv"))
}

fn preferences_file_path() -> Result<PathBuf, String> {
    Ok(naite_data_dir()?.join("preferences.tsv"))
}

fn naite_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("naite"))
}

fn naite_cache_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Caches")
        .join("naite"))
}

fn avatar_cache_path(url: &str) -> Result<PathBuf, String> {
    Ok(naite_cache_dir()?
        .join("avatars")
        .join(format!("{:016x}.bin", stable_hash_u64(url))))
}

// FNV-1a 64-bit. Stable across Rust versions, unlike DefaultHasher.
// Picked because the alternative is pulling in a hashing crate, and a
// stable hash matters for a long-lived on-disk cache.
fn stable_hash_u64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce4_84222325;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn load_avatar_bytes(url: &str) -> Option<Vec<u8>> {
    let path = avatar_cache_path(url).ok()?;
    fs::read(&path).ok()
}

pub fn save_avatar_bytes(url: &str, bytes: &[u8]) -> Result<(), String> {
    let path = avatar_cache_path(url)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create avatar cache directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&path, bytes)
        .map_err(|err| format!("failed to write avatar cache at {}: {err}", path.display()))
}

/// Snapshot of persisted tab state. `open` is in display order; the first
/// entry (if any) is treated as the active tab on restore.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenTabsSnapshot {
    pub open: Vec<PathBuf>,
}

pub fn load_open_tabs() -> Result<OpenTabsSnapshot, String> {
    let path = open_tabs_file_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) => Ok(parse_open_tabs(&raw)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(OpenTabsSnapshot::default()),
        Err(err) => Err(format!(
            "failed to read open tabs at {}: {err}",
            path.display()
        )),
    }
}

pub fn save_open_tabs(snapshot: &OpenTabsSnapshot) -> Result<(), String> {
    let path = open_tabs_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create open tabs directory {}: {err}",
                parent.display()
            )
        })?;
    }

    fs::write(&path, serialize_open_tabs(snapshot))
        .map_err(|err| format!("failed to write open tabs at {}: {err}", path.display()))
}

pub fn load_preferences() -> Result<PreferencesState, String> {
    let path = preferences_file_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) => Ok(parse_preferences(&raw)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(PreferencesState::default()),
        Err(err) => Err(format!(
            "failed to read preferences at {}: {err}",
            path.display()
        )),
    }
}

pub fn save_preferences(preferences: &PreferencesState) -> Result<(), String> {
    let path = preferences_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create preferences directory {}: {err}",
                parent.display()
            )
        })?;
    }

    fs::write(&path, serialize_preferences(preferences))
        .map_err(|err| format!("failed to write preferences at {}: {err}", path.display()))
}

pub(crate) async fn save_preferences_task(preferences: PreferencesState) -> Result<(), String> {
    tokio::task::spawn_blocking(move || save_preferences(&preferences))
        .await
        .map_err(|e| format!("worker join error: {e}"))?
}

fn serialize_open_tabs(snapshot: &OpenTabsSnapshot) -> String {
    let mut out = String::new();
    for path in &snapshot.open {
        out.push_str(&path.to_string_lossy());
        out.push('\n');
    }
    out
}

fn parse_open_tabs(raw: &str) -> OpenTabsSnapshot {
    let mut open = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = Path::new(trimmed);
        if !path.is_absolute() {
            continue;
        }
        open.push(path.to_path_buf());
    }
    OpenTabsSnapshot { open }
}

fn serialize_preferences(preferences: &PreferencesState) -> String {
    let mut out = String::new();
    out.push_str(&format!("theme\t{}\n", theme_to_str(preferences.theme)));
    out.push_str(&format!(
        "density\t{}\n",
        density_to_str(preferences.density)
    ));
    out.push_str(&format!(
        "show_commit_author\t{}\n",
        bool_to_str(preferences.display.show_commit_author)
    ));
    out.push_str(&format!(
        "show_file_inspection\t{}\n",
        bool_to_str(preferences.display.show_file_inspection)
    ));
    out.push_str(&format!(
        "show_pr_metadata\t{}\n",
        bool_to_str(preferences.display.show_pr_metadata)
    ));
    out.push_str(&format!(
        "show_workspace_details\t{}\n",
        bool_to_str(preferences.display.show_workspace_details)
    ));
    out.push_str(&format!(
        "sidebar_ratio\t{:.4}\n",
        preferences.sidebar_ratio
    ));
    out.push_str(&format!("detail_ratio\t{:.4}\n", preferences.detail_ratio));
    for (path, profile) in &preferences.release_profiles {
        out.push_str("release_profile\t");
        out.push_str(&path.to_string_lossy());
        out.push('\t');
        out.push_str(&profile.remote);
        out.push('\t');
        out.push_str(&profile.source_branch);
        out.push('\t');
        out.push_str(&profile.target_branch);
        out.push('\n');
    }
    out
}

fn parse_preferences(raw: &str) -> PreferencesState {
    let mut preferences = PreferencesState::default();
    for line in raw.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        match key {
            "theme" => preferences.theme = parse_theme(value),
            "density" => preferences.density = parse_density(value),
            "show_commit_author" => {
                preferences.display.show_commit_author = parse_bool(value);
            }
            "show_file_inspection" => {
                preferences.display.show_file_inspection = parse_bool(value);
            }
            "show_pr_metadata" => {
                preferences.display.show_pr_metadata = parse_bool(value);
            }
            "show_workspace_details" => {
                preferences.display.show_workspace_details = parse_bool(value);
            }
            "sidebar_ratio" => {
                if let Ok(ratio) = value.parse::<f32>() {
                    preferences.sidebar_ratio = ratio.clamp(0.14, 0.36);
                }
            }
            "detail_ratio" => {
                if let Ok(ratio) = value.parse::<f32>() {
                    preferences.detail_ratio = ratio.clamp(0.50, 0.78);
                }
            }
            "release_profile" => {
                let fields = value.split('\t').collect::<Vec<_>>();
                if let [path, remote, source_branch, target_branch] = fields.as_slice() {
                    let path = Path::new(path);
                    if path.is_absolute()
                        && !remote.trim().is_empty()
                        && !source_branch.trim().is_empty()
                        && !target_branch.trim().is_empty()
                    {
                        preferences.release_profiles.insert(
                            path.to_path_buf(),
                            ReleaseProfile {
                                remote: (*remote).to_string(),
                                source_branch: (*source_branch).to_string(),
                                target_branch: (*target_branch).to_string(),
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }
    preferences.display_options_open = false;
    preferences.shortcuts_open = false;
    preferences
}

fn theme_to_str(theme: ThemePreference) -> &'static str {
    match theme {
        ThemePreference::Dark => "dark",
        ThemePreference::HighContrast => "high-contrast",
    }
}

fn parse_theme(value: &str) -> ThemePreference {
    match value {
        "high-contrast" => ThemePreference::HighContrast,
        _ => ThemePreference::Dark,
    }
}

fn density_to_str(density: DensityPreference) -> &'static str {
    match density {
        DensityPreference::Comfortable => "comfortable",
        DensityPreference::Compact => "compact",
        DensityPreference::Dense => "dense",
    }
}

fn parse_density(value: &str) -> DensityPreference {
    match value {
        "comfortable" => DensityPreference::Comfortable,
        "dense" => DensityPreference::Dense,
        _ => DensityPreference::Compact,
    }
}

fn bool_to_str(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes" | "on")
}

fn serialize_repository_catalog(catalog: &RepositoryCatalog) -> String {
    let mut out = String::new();
    for entry in &catalog.entries {
        out.push_str(if entry.favorite { "1" } else { "0" });
        out.push('\t');
        out.push_str(&entry.path.to_string_lossy());
        out.push('\n');
    }
    out
}

fn parse_repository_catalog(raw: &str) -> RepositoryCatalog {
    let mut entries = Vec::new();
    for line in raw.lines() {
        let Some((favorite, path)) = line.split_once('\t') else {
            continue;
        };
        if path.trim().is_empty() {
            continue;
        }
        let path = Path::new(path);
        if !path.is_absolute() {
            continue;
        }

        entries.push(RepositoryEntry {
            path: path.to_path_buf(),
            favorite: favorite == "1",
        });
    }
    RepositoryCatalog { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_catalog_round_trips() {
        let catalog = RepositoryCatalog {
            entries: vec![
                RepositoryEntry {
                    path: PathBuf::from("/tmp/one"),
                    favorite: true,
                },
                RepositoryEntry {
                    path: PathBuf::from("/tmp/two"),
                    favorite: false,
                },
            ],
        };

        let raw = serialize_repository_catalog(&catalog);
        let parsed = parse_repository_catalog(&raw);

        assert_eq!(parsed, catalog);
    }

    #[test]
    fn repository_catalog_parser_skips_malformed_rows() {
        let parsed = parse_repository_catalog("bad\n1\t/tmp/repo\n0\t\n1\t.\n");

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].path, PathBuf::from("/tmp/repo"));
        assert!(parsed.entries[0].favorite);
    }

    #[test]
    fn preferences_round_trip_display_and_pane_state() {
        let mut preferences = PreferencesState {
            theme: ThemePreference::HighContrast,
            density: DensityPreference::Dense,
            sidebar_ratio: 0.24,
            detail_ratio: 0.70,
            ..PreferencesState::default()
        };
        preferences.display.show_commit_author = false;
        preferences.display.show_file_inspection = false;

        let raw = serialize_preferences(&preferences);
        let parsed = parse_preferences(&raw);

        assert_eq!(parsed.theme, ThemePreference::HighContrast);
        assert_eq!(parsed.density, DensityPreference::Dense);
        assert!(!parsed.display.show_commit_author);
        assert!(!parsed.display.show_file_inspection);
        assert!((parsed.sidebar_ratio - 0.24).abs() < f32::EPSILON);
        assert!((parsed.detail_ratio - 0.70).abs() < f32::EPSILON);
        assert!(!parsed.display_options_open);
        assert!(!parsed.shortcuts_open);
    }

    #[test]
    fn preferences_round_trip_release_profiles() {
        let mut preferences = PreferencesState::default();
        preferences.release_profiles.insert(
            PathBuf::from("/tmp/repo"),
            ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            },
        );

        let raw = serialize_preferences(&preferences);
        let parsed = parse_preferences(&raw);

        assert_eq!(
            parsed.release_profiles.get(Path::new("/tmp/repo")),
            Some(&ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            })
        );
    }
}
