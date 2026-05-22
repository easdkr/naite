use std::collections::{HashMap, HashSet};

use crate::diff::{ChangeStatus, CommitDiff, DiffLine, FileChange, Hunk};
use crate::text::compose_hangul;

pub(crate) fn diff_from_outputs(name_status: &str, patch: &str) -> CommitDiff {
    let mut files = parse_name_status(name_status);
    let (mut hunks_by_file, binary_files) = parse_unified_diff(patch);
    let truncated_files = truncate_large_hunks(&mut hunks_by_file);

    for file in &mut files {
        file.is_binary = binary_files.contains(&file.path);
        file.is_truncated = truncated_files.contains(&file.path);
    }

    CommitDiff {
        files,
        hunks_by_file,
    }
}

fn parse_name_status(output: &str) -> Vec<FileChange> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let status = parts.next()?;
            let first = parts.next()?.to_string();
            let status_code = status.chars().next()?;

            let (path, old_path, status) = match status_code {
                'A' => (first, None, ChangeStatus::Added),
                'D' => (first, None, ChangeStatus::Deleted),
                'R' => {
                    let new_path = parts.next()?.to_string();
                    (new_path, Some(first), ChangeStatus::Renamed)
                }
                'C' => {
                    let new_path = parts.next()?.to_string();
                    (new_path, Some(first), ChangeStatus::Copied)
                }
                _ => (first, None, ChangeStatus::Modified),
            };

            Some(FileChange {
                path,
                status,
                old_path,
                is_binary: false,
                is_truncated: false,
            })
        })
        .collect()
}

fn parse_unified_diff(output: &str) -> (HashMap<String, Vec<Hunk>>, HashSet<String>) {
    let mut hunks_by_file: HashMap<String, Vec<Hunk>> = HashMap::new();
    let mut binary_files = HashSet::new();
    let mut current_file: Option<String> = None;
    let mut old_file: Option<String> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in output.lines() {
        if line.starts_with("diff --git ") {
            flush_hunk(&mut hunks_by_file, &current_file, &mut current_hunk);
            let paths = parse_diff_git_paths(line);
            old_file = paths.as_ref().and_then(|(old_path, _)| old_path.clone());
            current_file = paths.and_then(|(old_path, new_path)| new_path.or(old_path));
            continue;
        }

        if let Some(raw_path) = line.strip_prefix("--- ") {
            old_file = parse_patch_path(raw_path);
            continue;
        }

        if let Some(raw_path) = line.strip_prefix("+++ ") {
            current_file = parse_patch_path(raw_path).or_else(|| old_file.clone());
            continue;
        }

        if line.starts_with("Binary files ") {
            if let Some(path) = current_file
                .clone()
                .or_else(|| old_file.clone())
                .or_else(|| parse_binary_diff_path(line))
            {
                binary_files.insert(path.clone());
            }
            continue;
        }

        if line.starts_with("@@ ") {
            flush_hunk(&mut hunks_by_file, &current_file, &mut current_hunk);
            current_hunk = parse_hunk_header(line);
            continue;
        }

        if let Some(hunk) = &mut current_hunk {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }

            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine::Add(compose_hangul(rest)));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine::Del(compose_hangul(rest)));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine::Ctx(compose_hangul(rest)));
            }
        }
    }

    flush_hunk(&mut hunks_by_file, &current_file, &mut current_hunk);
    (hunks_by_file, binary_files)
}

fn flush_hunk(
    hunks_by_file: &mut HashMap<String, Vec<Hunk>>,
    current_file: &Option<String>,
    current_hunk: &mut Option<Hunk>,
) {
    if let (Some(file), Some(hunk)) = (current_file, current_hunk.take()) {
        hunks_by_file.entry(file.clone()).or_default().push(hunk);
    }
}

fn parse_patch_path(raw: &str) -> Option<String> {
    if raw == "/dev/null" {
        return None;
    }

    Some(
        raw.strip_prefix("a/")
            .or_else(|| raw.strip_prefix("b/"))
            .unwrap_or(raw)
            .to_string(),
    )
}

fn parse_diff_git_paths(line: &str) -> Option<(Option<String>, Option<String>)> {
    let raw = line.strip_prefix("diff --git ")?;
    let old_start = raw.strip_prefix("a/")?;
    let (old_path, new_path) = old_start.split_once(" b/")?;
    Some((parse_header_path(old_path), parse_header_path(new_path)))
}

fn parse_header_path(raw: &str) -> Option<String> {
    if raw == "/dev/null" {
        None
    } else {
        Some(raw.to_string())
    }
}

fn parse_binary_diff_path(line: &str) -> Option<String> {
    let raw = line
        .strip_prefix("Binary files ")?
        .strip_suffix(" differ")?;

    if let Some((old_path, new_path)) = raw.rsplit_once(" and b/") {
        let new_path = format!("b/{new_path}");
        return parse_patch_path(&new_path).or_else(|| parse_patch_path(old_path));
    }

    if let Some((old_path, _)) = raw.rsplit_once(" and /dev/null") {
        return parse_patch_path(old_path);
    }

    let (old_path, new_path) = raw.rsplit_once(" and ")?;

    parse_patch_path(new_path).or_else(|| parse_patch_path(old_path))
}

fn parse_hunk_header(line: &str) -> Option<Hunk> {
    let mut parts = line.split_whitespace();
    let _at = parts.next()?;
    let old = parts.next()?;
    let new = parts.next()?;

    let (old_start, old_lines) = parse_range(old.strip_prefix('-')?);
    let (new_start, new_lines) = parse_range(new.strip_prefix('+')?);

    Some(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        header: line.to_string(),
        lines: Vec::new(),
    })
}

