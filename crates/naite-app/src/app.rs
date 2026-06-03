use iced::widget::{pane_grid, scrollable, text_input};
use naite_core::{
    BranchSyncStatus, CommitSummary, Hunk, PullMode, RebaseAction, RefKind, RefSummary,
    StashSummary, WorktreeDiffTarget, WorktreeStatus, WorktreeSummary,
};

use crate::features::fetch::FetchScope;
use crate::features::history;
use crate::features::rebase::{InteractiveRebaseSession, RebaseApplyMode};
use crate::state::{
    AvatarCache, BranchCreateBase, BranchCreateState, BranchManageRenameState, CommandPaletteState,
    CommitFormState, FileInsightState, GitHubIssuesState, HistoryRewordState, OperationState,
    PreferencesState, PullRequestsState, ReleasePrepState, RepositoryCatalog,
    RepositoryManagerState, RepositoryState, RepositoryTabsState, SelectionState, SidebarState,
    StashBranchState, StashCreateState, TagCreateState, TerminalState, UndoCheckpoint,
    WorkspaceState, WorktreeCreateState,
};

pub struct App {
    pub(crate) repo: RepositoryState,
    pub(crate) selection: SelectionState,
    pub(crate) operation: OperationState,
    pub(crate) catalog: RepositoryCatalog,
    pub(crate) tabs: RepositoryTabsState,
    pub(crate) workspace: WorkspaceState,
    pub(crate) pull_requests: PullRequestsState,
    pub(crate) github_issues: GitHubIssuesState,
    pub(crate) avatars: AvatarCache,
    pub(crate) preferences: PreferencesState,
    pub(crate) release_prep: ReleasePrepState,
    pub(crate) sidebar: SidebarState,
    pub(crate) manager: RepositoryManagerState,
    pub(crate) worktree_create: WorktreeCreateState,
    pub(crate) terminal: TerminalState,
    pub(crate) commit_form: CommitFormState,
    pub(crate) branch_create: BranchCreateState,
    pub(crate) branch_manage_rename: BranchManageRenameState,
    pub(crate) stash_create: StashCreateState,
    pub(crate) stash_branch: StashBranchState,
    pub(crate) history_reword: HistoryRewordState,
    pub(crate) tag_create: TagCreateState,
    pub(crate) file_insight: FileInsightState,
    pub(crate) rebase: Option<InteractiveRebaseSession>,
    pub(crate) undo_stack: Vec<UndoCheckpoint>,
    pub(crate) redo_stack: Vec<UndoCheckpoint>,
    pub(crate) command_palette: CommandPaletteState,
    pub(crate) search_query: String,
    pub(crate) panes: pane_grid::State<PaneId>,
    pub(crate) commit_list_id: scrollable::Id,
    pub(crate) search_input_id: text_input::Id,
    pub(crate) branch_create_input_id: text_input::Id,
    pub(crate) branch_manage_input_id: text_input::Id,
    pub(crate) stash_create_input_id: text_input::Id,
    pub(crate) stash_branch_input_id: text_input::Id,
    pub(crate) history_reword_input_id: text_input::Id,
    pub(crate) tag_create_input_id: text_input::Id,
    pub(crate) worktree_path_input_id: text_input::Id,
    pub(crate) worktree_start_input_id: text_input::Id,
    pub(crate) worktree_branch_input_id: text_input::Id,
    pub(crate) pull_request_search_input_id: text_input::Id,
    pub(crate) pull_request_create_base_input_id: text_input::Id,
    pub(crate) pull_request_worktree_path_input_id: text_input::Id,
    pub(crate) pull_request_worktree_branch_input_id: text_input::Id,
    pub(crate) terminal_input_id: text_input::Id,
    pub(crate) command_palette_input_id: text_input::Id,
    pub(crate) window_width: f32,
    pub(crate) window_height: f32,
    /// Most recent absolute scroll offset reported by the commit list. Tracked
    /// so keyboard navigation can decide whether the target row is already
    /// visible and skip the scroll-to-top jump.
    pub(crate) commit_list_scroll_y: f32,
    /// Most recent visible height of the commit list scrollable. Zero until
    /// the first `on_scroll` event arrives; in that case we fall back to
    /// anchoring the selected row at the top.
    pub(crate) commit_list_viewport_height: f32,
    /// Set once a provider-CLI failure (e.g. `gh` not found) has been surfaced
    /// to the user, so the transient notice isn't re-shown on every avatar
    /// load / pagination during the session.
    pub(crate) provider_cli_notice_shown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneId {
    Sidebar,
    List,
    Detail,
}

#[derive(Debug, Clone)]
pub struct CheckoutPrompt {
    pub target: RefSummary,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone)]
