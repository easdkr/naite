use std::path::PathBuf;

use iced::widget::{pane_grid, scrollable};
use iced::Point;
use naite_core::{
    CommitDiff, HighlightedDiff, RefSummary, StashSummary, WorktreeDiffTarget, WorktreeStatusDetail,
};

use crate::features::repo_open::LoadedRepo;
use crate::features::{
    branch_create, branch_manage, catalog, checkout, cherry_pick, command_palette, commit, discard,
    fetch, file_inspect, github_issue, history, pull, pull_request, push, rebase, release_prep,
    repo_open, reset, revert, stage, stash, tag, terminal, workspace, worktree,
};
use crate::state::{
    ContextMenuKind, DensityPreference, DiffViewMode, SidebarSection, ThemePreference,
};

#[derive(Debug, Clone)]
pub enum Message {
    NoOp,
    Catalog(catalog::Message),
    RepoOpen(repo_open::Message),
    Checkout(checkout::Message),
    Fetch(fetch::Message),
    Pull(pull::Message),
    PullRequest(pull_request::Message),
    GitHubIssue(github_issue::Message),
    Push(push::Message),
    Rebase(rebase::Message),
    ReleasePrep(release_prep::Message),
    Stage(stage::Message),
    Discard(discard::Message),
    Commit(commit::Message),
    History(history::Message),
    BranchCreate(branch_create::Message),
    BranchManage(branch_manage::Message),
    Tag(tag::Message),
    FileInspect(file_inspect::Message),
    Stash(stash::Message),
    CherryPick(cherry_pick::Message),
    Revert(revert::Message),
    Reset(reset::Message),
    Worktree(worktree::Message),
    Workspace(workspace::Message),
    Terminal(terminal::Message),
    CommandPalette(command_palette::Message),
    Tabs(TabsMessage),
    WindowFocused,
    WorktreeStatusDetailLoaded(Result<WorktreeStatusDetail, String>),
    WipSelected,
    WipStatusPathSelected(WorktreeDiffTarget),
    WipDiffLoaded {
        target: WorktreeDiffTarget,
        result: Result<(CommitDiff, HighlightedDiff), String>,
    },
    StashSelected(StashSummary),
    StashDiffLoaded {
        selector: String,
        result: Result<(CommitDiff, HighlightedDiff), String>,
    },
    CommitSelected(usize),
    CommitListScrolled(scrollable::Viewport),
    DiffLoaded {
        commit_id: String,
        result: Result<(CommitDiff, HighlightedDiff), String>,
    },
    DetailFileSelected(usize),
    DetailHunkSelected(usize),
    DetailNextHunk,
    DetailPreviousHunk,
    DiffViewModeChanged(DiffViewMode),
    Keyboard(KeyAction),
    SearchChanged(String),
    WindowResized(iced::Size),
    PaneResized(pane_grid::ResizeEvent),
    ToggleDisplayOptions,
    ToggleShortcutOverlay,
    ThemePreferenceChanged(ThemePreference),
    DensityPreferenceChanged(DensityPreference),
    DisplayCommitAuthorToggled,
    DisplayFileInspectionToggled,
    DisplayPrMetadataToggled,
    DisplayWorkspaceDetailsToggled,
    PreferencesSaved(Result<(), String>),
    CursorMoved(Point),
    ContextMenuOpened(ContextMenuKind),
    ContextMenuClosed,
    SidebarRefPressed(RefSummary),
    SidebarRefHovered(RefSummary),
    SidebarRefUnhovered(RefSummary),
    SidebarSectionToggled(SidebarSection),
    SidebarTreeFolderToggled {
        section: SidebarSection,
        path: String,
    },
    AutoFetchTick,
    TransientStatusTick,
    ReleasePrepTick,
    CopyText(String),
    ClearError,
    AvatarFetched {
        url: String,
        bytes: Result<Vec<u8>, String>,
    },
}

impl From<branch_create::Message> for Message {
    fn from(message: branch_create::Message) -> Self {
        Self::BranchCreate(message)
    }
}

impl From<branch_manage::Message> for Message {
    fn from(message: branch_manage::Message) -> Self {
        Self::BranchManage(message)
    }
}

impl From<catalog::Message> for Message {
    fn from(message: catalog::Message) -> Self {
        Self::Catalog(message)
    }
}

