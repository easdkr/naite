use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use iced::{keyboard::Modifiers, Point};
use naite_core::{
    BlameLine, BranchSyncStatus, CommitDiff, CommitPageCursor, CommitSummary, FileHistoryEntry,
    GitHubIssueFilter, GitHubIssueSummary, GitOperationState, GraphLayout, HighlightedDiff, Hunk,
    PullRequestFilter, PullRequestSummary, RefKind, RefSummary, Refs, ReleaseProfile,
    ReleaseProfileSuggestion, ReleaseSyncCheck, StashSummary, WorkspaceRepoSummary,
    WorktreeDiffKind, WorktreeDiffTarget, WorktreeStatusDetail, WorktreeSummary,
};

use crate::{
    features::release_prep::ReleasePrepAction,
    features::terminal::{TerminalSessionId, TerminalTarget},
    BranchDeletePrompt, CheckoutPrompt, DiscardPrompt, ForcePushPrompt, ForceSyncPrompt,
    HistoryPrompt, RebasePrompt, ResetPrompt, StashPrompt, TagDeletePrompt, UndoPrompt,
    WorktreeRemovePrompt,
};

pub const RELEASE_PREP_MODAL_ANIMATION_FRAMES: usize = 10;

#[derive(Debug, Clone, Default)]
pub struct RepositoryState {
    pub path: Option<PathBuf>,
    pub commits: Vec<CommitSummary>,
    pub commit_page_cursor: Option<CommitPageCursor>,
    pub commits_loading_more: bool,
    pub refs: Refs,
    pub stashes: Vec<StashSummary>,
    pub worktrees: Vec<WorktreeSummary>,
    pub pull_requests: Vec<PullRequestSummary>,
    pub github_issues: Vec<GitHubIssueSummary>,
    pub head_branch: Option<String>,
    pub sync_status: BranchSyncStatus,
    pub operation_state: GitOperationState,
    pub graph_layout: GraphLayout,
    pub status_detail: WorktreeStatusDetail,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    pub selected: Option<usize>,
    pub selected_commit_id: Option<String>,
    pub selected_wip: bool,
    pub selected_wip_file: Option<WorktreeDiffTarget>,
    pub selected_stash: Option<StashSummary>,
    pub selected_worktree: Option<WorktreeSummary>,
    pub selected_pull_request: Option<PullRequestSummary>,
    pub selected_github_issue: Option<GitHubIssueSummary>,
    pub selected_file: Option<usize>,
    pub selected_hunk: Option<usize>,
    pub diff_view_mode: DiffViewMode,
    pub context_menu: Option<ContextMenuState>,
    pub cursor_position: Option<Point>,
    pub last_sidebar_click: Option<SidebarClickState>,
    pub checkout_confirmation: Option<CheckoutPrompt>,
    pub force_sync_confirmation: Option<ForceSyncPrompt>,
    pub force_push_confirmation: Option<ForcePushPrompt>,
    pub branch_delete_confirmation: Option<BranchDeletePrompt>,
    pub discard_confirmation: Option<DiscardPrompt>,
    pub stash_confirmation: Option<StashPrompt>,
    pub history_confirmation: Option<HistoryPrompt>,
    pub rebase_confirmation: Option<RebasePrompt>,
    pub reset_confirmation: Option<ResetPrompt>,
    pub tag_delete_confirmation: Option<TagDeletePrompt>,
    pub undo_confirmation: Option<UndoPrompt>,
    pub worktree_remove_confirmation: Option<WorktreeRemovePrompt>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub kind: ContextMenuKind,
    pub position: Point,
}

#[derive(Debug, Clone)]
pub enum ContextMenuKind {
    Ref(RefSummary),
    LocalBranchFolder {
        label: String,
        branches: Vec<RefSummary>,
    },
    RemoteBranchFolder {
        label: String,
        branches: Vec<RefSummary>,
    },
    Stash(StashSummary),
    Worktree(WorktreeSummary),
    Commit(CommitSummary),
    WipFile(WorktreeDiffTarget),
    PullRequest(PullRequestSummary),
    GitHubIssue(GitHubIssueSummary),
    CommitFile {
        path: String,
    },
    HunkHeader {
        path: String,
        hunk: Hunk,
        kind: WorktreeDiffKind,
    },
    RecentRepo(PathBuf),
    PushMenu {
        force_with_lease_available: bool,
    },
    StashMenu {
        dirty: bool,
        latest_stash: Option<StashSummary>,
    },
}