pub struct ForceSyncPrompt {
    pub target: RefSummary,
    pub local_branch: String,
    pub sync_status: Option<BranchSyncStatus>,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForcePushPrompt {
    pub branch: String,
    pub upstream: String,
    pub head_short_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchDeletePrompt {
    pub target: BranchDeleteTarget,
    pub delete_matching_local_branches: bool,
    pub matching_local_branches: Vec<String>,
    pub delete_linked_worktrees: bool,
    pub linked_worktrees: Vec<LinkedWorktreeDeleteTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchDeleteTarget {
    LocalBranch(RefSummary),
    LocalBranches {
        label: String,
        branches: Vec<RefSummary>,
    },
    RemoteBranches {
        label: String,
        branches: Vec<RefSummary>,
    },
}

impl BranchDeleteTarget {
    pub(crate) fn label(&self) -> &str {
        match self {
            Self::LocalBranch(target) => &target.short_name,
            Self::LocalBranches { label, .. } => label,
            Self::RemoteBranches { label, .. } => label,
        }
    }

    pub(crate) fn remote_branches(&self) -> Option<&[RefSummary]> {
        match self {
            Self::LocalBranch(_) | Self::LocalBranches { .. } => None,
            Self::RemoteBranches { branches, .. } => Some(branches),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedWorktreeDeleteTarget {
    pub branch: String,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscardPrompt {
    pub target: DiscardTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscardTarget {
    File(WorktreeDiffTarget),
    Hunk { path: String, hunk: Hunk },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashPrompt {
    pub action: StashPromptAction,
    pub stash: StashSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashPromptAction {
    Pop,
    Drop,
}

#[derive(Debug, Clone)]
pub struct HistoryPrompt {
    pub operation: history::Operation,
}

#[derive(Debug, Clone)]
pub struct RebasePrompt {
    pub title: String,
    pub detail: String,
    pub preview_rows: Vec<RebasePromptRow>,
    pub hidden_row_count: usize,
    pub apply_mode: RebaseApplyMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePromptRow {
    pub action: RebaseAction,
    pub short_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetPrompt {
    pub target: CommitSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDeletePrompt {
    pub target: RefSummary,
}

#[derive(Debug, Clone)]
pub struct UndoPrompt {
    pub action: UndoPromptAction,
    pub checkpoint: UndoCheckpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoPromptAction {
    Undo,
    Redo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRemovePrompt {
    pub target: WorktreeSummary,
    pub delete_branch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    OpenRepository,
    InitRepository,
    CloneRepository,
    ToggleWorkspaceDashboard,
    RefreshWorkspace,
    WorkspaceFetchAll,
    WorkspacePullAll,
    RefreshPullRequests,
    FilterAllPullRequests,
    FilterMyPullRequests,
    FilterNeedsReviewPullRequests,
    FilterDraftPullRequests,
    FilterFailingPullRequests,
    FilterCurrentBranchPullRequests,
    SearchPullRequests,
    CreatePullRequest,
    CheckoutSelectedPullRequest,
    CheckoutSelectedPullRequestIntoWorktree,
    OpenSelectedPullRequest,
    RefreshGitHubIssues,
    FilterOpenGitHubIssues,
    FilterAssignedGitHubIssues,
    FilterMentionedGitHubIssues,
    FilterClosedGitHubIssues,
    SearchGitHubIssues,
    OpenSelectedGitHubIssue,
    ToggleDisplayOptions,
    ToggleShortcutHelp,
    ToggleTheme,
    SetDensityComfortable,
    SetDensityCompact,
    SetDensityDense,
    ToggleCommitAuthorDisplay,
    ToggleFileInspectionDisplay,
    TogglePullRequestMetadataDisplay,
    ToggleWorkspaceDetailsDisplay,
    FocusSearch,
    SelectWip,
    CreateBranch,
    CheckoutSelectedRef,
    ForceSyncSelectedRef,
    RenameSelectedBranch,
    DeleteSelectedBranch,
    CreateWorktree,
    OpenSelectedWorktree,
    LockSelectedWorktree,
    UnlockSelectedWorktree,
    RemoveSelectedWorktree,
    OpenTerminal,
    RunTerminalCommand,
    RestartTerminalCommand,
    KillTerminalCommand,
    Fetch,
    FetchAll,
    PullFastForwardOnly,
    PullFastForward,
    PullRebase,
    Push,
    PushForceWithLease,
    PrepareProductionRelease,
    ReleaseUpdateTargetFromSource,
    ReleasePushTarget,
    ReleaseSyncSourceFromTarget,
    MergeSelectedRef,
    RebaseOntoSelectedRef,
    AbortMerge,
    AbortRebase,
    ContinueRebase,
    RewordSelectedCommit,
    DropSelectedCommit,
    SquashSelectedCommit,
    FixupSelectedCommit,
    EditSelectedCommit,
    MoveSelectedCommitEarlier,
    MoveSelectedCommitLater,
    CherryPickSelectedCommit,
    RevertSelectedCommit,
    ResetToSelectedCommit,
    CreateBranchFromSelectedCommit,
    CreateTag,
    CreateAndPushTag,
    DeleteSelectedTag,
    Undo,
    Redo,
    ShowFileHistory,
    ShowFileBlame,
    StashChanges,
    PopLatestStash,
    CreateBranchFromSelectedStash,
    StageAll,
    UnstageAll,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteItem {
    pub id: CommandId,
    pub label: &'static str,
    pub description: &'static str,
    pub shortcut: &'static str,
    pub disabled_reason: Option<&'static str>,
}

impl CommandPaletteItem {
    pub fn enabled(self) -> bool {
        self.disabled_reason.is_none()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::with_preferences(PreferencesState::default())
    }
}

impl App {
    pub(crate) fn with_preferences(preferences: PreferencesState) -> Self {
        Self {
            repo: RepositoryState::default(),
            selection: SelectionState::default(),
            operation: OperationState::default(),
            catalog: RepositoryCatalog::default(),
            tabs: RepositoryTabsState::default(),
            workspace: WorkspaceState::default(),
            pull_requests: PullRequestsState::default(),
            github_issues: GitHubIssuesState::default(),
            avatars: AvatarCache::default(),
            preferences: preferences.clone(),
            release_prep: ReleasePrepState::default(),
            sidebar: SidebarState::default(),
            manager: RepositoryManagerState::default(),
            worktree_create: WorktreeCreateState::default(),
            terminal: TerminalState::default(),
            commit_form: CommitFormState::default(),
            branch_create: BranchCreateState::default(),
            branch_manage_rename: BranchManageRenameState::default(),
            stash_create: StashCreateState::default(),
            stash_branch: StashBranchState::default(),
            history_reword: HistoryRewordState::default(),
            tag_create: TagCreateState::default(),
            file_insight: FileInsightState::default(),
            rebase: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            command_palette: CommandPaletteState::default(),
            search_query: String::new(),
            panes: initial_panes(&preferences),
            commit_list_id: scrollable::Id::unique(),
            search_input_id: text_input::Id::unique(),
            branch_create_input_id: text_input::Id::unique(),
            branch_manage_input_id: text_input::Id::unique(),
            stash_create_input_id: text_input::Id::unique(),
            stash_branch_input_id: text_input::Id::unique(),
            history_reword_input_id: text_input::Id::unique(),
            tag_create_input_id: text_input::Id::unique(),
            worktree_path_input_id: text_input::Id::unique(),
            worktree_start_input_id: text_input::Id::unique(),
            worktree_branch_input_id: text_input::Id::unique(),
            pull_request_search_input_id: text_input::Id::unique(),
            pull_request_create_base_input_id: text_input::Id::unique(),
            pull_request_worktree_path_input_id: text_input::Id::unique(),
            pull_request_worktree_branch_input_id: text_input::Id::unique(),
            terminal_input_id: text_input::Id::unique(),
            command_palette_input_id: text_input::Id::unique(),
            window_width: 1200.0,
            window_height: 760.0,
            commit_list_scroll_y: 0.0,
            commit_list_viewport_height: 0.0,
            provider_cli_notice_shown: false,
        }
    }
    pub(crate) fn visible_commit_indices(&self) -> Vec<usize> {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return (0..self.repo.commits.len()).collect();
        }

        self.repo
            .commits
            .iter()
            .enumerate()
            .filter_map(|(i, commit)| {
                let summary = commit.summary.to_lowercase();
                let author = commit.author_name.to_lowercase();
                (summary.contains(&query) || author.contains(&query)).then_some(i)
            })
            .collect()
    }

    pub(crate) fn visible_commits(&self) -> Vec<CommitSummary> {
        self.visible_commit_indices()
            .into_iter()
            .filter_map(|i| self.repo.commits.get(i).cloned())
            .collect()
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.selection
            .selected_commit_id
            .as_deref()
            .and_then(|id| self.repo.commits.iter().position(|commit| commit.id == id))
            .or(self.selection.selected)
    }

    pub(crate) fn current_branch_create_base(&self) -> BranchCreateBase {
        if !self.selection.selected_wip {
            if let Some(commit_id) = self.selection.selected_commit_id.as_deref() {
                if let Some(commit) = self
                    .repo
                    .commits
                    .iter()
                    .find(|commit| commit.id == commit_id)
                {
                    return BranchCreateBase::Commit {
                        id: commit.id.clone(),
                        short_id: commit.short_id.clone(),
                        summary: commit.summary.clone(),
                    };
                }
            }
        }

        BranchCreateBase::Head {
            label: self
                .repo
                .head_branch
                .as_ref()
                .map(|branch| format!("HEAD ({branch})"))
                .unwrap_or_else(|| "HEAD".into()),
        }
    }

    pub(crate) fn fetch_success_message(&self, scope: FetchScope) -> String {
        if scope == FetchScope::AllRemotes {
            return "Fetched all remotes just now".into();
        }

        match self.repo.sync_status.upstream.as_deref() {
            Some(upstream) => format!("Fetched {upstream} just now"),
            None => "Fetched remote just now".into(),
        }
    }

    pub(crate) fn pull_success_message(&self, mode: PullMode) -> String {
        let mode_label = match mode {
            PullMode::FastForwardOnly => "with fast-forward only",
            PullMode::FastForward => "with fast-forward mode",
            PullMode::Rebase => "with rebase",
        };
        match self.repo.sync_status.upstream.as_deref() {
            Some(upstream) => format!("Pulled {upstream} {mode_label}"),
            None => format!("Pulled remote {mode_label}"),
        }
    }

    pub(crate) fn error_recovery_action(&self) -> Option<crate::widgets::ErrorRecovery<'static>> {
        let err = self.operation.error.as_deref()?;
        if !crate::features::push::is_non_fast_forward_rejection(err) {
            return None;
        }
        if self.operation.loading
            || self.repo.path.is_none()
            || self.repo.head_branch.is_none()
            || self.repo.sync_status.upstream.is_none()
        {
            return None;
        }
        Some(crate::widgets::ErrorRecovery {
            label: "Force push (with lease)",
            message: crate::Message::from(
                crate::features::push::Message::ForceWithLeaseConfirmationRequested,
            ),
        })
    }

    pub(crate) fn push_success_message(&self, mode: naite_core::PushMode) -> String {
        let verb = match mode {
            naite_core::PushMode::Normal => "Pushed",
            naite_core::PushMode::ForceWithLease => "Force-pushed (with lease)",
        };
        match (
            self.repo.sync_status.upstream.as_deref(),
            self.repo.head_branch.as_deref(),
        ) {
            (Some(upstream), _) => format!("{verb} {upstream} just now"),
            (None, Some(branch)) => format!("{verb} origin/{branch} and set upstream"),
            (None, None) => format!("{verb} current branch just now"),
        }
    }

    pub(crate) fn force_push_prompt_for_current_branch(&self) -> Result<ForcePushPrompt, String> {
        if self.repo.operation_state.is_busy() {
            return Err("another Git operation is already in progress".into());
        }
        if self.repo.status_detail.is_dirty() {
            return Err("worktree has local changes".into());
        }
        let branch = self
            .repo
            .head_branch
            .clone()
            .ok_or_else(|| "current HEAD is detached".to_string())?;
        let upstream = self
            .repo
            .sync_status
            .upstream
            .clone()
            .ok_or_else(|| "current branch has no upstream".to_string())?;
        let head_short_id = self
            .repo
            .commits
            .first()
            .map(|commit| commit.short_id.clone())
            .unwrap_or_else(|| "HEAD".into());

        Ok(ForcePushPrompt {
            branch,
            upstream,
            head_short_id,
        })
    }

    pub(crate) fn filtered_command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let query = self.command_palette.query.trim().to_lowercase();
        self.command_palette_items()
            .into_iter()
            .filter(|item| {
                query.is_empty()
                    || item.label.to_lowercase().contains(&query)
                    || item.description.to_lowercase().contains(&query)
                    || item.shortcut.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub(crate) fn command_palette_items(&self) -> Vec<CommandPaletteItem> {
        let has_repo = self.repo.path.is_some();
        let has_conflicts = !self.repo.status_detail.conflicted.is_empty();
        let has_stashable = self.repo.status_detail.is_dirty();
        let has_stashes = !self.repo.stashes.is_empty();
        let has_stageable = !self.repo.status_detail.unstaged.is_empty()
            || !self.repo.status_detail.untracked.is_empty();
        let has_staged = !self.repo.status_detail.staged.is_empty();
        let commit_title_empty = self.commit_form.title.trim().is_empty();
        let has_upstream = self.repo.sync_status.upstream.is_some();
        let has_branch = self.repo.head_branch.is_some();
        let commit_push_without_branch = self.commit_form.push_after && !has_branch;
        let has_selected_stash = self.selection.selected_stash.is_some();
        let selected_checkoutable_ref = self.selected_context_checkoutable_ref();
        let selected_force_sync_target = self.selected_context_force_sync_target();
        let selected_mergeable_ref = self.selected_context_mergeable_ref();
        let selected_local_branch = self.selected_context_local_branch();
        let selected_tag = self.selected_context_tag();
        let selected_deletable_branch = selected_local_branch.is_some_and(|branch| !branch.is_head);
        let selected_worktree = self.selection.selected_worktree.as_ref();
        let selected_worktree_removable =
            selected_worktree.is_some_and(|worktree| !worktree.is_current && !worktree.locked);
        let selected_worktree_locked = selected_worktree.is_some_and(|worktree| worktree.locked);
        let selected_commit = self.selected_commit();
        let selected_file_path = self.selected_detail_file_path();
        let terminal_session = self.terminal.active_session();
        let terminal_started = terminal_session.is_some_and(|session| {
            matches!(
                session.status,
                crate::state::TerminalStatus::Starting | crate::state::TerminalStatus::Running
            )
        });
        let terminal_restartable = terminal_session.is_some_and(|session| {
            matches!(
                session.status,
                crate::state::TerminalStatus::Exited | crate::state::TerminalStatus::Error
            )
        });
        let selected_pull_request = self.selection.selected_pull_request.as_ref();
        let selected_github_issue = self.selection.selected_github_issue.as_ref();
        let has_undo = !self.undo_stack.is_empty();
        let has_redo = !self.redo_stack.is_empty();
        let merge_in_progress = self.repo.operation_state.merge_in_progress;
        let rebase_in_progress = self.repo.operation_state.rebase_in_progress;
        let release_profile_active = self.release_prep.active_profile.is_some();
        let release_auto_running = self.release_prep.auto_running;
        let release_update_target_complete = self
            .release_prep
            .completed_actions
            .contains(&crate::features::release_prep::ReleasePrepAction::UpdateTargetFromSource);
        let release_push_target_complete = self
            .release_prep
            .completed_actions
            .contains(&crate::features::release_prep::ReleasePrepAction::PushTarget);
        let release_sync_source_complete = self
            .release_prep
            .completed_actions
            .contains(&crate::features::release_prep::ReleasePrepAction::SyncSourceFromTarget);

        vec![
            CommandPaletteItem {
                id: CommandId::OpenRepository,
                label: "Open repository",
                description: "Choose a local Git repository",
                shortcut: "Cmd O",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::InitRepository,
                label: "Initialize repository",
                description: "Run git init in an existing folder",
                shortcut: "",
                disabled_reason: self.operation.loading.then_some("Operation in progress"),
            },
            CommandPaletteItem {
                id: CommandId::CloneRepository,
                label: "Clone repository",
                description: "Clone the URL entered in the sidebar",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if self.manager.clone_url.trim().is_empty() {
                    Some("Enter a clone URL in the sidebar")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ToggleWorkspaceDashboard,
                label: "Toggle workspace dashboard",
                description: "Show or hide local workspace repository status",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::RefreshWorkspace,
                label: "Refresh workspace",
                description: "Reload local workspace repository summaries",
                shortcut: "",
                disabled_reason: self
                    .workspace
                    .loading
                    .then_some("Workspace refresh in progress"),
            },
            CommandPaletteItem {
                id: CommandId::WorkspaceFetchAll,
                label: "Fetch workspace",
                description: "Fetch all remotes for workspace repositories",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if self.workspace.summaries.is_empty() {
                    Some("No workspace repositories")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::WorkspacePullAll,
                label: "Pull workspace",
                description: "Pull --ff-only for workspace repositories",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if self.workspace.summaries.is_empty() {
                    Some("No workspace repositories")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RefreshPullRequests,
                label: "Refresh pull requests",
                description: "Load GitHub pull requests through the provider integration",
                shortcut: "",
                disabled_reason: if !has_repo {
                    Some("Open a repository first")
                } else if self.pull_requests.loading {
                    Some("Pull requests are refreshing")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::FilterAllPullRequests,
                label: "Show all pull requests",
                description: "List open GitHub pull requests",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterMyPullRequests,
                label: "Show my pull requests",
                description: "List GitHub pull requests authored by you",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterNeedsReviewPullRequests,
                label: "Show pull requests needing review",
                description: "Filter GitHub pull requests by required review",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterDraftPullRequests,
                label: "Show draft pull requests",
                description: "Filter GitHub pull requests to drafts",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterFailingPullRequests,
                label: "Show failing pull requests",
                description: "Filter GitHub pull requests by failing checks",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterCurrentBranchPullRequests,
                label: "Show current branch pull request",
                description: "Filter GitHub pull requests to the current branch",
                shortcut: "",
                disabled_reason: if !has_repo {
                    Some("Open a repository first")
                } else if !has_branch {
                    Some("Current HEAD is detached")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::SearchPullRequests,
                label: "Search pull requests",
                description: "Run the custom GitHub pull request search",
                shortcut: "",
                disabled_reason: if !has_repo {
                    Some("Open a repository first")
                } else if self.pull_requests.search_query.trim().is_empty() {
                    Some("Enter a pull request search query")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreatePullRequest,
                label: "Create pull request",
                description: "Open the GitHub pull request creation form",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_branch {
                    Some("Current HEAD is detached")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CheckoutSelectedPullRequest,
                label: "Checkout selected pull request",
                description: "Checkout the selected GitHub pull request locally",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_pull_request.is_none() {
                    Some("Select a pull request first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CheckoutSelectedPullRequestIntoWorktree,
                label: "Checkout selected pull request into worktree",
                description: "Create a linked worktree for the selected GitHub pull request",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_pull_request.is_none() {
                    Some("Select a pull request first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::OpenSelectedPullRequest,
                label: "Open selected pull request",
                description: "Open the selected GitHub pull request in the browser",
                shortcut: "",
                disabled_reason: if selected_pull_request.is_none() {
                    Some("Select a pull request first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RefreshGitHubIssues,
                label: "Refresh GitHub issues",
                description: "Load GitHub issues through the gh CLI",
                shortcut: "",
                disabled_reason: if !has_repo {
                    Some("Open a repository first")
                } else if self.github_issues.loading {
                    Some("GitHub issues are refreshing")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::FilterOpenGitHubIssues,
                label: "Show open GitHub issues",
                description: "List open GitHub issues",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterAssignedGitHubIssues,
                label: "Show assigned GitHub issues",
                description: "List GitHub issues assigned to you",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterMentionedGitHubIssues,
                label: "Show mentioned GitHub issues",
                description: "List GitHub issues mentioning you",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::FilterClosedGitHubIssues,
                label: "Show closed GitHub issues",
                description: "List recently closed GitHub issues",
                shortcut: "",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::SearchGitHubIssues,
                label: "Search GitHub issues",
                description: "Run the custom GitHub issue search",
                shortcut: "",
                disabled_reason: if !has_repo {
                    Some("Open a repository first")
                } else if self.github_issues.search_query.trim().is_empty() {
                    Some("Enter an issue search query")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::OpenSelectedGitHubIssue,
                label: "Open selected GitHub issue",
                description: "Open the selected GitHub issue in the browser",
                shortcut: "",
                disabled_reason: if selected_github_issue.is_none() {
                    Some("Select a GitHub issue first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ToggleDisplayOptions,
                label: "Toggle display options",
                description: "Show graph, file, PR, workspace, density, and theme preferences",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::ToggleShortcutHelp,
                label: "Show keyboard shortcuts",
                description: "Open the shortcut help overlay",
                shortcut: "?",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::ToggleTheme,
                label: "Toggle theme",
                description: "Switch between dark and high-contrast theme preferences",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::SetDensityComfortable,
                label: "Set density: comfortable",
                description: "Use more relaxed display density",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::SetDensityCompact,
                label: "Set density: compact",
                description: "Use balanced display density",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::SetDensityDense,
                label: "Set density: dense",
                description: "Use the tightest display density",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::ToggleCommitAuthorDisplay,
                label: "Toggle graph author column",
                description: "Show or hide commit authors in the graph list",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::ToggleFileInspectionDisplay,
                label: "Toggle file inspection panels",
                description: "Show or hide history and blame panels in file diffs",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::TogglePullRequestMetadataDisplay,
                label: "Toggle PR metadata cards",
                description: "Show or hide PR people and status cards",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::ToggleWorkspaceDetailsDisplay,
                label: "Toggle workspace detail rows",
                description: "Show or hide remote, worktree, fetch, and path rows",
                shortcut: "",
                disabled_reason: None,
            },
            CommandPaletteItem {
                id: CommandId::FocusSearch,
                label: "Search commits",
                description: "Focus the commit filter",
                shortcut: "Cmd F",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::SelectWip,
                label: "Select working tree",
                description: "Show current staged and unstaged changes",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !self.repo.status_detail.is_dirty() {
                    Some("Working tree is clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreateBranch,
                label: "Create branch",
                description: "Create and checkout a local branch",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CheckoutSelectedRef,
                label: "Checkout selected ref",
                description: "Checkout the branch open in the sidebar menu",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if selected_checkoutable_ref.is_none() {
                    Some("Open a local or remote branch menu first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ForceSyncSelectedRef,
                label: "Reset local to remote",
                description: "Discard local branch differences by resetting to the matching remote",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if selected_force_sync_target.is_none() {
                    Some("Open a matching local/remote branch menu first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RenameSelectedBranch,
                label: "Rename selected branch",
                description: "Rename the local branch open in the sidebar menu",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if selected_local_branch.is_none() {
                    Some("Open a local branch menu first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::DeleteSelectedBranch,
                label: "Delete selected branch",
                description: "Delete the local branch open in the sidebar menu",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if selected_local_branch.is_none() {
                    Some("Open a local branch menu first")
                } else if !selected_deletable_branch {
                    Some("Cannot delete current branch")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreateWorktree,
                label: "Create worktree",
                description: "Create a linked worktree from a branch or commit",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::OpenSelectedWorktree,
                label: "Open selected worktree",
                description: "Open the selected worktree as the active repository",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_worktree.is_none() {
                    Some("Select a worktree first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::LockSelectedWorktree,
                label: "Lock selected worktree",
                description: "Prevent accidental worktree removal",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_worktree.is_none() {
                    Some("Select a worktree first")
                } else if selected_worktree_locked {
                    Some("Worktree is already locked")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::UnlockSelectedWorktree,
                label: "Unlock selected worktree",
                description: "Allow a locked worktree to be removed",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_worktree.is_none() {
                    Some("Select a worktree first")
                } else if !selected_worktree_locked {
                    Some("Worktree is not locked")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RemoveSelectedWorktree,
                label: "Remove selected worktree",
                description: "Remove the selected linked worktree",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_worktree.is_none() {
                    Some("Select a worktree first")
                } else if !selected_worktree_removable {
                    Some("Cannot remove current or locked worktree")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::OpenTerminal,
                label: "Open terminal",
                description: "Open the terminal panel for the active repo or worktree",
                shortcut: "Cmd `",
                disabled_reason: (!has_repo).then_some("Open a repository first"),
            },
            CommandPaletteItem {
                id: CommandId::RunTerminalCommand,
                label: "Start terminal session",
                description: "Start an interactive shell in the terminal panel",
                shortcut: "",
                disabled_reason: if terminal_started {
                    Some("Terminal session is already running")
                } else if terminal_session.is_none() {
                    Some("Open a terminal first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RestartTerminalCommand,
                label: "Restart terminal session",
                description: "Restart the active interactive shell",
                shortcut: "",
                disabled_reason: if terminal_started {
                    Some("Terminal session is already running")
                } else if !terminal_restartable {
                    Some("No exited terminal session is selected")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::KillTerminalCommand,
                label: "Kill terminal session",
                description: "Force stop the active terminal shell",
                shortcut: "",
                disabled_reason: (!terminal_started).then_some("No terminal session is running"),
            },
            CommandPaletteItem {
                id: CommandId::Fetch,
                label: "Fetch",
                description: "Fetch the current branch remote",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_upstream {
                    Some("Current branch has no upstream")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::FetchAll,
                label: "Fetch all remotes",
                description: "Run git fetch --all for this repository",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::PullFastForwardOnly,
                label: "Pull --ff-only",
                description: "Fast-forward the current branch from upstream",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_upstream {
                    Some("Current branch has no upstream")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::PullFastForward,
                label: "Pull --ff",
                description: "Pull from upstream with fast-forward mode",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_upstream {
                    Some("Current branch has no upstream")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::PullRebase,
                label: "Pull --rebase",
                description: "Rebase current branch onto upstream",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_upstream {
                    Some("Current branch has no upstream")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::Push,
                label: "Push",
                description: "Push the current branch and set origin upstream if needed",
                shortcut: "Cmd Shift P",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_branch {
                    Some("Current HEAD is detached")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::PushForceWithLease,
                label: "Push --force-with-lease",
                description: "Overwrite the upstream ref, refusing if it moved unseen",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_branch {
                    Some("Current HEAD is detached")
                } else if !has_upstream {
                    Some("Current branch has no upstream")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::PrepareProductionRelease,
                label: "Plan release promotion",
                description: "Fetch, verify release branches, and open a prefilled rebase plan",
                shortcut: "Cmd Shift R",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if release_auto_running {
                    Some("Auto promotion in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Commit, stash, or resolve local changes first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ReleaseUpdateTargetFromSource,
                label: "Update release target from source",
                description: "Fast-forward the configured release target from the source branch",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if release_auto_running {
                    Some("Auto promotion in progress")
                } else if release_update_target_complete {
                    Some("Release step already complete")
                } else if !release_profile_active {
                    Some("Plan a release promotion first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Commit, stash, or resolve local changes first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ReleasePushTarget,
                label: "Push release target",
                description: "Push the configured release target branch to its remote",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if release_auto_running {
                    Some("Auto promotion in progress")
                } else if release_push_target_complete {
                    Some("Release step already complete")
                } else if !release_profile_active {
                    Some("Plan a release promotion first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ReleaseSyncSourceFromTarget,
                label: "Rebase release source onto target",
                description:
                    "Rebase the release source branch onto target and push with --force-with-lease",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if release_auto_running {
                    Some("Auto promotion in progress")
                } else if release_sync_source_complete {
                    Some("Release step already complete")
                } else if !release_profile_active {
                    Some("Plan a release promotion first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Commit, stash, or resolve local changes first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::MergeSelectedRef,
                label: "Merge selected ref",
                description: "Merge the branch open in the sidebar menu",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if selected_mergeable_ref.is_none() {
                    Some("Open a branch menu first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Commit, stash, or resolve local changes first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RebaseOntoSelectedRef,
                label: "Rebase onto selected ref",
                description: "Rebase the current branch onto the branch open in the sidebar menu",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if selected_mergeable_ref.is_none() {
                    Some("Open a branch menu first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Commit, stash, or resolve local changes first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::AbortMerge,
                label: "Abort merge",
                description: "Abort the in-progress merge",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !merge_in_progress {
                    Some("No merge in progress")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::AbortRebase,
                label: "Abort rebase",
                description: "Abort the in-progress rebase",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !rebase_in_progress {
                    Some("No rebase in progress")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ContinueRebase,
                label: "Continue rebase",
                description: "Continue after conflict files are staged",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !rebase_in_progress {
                    Some("No rebase in progress")
                } else if !self.repo.status_detail.conflicted.is_empty() {
                    Some("Resolve conflicts first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RewordSelectedCommit,
                label: "Reword selected commit",
                description: "Open the safe interactive rebase reword form",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::DropSelectedCommit,
                label: "Drop selected commit",
                description: "Drop the selected local commit with interactive rebase",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::SquashSelectedCommit,
                label: "Squash selected commit",
                description: "Squash the selected commit into its parent",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::FixupSelectedCommit,
                label: "Fixup selected commit",
                description: "Fixup the selected commit into its parent",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::EditSelectedCommit,
                label: "Edit selected commit",
                description: "Stop interactive rebase at the selected commit",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::MoveSelectedCommitEarlier,
                label: "Move selected commit earlier",
                description: "Swap the selected commit with its previous commit",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::MoveSelectedCommitLater,
                label: "Move selected commit later",
                description: "Swap the selected commit with its next commit",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CherryPickSelectedCommit,
                label: "Cherry-pick selected commit",
                description: "Apply the selected commit onto the current branch",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if has_conflicts {
                    Some("Resolve conflicts first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::RevertSelectedCommit,
                label: "Revert selected commit",
                description: "Create a new commit that reverses the selected commit",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else if has_conflicts {
                    Some("Resolve conflicts first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ResetToSelectedCommit,
                label: "Reset to selected commit",
                description: "Move HEAD to the selected commit (soft / mixed / hard)",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreateBranchFromSelectedCommit,
                label: "Create branch from selected commit",
                description: "Create and checkout a branch starting at the selected commit",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_commit.is_none() {
                    Some("Select a commit first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreateTag,
                label: "Create tag",
                description: "Create a lightweight tag on the selected commit or HEAD",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreateAndPushTag,
                label: "Create and push tag",
                description: "Create a lightweight tag on the selected commit or HEAD, then push it to origin",
                shortcut: "Cmd Shift T",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::DeleteSelectedTag,
                label: "Delete selected tag",
                description: "Delete the tag open in the sidebar menu",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_tag.is_none() {
                    Some("Open a tag menu first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::Undo,
                label: "Undo last supported action",
                description: "Reset back to the checkpoint captured before the last history action",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else if !has_undo {
                    Some("No undo checkpoint")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::Redo,
                label: "Redo last undone action",
                description: "Reset back to the checkpoint captured before undo",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if self.repo.status_detail.is_dirty() {
                    Some("Working tree must be clean")
                } else if !has_redo {
                    Some("No redo checkpoint")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ShowFileHistory,
                label: "Show file history",
                description: "Inspect commits touching the selected file",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_file_path.is_none() {
                    Some("Select a file in the detail pane")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::ShowFileBlame,
                label: "Show file blame",
                description: "Inspect line ownership for the selected file",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if selected_file_path.is_none() {
                    Some("Select a file in the detail pane")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::StashChanges,
                label: "Stash changes",
                description: "Save working tree changes to a stash",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if has_conflicts {
                    Some("Resolve conflicts first")
                } else if !has_stashable {
                    Some("Working tree is clean")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::PopLatestStash,
                label: "Pop latest stash",
                description: "Apply and remove the latest stash",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_stashes {
                    Some("No stashes")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::CreateBranchFromSelectedStash,
                label: "Create branch from stash",
                description: "Create and checkout a branch from the selected stash",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if !has_repo {
                    Some("Open a repository first")
                } else if !has_selected_stash {
                    Some("Select a stash first")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::StageAll,
                label: "Stage all",
                description: "Stage unstaged and new files",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if has_conflicts {
                    Some("Resolve conflicts first")
                } else if !has_stageable {
                    Some("No unstaged or new files")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::UnstageAll,
                label: "Unstage all",
                description: "Move staged files back to the working tree",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if has_conflicts {
                    Some("Resolve conflicts first")
                } else if !has_staged {
                    Some("No staged files")
                } else {
                    None
                },
            },
            CommandPaletteItem {
                id: CommandId::Commit,
                label: "Commit",
                description: "Create a commit from staged files",
                shortcut: "",
                disabled_reason: if self.operation.loading {
                    Some("Operation in progress")
                } else if has_conflicts {
                    Some("Resolve conflicts first")
                } else if !has_staged {
                    Some("Stage changes first")
                } else if commit_title_empty {
                    Some("Enter a commit summary")
                } else if commit_push_without_branch {
                    Some("Current HEAD is detached")
                } else {
                    None
                },
            },
        ]
    }

    pub(crate) fn context_menu_ref(&self) -> Option<&RefSummary> {
        self.selection
            .context_menu
            .as_ref()
            .and_then(|menu| menu.kind.as_ref())
    }

    pub(crate) fn selected_context_local_branch(&self) -> Option<&RefSummary> {
        self.context_menu_ref()
            .filter(|ref_summary| ref_summary.kind == naite_core::RefKind::LocalBranch)
    }

    pub(crate) fn selected_context_checkoutable_ref(&self) -> Option<&RefSummary> {
        self.context_menu_ref().filter(|ref_summary| {
            matches!(
                ref_summary.kind,
                naite_core::RefKind::LocalBranch | naite_core::RefKind::RemoteBranch
            )
        })
    }

    pub(crate) fn selected_context_force_sync_target(&self) -> Option<RefSummary> {
        self.context_menu_ref()
            .and_then(|ref_summary| self.force_sync_target_for_ref(ref_summary))
    }

    pub(crate) fn selected_context_mergeable_ref(&self) -> Option<&RefSummary> {
        self.context_menu_ref().filter(|ref_summary| {
            matches!(
                ref_summary.kind,
                naite_core::RefKind::LocalBranch | naite_core::RefKind::RemoteBranch
            ) && !ref_summary.is_head
        })
    }

    pub(crate) fn selected_context_tag(&self) -> Option<&RefSummary> {
        self.context_menu_ref()
            .filter(|ref_summary| ref_summary.kind == RefKind::Tag)
    }

    pub(crate) fn selected_commit(&self) -> Option<CommitSummary> {
        if self.selection.selected_wip || self.selection.selected_stash.is_some() {
            return None;
        }
        self.selected_index()
            .and_then(|index| self.repo.commits.get(index).cloned())
    }

    pub(crate) fn selected_detail_file_path(&self) -> Option<String> {
        let diff = self.operation.current_diff.as_ref()?;
        let selected = self.selection.selected_file.unwrap_or(0);
        diff.files.get(selected).map(|file| file.path.clone())
    }

    pub(crate) fn existing_local_branch_for_remote_ref(
        &self,
        ref_summary: &RefSummary,
    ) -> Option<String> {
        let local_branch = remote_ref_local_branch(ref_summary)?;
        self.repo
            .refs
            .local
            .iter()
            .any(|branch| branch.short_name == local_branch)
            .then(|| local_branch.to_string())
    }

    pub(crate) fn force_sync_target_for_ref(&self, ref_summary: &RefSummary) -> Option<RefSummary> {
        match ref_summary.kind {
            RefKind::RemoteBranch => self
                .existing_local_branch_for_remote_ref(ref_summary)
                .map(|_| ref_summary.clone()),
            RefKind::LocalBranch => self.remote_branch_for_local_upstream(ref_summary).cloned(),
            RefKind::Tag => None,
        }
    }

    pub(crate) fn force_sync_prompt_for_remote_ref(
        &self,
        target: RefSummary,
        status: WorktreeStatus,
    ) -> Option<ForceSyncPrompt> {
        let local_branch = self.existing_local_branch_for_remote_ref(&target)?;
        let sync_status = self
            .repo
            .refs
            .local
            .iter()
            .find(|branch| branch.short_name == local_branch)
            .and_then(|branch| branch.sync_status.clone());

        Some(ForceSyncPrompt {
            target,
            local_branch,
            sync_status,
            status,
        })
    }

    fn remote_branch_for_local_upstream(&self, ref_summary: &RefSummary) -> Option<&RefSummary> {
        if ref_summary.kind != RefKind::LocalBranch {
            return None;
        }

        let local_branch = self
            .repo
            .refs
            .local
            .iter()
            .find(|branch| branch.short_name == ref_summary.short_name)
            .unwrap_or(ref_summary);
        let upstream = local_branch.sync_status.as_ref()?.upstream.as_deref()?;
        let upstream_local_branch = upstream_branch_local_name(upstream)?;

        if upstream_local_branch != local_branch.short_name {
            return None;
        }

        self.repo
            .refs
            .remote
            .iter()
            .find(|branch| branch.short_name == upstream)
    }
}

pub(crate) fn remote_ref_local_branch(ref_summary: &RefSummary) -> Option<&str> {
    if ref_summary.kind != naite_core::RefKind::RemoteBranch {
        return None;
    }

    let name = ref_summary.full_name.strip_prefix("refs/remotes/")?;
    let (_remote, branch) = name.split_once('/')?;
    (!branch.is_empty() && branch != "HEAD").then_some(branch)
}

fn upstream_branch_local_name(upstream: &str) -> Option<&str> {
    let (_remote, branch) = upstream.split_once('/')?;
    (!branch.is_empty() && branch != "HEAD").then_some(branch)
}

pub(crate) fn initial_panes(preferences: &PreferencesState) -> pane_grid::State<PaneId> {
    let (mut panes, sidebar) = pane_grid::State::new(PaneId::Sidebar);
    if let Some((list, sidebar_split)) =
        panes.split(pane_grid::Axis::Vertical, sidebar, PaneId::List)
    {
        panes.resize(sidebar_split, preferences.sidebar_ratio.clamp(0.14, 0.36));
        if let Some((_detail, detail_split)) =
            panes.split(pane_grid::Axis::Vertical, list, PaneId::Detail)
        {
            panes.resize(detail_split, preferences.detail_ratio.clamp(0.50, 0.78));
        }
    }
    panes
}