impl From<checkout::Message> for Message {
    fn from(message: checkout::Message) -> Self {
        Self::Checkout(message)
    }
}

impl From<cherry_pick::Message> for Message {
    fn from(message: cherry_pick::Message) -> Self {
        Self::CherryPick(message)
    }
}

impl From<command_palette::Message> for Message {
    fn from(message: command_palette::Message) -> Self {
        Self::CommandPalette(message)
    }
}

impl From<commit::Message> for Message {
    fn from(message: commit::Message) -> Self {
        Self::Commit(message)
    }
}

impl From<file_inspect::Message> for Message {
    fn from(message: file_inspect::Message) -> Self {
        Self::FileInspect(message)
    }
}

impl From<history::Message> for Message {
    fn from(message: history::Message) -> Self {
        Self::History(message)
    }
}

impl From<discard::Message> for Message {
    fn from(message: discard::Message) -> Self {
        Self::Discard(message)
    }
}

impl From<fetch::Message> for Message {
    fn from(message: fetch::Message) -> Self {
        Self::Fetch(message)
    }
}

impl From<pull::Message> for Message {
    fn from(message: pull::Message) -> Self {
        Self::Pull(message)
    }
}

impl From<pull_request::Message> for Message {
    fn from(message: pull_request::Message) -> Self {
        Self::PullRequest(message)
    }
}

impl From<github_issue::Message> for Message {
    fn from(message: github_issue::Message) -> Self {
        Self::GitHubIssue(message)
    }
}

impl From<push::Message> for Message {
    fn from(message: push::Message) -> Self {
        Self::Push(message)
    }
}

impl From<rebase::Message> for Message {
    fn from(message: rebase::Message) -> Self {
        Self::Rebase(message)
    }
}

impl From<release_prep::Message> for Message {
    fn from(message: release_prep::Message) -> Self {
        Self::ReleasePrep(message)
    }
}

impl From<repo_open::Message> for Message {
    fn from(message: repo_open::Message) -> Self {
        Self::RepoOpen(message)
    }
}

impl From<reset::Message> for Message {
    fn from(message: reset::Message) -> Self {
        Self::Reset(message)
    }
}

impl From<revert::Message> for Message {
    fn from(message: revert::Message) -> Self {
        Self::Revert(message)
    }
}

impl From<stage::Message> for Message {
    fn from(message: stage::Message) -> Self {
        Self::Stage(message)
    }
}

impl From<stash::Message> for Message {
    fn from(message: stash::Message) -> Self {
        Self::Stash(message)
    }
}

impl From<tag::Message> for Message {
    fn from(message: tag::Message) -> Self {
        Self::Tag(message)
    }
}

impl From<terminal::Message> for Message {
    fn from(message: terminal::Message) -> Self {
        Self::Terminal(message)
    }
}

impl From<worktree::Message> for Message {
    fn from(message: worktree::Message) -> Self {
        Self::Worktree(message)
    }
}

impl From<workspace::Message> for Message {
    fn from(message: workspace::Message) -> Self {
        Self::Workspace(message)
    }
}

#[derive(Debug, Clone)]
pub enum TabsMessage {
    Activate(PathBuf),
    Close(PathBuf),
    RefreshDone {
        path: PathBuf,
        result: Box<Result<LoadedRepo, String>>,
    },
    Restored(Result<Vec<PathBuf>, String>),
    Saved(Result<(), String>),
}

impl From<TabsMessage> for Message {
    fn from(message: TabsMessage) -> Self {
        Self::Tabs(message)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum KeyAction {
    NextCommit,
    PreviousCommit,
    OpenRepository,
    OpenTerminal,
    FocusSearch,
    OpenCommandPalette,
    ToggleShortcutOverlay,
    CommandPaletteNext,
    CommandPalettePrevious,
    CommandPaletteRun,
    ReleasePromotion,
    CreateAndPushTag,
    Enter,
    Escape,
    NextHunk,
    PreviousHunk,
    Push,
    RewordSelectedCommit,
    SquashSelectedCommit,
    FixupSelectedCommit,
    EditSelectedCommit,
    DropSelectedCommit,
    TagSelectedCommit,
    CopySelectedCommitHash,
}