fn parse_range(raw: &str) -> (u32, u32) {
    let mut parts = raw.split(',');
    let start = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    let lines = parts.next().and_then(|n| n.parse().ok()).unwrap_or(1);
    (start, lines)
}

fn truncate_large_hunks(hunks_by_file: &mut HashMap<String, Vec<Hunk>>) -> HashSet<String> {
    const MAX_DIFF_BYTES: usize = 50 * 1024;

    let mut truncated = HashSet::new();
    for (path, hunks) in hunks_by_file.iter_mut() {
        let mut bytes = 0usize;
        for hunk in hunks.iter_mut() {
            let mut keep = Vec::new();
            for line in hunk.lines.drain(..) {
                bytes += diff_line_len(&line);
                if bytes <= MAX_DIFF_BYTES {
                    keep.push(line);
                } else {
                    truncated.insert(path.clone());
                    break;
                }
            }
            hunk.lines = keep;
            if truncated.contains(path) {
                break;
            }
        }
    }
    truncated
}

fn diff_line_len(line: &DiffLine) -> usize {
    match line {
        DiffLine::Ctx(s) | DiffLine::Add(s) | DiffLine::Del(s) => s.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_status_handles_renames() {
        let files = parse_name_status("R100\told.txt\tnew.txt\nA\tadded.txt\n");

        assert_eq!(files[0].status, ChangeStatus::Renamed);
        assert_eq!(files[0].old_path.as_deref(), Some("old.txt"));
        assert_eq!(files[0].path, "new.txt");
        assert_eq!(files[1].status, ChangeStatus::Added);
    }

    #[test]
    fn parse_unified_diff_extracts_hunks() {
        let patch = "\
diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
 old ctx
-old
+new
";

        let (hunks, binary) = parse_unified_diff(patch);

        assert!(binary.is_empty());
        let hunk = &hunks["file.txt"][0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.lines.len(), 3);
    }

    #[test]
    fn parse_unified_diff_preserves_paths_with_spaces() {
        let patch = "\
diff --git a/dir/old name.txt b/dir/new name.txt
similarity index 88%
rename from dir/old name.txt
rename to dir/new name.txt
--- a/dir/old name.txt
+++ b/dir/new name.txt
@@ -1 +1 @@
-old
+new
";

        let (hunks, binary) = parse_unified_diff(patch);

        assert!(binary.is_empty());
        assert!(hunks.contains_key("dir/new name.txt"));
    }

    #[test]
    fn parse_unified_diff_uses_old_path_for_deleted_files() {
        let patch = "\
diff --git a/deleted name.txt b/deleted name.txt
deleted file mode 100644
--- a/deleted name.txt
+++ /dev/null
@@ -1 +0,0 @@
-old
";

        let (hunks, binary) = parse_unified_diff(patch);

        assert!(binary.is_empty());
        assert!(hunks.contains_key("deleted name.txt"));
    }

    #[test]
    fn parse_unified_diff_marks_binary_paths_with_spaces() {
        let patch = "\
diff --git a/assets/old icon.bin b/assets/new icon.bin
Binary files a/assets/old icon.bin and b/assets/new icon.bin differ
";

        let (_hunks, binary) = parse_unified_diff(patch);

        assert!(binary.contains("assets/new icon.bin"));
    }

    #[test]
    fn parse_unified_diff_preserves_trailing_space_paths() {
        let patch = concat!(
            "diff --git a/name  b/name \n",
            "--- a/name \n",
            "+++ b/name \n",
            "@@ -1 +1 @@\n",
            "-old\n",
            "+new\n",
        );

        let (hunks, binary) = parse_unified_diff(patch);

        assert!(binary.is_empty());
        assert!(hunks.contains_key("name "));
    }

    #[test]
    fn parse_unified_diff_marks_binary_paths_containing_and() {
        let patch = "\
diff --git a/assets/old and icon.bin b/assets/new and icon.bin
Binary files a/assets/old and icon.bin and b/assets/new and icon.bin differ
";

        let (_hunks, binary) = parse_unified_diff(patch);

        assert!(binary.contains("assets/new and icon.bin"));
    }

    #[test]
    fn parse_unified_diff_prefers_header_for_binary_paths_with_separator_text() {
        let patch = "\
diff --git a/assets/old.bin b/assets/new and b/icon.bin
Binary files a/assets/old.bin and b/assets/new and b/icon.bin differ
";

        let (_hunks, binary) = parse_unified_diff(patch);

        assert!(binary.contains("assets/new and b/icon.bin"));
    }

    #[test]
    fn parse_diff_git_paths_preserves_trailing_space_paths() {
        let paths = parse_diff_git_paths("diff --git a/old name  b/new name ");

        assert_eq!(
            paths,
            Some((Some("old name ".into()), Some("new name ".into())))
        );
    }

    #[test]
    fn parse_diff_git_paths_preserves_real_a_or_b_prefixes() {
        let paths = parse_diff_git_paths("diff --git a/a/foo.bin b/b/foo.bin");

        assert_eq!(
            paths,
            Some((Some("a/foo.bin".into()), Some("b/foo.bin".into())))
        );
    }

    #[test]
    fn parse_unified_diff_marks_binary_paths_with_real_a_prefix() {
        let patch = "\
diff --git a/a/foo.bin b/a/foo.bin
Binary files a/a/foo.bin and b/a/foo.bin differ
";

        let (_hunks, binary) = parse_unified_diff(patch);

        assert!(binary.contains("a/foo.bin"));
    }
}
