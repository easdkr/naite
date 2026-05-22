use std::collections::HashMap;

pub(crate) mod commit;
pub(crate) mod parser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub status: ChangeStatus,
    pub old_path: Option<String>,
    pub is_binary: bool,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Ctx(String),
    Add(String),
    Del(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitDiff {
    pub files: Vec<FileChange>,
    pub hunks_by_file: HashMap<String, Vec<Hunk>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum HunkPatchMode {
    Normal,
    NewFile,
}

pub(crate) fn hunk_patch(path: &str, hunk: &Hunk, mode: HunkPatchMode) -> String {
    let mut patch = match mode {
        HunkPatchMode::Normal => format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{}\n",
            hunk.header
        ),
        HunkPatchMode::NewFile => format!(
            "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n{}\n",
            hunk.header
        ),
    };

    for line in &hunk.lines {
        match line {
            DiffLine::Ctx(value) => {
                patch.push(' ');
                patch.push_str(value);
            }
            DiffLine::Add(value) => {
                patch.push('+');
                patch.push_str(value);
            }
            DiffLine::Del(value) => {
                patch.push('-');
                patch.push_str(value);
            }
        }
        patch.push('\n');
    }

    patch
}