impl ContextMenuKind {
    pub fn as_ref(&self) -> Option<&RefSummary> {
        match self {
            Self::Ref(ref_summary) => Some(ref_summary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiffViewMode {
    #[default]
    Unified,
    FocusedHunk,
    Inline,
    Split,
}

#[derive(Debug, Clone)]
pub struct SidebarClickState {
    pub ref_summary: RefSummary,
    pub at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct OperationState {
    pub current_diff: Option<CommitDiff>,
    pub current_diff_highlight: Option<HighlightedDiff>,
    pub diff_loading: bool,
    pub diff_error: Option<String>,
    pub pending_diff_commit_id: Option<String>,
    pub pending_wip_diff_target: Option<WorktreeDiffTarget>,
    pub pending_stash_diff_selector: Option<String>,
    pub transient_status: Option<TransientStatus>,
    pub pending_transient_status_after_reload: Option<String>,
    pub pending_error_after_reload: Option<String>,
    pub pending_force_push_after_reload: bool,
    pub error: Option<String>,
    pub loading: bool,
    pub auto_fetch_path: Option<PathBuf>,
    pub auto_fetch_last_started: Option<(PathBuf, Instant)>,
}

#[derive(Debug, Clone)]
pub struct TransientStatus {
    pub message: String,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct RepositoryManagerState {
    pub clone_url: String,
    pub clone_open: bool,
    pub new_repo_menu_open: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CommitFormState {
    pub title: String,
    pub body: String,
    pub co_authors: String,
    pub amend: bool,
    pub skip_hooks: bool,
    pub push_after: bool,
}

/// State for the reword editor. `body_content` holds the multi-line
/// editor backing buffer — `text_editor::Content` is not `Clone`/`Eq`
/// in iced 0.13 so this struct cannot carry those traits either.
#[derive(Debug, Default)]
pub struct HistoryRewordState {
    pub open: bool,
    pub loading: bool,
    pub commit: Option<CommitSummary>,
    pub title: String,
    pub body_content: iced::widget::text_editor::Content,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagCreateState {
    pub open: bool,
    pub target_commit: Option<CommitSummary>,
    pub name: String,
    pub name_mode: TagNameMode,
    pub push_after_create: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TagNameMode {
    #[default]
    Timestamp,
    SemVerNext,
    BranchSlug,
}

impl TagNameMode {
    pub const ALL: [Self; 3] = [Self::Timestamp, Self::SemVerNext, Self::BranchSlug];

    pub fn label(self) -> &'static str {
        match self {
            Self::Timestamp => "Timestamp",
            Self::SemVerNext => "SemVer next",
            Self::BranchSlug => "Branch slug",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileInsightMode {
    #[default]
    History,
    Blame,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInsightState {
    pub path: Option<String>,
    pub mode: FileInsightMode,
    pub loading: bool,
    pub error: Option<String>,
    pub history: Vec<FileHistoryEntry>,
    pub blame: Vec<BlameLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoCheckpoint {
    pub label: String,
    pub head_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThemePreference {
    #[default]
    Dark,
    HighContrast,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DensityPreference {
    Comfortable,
    #[default]
    Compact,
    Dense,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayOptionsState {
    pub show_commit_author: bool,
    pub show_file_inspection: bool,
    pub show_pr_metadata: bool,
    pub show_workspace_details: bool,
}

impl Default for DisplayOptionsState {
    fn default() -> Self {
        Self {
            show_commit_author: true,
            show_file_inspection: true,
            show_pr_metadata: true,
            show_workspace_details: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesState {
    pub theme: ThemePreference,
    pub density: DensityPreference,
    pub display: DisplayOptionsState,
    pub release_profiles: HashMap<PathBuf, ReleaseProfile>,
    pub display_options_open: bool,
    pub shortcuts_open: bool,
    pub sidebar_ratio: f32,
    pub detail_ratio: f32,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            density: DensityPreference::Compact,
            display: DisplayOptionsState::default(),
            release_profiles: HashMap::new(),
            display_options_open: false,
            shortcuts_open: false,
            sidebar_ratio: 0.20,
            detail_ratio: 0.66,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleasePrepState {
    pub phase: ReleasePrepPhase,
    pub remote: String,
    pub source_branch: String,
    pub target_branch: String,
    pub validation_script: String,
    pub backup_before_rebase: bool,
    /// Set by `ConfigureRequested` so the next `SuggestionLoaded` prefills
    /// the config form from the saved profile instead of fast-starting.
    pub force_config: bool,
    pub animation_frame: usize,
    pub error: Option<String>,
    pub suggestion: Option<ReleaseProfileSuggestion>,
    pub sync_check: Option<ReleaseSyncCheck>,
    pub active_profile: Option<ReleaseProfile>,
    pub auto_running: bool,
    pub auto_next_action: Option<ReleasePrepAction>,
    pub active_action: Option<ReleasePrepAction>,
    pub completed_actions: Vec<ReleasePrepAction>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReleasePrepPhase {
    #[default]
    Idle,
    Configuring,
    Preparing,
    Actions,
    RunningAction,
}

impl ReleasePrepState {
    pub fn profile_from_inputs(&self) -> ReleaseProfile {
        let validation_script = self.validation_script.trim();
        ReleaseProfile {
            remote: self.remote.trim().to_string(),
            source_branch: self.source_branch.trim().to_string(),
            target_branch: self.target_branch.trim().to_string(),
            validation_script: (!validation_script.is_empty())
                .then(|| validation_script.to_string()),
        }
    }
}

impl PreferencesState {
    pub fn panel_padding(&self) -> u16 {
        match self.density {
            DensityPreference::Comfortable => 18,
            DensityPreference::Compact => 16,
            DensityPreference::Dense => 12,
        }
    }

    pub fn row_padding(&self) -> u16 {
        match self.density {
            DensityPreference::Comfortable => 14,
            DensityPreference::Compact => 12,
            DensityPreference::Dense => 8,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StashCreateState {
    pub open: bool,
    pub message: String,
    pub include_untracked: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StashBranchState {
    pub open: bool,
    pub name: String,
    pub stash: Option<StashSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchCreateBase {
    Head {
        label: String,
    },
    Commit {
        id: String,
        short_id: String,
        summary: String,
    },
}

impl BranchCreateBase {
    pub fn start_point(&self) -> Option<&str> {
        match self {
            Self::Head { .. } => None,
            Self::Commit { id, .. } => Some(id.as_str()),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Head { label } => label.clone(),
            Self::Commit {
                short_id, summary, ..
            } => format!("{short_id} {summary}"),
        }
    }
}

impl Default for BranchCreateBase {
    fn default() -> Self {
        Self::Head {
            label: "HEAD".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchCreateState {
    pub open: bool,
    pub name: String,
    pub base: BranchCreateBase,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchManageRenameState {
    pub open: bool,
    pub target: Option<RefSummary>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryEntry {
    pub path: PathBuf,
    pub favorite: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryCatalog {
    pub entries: Vec<RepositoryEntry>,
}

impl RepositoryCatalog {
    pub fn remember(&mut self, path: PathBuf) {
        let favorite = self.is_favorite(&path);
        self.entries.retain(|entry| entry.path != path);
        self.entries.insert(0, RepositoryEntry { path, favorite });
        self.entries.truncate(20);
    }

    pub fn toggle_favorite(&mut self, path: PathBuf) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path) {
            entry.favorite = !entry.favorite;
        } else {
            self.entries.insert(
                0,
                RepositoryEntry {
                    path,
                    favorite: true,
                },
            );
        }
    }

    pub fn remove_favorite(&mut self, path: &Path) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path) {
            entry.favorite = false;
        }
    }

    pub fn remove_entry(&mut self, path: &Path) {
        self.entries.retain(|entry| entry.path != path);
    }

    pub fn is_favorite(&self, path: &Path) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.favorite)
            .unwrap_or(false)
    }
}

pub const MAX_OPEN_TABS: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct RepositoryTabsState {
    pub open: Vec<PathBuf>,
    pub active: Option<PathBuf>,
    pub cache: HashMap<PathBuf, RepositoryState>,
    pub refreshing: HashSet<PathBuf>,
    pub last_refreshed: HashMap<PathBuf, Instant>,
}

impl RepositoryTabsState {
    /// Move `path` to the front of the tab list and mark it active.
    /// Returns the path evicted by the LRU cap, if any, so callers can drop
    /// the evicted tab's cached state outside this struct as well.
    pub fn remember(&mut self, path: PathBuf) -> Option<PathBuf> {
        self.open.retain(|candidate| candidate != &path);
        self.open.insert(0, path.clone());
        let evicted = if self.open.len() > MAX_OPEN_TABS {
            self.open.pop()
        } else {
            None
        };
        self.active = Some(path);
        if let Some(ref evicted) = evicted {
            self.forget_cached(evicted);
        }
        evicted
    }

    /// Remove a tab. If the removed tab was active, picks an adjacent tab as
    /// the new active (next preferred, then previous, then None). Returns the
    /// new active path so the caller can swap in cached state.
    pub fn close(&mut self, path: &Path) -> CloseOutcome {
        let was_active = self.active.as_deref() == Some(path);
        let removed_index = self.open.iter().position(|candidate| candidate == path);
        self.open.retain(|candidate| candidate != path);
        self.forget_cached(path);

        let new_active = if was_active {
            removed_index.and_then(|idx| {
                self.open
                    .get(idx)
                    .cloned()
                    .or_else(|| self.open.last().cloned())
            })
        } else {
            self.active.clone()
        };
        self.active = new_active.clone();

        CloseOutcome {
            was_active,
            new_active,
        }
    }

    fn forget_cached(&mut self, path: &Path) {
        self.cache.remove(path);
        self.refreshing.remove(path);
        self.last_refreshed.remove(path);
    }

    pub fn is_refreshing(&self, path: &Path) -> bool {
        self.refreshing.contains(path)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CloseOutcome {
    pub was_active: bool,
    pub new_active: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceState {
    pub dashboard_open: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub summaries: Vec<WorkspaceRepoSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestsState {
    pub loading: bool,
    pub filter: PullRequestFilter,
    pub last_non_search_filter: PullRequestFilter,
    pub search_query: String,
    pub error: Option<String>,
    pub create: PullRequestCreateState,
    pub checkout_worktree: PullRequestWorktreeCheckoutState,
}

impl Default for PullRequestsState {
    fn default() -> Self {
        Self {
            loading: false,
            filter: PullRequestFilter::All,
            last_non_search_filter: PullRequestFilter::All,
            search_query: String::new(),
            error: None,
            create: PullRequestCreateState::default(),
            checkout_worktree: PullRequestWorktreeCheckoutState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIssuesState {
    pub loading: bool,
    pub filter: GitHubIssueFilter,
    pub last_non_search_filter: GitHubIssueFilter,
    pub search_query: String,
    pub error: Option<String>,
}

impl Default for GitHubIssuesState {
    fn default() -> Self {
        Self {
            loading: false,
            filter: GitHubIssueFilter::Open,
            last_non_search_filter: GitHubIssueFilter::Open,
            search_query: String::new(),
            error: None,
        }
    }
}

/// Session-scoped cache of fetched avatar bitmaps keyed by source URL.
/// Tracks in-flight URLs to avoid duplicate fetches, and permanently failed
/// URLs to avoid hot-looping on broken responses.
#[derive(Debug, Clone, Default)]
pub struct AvatarCache {
    pub handles: HashMap<String, iced::widget::image::Handle>,
    pub in_flight: HashSet<String>,
    pub failed: HashSet<String>,
}

impl AvatarCache {
    pub fn needs_fetch(&self, url: &str) -> bool {
        !self.handles.contains_key(url)
            && !self.in_flight.contains(url)
            && !self.failed.contains(url)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestCreateState {
    pub open: bool,
    pub base_branch: String,
    pub draft: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestWorktreeCheckoutState {
    pub open: bool,
    pub pull_request: Option<PullRequestSummary>,
    pub path: String,
    pub branch_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorktreeCreateState {
    pub open: bool,
    pub path: String,
    pub start_point: String,
    pub new_branch: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalState {
    pub open: bool,
    pub runtime_ready: bool,
    pub sessions: Vec<TerminalSession>,
    pub active: Option<TerminalSessionId>,
    pub next_session_id: u64,
    pub pointer_grid_position: Option<TerminalGridPoint>,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSession {
    pub id: TerminalSessionId,
    pub target: TerminalTarget,
    pub label: String,
    pub screen: TerminalScreen,
    pub status: TerminalStatus,
    pub minimized: bool,
    pub shell: String,
    pub title: Option<String>,
    pub last_exit: Option<i32>,
    pub cols: u16,
    pub rows: u16,
    pub pending_start: bool,
    pub error: Option<String>,
    pub shell_kind: crate::features::terminal::zsh_integration::ShellKind,
    pub integration_status: IntegrationStatus,
    pub shell_cwd: Option<std::path::PathBuf>,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub last_command: Option<String>,
    pub last_exit_code: Option<i32>,
    pub shell_history: Vec<String>,
    pub active_suggestion: Option<crate::features::terminal::suggestion::ActiveSuggestion>,
    pub selection: Option<TerminalSelection>,
    pub ime_preedit: Option<TerminalImePreedit>,
    pub ime_modified_delete_pending: Option<TerminalImeDeleteAction>,
    pub ime_suppressed_commit: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalImePreedit {
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalImeDeleteAction {
    KillLine,
    KillWord,
}

#[allow(dead_code)]
pub const SHELL_HISTORY_CAP: usize = 500;

impl TerminalSession {
    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let (start, end) = selection.normalized();
        if start == end {
            return None;
        }

        let mut selected = Vec::new();
        for row in start.row..=end.row {
            let Some(line) = self.screen.lines.get(row) else {
                continue;
            };
            let chars: Vec<char> = line.text().chars().collect();
            let row_start = if row == start.row { start.col } else { 0 };
            let row_end = if row == end.row { end.col } else { chars.len() };
            let row_start = row_start.min(chars.len());
            let row_end = row_end.min(chars.len());
            if row_start <= row_end {
                selected.push(chars[row_start..row_end].iter().collect::<String>());
            }
        }

        (!selected.is_empty()).then(|| selected.join("\n"))
    }

    #[allow(dead_code)]
    pub fn push_history_capped(&mut self, entries: Vec<String>) {
        for entry in entries {
            if self.shell_history.len() >= SHELL_HISTORY_CAP {
                self.shell_history.remove(0);
            }
            self.shell_history.push(entry);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalGridPoint {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSelection {
    pub anchor: TerminalGridPoint,
    pub focus: TerminalGridPoint,
    pub active: bool,
}

impl TerminalSelection {
    pub fn normalized(&self) -> (TerminalGridPoint, TerminalGridPoint) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScreen {
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<TerminalLine>,
    pub cursor: Option<TerminalCursor>,
    pub scrollback_len: usize,
}

impl Default for TerminalScreen {
    fn default() -> Self {
        Self {
            cols: 100,
            rows: 24,
            lines: Vec::new(),
            cursor: None,
            scrollback_len: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalLine {
    pub cells: Vec<TerminalCell>,
}

impl TerminalLine {
    pub fn text(&self) -> String {
        let mut text: String = self.visible_chars().into_iter().collect();
        while text.ends_with(' ') {
            text.pop();
        }
        text
    }

    /// Characters that should be rendered for this line. Wide-character
    /// spacer cells are skipped so multi-cell glyphs (e.g. Hangul) render
    /// at their natural width instead of being followed by phantom spaces.
    pub fn visible_chars(&self) -> Vec<char> {
        self.cells
            .iter()
            .filter(|cell| !cell.spacer && !cell.hidden)
            .filter_map(|cell| terminal_display_char(cell.ch))
            .collect()
    }

    /// Map an alacritty cell column to the index inside `visible_chars()`,
    /// so the cursor lands on the right glyph after spacer cells are dropped.
    pub fn cell_col_to_char_idx(&self, cell_col: usize) -> usize {
        let mut idx = 0;
        for (i, cell) in self.cells.iter().enumerate() {
            if i >= cell_col {
                return idx;
            }
            if !cell.spacer && !cell.hidden && terminal_display_char(cell.ch).is_some() {
                idx += 1;
            }
        }
        idx
    }
}

fn terminal_display_char(ch: char) -> Option<char> {
    match ch as u32 {
        // Variation selectors are often emitted after emoji presentation
        // symbols. The fixed terminal font renders them as tofu boxes, so drop
        // them from the display stream.
        0xfe00..=0xfe0f | 0xe0100..=0xe01ef => None,
        // Many watch-mode tools prefix logs with emoji status markers. The
        // terminal font cannot render large emoji ranges reliably, so use a
        // narrow ASCII marker instead of showing replacement boxes.
        0x1f000..=0x1faff => Some('*'),
        _ => Some(ch),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCell {
    pub ch: char,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub hidden: bool,
    /// Trailing half of a wide character. The previous cell holds the glyph;
    /// this cell should be skipped during rendering.
    pub spacer: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
            hidden: false,
            spacer: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursor {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalStatus {
    #[default]
    Idle,
    Starting,
    Running,
    Exited,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IntegrationStatus {
    #[default]
    Disabled,
    Pending,
    Ready,
    Failed(String),
}

impl TerminalState {
    pub fn ensure_session(&mut self, cwd: PathBuf, label: String) -> TerminalSessionId {
        if let Some(session) = self
            .sessions
            .iter()
            .find(|session| session.target.cwd == cwd)
        {
            self.active = Some(session.id);
            return session.id;
        }
        self.create_session(
            TerminalTarget::new(cwd, None, None),
            label,
            default_terminal_shell(),
            100,
            24,
        )
    }

    pub fn create_session(
        &mut self,
        target: TerminalTarget,
        label: String,
        shell: String,
        cols: u16,
        rows: u16,
    ) -> TerminalSessionId {
        self.next_session_id += 1;
        let id = TerminalSessionId(self.next_session_id);
        self.sessions.push(TerminalSession {
            id,
            target,
            label,
            screen: TerminalScreen {
                cols,
                rows,
                ..Default::default()
            },
            status: TerminalStatus::Idle,
            minimized: false,
            shell: shell.clone(),
            title: None,
            last_exit: None,
            cols,
            rows,
            pending_start: false,
            error: None,
            shell_kind: crate::features::terminal::zsh_integration::detect_shell_kind(&shell),
            integration_status: IntegrationStatus::default(),
            shell_cwd: None,
            input_buffer: String::new(),
            input_cursor: 0,
            last_command: None,
            last_exit_code: None,
            shell_history: Vec::new(),
            active_suggestion: None,
            selection: None,
            ime_preedit: None,
            ime_modified_delete_pending: None,
            ime_suppressed_commit: None,
        });
        self.active = Some(id);
        id
    }

    pub fn active_session(&self) -> Option<&TerminalSession> {
        let active = self.active?;
        self.session(active)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut TerminalSession> {
        let active = self.active?;
        self.session_mut(active)
    }

    pub fn session(&self, id: TerminalSessionId) -> Option<&TerminalSession> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn session_mut(&mut self, id: TerminalSessionId) -> Option<&mut TerminalSession> {
        self.sessions.iter_mut().find(|session| session.id == id)
    }

    pub fn remove_session(&mut self, id: TerminalSessionId) {
        self.sessions.retain(|session| session.id != id);
        if self.active == Some(id) {
            self.active = self.sessions.last().map(|session| session.id);
        }
    }

    pub fn captures_keyboard(&self) -> bool {
        self.open
            && self.active_session().is_some_and(|session| {
                !session.minimized && session.status != TerminalStatus::Exited
            })
    }
}

pub fn default_terminal_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    RecentRepositories,
    LocalBranches,
    RemoteBranches,
    PullRequests,
    Issues,
    Tags,
    Stashes,
    Worktrees,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarState {
    pub recent_repositories_expanded: bool,
    pub local_branches_expanded: bool,
    pub remote_branches_expanded: bool,
    pub pull_requests_expanded: bool,
    pub issues_expanded: bool,
    pub tags_expanded: bool,
    pub stashes_expanded: bool,
    pub worktrees_expanded: bool,
    pub hovered_ref: Option<SidebarRefKey>,
    collapsed_tree_folders: BTreeSet<String>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            recent_repositories_expanded: false,
            local_branches_expanded: true,
            remote_branches_expanded: false,
            pull_requests_expanded: false,
            issues_expanded: false,
            tags_expanded: false,
            stashes_expanded: false,
            worktrees_expanded: false,
            hovered_ref: None,
            collapsed_tree_folders: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarRefKey {
    pub kind: RefKind,
    pub full_name: String,
}

impl SidebarRefKey {
    pub fn new(ref_summary: &RefSummary) -> Self {
        Self {
            kind: ref_summary.kind,
            full_name: ref_summary.full_name.clone(),
        }
    }
}

impl SidebarState {
    pub fn is_expanded(&self, section: SidebarSection) -> bool {
        match section {
            SidebarSection::RecentRepositories => self.recent_repositories_expanded,
            SidebarSection::LocalBranches => self.local_branches_expanded,
            SidebarSection::RemoteBranches => self.remote_branches_expanded,
            SidebarSection::PullRequests => self.pull_requests_expanded,
            SidebarSection::Issues => self.issues_expanded,
            SidebarSection::Tags => self.tags_expanded,
            SidebarSection::Stashes => self.stashes_expanded,
            SidebarSection::Worktrees => self.worktrees_expanded,
        }
    }

    pub fn toggle(&mut self, section: SidebarSection) {
        match section {
            SidebarSection::RecentRepositories => {
                self.recent_repositories_expanded = !self.recent_repositories_expanded;
            }
            SidebarSection::LocalBranches => {
                self.local_branches_expanded = !self.local_branches_expanded;
            }
            SidebarSection::RemoteBranches => {
                self.remote_branches_expanded = !self.remote_branches_expanded;
            }
            SidebarSection::PullRequests => {
                self.pull_requests_expanded = !self.pull_requests_expanded;
            }
            SidebarSection::Issues => {
                self.issues_expanded = !self.issues_expanded;
            }
            SidebarSection::Tags => {
                self.tags_expanded = !self.tags_expanded;
            }
            SidebarSection::Stashes => {
                self.stashes_expanded = !self.stashes_expanded;
            }
            SidebarSection::Worktrees => {
                self.worktrees_expanded = !self.worktrees_expanded;
            }
        }
    }

    pub fn is_tree_folder_expanded(&self, section: SidebarSection, path: &str) -> bool {
        !self
            .collapsed_tree_folders
            .contains(&tree_folder_key(section, path))
    }

    pub fn toggle_tree_folder(&mut self, section: SidebarSection, path: String) {
        let key = tree_folder_key(section, &path);
        if !self.collapsed_tree_folders.remove(&key) {
            self.collapsed_tree_folders.insert(key);
        }
    }

    pub fn hover_ref(&mut self, ref_summary: &RefSummary) {
        self.hovered_ref = Some(SidebarRefKey::new(ref_summary));
    }

    pub fn clear_hovered_ref(&mut self, ref_summary: &RefSummary) {
        if self.is_ref_hovered(ref_summary) {
            self.hovered_ref = None;
        }
    }

    pub fn is_ref_hovered(&self, ref_summary: &RefSummary) -> bool {
        self.hovered_ref.as_ref().is_some_and(|key| {
            key.kind == ref_summary.kind && key.full_name == ref_summary.full_name
        })
    }
}

fn tree_folder_key(section: SidebarSection, path: &str) -> String {
    let prefix = match section {
        SidebarSection::RecentRepositories => "recent",
        SidebarSection::LocalBranches => "local",
        SidebarSection::RemoteBranches => "remote",
        SidebarSection::PullRequests => "pull-requests",
        SidebarSection::Issues => "issues",
        SidebarSection::Tags => "tags",
        SidebarSection::Stashes => "stashes",
        SidebarSection::Worktrees => "worktrees",
    };
    format!("{prefix}:{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_moves_existing_entry_to_front_without_losing_favorite() {
        let mut catalog = RepositoryCatalog::default();
        catalog.remember(PathBuf::from("/tmp/one"));
        catalog.toggle_favorite(PathBuf::from("/tmp/one"));
        catalog.remember(PathBuf::from("/tmp/two"));
        catalog.remember(PathBuf::from("/tmp/one"));

        assert_eq!(catalog.entries[0].path, PathBuf::from("/tmp/one"));
        assert!(catalog.entries[0].favorite);
        assert_eq!(catalog.entries.len(), 2);
    }

    #[test]
    fn toggle_favorite_adds_missing_path() {
        let mut catalog = RepositoryCatalog::default();

        catalog.toggle_favorite(PathBuf::from("/tmp/repo"));

        assert_eq!(catalog.entries.len(), 1);
        assert!(catalog.entries[0].favorite);
    }

    #[test]
    fn remove_favorite_keeps_recent_entry() {
        let mut catalog = RepositoryCatalog::default();
        catalog.remember(PathBuf::from("/tmp/repo"));
        catalog.toggle_favorite(PathBuf::from("/tmp/repo"));

        catalog.remove_favorite(Path::new("/tmp/repo"));

        assert_eq!(catalog.entries.len(), 1);
        assert!(!catalog.entries[0].favorite);
    }

    #[test]
    fn remove_entry_deletes_recent_and_favorite_state() {
        let mut catalog = RepositoryCatalog::default();
        catalog.remember(PathBuf::from("/tmp/repo"));
        catalog.toggle_favorite(PathBuf::from("/tmp/repo"));

        catalog.remove_entry(Path::new("/tmp/repo"));

        assert!(catalog.entries.is_empty());
    }

    #[test]
    fn repository_tabs_move_active_repo_to_front() {
        let mut tabs = RepositoryTabsState::default();
        tabs.remember(PathBuf::from("/tmp/one"));
        tabs.remember(PathBuf::from("/tmp/two"));
        tabs.remember(PathBuf::from("/tmp/one"));

        assert_eq!(
            tabs.open,
            vec![PathBuf::from("/tmp/one"), PathBuf::from("/tmp/two")]
        );
        assert_eq!(tabs.active, Some(PathBuf::from("/tmp/one")));
    }

    #[test]
    fn sidebar_sections_default_to_local_only_expanded_and_toggle_independently() {
        let mut state = SidebarState::default();

        assert!(!state.is_expanded(SidebarSection::RecentRepositories));
        assert!(state.is_expanded(SidebarSection::LocalBranches));
        assert!(!state.is_expanded(SidebarSection::RemoteBranches));
        assert!(!state.is_expanded(SidebarSection::PullRequests));
        assert!(!state.is_expanded(SidebarSection::Issues));
        assert!(!state.is_expanded(SidebarSection::Tags));
        assert!(!state.is_expanded(SidebarSection::Stashes));
        assert!(!state.is_expanded(SidebarSection::Worktrees));

        state.toggle(SidebarSection::RemoteBranches);
        state.toggle(SidebarSection::RecentRepositories);

        assert!(state.is_expanded(SidebarSection::RecentRepositories));
        assert!(state.is_expanded(SidebarSection::LocalBranches));
        assert!(state.is_expanded(SidebarSection::RemoteBranches));
        assert!(!state.is_expanded(SidebarSection::Worktrees));
    }

    #[test]
    fn sidebar_tree_folders_start_expanded_and_toggle_by_section_path() {
        let mut state = SidebarState::default();

        assert!(state.is_tree_folder_expanded(SidebarSection::LocalBranches, "feature"));

        state.toggle_tree_folder(SidebarSection::LocalBranches, "feature".into());

        assert!(!state.is_tree_folder_expanded(SidebarSection::LocalBranches, "feature"));
        assert!(state.is_tree_folder_expanded(SidebarSection::RemoteBranches, "feature"));

        state.toggle_tree_folder(SidebarSection::LocalBranches, "feature".into());

        assert!(state.is_tree_folder_expanded(SidebarSection::LocalBranches, "feature"));
    }
}
