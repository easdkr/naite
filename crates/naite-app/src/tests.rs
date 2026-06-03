#![cfg(test)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::event;
use iced::keyboard::{
    key::{Code, Key, Named, Physical},
    Modifiers,
};
use iced::window;
use naite_core::{
    BranchSyncStatus, ChangeStatus, CommitAuthorAvatar, CommitDiff, CommitPage, CommitPageCursor,
    CommitSummary, DiffLine, FileChange, GitOperationState, HistoryCommit, Hunk, RebaseAction,
    RefKind, RefSummary, Refs, ReleaseBranchSync, ReleaseProfile, ReleaseProfileSuggestion,
    ReleaseSyncCheck, StashSummary, StatusEntry, StatusKind, WorkspaceRepoSummary,
    WorktreeDiffKind, WorktreeDiffTarget, WorktreeStatus, WorktreeStatusDetail, WorktreeSummary,
};

use crate::features::commit::CommitOutcome;
use crate::features::{
    branch_create, branch_manage, checkout, command_palette, commit as commit_feature, discard,
    fetch, history, pull, push, rebase, release_prep, repo_open, stage, stash, tag as tag_feature,
    terminal, workspace, worktree,
};
use crate::message::{KeyAction, Message, TabsMessage};
use crate::state::{
    BranchCreateBase, BranchCreateState, BranchManageRenameState, CommandPaletteState,
    CommitFormState, DiffViewMode, OperationState, ReleasePrepPhase, ReleasePrepState,
    RepositoryState, SelectionState, SidebarClickState, SidebarSection, StashBranchState,
    TagNameMode, TerminalCell, TerminalGridPoint, TerminalImeDeleteAction, TerminalImePreedit,
    TerminalScreen, TerminalSelection, TerminalStatus, TransientStatus, UndoCheckpoint,
};
use crate::subscription::{app_event, keyboard_shortcut, terminal_app_event};
use crate::{
    App, BranchDeletePrompt, BranchDeleteTarget, CheckoutPrompt, CommandId, DiscardPrompt,
    DiscardTarget, ForceSyncPrompt, StashPromptAction, UndoPromptAction,
};

#[test]
fn app_icon_uses_expected_rgba_dimensions() {
    let icon = crate::app_icon::window_icon().expect("embedded app icon should be valid RGBA");
    let (rgba, size) = icon.into_raw();

    assert_eq!(size.width, 256);
    assert_eq!(size.height, 256);
    assert_eq!(rgba.len(), 256 * 256 * 4);
    assert_eq!(rgba.len(), crate::app_icon::raw_icon_bytes().len());
}

fn commit(id: &str, summary: &str, author: &str) -> CommitSummary {
    CommitSummary {
        id: id.to_string(),
        short_id: id.chars().take(7).collect(),
        summary: summary.to_string(),
        author_name: author.to_string(),
        author_email: format!("{author}@example.com"),
        author_avatar_url: None,
        time_seconds: 0,
        parent_ids: Vec::new(),
    }
}

fn dirty_status_detail() -> WorktreeStatusDetail {
    WorktreeStatusDetail {
        unstaged: vec![StatusEntry {
            path: "src/main.rs".into(),
            old_path: None,
            status: StatusKind::Modified,
        }],
        ..Default::default()
    }
}

fn staged_status_detail() -> WorktreeStatusDetail {
    WorktreeStatusDetail {
        staged: vec![StatusEntry {
            path: "src/main.rs".into(),
            old_path: None,
            status: StatusKind::Modified,
        }],
        ..Default::default()
    }
}

fn loaded_repo(path: impl Into<PathBuf>, branch: &str) -> repo_open::LoadedRepo {
    (
        path.into(),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some(branch.into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus::default(),
        GitOperationState::default(),
    )
}

fn worktree_summary(path: &str, branch: &str) -> WorktreeSummary {
    WorktreeSummary {
        path: PathBuf::from(path),
        branch: Some(branch.into()),
        head_short_id: "a111111".into(),
        dirty: false,
        ahead: 0,
        behind: 0,
        locked: false,
        lock_reason: None,
        is_current: false,
    }
}

fn terminal_line(text: &str) -> crate::state::TerminalLine {
    crate::state::TerminalLine {
        cells: text
            .chars()
            .map(|ch| TerminalCell {
                ch,
                ..Default::default()
            })
            .collect(),
    }
}

fn mixed_staged_unstaged_status_detail() -> WorktreeStatusDetail {
    WorktreeStatusDetail {
        staged: vec![StatusEntry {
            path: "src/main.rs".into(),
            old_path: None,
            status: StatusKind::Modified,
        }],
        unstaged: vec![StatusEntry {
            path: "src/main.rs".into(),
            old_path: None,
            status: StatusKind::Modified,
        }],
        ..Default::default()
    }
}

fn untracked_status_detail() -> WorktreeStatusDetail {
    WorktreeStatusDetail {
        untracked: vec![StatusEntry {
            path: "new.rs".into(),
            old_path: None,
            status: StatusKind::Untracked,
        }],
        ..Default::default()
    }
}

fn wip_target(kind: WorktreeDiffKind, path: &str) -> WorktreeDiffTarget {
    WorktreeDiffTarget {
        kind,
        path: path.into(),
    }
}

fn diff_with_hunks(files: Vec<(&str, usize)>) -> CommitDiff {
    let mut hunks_by_file = HashMap::new();
    let file_changes = files
        .into_iter()
        .map(|(path, hunk_count)| {
            let hunks = (0..hunk_count)
                .map(|index| Hunk {
                    old_start: index as u32 + 1,
                    old_lines: 1,
                    new_start: index as u32 + 1,
                    new_lines: 1,
                    header: format!("@@ -{},1 +{},1 @@", index + 1, index + 1),
                    lines: vec![
                        DiffLine::Del(format!("old {index}")),
                        DiffLine::Add(format!("new {index}")),
                    ],
                })
                .collect::<Vec<_>>();
            hunks_by_file.insert(path.to_string(), hunks);
            FileChange {
                path: path.into(),
                status: ChangeStatus::Modified,
                old_path: None,
                is_binary: false,
                is_truncated: false,
            }
        })
        .collect();

    CommitDiff {
        files: file_changes,
        hunks_by_file,
    }
}

fn stash_summary(selector: &str) -> StashSummary {
    StashSummary {
        selector: selector.into(),
        short_id: "abc1234".into(),
        branch: "main".into(),
        date: "2 minutes ago".into(),
        message: "work in progress".into(),
    }
}

fn local_branch(name: &str, is_head: bool) -> RefSummary {
    RefSummary {
        kind: RefKind::LocalBranch,
        short_name: name.into(),
        full_name: format!("refs/heads/{name}"),
        target_short_id: "abc1234".into(),
        is_head,
        sync_status: None,
    }
}

fn local_branch_with_upstream(name: &str, upstream: &str, ahead: u32, behind: u32) -> RefSummary {
    RefSummary {
        sync_status: Some(BranchSyncStatus {
            upstream: Some(upstream.into()),
            ahead,
            behind,
        }),
        ..local_branch(name, false)
    }
}

fn remote_branch(name: &str) -> RefSummary {
    RefSummary {
        kind: RefKind::RemoteBranch,
        short_name: name.into(),
        full_name: format!("refs/remotes/{name}"),
        target_short_id: "abc1234".into(),
        is_head: false,
        sync_status: None,
    }
}

fn tag(name: &str) -> RefSummary {
    RefSummary {
        kind: RefKind::Tag,
        short_name: name.into(),
        full_name: format!("refs/tags/{name}"),
        target_short_id: "abc1234".into(),
        is_head: false,
        sync_status: None,
    }
}

fn rebase_row(id: &str, summary: &str) -> rebase::RebasePlanRow {
    rebase_row_with_author(id, summary, "author", "author@example.com")
}

fn rebase_row_with_author(
    id: &str,
    summary: &str,
    author_name: &str,
    author_email: &str,
) -> rebase::RebasePlanRow {
    rebase::RebasePlanRow {
        action: RebaseAction::Pick,
        commit: HistoryCommit {
            id: id.into(),
            summary: summary.into(),
            author_name: author_name.into(),
            author_email: author_email.into(),
        },
        author_avatar_url: None,
    }
}

#[test]
fn selection_survives_filter_by_commit_id() {
    let app = App {
        repo: RepositoryState {
            commits: vec![
                commit("a111111", "add app shell", "june"),
                commit("b222222", "fix diff pane", "alex"),
            ],
            ..Default::default()
        },
        selection: SelectionState {
            selected_commit_id: Some("b222222".into()),
            ..Default::default()
        },
        search_query: "diff".into(),
        ..Default::default()
    };

    assert_eq!(app.visible_commit_indices(), vec![1]);
    assert_eq!(app.selected_index(), Some(1));
}

#[test]
fn stale_diff_result_is_ignored() {
    let mut app = App {
        repo: RepositoryState {
            commits: vec![
                commit("a111111", "add app shell", "june"),
                commit("b222222", "fix diff pane", "alex"),
            ],
            ..Default::default()
        },
        selection: SelectionState {
            selected_commit_id: Some("b222222".into()),
            ..Default::default()
        },
        operation: OperationState {
            pending_diff_commit_id: Some("b222222".into()),
            diff_loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::DiffLoaded {
        commit_id: "a111111".into(),
        result: Ok((
            CommitDiff::default(),
            naite_core::highlight_diff(&CommitDiff::default()),
        )),
    });

    assert!(app.operation.current_diff.is_none());
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_diff_commit_id.as_deref(),
        Some("b222222")
    );
}

#[test]
fn diff_load_selects_first_hunk_for_selected_file() {
    let mut app = App {
        selection: SelectionState {
            selected_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        operation: OperationState {
            pending_diff_commit_id: Some("a111111".into()),
            diff_loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::DiffLoaded {
        commit_id: "a111111".into(),
        result: Ok({
            let d = diff_with_hunks(vec![("src/main.rs", 2)]);
            let hl = naite_core::highlight_diff(&d);
            (d, hl)
        }),
    });

    assert_eq!(app.selection.selected_file, Some(0));
    assert_eq!(app.selection.selected_hunk, Some(0));
}

#[test]
fn detail_file_selection_resets_hunk_to_first_hunk_in_new_file() {
    let mut app = App {
        selection: SelectionState {
            selected_file: Some(0),
            selected_hunk: Some(1),
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(diff_with_hunks(vec![("src/main.rs", 2), ("src/lib.rs", 1)])),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::DetailFileSelected(1));

    assert_eq!(app.selection.selected_file, Some(1));
    assert_eq!(app.selection.selected_hunk, Some(0));
}

#[test]
fn hunk_navigation_clamps_to_available_hunks() {
    let mut app = App {
        selection: SelectionState {
            selected_file: Some(0),
            selected_hunk: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(diff_with_hunks(vec![("src/main.rs", 2)])),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::DetailPreviousHunk);
    assert_eq!(app.selection.selected_hunk, Some(0));

    let _ = app.update(Message::DetailNextHunk);
    assert_eq!(app.selection.selected_hunk, Some(1));

    let _ = app.update(Message::DetailNextHunk);
    assert_eq!(app.selection.selected_hunk, Some(1));
}

#[test]
fn diff_mode_change_clamps_missing_hunk_selection() {
    let mut app = App {
        selection: SelectionState {
            selected_file: Some(0),
            selected_hunk: None,
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(diff_with_hunks(vec![("src/main.rs", 1)])),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::DiffViewModeChanged(DiffViewMode::FocusedHunk));

    assert_eq!(app.selection.diff_view_mode, DiffViewMode::FocusedHunk);
    assert_eq!(app.selection.selected_hunk, Some(0));

    let _ = app.update(Message::DiffViewModeChanged(DiffViewMode::Inline));
    assert_eq!(app.selection.diff_view_mode, DiffViewMode::Inline);

    let _ = app.update(Message::DiffViewModeChanged(DiffViewMode::Split));
    assert_eq!(app.selection.diff_view_mode, DiffViewMode::Split);
}

#[test]
fn repo_load_selects_wip_when_worktree_is_dirty() {
    let mut app = App::default();

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        dirty_status_detail(),
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 1,
            behind: 2,
        },
        GitOperationState::default(),
    ))))));

    assert_eq!(
        app.repo.sync_status,
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 1,
            behind: 2,
        }
    );
    assert!(app.selection.selected_wip);
    assert!(app.selection.selected_commit_id.is_none());
    assert!(app.operation.current_diff.is_none());
}

#[test]
fn repo_load_with_upstream_starts_auto_fetch() {
    let mut app = App::default();

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    assert_eq!(
        app.operation.auto_fetch_path.as_deref(),
        Some(Path::new("/tmp/naite"))
    );
    assert!(!app.operation.loading);
}

#[test]
fn repo_load_starts_terminal_session_when_panel_is_open() {
    let path = PathBuf::from("/tmp/naite-terminal-open");
    let mut app = App::default();
    app.terminal.open = true;

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok(
        loaded_repo(path.clone(), "main"),
    )))));

    let session = app.terminal.active_session().unwrap();
    assert_eq!(session.target.cwd, path);
    assert_eq!(session.status, TerminalStatus::Starting);
    assert!(session.pending_start);
}

#[test]
fn repo_load_keeps_terminal_session_idle_when_panel_is_closed() {
    let path = PathBuf::from("/tmp/naite-terminal-closed");
    let mut app = App::default();

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok(
        loaded_repo(path.clone(), "main"),
    )))));

    let session = app.terminal.active_session().unwrap();
    assert_eq!(session.target.cwd, path);
    assert_eq!(session.status, TerminalStatus::Idle);
    assert!(!session.pending_start);
}

#[test]
fn cached_tab_activation_starts_terminal_session_when_panel_is_open() {
    let active_path = PathBuf::from("/tmp/naite-active");
    let cached_path = PathBuf::from("/tmp/naite-cached");
    let mut app = App {
        repo: RepositoryState {
            path: Some(active_path.clone()),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    app.tabs.open = vec![active_path.clone(), cached_path.clone()];
    app.tabs.active = Some(active_path);
    app.tabs.cache.insert(
        cached_path.clone(),
        RepositoryState {
            path: Some(cached_path.clone()),
            head_branch: Some("feature/cached".into()),
            ..Default::default()
        },
    );
    app.terminal.open = true;

    let _ = app.update(Message::from(TabsMessage::Activate(cached_path.clone())));

    let session = app.terminal.active_session().unwrap();
    assert_eq!(session.target.cwd, cached_path);
    assert_eq!(session.status, TerminalStatus::Starting);
    assert!(session.pending_start);
}

#[test]
fn cached_tab_activation_prefetches_known_commit_avatars() {
    let active_path = PathBuf::from("/tmp/naite-active");
    let cached_path = PathBuf::from("/tmp/naite-cached");
    let avatar_url = "https://avatars.githubusercontent.com/u/1?v=4".to_string();
    let mut cached_commit = commit("b222222", "update graph", "octocat");
    cached_commit.author_avatar_url = Some(avatar_url.clone());
    let mut app = App {
        repo: RepositoryState {
            path: Some(active_path.clone()),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    app.tabs.open = vec![active_path.clone(), cached_path.clone()];
    app.tabs.active = Some(active_path);
    app.tabs
        .last_refreshed
        .insert(cached_path.clone(), Instant::now());
    app.tabs.cache.insert(
        cached_path.clone(),
        RepositoryState {
            path: Some(cached_path.clone()),
            commits: vec![cached_commit],
            head_branch: Some("feature/cached".into()),
            ..Default::default()
        },
    );

    let _ = app.update(Message::from(TabsMessage::Activate(cached_path)));

    assert!(app.avatars.in_flight.contains(&avatar_url));
}

#[test]
fn closing_active_tab_starts_new_active_terminal_session_when_panel_is_open() {
    let active_path = PathBuf::from("/tmp/naite-active");
    let cached_path = PathBuf::from("/tmp/naite-cached");
    let mut app = App {
        repo: RepositoryState {
            path: Some(active_path.clone()),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    app.tabs.open = vec![active_path.clone(), cached_path.clone()];
    app.tabs.active = Some(active_path.clone());
    app.tabs.cache.insert(
        cached_path.clone(),
        RepositoryState {
            path: Some(cached_path.clone()),
            head_branch: Some("feature/cached".into()),
            ..Default::default()
        },
    );
    app.terminal.open = true;

    let _ = app.update(Message::from(TabsMessage::Close(active_path)));

    let session = app.terminal.active_session().unwrap();
    assert_eq!(session.target.cwd, cached_path);
    assert_eq!(session.status, TerminalStatus::Starting);
    assert!(session.pending_start);
}

#[test]
fn tab_refresh_prefetches_preserved_commit_avatars_for_active_tab() {
    let path = PathBuf::from("/tmp/naite");
    let provider_avatar_url = "https://avatars.githubusercontent.com/u/1?v=4".to_string();
    let mut existing = commit("a111111", "add app shell", "octocat");
    existing.author_avatar_url = Some(provider_avatar_url.clone());
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            commits: vec![existing],
            ..Default::default()
        },
        ..Default::default()
    };
    app.tabs.active = Some(path.clone());
    app.tabs.refreshing.insert(path.clone());

    let fresh = commit("b222222", "update graph", "octocat");
    let _ = app.update(Message::from(TabsMessage::RefreshDone {
        path: path.clone(),
        result: Box::new(Ok((
            path,
            vec![fresh],
            None,
            Refs::default(),
            vec![],
            vec![],
            Some("main".into()),
            WorktreeStatusDetail::default(),
            BranchSyncStatus::default(),
            GitOperationState::default(),
        ))),
    }));

    assert_eq!(
        app.repo.commits[0].author_avatar_url.as_deref(),
        Some(provider_avatar_url.as_str())
    );
    assert!(app.avatars.in_flight.contains(&provider_avatar_url));
}

#[test]
fn provider_avatar_results_update_cached_inactive_tab() {
    let active_path = PathBuf::from("/tmp/repo-active");
    let cached_path = PathBuf::from("/tmp/repo-cached");
    let provider_avatar_url = "https://avatars.githubusercontent.com/u/1?v=4".to_string();
    let cached_commit = commit("b222222", "update graph", "octocat");
    let mut app = App {
        repo: RepositoryState {
            path: Some(active_path.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    app.tabs.active = Some(active_path);
    app.tabs.cache.insert(
        cached_path.clone(),
        RepositoryState {
            path: Some(cached_path.clone()),
            commits: vec![cached_commit],
            ..Default::default()
        },
    );

    let _ = app.update(Message::from(
        repo_open::Message::CommitAuthorAvatarsLoaded {
            path: cached_path.clone(),
            result: Ok(vec![CommitAuthorAvatar {
                commit_id: "b222222".into(),
                author_avatar_url: Some(provider_avatar_url.clone()),
            }]),
        },
    ));

    let cached = app.tabs.cache.get(&cached_path).unwrap();
    assert_eq!(
        cached.commits[0].author_avatar_url.as_deref(),
        Some(provider_avatar_url.as_str())
    );
    assert!(app.avatars.in_flight.contains(&provider_avatar_url));
}

#[test]
fn repo_reload_after_manual_status_does_not_start_auto_fetch() {
    let mut app = App {
        operation: OperationState {
            pending_transient_status_after_reload: Some("Fetched origin/main just now".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    assert!(app.operation.auto_fetch_path.is_none());
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Fetched origin/main just now")
    );
}

#[test]
fn repo_load_stores_stashes() {
    let mut app = App::default();
    let stash = stash_summary("stash@{0}");

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![stash.clone()],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus::default(),
        GitOperationState::default(),
    ))))));

    assert_eq!(app.repo.stashes, vec![stash]);
}

#[test]
fn load_more_commits_appends_page_and_updates_cursor() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            commits: vec![commit("a111111", "first page", "june")],
            commit_page_cursor: Some(CommitPageCursor { offset: 1 }),
            commits_loading_more: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(repo_open::Message::MoreCommitsLoaded {
        path,
        result: Ok(CommitPage {
            commits: vec![
                commit("b222222", "second page", "june"),
                commit("a111111", "duplicate", "june"),
            ],
            next_cursor: Some(CommitPageCursor { offset: 3 }),
        }),
    }));

    assert!(!app.repo.commits_loading_more);
    assert_eq!(
        app.repo
            .commits
            .iter()
            .map(|commit| commit.summary.as_str())
            .collect::<Vec<_>>(),
        vec!["first page", "second page"]
    );
    assert_eq!(
        app.repo.commit_page_cursor,
        Some(CommitPageCursor { offset: 3 })
    );
}

#[test]
fn scrolling_near_end_requests_more_commits() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: (0..20)
                .map(|index| commit(&format!("{index:07}"), "loaded", "june"))
                .collect(),
            commit_page_cursor: Some(CommitPageCursor { offset: 20 }),
            ..Default::default()
        },
        commit_list_scroll_y: 480.0,
        commit_list_viewport_height: 220.0,
        ..Default::default()
    };

    let _ = app.load_more_commits_if_near_end();

    assert!(app.repo.commits_loading_more);
}

#[test]
fn repo_load_prefetches_unique_commit_author_avatars() {
    let mut app = App::default();
    let avatar_url = "https://github.com/octocat.png?size=40".to_string();
    let mut first = commit("a111111", "add app shell", "octocat");
    first.author_avatar_url = Some(avatar_url.clone());
    let mut second = commit("b222222", "update graph", "octocat");
    second.author_avatar_url = Some(avatar_url.clone());
    let third = commit("c333333", "docs", "june");

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![first, second, third],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus::default(),
        GitOperationState::default(),
    ))))));

    assert_eq!(app.avatars.in_flight, HashSet::from([avatar_url]));
}

#[test]
fn avatar_fetched_updates_cache_success_and_failure_state() {
    let mut app = App::default();
    let success_url = "https://github.com/octocat.png?size=40".to_string();
    let failed_url = "https://github.com/missing.png?size=40".to_string();
    app.avatars.in_flight.insert(success_url.clone());
    app.avatars.in_flight.insert(failed_url.clone());

    let _ = app.update(Message::AvatarFetched {
        url: success_url.clone(),
        bytes: Ok(vec![137, 80, 78, 71]),
    });
    let _ = app.update(Message::AvatarFetched {
        url: failed_url.clone(),
        bytes: Err("HTTP 404".into()),
    });

    assert!(app.avatars.handles.contains_key(&success_url));
    assert!(!app.avatars.in_flight.contains(&success_url));
    assert!(app.avatars.failed.contains(&failed_url));
    assert!(!app.avatars.in_flight.contains(&failed_url));
}

#[test]
fn transient_avatar_fetch_failure_can_be_retried() {
    let mut app = App::default();
    let url = "https://github.com/octocat.png?size=40".to_string();
    app.avatars.in_flight.insert(url.clone());

    let _ = app.update(Message::AvatarFetched {
        url: url.clone(),
        bytes: Err("operation timed out".into()),
    });

    assert!(!app.avatars.in_flight.contains(&url));
    assert!(!app.avatars.failed.contains(&url));
    assert!(app.avatars.needs_fetch(&url));
}

#[test]
fn repo_reload_reuses_known_provider_avatar_url_by_author() {
    let mut app = App::default();
    let path = PathBuf::from("/tmp/naite");
    let provider_avatar_url = "https://avatars.githubusercontent.com/u/1?v=4".to_string();
    let mut existing = commit("a111111", "add app shell", "octocat");
    existing.author_avatar_url = Some(provider_avatar_url.clone());
    app.repo.path = Some(path.clone());
    app.repo.commits = vec![existing];

    let mut fresh = commit("b222222", "update graph", "octocat");
    fresh.author_avatar_url = Some("https://github.com/octocat.png?size=128".into());
    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        path,
        vec![fresh],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus::default(),
        GitOperationState::default(),
    ))))));

    assert_eq!(
        app.repo.commits[0].author_avatar_url.as_deref(),
        Some(provider_avatar_url.as_str())
    );
}

#[test]
fn tab_refresh_reuses_cached_avatar_url_by_author() {
    let mut app = App::default();
    let active_path = PathBuf::from("/tmp/repo-active");
    let cached_path = PathBuf::from("/tmp/repo-cached");
    let provider_avatar_url = "https://avatars.githubusercontent.com/u/1?v=4".to_string();
    let mut existing = commit("a111111", "add app shell", "octocat");
    existing.author_avatar_url = Some(provider_avatar_url.clone());
    app.tabs.active = Some(active_path);
    app.tabs.cache.insert(
        cached_path.clone(),
        RepositoryState {
            path: Some(cached_path.clone()),
            commits: vec![existing],
            ..Default::default()
        },
    );
    app.tabs.refreshing.insert(cached_path.clone());

    let fresh = commit("b222222", "update graph", "octocat");
    let _ = app.update(Message::from(TabsMessage::RefreshDone {
        path: cached_path.clone(),
        result: Box::new(Ok((
            cached_path.clone(),
            vec![fresh],
            None,
            Refs::default(),
            vec![],
            vec![],
            Some("main".into()),
            WorktreeStatusDetail::default(),
            BranchSyncStatus::default(),
            GitOperationState::default(),
        ))),
    }));

    let cached = app.tabs.cache.get(&cached_path).unwrap();
    assert_eq!(
        cached.commits[0].author_avatar_url.as_deref(),
        Some(provider_avatar_url.as_str())
    );
}

#[test]
fn wip_selection_clears_commit_diff_state() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("a111111", "add app shell", "june")],
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            selected_file: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(CommitDiff::default()),
            diff_loading: true,
            pending_diff_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::WipSelected);

    assert!(app.selection.selected_wip);
    assert!(app.selection.selected.is_none());
    assert!(app.selection.selected_commit_id.is_none());
    assert!(app.selection.selected_file.is_none());
    assert!(app.operation.current_diff.is_none());
    assert!(app.operation.diff_loading);
    assert!(app.operation.pending_diff_commit_id.is_none());
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs"))
    );
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn stash_selection_clears_commit_and_wip_state() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            stashes: vec![stash_summary("stash@{0}")],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            selected_file: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(CommitDiff::default()),
            pending_diff_commit_id: Some("a111111".into()),
            pending_wip_diff_target: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            diff_loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::StashSelected(stash_summary("stash@{0}")));

    assert_eq!(
        app.selection
            .selected_stash
            .as_ref()
            .map(|stash| stash.selector.as_str()),
        Some("stash@{0}")
    );
    assert!(app.selection.selected.is_none());
    assert!(app.selection.selected_commit_id.is_none());
    assert!(!app.selection.selected_wip);
    assert!(app.selection.selected_wip_file.is_none());
    assert!(app.selection.selected_file.is_none());
    assert!(app.operation.current_diff.is_none());
    assert!(app.operation.diff_loading);
    assert!(app.operation.pending_diff_commit_id.is_none());
    assert!(app.operation.pending_wip_diff_target.is_none());
    assert_eq!(
        app.operation.pending_stash_diff_selector.as_deref(),
        Some("stash@{0}")
    );
}

#[test]
fn stale_stash_diff_result_is_ignored() {
    let mut app = App {
        selection: SelectionState {
            selected_stash: Some(stash_summary("stash@{1}")),
            ..Default::default()
        },
        operation: OperationState {
            pending_stash_diff_selector: Some("stash@{1}".into()),
            diff_loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::StashDiffLoaded {
        selector: "stash@{0}".into(),
        result: Ok((
            CommitDiff::default(),
            naite_core::highlight_diff(&CommitDiff::default()),
        )),
    });

    assert!(app.operation.current_diff.is_none());
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_stash_diff_selector.as_deref(),
        Some("stash@{1}")
    );
}

#[test]
fn wip_selection_starts_first_file_diff_by_status_priority() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: WorktreeStatusDetail {
                staged: vec![StatusEntry {
                    path: "staged.rs".into(),
                    old_path: None,
                    status: StatusKind::Modified,
                }],
                unstaged: vec![StatusEntry {
                    path: "unstaged.rs".into(),
                    old_path: None,
                    status: StatusKind::Modified,
                }],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::WipSelected);

    assert!(app.selection.selected_wip);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Staged, "staged.rs"))
    );
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn wip_file_selection_clears_commit_diff_state() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            selected_file: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(CommitDiff::default()),
            diff_loading: true,
            pending_diff_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let target = wip_target(WorktreeDiffKind::Unstaged, "src/main.rs");
    let _ = app.update(Message::WipStatusPathSelected(target.clone()));

    assert!(app.selection.selected_wip);
    assert!(app.selection.selected.is_none());
    assert!(app.selection.selected_commit_id.is_none());
    assert_eq!(app.selection.selected_wip_file, Some(target.clone()));
    assert!(app.selection.selected_file.is_none());
    assert!(app.operation.current_diff.is_none());
    assert!(app.operation.diff_loading);
    assert!(app.operation.pending_diff_commit_id.is_none());
    assert_eq!(app.operation.pending_wip_diff_target, Some(target));
}

#[test]
fn stale_wip_diff_result_is_ignored() {
    let selected = wip_target(WorktreeDiffKind::Unstaged, "src/main.rs");
    let stale = wip_target(WorktreeDiffKind::Staged, "src/main.rs");
    let mut app = App {
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(selected.clone()),
            ..Default::default()
        },
        operation: OperationState {
            pending_wip_diff_target: Some(selected.clone()),
            diff_loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::WipDiffLoaded {
        target: stale,
        result: Ok((
            CommitDiff::default(),
            naite_core::highlight_diff(&CommitDiff::default()),
        )),
    });

    assert!(app.operation.current_diff.is_none());
    assert!(app.operation.diff_loading);
    assert_eq!(app.operation.pending_wip_diff_target, Some(selected));
}

#[test]
fn focused_window_event_requests_status_refresh() {
    let message = app_event(
        iced::Event::Window(window::Event::Focused),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(message, Some(Message::WindowFocused)));
}

#[test]
fn focus_refresh_preserves_commit_selection_while_updating_status() {
    let mut app = App {
        repo: RepositoryState {
            status_detail: WorktreeStatusDetail::default(),
            commits: vec![commit("a111111", "add app shell", "june")],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            selected_file: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            current_diff: Some(CommitDiff::default()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::WorktreeStatusDetailLoaded(Ok(
        dirty_status_detail(),
    )));

    assert!(!app.operation.loading);
    assert_eq!(app.repo.status_detail, dirty_status_detail());
    assert_eq!(app.selection.selected, Some(0));
    assert_eq!(app.selection.selected_commit_id.as_deref(), Some("a111111"));
    assert!(!app.selection.selected_wip);
    assert!(app.operation.current_diff.is_some());
}

#[test]
fn focus_refresh_reloads_selected_wip_diff() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::WorktreeStatusDetailLoaded(Ok(
        staged_status_detail(),
    )));

    assert!(!app.operation.loading);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Staged, "src/main.rs"))
    );
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn focus_refresh_clears_wip_selection_when_worktree_becomes_clean() {
    let mut app = App {
        repo: RepositoryState {
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            selected_file: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            current_diff: Some(CommitDiff::default()),
            diff_loading: true,
            pending_wip_diff_target: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::WorktreeStatusDetailLoaded(Ok(
        WorktreeStatusDetail::default(),
    )));

    assert!(!app.operation.loading);
    assert!(!app.selection.selected_wip);
    assert!(app.selection.selected_wip_file.is_none());
    assert!(app.selection.selected_file.is_none());
    assert!(app.operation.current_diff.is_none());
    assert!(!app.operation.diff_loading);
    assert!(app.operation.pending_wip_diff_target.is_none());
}

#[test]
fn stage_success_reselects_same_path_in_new_status_group() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stage::Message::Done(Ok(
        staged_status_detail(),
    ))));

    assert!(!app.operation.loading);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Staged, "src/main.rs"))
    );
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn stage_operation_success_updates_status_and_keeps_wip_selection() {
    let mut app = App {
        repo: RepositoryState {
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stage::Message::Done(Ok(
        staged_status_detail(),
    ))));

    assert!(!app.operation.loading);
    assert!(app.selection.selected_wip);
    assert_eq!(app.repo.status_detail, staged_status_detail());
    assert!(app.operation.error.is_none());
}

#[test]
fn hunk_stage_success_keeps_unstaged_selection_when_path_remains_unstaged() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stage::Message::Done(Ok(
        mixed_staged_unstaged_status_detail(),
    ))));

    assert!(!app.operation.loading);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs"))
    );
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn hunk_unstage_success_keeps_staged_selection_when_path_remains_staged() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Staged, "src/main.rs")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stage::Message::Done(Ok(
        mixed_staged_unstaged_status_detail(),
    ))));

    assert!(!app.operation.loading);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Staged, "src/main.rs"))
    );
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn hunk_unstage_success_moves_to_unstaged_when_staged_path_is_gone() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Staged, "src/main.rs")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stage::Message::Done(Ok(
        dirty_status_detail(),
    ))));

    assert!(!app.operation.loading);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs"))
    );
    assert_eq!(
        app.operation.pending_wip_diff_target,
        app.selection.selected_wip_file
    );
}

#[test]
fn stage_operation_error_preserves_existing_status() {
    let original_status = dirty_status_detail();
    let mut app = App {
        repo: RepositoryState {
            status_detail: original_status.clone(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stage::Message::Done(
        Err("git failed".into()),
    )));

    assert!(!app.operation.loading);
    assert!(app.selection.selected_wip);
    assert_eq!(app.repo.status_detail, original_status);
    assert_eq!(app.operation.error.as_deref(), Some("git failed"));
}

#[test]
fn discard_file_request_opens_confirmation_without_mutating_status() {
    let original_status = dirty_status_detail();
    let target = wip_target(WorktreeDiffKind::Unstaged, "src/main.rs");
    let mut app = App {
        repo: RepositoryState {
            status_detail: original_status.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(discard::Message::FileRequested(
        target.clone(),
    )));

    assert_eq!(app.repo.status_detail, original_status);
    assert_eq!(
        app.selection.discard_confirmation,
        Some(DiscardPrompt {
            target: DiscardTarget::File(target),
        })
    );
}

#[test]
fn discard_cancel_closes_confirmation() {
    let mut app = App {
        selection: SelectionState {
            discard_confirmation: Some(DiscardPrompt {
                target: DiscardTarget::File(wip_target(WorktreeDiffKind::Untracked, "new.rs")),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(discard::Message::Cancelled));

    assert!(app.selection.discard_confirmation.is_none());
}

#[test]
fn discard_success_updates_status_and_keeps_wip_selection() {
    let mut app = App {
        repo: RepositoryState {
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            discard_confirmation: Some(DiscardPrompt {
                target: DiscardTarget::File(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            }),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(discard::Message::Done(Ok(
        untracked_status_detail(),
    ))));

    assert!(!app.operation.loading);
    assert!(app.selection.discard_confirmation.is_none());
    assert!(app.selection.selected_wip);
    assert_eq!(app.repo.status_detail, untracked_status_detail());
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Untracked, "new.rs"))
    );
}

#[test]
fn discard_error_preserves_existing_status() {
    let original_status = dirty_status_detail();
    let mut app = App {
        repo: RepositoryState {
            status_detail: original_status.clone(),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            discard_confirmation: Some(DiscardPrompt {
                target: DiscardTarget::File(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            }),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(discard::Message::Done(Err(
        "git failed".into()
    ))));

    assert!(!app.operation.loading);
    assert!(app.selection.discard_confirmation.is_none());
    assert!(app.selection.selected_wip);
    assert_eq!(app.repo.status_detail, original_status);
    assert_eq!(app.operation.error.as_deref(), Some("git failed"));
}

#[test]
fn escape_closes_discard_confirmation_first() {
    let mut app = App {
        selection: SelectionState {
            selected_wip: true,
            selected_wip_file: Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            discard_confirmation: Some(DiscardPrompt {
                target: DiscardTarget::File(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs")),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(app.selection.discard_confirmation.is_none());
    assert!(app.selection.selected_wip);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs"))
    );
}

#[test]
fn commit_request_does_not_start_without_title_or_staged_changes() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(commit_feature::Message::Requested));
    assert!(!app.operation.loading);

    app.commit_form.title = "commit title".into();
    app.repo.status_detail = dirty_status_detail();

    let _ = app.update(Message::from(commit_feature::Message::Requested));
    assert!(!app.operation.loading);
}

#[test]
fn commit_request_does_not_start_push_after_without_branch() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            head_branch: None,
            ..Default::default()
        },
        commit_form: CommitFormState {
            title: "commit title".into(),
            push_after: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(commit_feature::Message::Requested));

    assert!(!app.operation.loading);
}

#[test]
fn commit_form_messages_update_advanced_options() {
    let mut app = App::default();

    let _ = app.update(Message::from(commit_feature::Message::CoAuthorsChanged(
        "Ada <ada@example.com>; Grace <grace@example.com>".into(),
    )));
    let _ = app.update(Message::from(commit_feature::Message::AmendChanged(true)));
    let _ = app.update(Message::from(commit_feature::Message::SkipHooksChanged(
        true,
    )));
    let _ = app.update(Message::from(commit_feature::Message::PushAfterChanged(
        true,
    )));

    assert_eq!(
        app.commit_form.co_authors,
        "Ada <ada@example.com>; Grace <grace@example.com>"
    );
    assert!(app.commit_form.amend);
    assert!(app.commit_form.skip_hooks);
    assert!(app.commit_form.push_after);
}

#[test]
fn commit_success_clears_form_and_starts_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            ..Default::default()
        },
        commit_form: CommitFormState {
            title: "commit title".into(),
            body: "body".into(),
            co_authors: "Ada <ada@example.com>".into(),
            amend: true,
            skip_hooks: true,
            push_after: true,
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(commit_feature::Message::Done(Ok(
        CommitOutcome { pushed: true },
    ))));

    assert!(app.operation.loading);
    assert!(app.commit_form.title.is_empty());
    assert!(app.commit_form.body.is_empty());
    assert!(app.commit_form.co_authors.is_empty());
    assert!(!app.commit_form.amend);
    assert!(!app.commit_form.skip_hooks);
    assert!(!app.commit_form.push_after);
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Committed and pushed current branch")
    );
    assert!(app.operation.error.is_none());
}

#[test]
fn commit_error_preserves_form_and_existing_status() {
    let original_status = staged_status_detail();
    let mut app = App {
        repo: RepositoryState {
            status_detail: original_status.clone(),
            ..Default::default()
        },
        commit_form: CommitFormState {
            title: "commit title".into(),
            body: "body".into(),
            co_authors: "Ada <ada@example.com>".into(),
            amend: true,
            skip_hooks: true,
            push_after: true,
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(commit_feature::Message::Done(Err(
        "nothing to commit".into(),
    ))));

    assert!(!app.operation.loading);
    assert_eq!(app.commit_form.title, "commit title");
    assert_eq!(app.commit_form.body, "body");
    assert_eq!(app.commit_form.co_authors, "Ada <ada@example.com>");
    assert!(app.commit_form.amend);
    assert!(app.commit_form.skip_hooks);
    assert!(app.commit_form.push_after);
    assert_eq!(app.repo.status_detail, original_status);
    assert_eq!(app.operation.error.as_deref(), Some("nothing to commit"));
}

#[test]
fn command_palette_open_resets_query_and_selection() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: false,
            query: "commit".into(),
            selected: 3,
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Opened));

    assert!(app.command_palette.open);
    assert!(app.command_palette.query.is_empty());
    assert_eq!(app.command_palette.selected, 0);
}

#[test]
fn command_palette_query_filters_commands_and_resets_selection() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            query: String::new(),
            selected: 4,
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::QueryChanged(
        "create a commit".into(),
    )));

    assert_eq!(app.command_palette.selected, 0);
    let labels: Vec<_> = app
        .filtered_command_palette_items()
        .into_iter()
        .map(|item| item.label)
        .collect();
    assert_eq!(labels, vec!["Commit"]);
}

#[test]
fn escape_closes_palette_without_clearing_commit_search_state() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            query: "stage".into(),
            selected: 1,
        },
        search_query: "diff".into(),
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(!app.command_palette.open);
    assert_eq!(app.command_palette.query, "stage");
    assert_eq!(app.search_query, "diff");
    assert_eq!(app.selection.selected, Some(0));
    assert_eq!(app.selection.selected_commit_id.as_deref(), Some("a111111"));
}

#[test]
fn escape_closes_branch_create_form_after_palette_priority() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        branch_create: BranchCreateState {
            open: true,
            name: "feature/demo".into(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(!app.command_palette.open);
    assert!(app.branch_create.open);

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(!app.branch_create.open);
    assert_eq!(app.branch_create.name, "feature/demo");
}

#[test]
fn escape_closes_shortcut_and_display_overlays_before_clearing_search() {
    let mut app = App {
        search_query: "release".into(),
        ..Default::default()
    };
    app.preferences.shortcuts_open = true;
    app.preferences.display_options_open = true;

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(!app.preferences.shortcuts_open);
    assert!(app.preferences.display_options_open);
    assert_eq!(app.search_query, "release");

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(!app.preferences.display_options_open);
    assert_eq!(app.search_query, "release");
}

#[test]
fn disabled_commit_command_does_not_start_loading() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::Commit,
    )));

    assert!(!app.operation.loading);
    assert!(app.command_palette.open);
}

#[test]
fn commit_command_requires_branch_when_push_after_is_enabled() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: staged_status_detail(),
            head_branch: None,
            ..Default::default()
        },
        commit_form: CommitFormState {
            title: "commit title".into(),
            push_after: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let commit = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Commit)
        .unwrap();
    assert_eq!(commit.disabled_reason, Some("Current HEAD is detached"));
}

#[test]
fn create_branch_command_requires_repo_and_idle_operation() {
    let app = App::default();
    let create_branch = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateBranch)
        .unwrap();
    assert_eq!(
        create_branch.disabled_reason,
        Some("Open a repository first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let create_branch = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateBranch)
        .unwrap();
    assert_eq!(create_branch.disabled_reason, Some("Operation in progress"));
}

#[test]
fn branch_manage_commands_require_selected_local_branch() {
    let app = App::default();
    let rename = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::RenameSelectedBranch)
        .unwrap();
    assert_eq!(rename.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(remote_branch("origin/main")),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let rename = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::RenameSelectedBranch)
        .unwrap();
    let delete = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::DeleteSelectedBranch)
        .unwrap();
    assert_eq!(
        rename.disabled_reason,
        Some("Open a local branch menu first")
    );
    assert_eq!(
        delete.disabled_reason,
        Some("Open a local branch menu first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(local_branch("main", true)),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let rename = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::RenameSelectedBranch)
        .unwrap();
    let delete = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::DeleteSelectedBranch)
        .unwrap();
    assert!(rename.enabled());
    assert_eq!(delete.disabled_reason, Some("Cannot delete current branch"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(local_branch("feature/demo", false)),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let delete = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::DeleteSelectedBranch)
        .unwrap();
    assert!(delete.enabled());
}

#[test]
fn history_commands_use_selected_branch_and_clean_worktree() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(local_branch("feature/demo", false)),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let merge = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::MergeSelectedRef)
        .unwrap();
    assert!(merge.enabled());

    let _ = app.run_command_palette_command(CommandId::MergeSelectedRef);
    assert!(matches!(
        app.selection
            .history_confirmation
            .as_ref()
            .map(|prompt| &prompt.operation),
        Some(history::Operation::Merge(target)) if target.short_name == "feature/demo"
    ));

    let dirty_app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(local_branch("feature/demo", false)),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let rebase = dirty_app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::RebaseOntoSelectedRef)
        .unwrap();
    assert_eq!(
        rebase.disabled_reason,
        Some("Commit, stash, or resolve local changes first")
    );
}

#[test]
fn commit_history_tag_and_file_inspection_commands_open_phase2_surfaces() {
    let diff = diff_with_hunks(vec![("src/main.rs", 1)]);
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("a111111", "add app shell", "june")],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_file: Some(0),
            ..Default::default()
        },
        operation: OperationState {
            current_diff: Some(diff),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.run_command_palette_command(CommandId::RewordSelectedCommit);
    assert!(app.history_reword.open);
    assert_eq!(
        app.history_reword
            .commit
            .as_ref()
            .map(|commit| commit.short_id.as_str()),
        Some("a111111")
    );

    let _ = app.run_command_palette_command(CommandId::CreateTag);
    assert!(app.tag_create.open);
    assert_eq!(
        app.tag_create
            .target_commit
            .as_ref()
            .map(|commit| commit.short_id.as_str()),
        Some("a111111")
    );

    let _ = app.run_command_palette_command(CommandId::ShowFileHistory);
    assert!(app.file_insight.loading);
    assert_eq!(app.file_insight.path.as_deref(), Some("src/main.rs"));
}

#[test]
fn tag_create_defaults_to_timestamp_name() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.open_tag_create_form(None);

    assert!(app.tag_create.open);
    assert_eq!(app.tag_create.name_mode, TagNameMode::Timestamp);
    assert!(app.tag_create.name.starts_with('v'));
    assert!(!app.tag_create.name.trim().is_empty());
}

#[test]
fn tag_deployment_command_shows_keyboard_shortcut() {
    let app = App::default();
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateAndPushTag)
        .unwrap();

    assert_eq!(item.label, "Create and push tag");
    assert_eq!(item.shortcut, "Cmd Shift T");
    assert_eq!(item.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateAndPushTag)
        .unwrap();

    assert_eq!(item.disabled_reason, None);
}

#[test]
fn tag_deployment_command_opens_push_enabled_tag_modal() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("a111111", "prepare release tag", "june")],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.handle_key_action(KeyAction::CreateAndPushTag);

    assert!(!app.command_palette.open);
    assert!(app.tag_create.open);
    assert!(app.tag_create.push_after_create);
    assert_eq!(
        app.tag_create
            .target_commit
            .as_ref()
            .map(|commit| commit.short_id.as_str()),
        Some("a111111")
    );
}

#[test]
fn tag_create_from_context_menu_closes_context_menu() {
    let commit = commit("a111111", "add tag modal", "june");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Commit(commit.clone()),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update_tag(tag_feature::Message::CreateRequested(Some(commit)));

    assert!(app.tag_create.open);
    assert!(app.selection.context_menu.is_none());
}

#[test]
fn tag_delete_from_context_menu_closes_context_menu() {
    let target = tag("v1.0.0");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(target.clone()),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update_tag(tag_feature::Message::DeleteRequested(target));

    assert!(app.selection.tag_delete_confirmation.is_some());
    assert!(app.selection.context_menu.is_none());
}

#[test]
fn tag_create_semver_mode_suggests_next_patch_or_initial_version() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                tags: vec![
                    tag("v1.2.3"),
                    tag("v1.10.1"),
                    tag("v2026.3.30"),
                    tag("not-semver"),
                ],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.open_tag_create_form(None);
    let _ = app.update_tag(tag_feature::Message::CreateNameModeChanged(
        TagNameMode::SemVerNext,
    ));

    assert_eq!(app.tag_create.name, "v1.10.2");

    app.repo.refs.tags.clear();
    let _ = app.update_tag(tag_feature::Message::CreateNameModeChanged(
        TagNameMode::SemVerNext,
    ));

    assert_eq!(app.tag_create.name, "v0.1.0");
}

#[test]
fn tag_create_branch_slug_mode_uses_collision_free_branch_name() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("Feature/JIRA-123 Add tag UX".into()),
            refs: Refs {
                tags: vec![tag("feature-jira-123-add-tag-ux")],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.open_tag_create_form(None);
    let _ = app.update_tag(tag_feature::Message::CreateNameModeChanged(
        TagNameMode::BranchSlug,
    ));

    assert_eq!(app.tag_create.name, "feature-jira-123-add-tag-ux-2");
}

#[test]
fn tag_create_tracks_push_after_create_option() {
    let mut app = App::default();

    let _ = app.update_tag(tag_feature::Message::CreatePushAfterChanged(true));

    assert!(app.tag_create.push_after_create);
    assert_eq!(
        tag_feature::Operation::Create {
            name: "v1.0.0".into(),
            push_after_create: app.tag_create.push_after_create,
            target_commit: None,
        }
        .success_message(),
        "Created and pushed tag v1.0.0"
    );
}

#[test]
fn rebase_selection_tracks_operation_shown_in_detail_pane() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("author@example.com".into()),
            plan: vec![
                rebase_row("a111111", "first commit"),
                rebase_row("b222222", "second commit"),
                rebase_row("c333333", "third commit"),
            ],
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::RowSelected(1)));

    assert_eq!(
        app.rebase
            .as_ref()
            .and_then(rebase::InteractiveRebaseSession::selected_row)
            .map(|row| row.commit.id.as_str()),
        Some("b222222")
    );
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_diff_commit_id.as_deref(),
        Some("b222222")
    );

    let _ = app.update(Message::from(rebase::Message::ActionSet(
        1,
        RebaseAction::Drop,
    )));

    let selected = app
        .rebase
        .as_ref()
        .and_then(rebase::InteractiveRebaseSession::selected_row)
        .expect("selected rebase row should remain available");
    assert_eq!(selected.commit.summary, "second commit");
    assert_eq!(selected.action, RebaseAction::Drop);
}

#[test]
fn pick_mine_marks_only_matching_rebase_commits_for_replay() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("June@Example.com".into()),
            plan: vec![
                rebase_row_with_author("a111111", "mine one", "June", "june@example.com"),
                rebase_row_with_author("b222222", "teammate", "Alex", "alex@example.com"),
                rebase_row_with_author("c333333", "mine two", "June", "JUNE@example.com"),
            ],
            selected: 1,
            drag: None,
            reword_drafts: HashMap::from([("b222222".into(), "renamed".into())]),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::PickMineRequested));

    let session = app
        .rebase
        .as_ref()
        .expect("rebase session should remain open");
    assert_eq!(session.selected, 0);
    assert!(session.reword_drafts.is_empty());
    assert_eq!(session.plan[0].commit.id, "a111111");
    assert_eq!(session.plan[1].commit.id, "c333333");
    assert_eq!(session.plan[2].commit.id, "b222222");
    assert_eq!(session.plan[0].action, RebaseAction::Pick);
    assert_eq!(session.plan[1].action, RebaseAction::Pick);
    assert_eq!(session.plan[2].action, RebaseAction::Drop);
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Kept 2 authored commits and marked 1 for drop")
    );
    assert!(app.operation.diff_loading);
    assert_eq!(
        app.operation.pending_diff_commit_id.as_deref(),
        Some("a111111")
    );
}

#[test]
fn squash_mine_groups_authored_commits_at_first_authored_position() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("june@example.com".into()),
            plan: vec![
                rebase_row_with_author("a111111", "teammate before", "Alex", "alex@example.com"),
                rebase_row_with_author("b222222", "mine one", "June", "june@example.com"),
                rebase_row_with_author("c333333", "teammate middle", "Alex", "alex@example.com"),
                rebase_row_with_author("d444444", "mine two", "June", "JUNE@example.com"),
            ],
            selected: 2,
            drag: None,
            reword_drafts: HashMap::from([("c333333".into(), "stale".into())]),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::PresetRequested(
        rebase::RebasePlanPreset::SquashMine,
    )));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert_eq!(session.selected, 0);
    assert_eq!(
        session
            .plan
            .iter()
            .map(|row| row.commit.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a111111", "b222222", "d444444", "c333333"]
    );
    assert_eq!(session.plan[0].action, RebaseAction::Pick);
    assert_eq!(session.plan[1].action, RebaseAction::Reword);
    assert_eq!(session.plan[2].action, RebaseAction::Fixup);
    assert_eq!(session.plan[3].action, RebaseAction::Pick);
    assert_eq!(
        session.reword_drafts.get("b222222").map(String::as_str),
        Some("mine one")
    );
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Grouped and squashed 2 authored commits")
    );
}

#[test]
fn squash_mine_keeps_single_authored_commit_without_fixup() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("june@example.com".into()),
            plan: vec![
                rebase_row_with_author("a111111", "teammate", "Alex", "alex@example.com"),
                rebase_row_with_author("b222222", "mine", "June", "june@example.com"),
            ],
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::PresetRequested(
        rebase::RebasePlanPreset::SquashMine,
    )));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert_eq!(session.plan[0].commit.id, "a111111");
    assert_eq!(session.plan[0].action, RebaseAction::Pick);
    assert_eq!(session.plan[1].commit.id, "b222222");
    assert_eq!(session.plan[1].action, RebaseAction::Pick);
    assert!(session.reword_drafts.is_empty());
}

#[test]
fn squash_mine_without_matching_author_leaves_plan_unchanged() {
    let original_plan = vec![rebase_row_with_author(
        "a111111",
        "teammate",
        "Alex",
        "alex@example.com",
    )];
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("june@example.com".into()),
            plan: original_plan.clone(),
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::PresetRequested(
        rebase::RebasePlanPreset::SquashMine,
    )));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert_eq!(session.plan, original_plan);
    assert_eq!(
        app.operation.error.as_deref(),
        Some("No commits match configured author email june@example.com")
    );
}

#[test]
fn squash_mine_without_configured_author_leaves_plan_unchanged() {
    let original_plan = vec![rebase_row_with_author(
        "a111111",
        "mine",
        "June",
        "june@example.com",
    )];
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: None,
            plan: original_plan.clone(),
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::PresetRequested(
        rebase::RebasePlanPreset::SquashMine,
    )));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert_eq!(session.plan, original_plan);
    assert_eq!(
        app.operation.error.as_deref(),
        Some("git user.email is not configured for this repository")
    );
}

#[test]
fn apply_then_force_push_sets_pending_prompt_after_successful_rebase() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            head_branch: Some("feature/demo".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/feature/demo".into()),
                ahead: 1,
                behind: 0,
            },
            commits: vec![commit("abcdef123", "head", "june")],
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("june@example.com".into()),
            plan: vec![rebase_row("a111111", "first commit")],
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: true,
            scroll_offset: 0.0,
        }),
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::Done {
        result: Ok(rebase::task::ApplyOutcome::Applied),
        apply_mode: rebase::RebaseApplyMode::RebaseThenForcePush,
    }));

    assert!(app.rebase.is_none());
    assert!(app.operation.pending_force_push_after_reload);
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Interactive rebase applied; confirm force push")
    );
}

#[test]
fn release_promotion_rebase_rejects_apply_then_force_push() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path),
            head_branch: Some("staging".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/staging".into()),
                ahead: 1,
                behind: 0,
            },
            commits: vec![commit("abcdef123", "head", "june")],
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("staging", true),
            target: local_branch("main", false),
            current_author_email: Some("june@example.com".into()),
            plan: vec![rebase_row("a111111", "first commit")],
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: false,
            scroll_offset: 0.0,
        }),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::ApplyRequested(
        rebase::RebaseApplyMode::RebaseThenForcePush,
    )));

    assert!(app.selection.rebase_confirmation.is_none());
    assert_eq!(
        app.operation.error.as_deref(),
        Some("release promotion applies the rebase locally; push the target, then sync the source")
    );
}

#[test]
fn release_promotion_auto_rebase_sets_pending_pipeline() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path),
            head_branch: Some("staging".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/staging".into()),
                ahead: 1,
                behind: 0,
            },
            commits: vec![commit("abcdef123", "head", "june")],
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("staging", true),
            target: local_branch("main", false),
            current_author_email: Some("june@example.com".into()),
            plan: vec![rebase_row("a111111", "first commit")],
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: true,
            scroll_offset: 0.0,
        }),
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::Done {
        result: Ok(rebase::task::ApplyOutcome::Applied),
        apply_mode: rebase::RebaseApplyMode::ReleasePromotionAuto,
    }));

    assert!(app.release_prep.auto_running);
    assert_eq!(
        app.release_prep.auto_next_action,
        Some(release_prep::ReleasePrepAction::UpdateTargetFromSource)
    );
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Interactive rebase applied; starting auto promotion")
    );
}

#[test]
fn release_promotion_auto_continues_after_repo_reload() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            ..Default::default()
        },
        operation: OperationState {
            pending_transient_status_after_reload: Some(
                "Interactive rebase applied; starting auto promotion".into(),
            ),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            auto_running: true,
            auto_next_action: Some(release_prep::ReleasePrepAction::UpdateTargetFromSource),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        path,
        vec![commit("abcdef123", "head", "june")],
        None,
        Refs::default(),
        Vec::new(),
        Vec::new(),
        Some("staging".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/staging".into()),
            ahead: 1,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    assert!(app.release_prep.auto_running);
    assert_eq!(app.release_prep.auto_next_action, None);
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::RunningAction);
    assert!(app.operation.loading);
}

#[test]
fn release_promotion_auto_mode_ignores_manual_controls() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            phase: ReleasePrepPhase::Actions,
            auto_running: true,
            auto_next_action: Some(release_prep::ReleasePrepAction::PushTarget),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(release_prep::Message::ActionRequested(
        release_prep::ReleasePrepAction::UpdateTargetFromSource,
    )));

    assert!(app.release_prep.auto_running);
    assert_eq!(
        app.release_prep.auto_next_action,
        Some(release_prep::ReleasePrepAction::PushTarget)
    );
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Actions);
    assert!(!app.operation.loading);

    let _ = app.update(Message::from(release_prep::Message::Cancelled));
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Actions);
    assert!(app.release_prep.auto_running);

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Actions);
    assert!(app.release_prep.auto_running);

    let release_command = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ReleasePushTarget)
        .unwrap();
    assert_eq!(
        release_command.disabled_reason,
        Some("Auto promotion in progress")
    );
}

#[test]
fn release_promotion_action_waits_for_auto_fetch_to_finish() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            ..Default::default()
        },
        operation: OperationState {
            auto_fetch_path: Some(path.clone()),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            phase: ReleasePrepPhase::Actions,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(release_prep::Message::ActionRequested(
        release_prep::ReleasePrepAction::UpdateTargetFromSource,
    )));

    assert_eq!(
        app.operation.auto_fetch_path.as_deref(),
        Some(path.as_path())
    );
    assert!(!app.operation.loading);
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Actions);
    assert_eq!(app.release_prep.active_action, None);
}

#[test]
fn release_promotion_auto_waits_for_auto_fetch_without_losing_next_action() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            ..Default::default()
        },
        operation: OperationState {
            auto_fetch_path: Some(path.clone()),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            phase: ReleasePrepPhase::Actions,
            auto_running: true,
            auto_next_action: Some(release_prep::ReleasePrepAction::UpdateTargetFromSource),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.continue_release_prep_auto();

    assert_eq!(
        app.operation.auto_fetch_path.as_deref(),
        Some(path.as_path())
    );
    assert!(!app.operation.loading);
    assert!(app.release_prep.auto_running);
    assert_eq!(
        app.release_prep.auto_next_action,
        Some(release_prep::ReleasePrepAction::UpdateTargetFromSource)
    );
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Actions);
    assert_eq!(app.release_prep.active_action, None);
}

#[test]
fn release_promotion_completed_steps_cannot_run_again() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            completed_actions: vec![
                release_prep::ReleasePrepAction::UpdateTargetFromSource,
                release_prep::ReleasePrepAction::PushTarget,
                release_prep::ReleasePrepAction::SyncSourceFromTarget,
            ],
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(release_prep::Message::ActionRequested(
        release_prep::ReleasePrepAction::PushTarget,
    )));

    assert!(!app.operation.loading);
    assert!(!app.release_prep.auto_running);

    let _ = app.update(Message::from(release_prep::Message::AutoRequested));

    assert!(!app.operation.loading);
    assert!(!app.release_prep.auto_running);
    assert_eq!(app.release_prep.auto_next_action, None);
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Auto promotion already complete")
    );

    let release_command = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ReleasePushTarget)
        .unwrap();
    assert_eq!(
        release_command.disabled_reason,
        Some("Release step already complete")
    );
}

#[test]
fn force_push_prompt_opens_after_pending_rebase_reload() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        operation: OperationState {
            pending_force_push_after_reload: true,
            pending_transient_status_after_reload: Some(
                "Interactive rebase applied; confirm force push".into(),
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        path,
        vec![commit("abcdef123", "head", "june")],
        None,
        Refs::default(),
        Vec::new(),
        Vec::new(),
        Some("feature/demo".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/feature/demo".into()),
            ahead: 1,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    let prompt = app
        .selection
        .force_push_confirmation
        .as_ref()
        .expect("force push prompt should open");
    assert_eq!(prompt.branch, "feature/demo");
    assert_eq!(prompt.upstream, "origin/feature/demo");
    assert_eq!(prompt.head_short_id, "abcdef1");
    assert!(!app.operation.pending_force_push_after_reload);
}

#[test]
fn interactive_rebase_conflict_reloads_into_rebase_abort_ux() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            current_branch: local_branch("feature/demo", true),
            target: local_branch("main", false),
            current_author_email: Some("author@example.com".into()),
            plan: vec![rebase_row("a111111", "first commit")],
            selected: 0,
            drag: None,
            reword_drafts: HashMap::new(),
            applying: true,
            scroll_offset: 0.0,
        }),
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::Done {
        result: Ok(rebase::task::ApplyOutcome::Paused {
            message: "CONFLICT (content): Merge conflict in src/main.rs".into(),
        }),
        apply_mode: rebase::RebaseApplyMode::RebaseThenForcePush,
    }));

    assert!(app.rebase.is_none());
    assert!(app.operation.loading);
    assert!(!app.operation.pending_force_push_after_reload);
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Interactive rebase paused on conflicts")
    );
    assert_eq!(
        app.operation.pending_error_after_reload.as_deref(),
        Some("CONFLICT (content): Merge conflict in src/main.rs")
    );

    let conflict_status = WorktreeStatusDetail {
        conflicted: vec![StatusEntry {
            path: "src/main.rs".into(),
            old_path: None,
            status: StatusKind::Unmerged {
                index: 'U',
                worktree: 'U',
            },
        }],
        ..Default::default()
    };

    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        path,
        vec![commit("b222222", "paused rebase", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("feature/demo".into()),
        conflict_status,
        BranchSyncStatus::default(),
        GitOperationState {
            rebase_in_progress: true,
            ..Default::default()
        },
    ))))));

    assert!(!app.operation.loading);
    assert!(app.repo.operation_state.rebase_in_progress);
    assert!(app.selection.selected_wip);
    assert_eq!(
        app.selection.selected_wip_file.as_ref(),
        Some(&wip_target(WorktreeDiffKind::Conflict, "src/main.rs"))
    );
    assert_eq!(
        app.operation.error.as_deref(),
        Some("CONFLICT (content): Merge conflict in src/main.rs")
    );
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Interactive rebase paused on conflicts")
    );
}

fn rebase_session_with_three_commits() -> rebase::InteractiveRebaseSession {
    rebase::InteractiveRebaseSession {
        current_branch: local_branch("feature/demo", true),
        target: local_branch("main", false),
        current_author_email: Some("author@example.com".into()),
        plan: vec![
            rebase_row("a111111", "first commit"),
            rebase_row("b222222", "second commit"),
            rebase_row("c333333", "third commit"),
        ],
        selected: 0,
        drag: None,
        reword_drafts: HashMap::new(),
        applying: false,
        scroll_offset: 0.0,
    }
}

#[test]
fn rebase_press_then_release_without_motion_selects_without_reordering() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase_session_with_three_commits()),
        ..Default::default()
    };

    let _ = app.update(Message::CursorMoved(iced::Point::new(40.0, 120.0)));
    let _ = app.update(Message::from(rebase::Message::DragPressed(2)));
    // Tiny jitter under the 5px threshold should not start a real drag.
    let _ = app.update(Message::CursorMoved(iced::Point::new(41.0, 121.0)));
    let _ = app.update(Message::from(rebase::Message::DragEnded));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert!(session.drag.is_none(), "drag must be cleared after release");
    assert_eq!(session.selected, 2, "click selects the pressed row");
    assert_eq!(
        session
            .plan
            .iter()
            .map(|row| row.commit.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a111111", "b222222", "c333333"],
        "plan order must be unchanged after a click"
    );
}

#[test]
fn rebase_drag_past_threshold_reorders_on_release() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase_session_with_three_commits()),
        ..Default::default()
    };

    let _ = app.update(Message::CursorMoved(iced::Point::new(50.0, 100.0)));
    let _ = app.update(Message::from(rebase::Message::DragPressed(0)));
    // Drag down ~2 rows past the threshold; ROW_HEIGHT = 32px, so dy=80 -> 2 rows.
    let _ = app.update(Message::CursorMoved(iced::Point::new(50.0, 180.0)));

    let drag = app
        .rebase
        .as_ref()
        .and_then(|s| s.drag)
        .expect("drag should be active mid-motion");
    assert!(drag.started, "started must flip true past the threshold");
    assert_eq!(drag.source_index, 0);
    assert_eq!(drag.hover_index, 2);

    let _ = app.update(Message::from(rebase::Message::DragEnded));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert!(session.drag.is_none(), "drag should clear after drop");
    assert_eq!(
        session
            .plan
            .iter()
            .map(|row| row.commit.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b222222", "c333333", "a111111"],
        "row 0 should have moved to position 2"
    );
    assert_eq!(session.selected, 2);
}

#[test]
fn rebase_escape_during_drag_cancels_only_the_drag() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase_session_with_three_commits()),
        ..Default::default()
    };

    let _ = app.update(Message::CursorMoved(iced::Point::new(50.0, 100.0)));
    let _ = app.update(Message::from(rebase::Message::DragPressed(0)));
    let _ = app.update(Message::CursorMoved(iced::Point::new(50.0, 180.0)));
    let _ = app.update(Message::from(rebase::Message::EscapePressed));

    let session = app
        .rebase
        .as_ref()
        .expect("rebase session must stay open when Esc only cancels a drag");
    assert!(session.drag.is_none(), "Esc must clear the active drag");
    assert_eq!(
        session
            .plan
            .iter()
            .map(|row| row.commit.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a111111", "b222222", "c333333"],
        "plan order must be unchanged when Esc cancels a drag"
    );
}

#[test]
fn rebase_escape_without_drag_closes_the_session() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase_session_with_three_commits()),
        ..Default::default()
    };

    let _ = app.update(Message::from(rebase::Message::EscapePressed));

    assert!(
        app.rebase.is_none(),
        "with no active drag, Esc should close the rebase view"
    );
}

#[test]
fn rebase_cmd_arrow_down_moves_the_selected_row() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        rebase: Some(rebase::InteractiveRebaseSession {
            selected: 1,
            ..rebase_session_with_three_commits()
        }),
        ..Default::default()
    };

    // Cmd+ArrowDown is wired in `rebase_keyboard_shortcut` to MoveSelected(1).
    let _ = app.update(Message::from(rebase::Message::MoveSelected(1)));

    let session = app.rebase.as_ref().expect("rebase session should remain");
    assert_eq!(session.selected, 2);
    assert_eq!(
        session
            .plan
            .iter()
            .map(|row| row.commit.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a111111", "c333333", "b222222"],
    );
}

#[test]
fn undo_redo_commands_prompt_from_supported_history_checkpoints() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("b222222", "after rewrite", "june")],
            ..Default::default()
        },
        undo_stack: vec![UndoCheckpoint {
            label: "reword a111111".into(),
            head_id: "a111111".into(),
        }],
        ..Default::default()
    };

    let undo = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Undo)
        .unwrap();
    assert!(undo.enabled());

    let _ = app.run_command_palette_command(CommandId::Undo);
    assert_eq!(
        app.selection
            .undo_confirmation
            .as_ref()
            .map(|prompt| prompt.action),
        Some(UndoPromptAction::Undo)
    );

    let dirty_app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("b222222", "after rewrite", "june")],
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        undo_stack: vec![UndoCheckpoint {
            label: "reword a111111".into(),
            head_id: "a111111".into(),
        }],
        ..Default::default()
    };
    let undo = dirty_app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Undo)
        .unwrap();
    assert_eq!(undo.disabled_reason, Some("Working tree must be clean"));
}

#[test]
fn checkout_selected_ref_command_requires_checkoutable_sidebar_ref() {
    let app = App::default();
    let checkout = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CheckoutSelectedRef)
        .unwrap();
    assert_eq!(checkout.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let checkout = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CheckoutSelectedRef)
        .unwrap();
    assert_eq!(
        checkout.disabled_reason,
        Some("Open a local or remote branch menu first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(remote_branch("origin/main")),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let checkout = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CheckoutSelectedRef)
        .unwrap();
    assert_eq!(checkout.disabled_reason, Some("Operation in progress"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(tag("v1.0.0")),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let checkout = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CheckoutSelectedRef)
        .unwrap();
    assert_eq!(
        checkout.disabled_reason,
        Some("Open a local or remote branch menu first")
    );
}

#[test]
fn checkout_selected_ref_command_accepts_local_and_remote_branches() {
    for target in [
        local_branch("feature/local", false),
        remote_branch("origin/feature/remote"),
    ] {
        let app = App {
            repo: RepositoryState {
                path: Some(PathBuf::from("/tmp/naite")),
                ..Default::default()
            },
            selection: SelectionState {
                context_menu: Some(crate::state::ContextMenuState {
                    kind: crate::state::ContextMenuKind::Ref(target),
                    position: iced::Point::ORIGIN,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let checkout = app
            .command_palette_items()
            .into_iter()
            .find(|item| item.id == CommandId::CheckoutSelectedRef)
            .unwrap();
        assert!(checkout.enabled());
    }
}

#[test]
fn force_sync_selected_ref_command_requires_matching_branch_pair() {
    let app = App::default();
    let force_sync = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ForceSyncSelectedRef)
        .unwrap();
    assert_eq!(force_sync.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let force_sync = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ForceSyncSelectedRef)
        .unwrap();
    assert_eq!(
        force_sync.disabled_reason,
        Some("Open a matching local/remote branch menu first")
    );

    let target = remote_branch("origin/feature/demo");
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                local: vec![local_branch("feature/demo", false)],
                remote: vec![target.clone()],
                tags: Vec::new(),
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(target),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let force_sync = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ForceSyncSelectedRef)
        .unwrap();
    assert_eq!(force_sync.disabled_reason, Some("Operation in progress"));
}

#[test]
fn force_sync_selected_ref_command_accepts_remote_and_local_entrypoints() {
    let remote = remote_branch("origin/feature/demo");
    let local = local_branch_with_upstream("feature/demo", "origin/feature/demo", 0, 2);
    for selected in [remote.clone(), local.clone()] {
        let app = App {
            repo: RepositoryState {
                path: Some(PathBuf::from("/tmp/naite")),
                refs: Refs {
                    local: vec![local.clone()],
                    remote: vec![remote.clone()],
                    tags: Vec::new(),
                },
                ..Default::default()
            },
            selection: SelectionState {
                context_menu: Some(crate::state::ContextMenuState {
                    kind: crate::state::ContextMenuKind::Ref(selected),
                    position: iced::Point::ORIGIN,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let force_sync = app
            .command_palette_items()
            .into_iter()
            .find(|item| item.id == CommandId::ForceSyncSelectedRef)
            .unwrap();
        assert!(force_sync.enabled());
    }
}

#[test]
fn sidebar_checkout_supports_local_and_remote_branches_only() {
    assert!(crate::widgets::sidebar_ref_checkout_supported(
        &local_branch("main", true)
    ));
    assert!(crate::widgets::sidebar_ref_checkout_supported(
        &remote_branch("origin/main")
    ));
    assert!(!crate::widgets::sidebar_ref_checkout_supported(&tag(
        "v1.0.0"
    )));
}

#[test]
fn sidebar_ref_single_click_records_pending_click_without_checkout() {
    let target = remote_branch("origin/feature/demo");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            error: Some("keep me".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::SidebarRefPressed(target.clone()));

    assert_eq!(app.operation.error.as_deref(), Some("keep me"));
    assert_eq!(
        app.selection
            .last_sidebar_click
            .as_ref()
            .map(|click| click.ref_summary.full_name.as_str()),
        Some(target.full_name.as_str())
    );
}

#[test]
fn sidebar_section_toggle_updates_only_requested_section() {
    let mut app = App::default();

    let _ = app.update(Message::SidebarSectionToggled(
        SidebarSection::RemoteBranches,
    ));

    assert!(app.sidebar.local_branches_expanded);
    assert!(app.sidebar.remote_branches_expanded);
    assert!(!app.sidebar.recent_repositories_expanded);
    assert!(!app.sidebar.tags_expanded);
    assert!(!app.sidebar.stashes_expanded);
}

#[test]
fn sidebar_tree_folder_toggle_updates_requested_folder() {
    let mut app = App::default();

    let _ = app.update(Message::SidebarTreeFolderToggled {
        section: SidebarSection::LocalBranches,
        path: "feature".into(),
    });

    assert!(!app
        .sidebar
        .is_tree_folder_expanded(SidebarSection::LocalBranches, "feature"));
    assert!(app
        .sidebar
        .is_tree_folder_expanded(SidebarSection::RemoteBranches, "feature"));
}

#[test]
fn sidebar_ref_hover_updates_hovered_ref_state() {
    let mut app = App::default();
    let target = local_branch("feature/hover", false);

    let _ = app.update(Message::SidebarRefHovered(target.clone()));

    assert!(app.sidebar.is_ref_hovered(&target));
}

#[test]
fn sidebar_ref_unhover_clears_only_matching_hovered_ref() {
    let target = local_branch("feature/hover", false);
    let other = local_branch("feature/other", false);
    let mut app = App::default();

    let _ = app.update(Message::SidebarRefHovered(target.clone()));
    let _ = app.update(Message::SidebarRefUnhovered(other));

    assert!(app.sidebar.is_ref_hovered(&target));

    let _ = app.update(Message::SidebarRefUnhovered(target.clone()));

    assert!(!app.sidebar.is_ref_hovered(&target));
    assert!(app.sidebar.hovered_ref.is_none());
}

#[test]
fn recent_repo_remove_deletes_catalog_entry() {
    let mut app = App::default();
    let path = PathBuf::from("/tmp/naite");
    app.catalog.remember(path.clone());
    app.catalog.toggle_favorite(path.clone());

    let _ = app.update(Message::from(repo_open::Message::RemoveRecent(path)));

    assert!(app.catalog.entries.is_empty());
}

#[test]
fn sidebar_ref_double_click_requests_checkout_for_local_or_remote_branch() {
    for target in [
        local_branch("feature/local", false),
        remote_branch("origin/feature/remote"),
    ] {
        let mut app = App {
            repo: RepositoryState {
                path: Some(PathBuf::from("/tmp/naite")),
                ..Default::default()
            },
            operation: OperationState {
                error: Some("previous error".into()),
                ..Default::default()
            },
            ..Default::default()
        };

        let _ = app.update(Message::SidebarRefPressed(target.clone()));
        let _ = app.update(Message::SidebarRefPressed(target));

        assert!(app.operation.error.is_none());
        assert!(app.selection.last_sidebar_click.is_none());
    }
}

#[test]
fn sidebar_ref_double_click_ignores_tags_and_expired_clicks() {
    let target = tag("v1.0.0");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            error: Some("keep me".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::SidebarRefPressed(target.clone()));
    let _ = app.update(Message::SidebarRefPressed(target));

    assert_eq!(app.operation.error.as_deref(), Some("keep me"));
    assert!(app.selection.last_sidebar_click.is_some());

    let target = remote_branch("origin/feature/demo");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            error: Some("keep me".into()),
            ..Default::default()
        },
        selection: SelectionState {
            last_sidebar_click: Some(SidebarClickState {
                ref_summary: target.clone(),
                at: Instant::now() - Duration::from_millis(301),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::SidebarRefPressed(target));

    assert_eq!(app.operation.error.as_deref(), Some("keep me"));
    assert!(app.selection.last_sidebar_click.is_some());
}

#[test]
fn force_sync_target_accepts_remote_branch_with_existing_local_branch() {
    let target = remote_branch("origin/feature/demo");
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                local: vec![local_branch("feature/demo", false)],
                remote: vec![target.clone()],
                tags: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        app.force_sync_target_for_ref(&target)
            .map(|target| target.short_name),
        Some("origin/feature/demo".into())
    );
}

#[test]
fn force_sync_target_accepts_local_branch_with_matching_upstream() {
    let local = local_branch_with_upstream("feature/demo", "origin/feature/demo", 2, 3);
    let remote = remote_branch("origin/feature/demo");
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                local: vec![local.clone()],
                remote: vec![remote],
                tags: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        app.force_sync_target_for_ref(&local)
            .map(|target| target.short_name),
        Some("origin/feature/demo".into())
    );
}

#[test]
fn force_sync_target_rejects_local_branch_with_mismatched_upstream_name() {
    let local = local_branch_with_upstream("my-demo", "origin/feature/demo", 0, 1);
    let remote = remote_branch("origin/feature/demo");
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                local: vec![local.clone()],
                remote: vec![remote],
                tags: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(app.force_sync_target_for_ref(&local).is_none());
}

#[test]
fn force_sync_target_rejects_remote_branch_without_existing_local_branch() {
    let target = remote_branch("origin/feature/new");
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                remote: vec![target.clone()],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(app.force_sync_target_for_ref(&target).is_none());
}

#[test]
fn force_sync_status_loaded_with_existing_local_branch_prompts_force_sync() {
    let target = remote_branch("origin/feature/demo");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                local: vec![local_branch_with_upstream(
                    "feature/demo",
                    "origin/feature/demo",
                    2,
                    3,
                )],
                remote: vec![target.clone()],
                tags: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(checkout::Message::ForceSyncStatusLoaded {
        target,
        result: Ok(WorktreeStatus {
            has_unstaged: true,
            has_staged: false,
            has_untracked: true,
        }),
    }));

    let prompt = app.selection.force_sync_confirmation.as_ref().unwrap();
    assert_eq!(prompt.local_branch, "feature/demo");
    assert_eq!(prompt.target.short_name, "origin/feature/demo");
    assert_eq!(
        prompt
            .sync_status
            .as_ref()
            .map(|status| (status.ahead, status.behind)),
        Some((2, 3))
    );
    assert!(prompt.status.has_unstaged);
    assert!(prompt.status.has_untracked);
    assert!(!app.operation.loading);
    assert!(app.selection.checkout_confirmation.is_none());
}

#[test]
fn force_sync_request_closes_context_menu_before_status_load() {
    let target = remote_branch("origin/main");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            error: Some("previous error".into()),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(target.clone()),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(checkout::Message::ForceSyncRequested(target)));

    assert!(app.operation.error.is_none());
    assert!(app.selection.context_menu.is_none());
}

#[test]
fn remote_checkout_without_existing_local_branch_uses_checkout_prompt_for_dirty_worktree() {
    let target = remote_branch("origin/feature/new");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                remote: vec![target.clone()],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(checkout::Message::WorktreeStatusLoaded {
        target,
        result: Ok(WorktreeStatus {
            has_unstaged: true,
            has_staged: false,
            has_untracked: false,
        }),
    }));

    assert!(app.selection.checkout_confirmation.is_some());
    assert!(app.selection.force_sync_confirmation.is_none());
}

#[test]
fn escape_closes_checkout_confirmation() {
    let target = remote_branch("origin/feature/new");
    let mut app = App {
        selection: SelectionState {
            checkout_confirmation: Some(CheckoutPrompt {
                target,
                status: WorktreeStatus::default(),
            }),
            ..Default::default()
        },
        search_query: "keep".into(),
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Escape));

    assert!(app.selection.checkout_confirmation.is_none());
    assert_eq!(app.search_query, "keep");
}

#[test]
fn force_sync_confirmation_starts_force_sync_operation_and_closes_on_result() {
    let target = remote_branch("origin/main");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            force_sync_confirmation: Some(ForceSyncPrompt {
                target: target.clone(),
                local_branch: "main".into(),
                sync_status: None,
                status: WorktreeStatus::default(),
            }),
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(target.clone()),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(checkout::Message::ForceSyncConfirmed {
        target,
    }));

    assert!(app.operation.loading);

    let _ = app.update(Message::from(checkout::Message::ForceSyncDone(Err(
        "reset failed".into(),
    ))));

    assert!(!app.operation.loading);
    assert!(app.selection.force_sync_confirmation.is_none());
    assert!(app.selection.context_menu.is_none());
    assert_eq!(app.operation.error.as_deref(), Some("reset failed"));
}

#[test]
fn fetch_command_requires_repo_idle_operation_and_upstream() {
    let app = App::default();
    let fetch = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Fetch)
        .unwrap();
    assert_eq!(fetch.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let fetch = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Fetch)
        .unwrap();
    assert_eq!(fetch.disabled_reason, Some("Operation in progress"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let fetch = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Fetch)
        .unwrap();
    assert_eq!(
        fetch.disabled_reason,
        Some("Current branch has no upstream")
    );
}

#[test]
fn fetch_all_command_requires_repo_and_idle_operation_only() {
    let app = App::default();
    let fetch_all = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::FetchAll)
        .unwrap();
    assert_eq!(fetch_all.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let fetch_all = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::FetchAll)
        .unwrap();
    assert_eq!(fetch_all.disabled_reason, Some("Operation in progress"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let fetch_all = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::FetchAll)
        .unwrap();
    assert!(fetch_all.enabled());
}

#[test]
fn pull_command_requires_repo_idle_operation_and_upstream() {
    for command in [
        CommandId::PullFastForwardOnly,
        CommandId::PullFastForward,
        CommandId::PullRebase,
    ] {
        let app = App::default();
        let pull = app
            .command_palette_items()
            .into_iter()
            .find(|item| item.id == command)
            .unwrap();
        assert_eq!(pull.disabled_reason, Some("Open a repository first"));

        let app = App {
            repo: RepositoryState {
                path: Some(PathBuf::from("/tmp/naite")),
                ..Default::default()
            },
            operation: OperationState {
                loading: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let pull = app
            .command_palette_items()
            .into_iter()
            .find(|item| item.id == command)
            .unwrap();
        assert_eq!(pull.disabled_reason, Some("Operation in progress"));

        let app = App {
            repo: RepositoryState {
                path: Some(PathBuf::from("/tmp/naite")),
                ..Default::default()
            },
            ..Default::default()
        };
        let pull = app
            .command_palette_items()
            .into_iter()
            .find(|item| item.id == command)
            .unwrap();
        assert_eq!(pull.disabled_reason, Some("Current branch has no upstream"));
    }
}

#[test]
fn push_command_requires_repo_idle_operation_and_attached_branch() {
    let app = App::default();
    let push = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Push)
        .unwrap();
    assert_eq!(push.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let push = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Push)
        .unwrap();
    assert_eq!(push.disabled_reason, Some("Operation in progress"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: None,
            ..Default::default()
        },
        ..Default::default()
    };
    let push = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::Push)
        .unwrap();
    assert_eq!(push.disabled_reason, Some("Current HEAD is detached"));
}

#[test]
fn error_recovery_offers_force_push_on_non_fast_forward() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("staging".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/staging".into()),
                ahead: 1,
                behind: 1,
            },
            ..Default::default()
        },
        operation: OperationState {
            error: Some(
                "git push: ! [rejected] staging -> staging (non-fast-forward)\n\
                 error: failed to push some refs"
                    .into(),
            ),
            ..Default::default()
        },
        ..Default::default()
    };
    let recovery = app
        .error_recovery_action()
        .expect("force-with-lease recovery should be offered");
    assert_eq!(recovery.label, "Force push (with lease)");
    assert!(matches!(
        recovery.message,
        Message::Push(push::Message::ForceWithLeaseConfirmationRequested)
    ));
}

#[test]
fn error_recovery_skips_unrelated_errors() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("staging".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/staging".into()),
                ahead: 1,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            error: Some("authentication failed".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(app.error_recovery_action().is_none());
}

#[test]
fn error_recovery_skips_when_no_upstream() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("staging".into()),
            sync_status: BranchSyncStatus::default(),
            ..Default::default()
        },
        operation: OperationState {
            error: Some("! [rejected] (non-fast-forward)".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(app.error_recovery_action().is_none());
}

#[test]
fn key_action_push_starts_push_operation() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Push));

    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn key_action_push_noops_when_command_palette_open() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Push));

    assert!(!app.operation.loading);
    assert!(app.command_palette.open);
}

#[test]
fn key_action_release_promotion_runs_when_command_palette_open() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::ReleasePromotion));

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Preparing);
}

#[test]
fn push_force_with_lease_command_requires_upstream() {
    let app = App::default();
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PushForceWithLease)
        .unwrap();
    assert_eq!(item.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            sync_status: BranchSyncStatus::default(),
            ..Default::default()
        },
        ..Default::default()
    };
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PushForceWithLease)
        .unwrap();
    assert_eq!(item.disabled_reason, Some("Current branch has no upstream"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 1,
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PushForceWithLease)
        .unwrap();
    assert_eq!(item.disabled_reason, None);
}

#[test]
fn enabled_force_push_command_opens_confirmation_without_starting_push() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 0,
            },
            commits: vec![commit("abcdef123", "head", "june")],
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::PushForceWithLease,
    )));

    assert!(!app.command_palette.open);
    assert!(!app.operation.loading);
    let prompt = app
        .selection
        .force_push_confirmation
        .as_ref()
        .expect("force push should require confirmation");
    assert_eq!(prompt.branch, "main");
    assert_eq!(prompt.upstream, "origin/main");
}

#[test]
fn push_menu_normal_push_closes_context_menu() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::PushMenu {
                    force_with_lease_available: false,
                },
                position: iced::Point::new(24.0, 24.0),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(push::Message::Requested(
        push::PushMode::Normal,
    )));

    assert!(app.selection.context_menu.is_none());
    assert!(app.operation.loading);
}

#[test]
fn push_menu_force_push_opens_confirmation_and_closes_context_menu() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 0,
            },
            commits: vec![commit("abcdef123", "head", "june")],
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::PushMenu {
                    force_with_lease_available: true,
                },
                position: iced::Point::new(24.0, 24.0),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(
        push::Message::ForceWithLeaseConfirmationRequested,
    ));

    assert!(app.selection.context_menu.is_none());
    assert!(!app.operation.loading);
    let prompt = app
        .selection
        .force_push_confirmation
        .as_ref()
        .expect("force push should require confirmation");
    assert_eq!(prompt.branch, "main");
    assert_eq!(prompt.upstream, "origin/main");
}

#[test]
fn production_release_prep_command_requires_clean_open_repo() {
    let app = App::default();
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PrepareProductionRelease)
        .unwrap();
    assert_eq!(item.label, "Plan release promotion");
    assert_eq!(item.shortcut, "Cmd Shift R");
    assert_eq!(item.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PrepareProductionRelease)
        .unwrap();
    assert_eq!(
        item.disabled_reason,
        Some("Commit, stash, or resolve local changes first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            operation_state: GitOperationState {
                rebase_in_progress: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PrepareProductionRelease)
        .unwrap();
    assert_eq!(item.disabled_reason, None);

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PrepareProductionRelease)
        .unwrap();
    assert_eq!(item.disabled_reason, None);
}

#[test]
fn production_release_prep_ignores_cached_busy_state_before_fresh_task_check() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            operation_state: GitOperationState {
                merge_in_progress: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(release_prep::Message::Requested));

    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Preparing);
    assert!(app.operation.loading);
    assert_eq!(app.operation.error, None);
}

#[test]
fn production_release_prep_dirty_repo_loads_branch_suggestions_for_dropdowns() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(release_prep::Message::Requested));

    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Preparing);
    assert_eq!(
        app.release_prep.error.as_deref(),
        Some(release_prep::update::DIRTY_WORKTREE_RELEASE_ERROR)
    );
    assert!(app.operation.loading);
}

#[test]
fn production_release_prep_suggestion_preserves_dirty_repo_error() {
    let mut app = App {
        release_prep: ReleasePrepState {
            phase: ReleasePrepPhase::Preparing,
            error: Some(release_prep::update::DIRTY_WORKTREE_RELEASE_ERROR.into()),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let suggestion = ReleaseProfileSuggestion {
        remotes: vec!["origin".into()],
        source_candidates: vec!["staging".into()],
        target_candidates: vec!["main".into()],
        default_profile: ReleaseProfile {
            remote: "origin".into(),
            source_branch: "staging".into(),
            target_branch: "main".into(),
        },
    };

    let _ = app.update(Message::from(release_prep::Message::SuggestionLoaded(Ok(
        suggestion,
    ))));

    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Configuring);
    assert!(!app.operation.loading);
    assert_eq!(app.release_prep.remote, "origin");
    assert_eq!(app.release_prep.source_branch, "staging");
    assert_eq!(app.release_prep.target_branch, "main");
    assert_eq!(
        app.release_prep.error.as_deref(),
        Some(release_prep::update::DIRTY_WORKTREE_RELEASE_ERROR)
    );
    assert!(app.release_prep.suggestion.is_some());
}

#[test]
fn production_release_prep_uses_saved_profile_without_config_modal() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            ..Default::default()
        },
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        ..Default::default()
    };
    app.preferences.release_profiles.insert(
        path,
        ReleaseProfile {
            remote: "origin".into(),
            source_branch: "staging".into(),
            target_branch: "main".into(),
        },
    );

    let _ = app.run_command_palette_command(CommandId::PrepareProductionRelease);

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Preparing);
    assert_eq!(app.release_prep.remote, "origin");
    assert_eq!(app.release_prep.source_branch, "staging");
    assert_eq!(app.release_prep.target_branch, "main");
}

#[test]
fn production_release_prep_tick_animates_and_escape_closes_loading_modal() {
    let mut app = App {
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            phase: ReleasePrepPhase::Preparing,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::ReleasePrepTick);
    assert_eq!(app.release_prep.animation_frame, 1);

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Idle);
    assert!(app.operation.loading);
}

#[test]
fn production_release_prepare_applies_post_sync_repo_snapshot_before_rebase() {
    let path = PathBuf::from("/tmp/naite");
    let source_ref = RefSummary {
        kind: RefKind::LocalBranch,
        short_name: "staging".into(),
        full_name: "refs/heads/staging".into(),
        target_short_id: "abc1234".into(),
        is_head: true,
        sync_status: Some(BranchSyncStatus {
            upstream: Some("origin/staging".into()),
            ahead: 0,
            behind: 0,
        }),
    };
    let target_ref = RefSummary {
        kind: RefKind::LocalBranch,
        short_name: "main".into(),
        full_name: "refs/heads/main".into(),
        target_short_id: "def5678".into(),
        is_head: false,
        sync_status: Some(BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        }),
    };
    let profile = ReleaseProfile {
        remote: "origin".into(),
        source_branch: "staging".into(),
        target_branch: "main".into(),
    };
    let sync_check = ReleaseSyncCheck {
        profile: profile.clone(),
        source: ReleaseBranchSync {
            branch: "staging".into(),
            local_ref: "refs/heads/staging".into(),
            remote_ref: "refs/remotes/origin/staging".into(),
            local_oid: Some("abc1234".into()),
            remote_oid: Some("abc1234".into()),
            ahead: 0,
            behind: 0,
        },
        target: ReleaseBranchSync {
            branch: "main".into(),
            local_ref: "refs/heads/main".into(),
            remote_ref: "refs/remotes/origin/main".into(),
            local_oid: Some("def5678".into()),
            remote_oid: Some("def5678".into()),
            ahead: 0,
            behind: 0,
        },
    };
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            head_branch: Some("main".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 2,
                behind: 3,
            },
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(profile),
            phase: ReleasePrepPhase::Preparing,
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let outcome = release_prep::task::PrepareOutcome {
        sync_check,
        backup_branch: None,
        current_branch: source_ref.clone(),
        target: target_ref.clone(),
        current_author_email: None,
        plan: vec![rebase::RebasePlanRow {
            action: RebaseAction::Pick,
            commit: HistoryCommit {
                id: "abc123456789".into(),
                summary: "release commit".into(),
                author_name: "naite".into(),
                author_email: "naite@example.com".into(),
            },
            author_avatar_url: None,
        }],
        repo_snapshot: (
            path,
            Vec::new(),
            None,
            Refs {
                local: vec![source_ref, target_ref],
                remote: Vec::new(),
                tags: Vec::new(),
            },
            Vec::new(),
            Vec::new(),
            Some("staging".into()),
            WorktreeStatusDetail::default(),
            BranchSyncStatus {
                upstream: Some("origin/staging".into()),
                ahead: 0,
                behind: 0,
            },
            GitOperationState::default(),
        ),
    };

    let _ = app.update(Message::from(release_prep::Message::Prepared(Box::new(
        Ok(outcome),
    ))));

    assert_eq!(app.repo.head_branch.as_deref(), Some("staging"));
    assert_eq!(app.repo.sync_status.ahead, 0);
    assert_eq!(app.repo.sync_status.behind, 0);
    assert_eq!(app.repo.refs.local[0].short_name, "staging");
    assert!(app.rebase.is_some());
}

#[test]
fn production_release_prepare_failure_keeps_config_open_and_refreshes_repo_state() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            phase: ReleasePrepPhase::Preparing,
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(release_prep::Message::Prepared(Box::new(
        Err(
            "git command failed: git checkout staging: fatal: branch is checked out elsewhere"
                .into(),
        ),
    ))));

    assert_eq!(app.release_prep.phase, ReleasePrepPhase::Configuring);
    assert_eq!(
        app.release_prep.error.as_deref(),
        Some("git command failed: git checkout staging: fatal: branch is checked out elsewhere")
    );
    assert!(app.operation.loading);
}

#[test]
fn production_release_followup_commands_require_active_profile() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let update_target = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ReleaseUpdateTargetFromSource)
        .unwrap();
    assert_eq!(
        update_target.disabled_reason,
        Some("Plan a release promotion first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let update_target = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ReleaseUpdateTargetFromSource)
        .unwrap();
    let push_target = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ReleasePushTarget)
        .unwrap();
    let sync_source = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::ReleaseSyncSourceFromTarget)
        .unwrap();

    assert_eq!(update_target.disabled_reason, None);
    assert_eq!(push_target.disabled_reason, None);
    assert_eq!(sync_source.disabled_reason, None);
}

#[test]
fn stash_commands_require_repo_idle_state_and_stashable_inputs() {
    let app = App::default();
    let stash_changes = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::StashChanges)
        .unwrap();
    assert_eq!(
        stash_changes.disabled_reason,
        Some("Open a repository first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let stash_changes = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::StashChanges)
        .unwrap();
    assert_eq!(stash_changes.disabled_reason, Some("Operation in progress"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let stash_changes = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::StashChanges)
        .unwrap();
    assert_eq!(stash_changes.disabled_reason, Some("Working tree is clean"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: WorktreeStatusDetail {
                conflicted: vec![StatusEntry {
                    path: "conflict.rs".into(),
                    old_path: None,
                    status: StatusKind::Modified,
                }],
                ..dirty_status_detail()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let stash_changes = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::StashChanges)
        .unwrap();
    assert_eq!(
        stash_changes.disabled_reason,
        Some("Resolve conflicts first")
    );

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };
    let stash_changes = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::StashChanges)
        .unwrap();
    assert!(stash_changes.enabled());
}

#[test]
fn pop_latest_stash_command_requires_existing_stash() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };
    let pop = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PopLatestStash)
        .unwrap();
    assert_eq!(pop.disabled_reason, Some("No stashes"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            stashes: vec![stash_summary("stash@{0}")],
            ..Default::default()
        },
        ..Default::default()
    };
    let pop = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::PopLatestStash)
        .unwrap();
    assert!(pop.enabled());
}

#[test]
fn create_branch_from_stash_command_requires_selected_stash() {
    let app = App::default();
    let command = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateBranchFromSelectedStash)
        .unwrap();
    assert_eq!(command.disabled_reason, Some("Open a repository first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            stashes: vec![stash_summary("stash@{0}")],
            ..Default::default()
        },
        ..Default::default()
    };
    let command = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateBranchFromSelectedStash)
        .unwrap();
    assert_eq!(command.disabled_reason, Some("Select a stash first"));

    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            stashes: vec![stash_summary("stash@{0}")],
            ..Default::default()
        },
        selection: SelectionState {
            selected_stash: Some(stash_summary("stash@{0}")),
            ..Default::default()
        },
        ..Default::default()
    };
    let command = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::CreateBranchFromSelectedStash)
        .unwrap();
    assert!(command.enabled());
}

#[test]
fn enabled_fetch_command_closes_palette_and_starts_loading() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::Fetch,
    )));

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn enabled_fetch_all_command_closes_palette_and_starts_loading() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::FetchAll,
    )));

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn auto_fetch_tick_starts_background_fetch_without_global_loading() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::AutoFetchTick);

    assert_eq!(
        app.operation.auto_fetch_path.as_deref(),
        Some(Path::new("/tmp/naite"))
    );
    assert!(!app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn auto_fetch_tick_skips_while_release_prep_modal_is_open() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            phase: ReleasePrepPhase::Actions,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::AutoFetchTick);

    assert!(app.operation.auto_fetch_path.is_none());
    assert!(!app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn auto_fetch_done_refreshes_active_repo_without_status_noise() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            auto_fetch_path: Some(path.clone()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::AutoDone {
        path: path.clone(),
        result: Ok(()),
    }));

    assert!(app.operation.auto_fetch_path.is_none());
    assert!(!app.operation.loading);
    assert!(app.operation.error.is_none());
    assert!(app.operation.transient_status.is_none());
    assert!(app.tabs.refreshing.contains(&path));
}

#[test]
fn auto_fetch_done_skips_refresh_during_foreground_operation() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            auto_fetch_path: Some(path.clone()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::AutoDone {
        path: path.clone(),
        result: Ok(()),
    }));

    assert!(app.operation.auto_fetch_path.is_none());
    assert!(app.operation.loading);
    assert!(!app.tabs.refreshing.contains(&path));
    assert!(app.operation.error.is_none());
}

#[test]
fn auto_fetch_done_retargets_after_repo_switch() {
    let fetched_path = PathBuf::from("/tmp/naite-old");
    let current_path = PathBuf::from("/tmp/naite-current");
    let mut app = App {
        repo: RepositoryState {
            path: Some(current_path.clone()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            auto_fetch_path: Some(fetched_path.clone()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::AutoDone {
        path: fetched_path.clone(),
        result: Ok(()),
    }));

    assert_eq!(
        app.operation.auto_fetch_path.as_deref(),
        Some(current_path.as_path())
    );
    assert!(!app.tabs.refreshing.contains(&fetched_path));
    assert!(app.operation.error.is_none());
}

#[test]
fn auto_fetch_done_failure_resumes_pending_release_prep_auto() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            auto_fetch_path: Some(path.clone()),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            phase: ReleasePrepPhase::Actions,
            auto_running: true,
            auto_next_action: Some(release_prep::ReleasePrepAction::UpdateTargetFromSource),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::AutoDone {
        path: path.clone(),
        result: Err("network unreachable".into()),
    }));

    assert!(app.operation.auto_fetch_path.is_none());
    assert!(app.release_prep.auto_running);
    assert_eq!(app.release_prep.auto_next_action, None);
    assert_eq!(
        app.release_prep.active_action,
        Some(release_prep::ReleasePrepAction::UpdateTargetFromSource)
    );
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::RunningAction);
    assert!(app.operation.loading);
}

#[test]
fn auto_fetch_done_success_resumes_pending_release_prep_auto() {
    let path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(path.clone()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            auto_fetch_path: Some(path.clone()),
            ..Default::default()
        },
        release_prep: ReleasePrepState {
            active_profile: Some(ReleaseProfile {
                remote: "origin".into(),
                source_branch: "staging".into(),
                target_branch: "main".into(),
            }),
            phase: ReleasePrepPhase::Actions,
            auto_running: true,
            auto_next_action: Some(release_prep::ReleasePrepAction::UpdateTargetFromSource),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::AutoDone {
        path: path.clone(),
        result: Ok(()),
    }));

    assert!(app.operation.auto_fetch_path.is_none());
    assert!(!app.tabs.refreshing.contains(&path));
    assert!(app.release_prep.auto_running);
    assert_eq!(app.release_prep.auto_next_action, None);
    assert_eq!(
        app.release_prep.active_action,
        Some(release_prep::ReleasePrepAction::UpdateTargetFromSource)
    );
    assert_eq!(app.release_prep.phase, ReleasePrepPhase::RunningAction);
    assert!(app.operation.loading);
}

#[test]
fn enabled_stash_command_closes_palette_and_opens_form() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::StashChanges,
    )));

    assert!(!app.command_palette.open);
    assert!(app.stash_create.open);
    assert!(!app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn pop_latest_stash_command_closes_palette_and_prompts() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            stashes: vec![stash_summary("stash@{0}")],
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::PopLatestStash,
    )));

    assert!(!app.command_palette.open);
    let prompt = app.selection.stash_confirmation.as_ref().unwrap();
    assert_eq!(prompt.action, StashPromptAction::Pop);
    assert_eq!(prompt.stash.selector, "stash@{0}");
}

#[test]
fn create_branch_from_stash_command_closes_palette_and_opens_form() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            stashes: vec![stash_summary("stash@{0}")],
            ..Default::default()
        },
        selection: SelectionState {
            selected_stash: Some(stash_summary("stash@{0}")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::CreateBranchFromSelectedStash,
    )));

    assert!(!app.command_palette.open);
    assert!(app.stash_branch.open);
    assert_eq!(app.stash_branch.name, "stash/stash-0");
    assert_eq!(
        app.stash_branch
            .stash
            .as_ref()
            .map(|stash| stash.selector.as_str()),
        Some("stash@{0}")
    );
}

#[test]
fn branch_manage_palette_commands_open_existing_flows() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(local_branch("feature/demo", false)),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::RenameSelectedBranch,
    )));

    assert!(!app.command_palette.open);
    assert!(app.branch_manage_rename.open);
    assert_eq!(app.branch_manage_rename.name, "feature/demo");

    app.command_palette.open = true;
    app.branch_manage_rename = BranchManageRenameState::default();
    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::DeleteSelectedBranch,
    )));

    assert!(!app.command_palette.open);
    assert_eq!(
        app.selection
            .branch_delete_confirmation
            .as_ref()
            .map(|prompt| prompt.target.label()),
        Some("feature/demo")
    );
}

#[test]
fn enabled_pull_command_closes_palette_and_starts_loading() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 1,
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::PullFastForwardOnly,
    )));

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn enabled_pull_rebase_command_closes_palette_and_starts_loading() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 1,
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::PullRebase,
    )));

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn enabled_push_command_closes_palette_and_starts_loading() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::Push,
    )));

    assert!(!app.command_palette.open);
    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
}

#[test]
fn fetch_success_starts_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::Done {
        scope: fetch::FetchScope::CurrentRemote,
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Fetched remote just now")
    );
}

#[test]
fn fetch_all_success_starts_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 1,
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::Done {
        scope: fetch::FetchScope::AllRemotes,
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Fetched all remotes just now")
    );
}

#[test]
fn pull_success_starts_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(pull::Message::Done {
        mode: pull::PullMode::FastForwardOnly,
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Pulled remote with fast-forward only")
    );
}

#[test]
fn pull_rebase_success_starts_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 1,
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(pull::Message::Done {
        mode: pull::PullMode::Rebase,
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Pulled origin/main with rebase")
    );
}

#[test]
fn push_success_starts_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(push::Message::Done {
        mode: push::PushMode::Normal,
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.operation.error.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Pushed origin/main and set upstream")
    );
}

#[test]
fn fetch_success_status_appears_after_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 1,
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::Done {
        scope: fetch::FetchScope::CurrentRemote,
        result: Ok(()),
    }));
    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    assert!(!app.operation.loading);
    assert!(app
        .operation
        .pending_transient_status_after_reload
        .is_none());
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Fetched origin/main just now")
    );
}

#[test]
fn fetch_all_success_status_appears_after_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::Done {
        scope: fetch::FetchScope::AllRemotes,
        result: Ok(()),
    }));
    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus::default(),
        GitOperationState::default(),
    ))))));

    assert!(!app.operation.loading);
    assert!(app
        .operation
        .pending_transient_status_after_reload
        .is_none());
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Fetched all remotes just now")
    );
}

#[test]
fn pull_success_status_appears_after_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 1,
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(pull::Message::Done {
        mode: pull::PullMode::FastForwardOnly,
        result: Ok(()),
    }));
    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    assert!(!app.operation.loading);
    assert!(app
        .operation
        .pending_transient_status_after_reload
        .is_none());
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Pulled origin/main with fast-forward only")
    );
}

#[test]
fn push_success_status_appears_after_repo_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            sync_status: BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 1,
                behind: 0,
            },
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(push::Message::Done {
        mode: push::PushMode::Normal,
        result: Ok(()),
    }));
    let _ = app.update(Message::from(repo_open::Message::Loaded(Box::new(Ok((
        PathBuf::from("/tmp/naite"),
        vec![commit("a111111", "add app shell", "june")],
        None,
        Refs::default(),
        vec![],
        vec![],
        Some("main".into()),
        WorktreeStatusDetail::default(),
        BranchSyncStatus {
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
        },
        GitOperationState::default(),
    ))))));

    assert!(!app.operation.loading);
    assert!(app
        .operation
        .pending_transient_status_after_reload
        .is_none());
    assert_eq!(
        app.operation
            .transient_status
            .as_ref()
            .map(|status| status.message.as_str()),
        Some("Pushed origin/main just now")
    );
}

#[test]
fn fetch_error_preserves_repo_state_and_reports_error() {
    let sync_status = BranchSyncStatus {
        upstream: Some("origin/main".into()),
        ahead: 1,
        behind: 0,
    };
    let mut app = App {
        repo: RepositoryState {
            sync_status: sync_status.clone(),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(fetch::Message::Done {
        scope: fetch::FetchScope::CurrentRemote,
        result: Err("auth failed".into()),
    }));

    assert!(!app.operation.loading);
    assert_eq!(app.repo.sync_status, sync_status);
    assert_eq!(app.repo.status_detail, dirty_status_detail());
    assert_eq!(app.operation.error.as_deref(), Some("auth failed"));
}

#[test]
fn pull_error_preserves_repo_state_and_reports_error() {
    let sync_status = BranchSyncStatus {
        upstream: Some("origin/main".into()),
        ahead: 0,
        behind: 1,
    };
    let mut app = App {
        repo: RepositoryState {
            sync_status: sync_status.clone(),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(pull::Message::Done {
        mode: pull::PullMode::FastForwardOnly,
        result: Err("not possible to fast-forward".into()),
    }));

    assert!(!app.operation.loading);
    assert_eq!(app.repo.sync_status, sync_status);
    assert_eq!(app.repo.status_detail, dirty_status_detail());
    assert_eq!(
        app.operation.error.as_deref(),
        Some("not possible to fast-forward")
    );
}

#[test]
fn push_error_preserves_repo_state_and_reports_error() {
    let sync_status = BranchSyncStatus {
        upstream: Some("origin/main".into()),
        ahead: 1,
        behind: 0,
    };
    let mut app = App {
        repo: RepositoryState {
            sync_status: sync_status.clone(),
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(push::Message::Done {
        mode: push::PushMode::Normal,
        result: Err("authentication failed".into()),
    }));

    assert!(!app.operation.loading);
    assert_eq!(app.repo.sync_status, sync_status);
    assert_eq!(app.repo.status_detail, dirty_status_detail());
    assert_eq!(
        app.operation.error.as_deref(),
        Some("authentication failed")
    );
}

#[test]
fn transient_status_tick_clears_only_expired_status() {
    let mut app = App {
        operation: OperationState {
            transient_status: Some(TransientStatus {
                message: "Fetched origin/main just now".into(),
                expires_at: Instant::now() + Duration::from_secs(1),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::TransientStatusTick);
    assert!(app.operation.transient_status.is_some());

    app.operation.transient_status = Some(TransientStatus {
        message: "Fetched origin/main just now".into(),
        expires_at: Instant::now() - Duration::from_secs(1),
    });

    let _ = app.update(Message::TransientStatusTick);
    assert!(app.operation.transient_status.is_none());
}

#[test]
fn branch_create_form_uses_selected_commit_as_base() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![
                commit("a111111", "add app shell", "june"),
                commit("b222222", "fix diff pane", "alex"),
            ],
            head_branch: Some("main".into()),
            ..Default::default()
        },
        selection: SelectionState {
            selected_commit_id: Some("b222222".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_create::Message::Requested));

    assert!(app.branch_create.open);
    assert!(app.branch_create.name.is_empty());
    assert_eq!(
        app.branch_create.base,
        BranchCreateBase::Commit {
            id: "b222222".into(),
            short_id: "b222222".into(),
            summary: "fix diff pane".into(),
        }
    );
}

#[test]
fn branch_create_form_uses_head_without_commit_selection() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        selection: SelectionState {
            selected_wip: true,
            selected_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_create::Message::Requested));

    assert_eq!(
        app.branch_create.base,
        BranchCreateBase::Head {
            label: "HEAD (main)".into(),
        }
    );
}

#[test]
fn branch_create_submit_ignores_empty_name() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        branch_create: BranchCreateState {
            open: true,
            name: "  ".into(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_create::Message::Submitted));

    assert!(!app.operation.loading);
    assert!(app.branch_create.open);
}

#[test]
fn branch_create_success_closes_form_and_starts_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        branch_create: BranchCreateState {
            open: true,
            name: "feature/demo".into(),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_create::Message::Done(Ok(()))));

    assert!(app.operation.loading);
    assert!(!app.branch_create.open);
    assert!(app.branch_create.name.is_empty());
    assert!(app.operation.error.is_none());
}

#[test]
fn branch_create_error_preserves_form() {
    let mut app = App {
        branch_create: BranchCreateState {
            open: true,
            name: "feature/demo".into(),
            base: BranchCreateBase::Head {
                label: "HEAD (main)".into(),
            },
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_create::Message::Done(Err(
        "branch exists".into(),
    ))));

    assert!(!app.operation.loading);
    assert!(app.branch_create.open);
    assert_eq!(app.branch_create.name, "feature/demo");
    assert_eq!(
        app.branch_create.base,
        BranchCreateBase::Head {
            label: "HEAD (main)".into(),
        }
    );
    assert_eq!(app.operation.error.as_deref(), Some("branch exists"));
}

#[test]
fn branch_rename_requested_opens_form_with_current_name() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::RenameRequested(
        local_branch("feature/demo", false),
    )));

    assert!(app.branch_manage_rename.open);
    assert_eq!(app.branch_manage_rename.name, "feature/demo");
    assert_eq!(
        app.branch_manage_rename
            .target
            .as_ref()
            .map(|target| target.short_name.as_str()),
        Some("feature/demo")
    );
}

#[test]
fn branch_rename_success_closes_form_context_and_starts_reload() {
    let target = local_branch("feature/old", false);
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        branch_manage_rename: BranchManageRenameState {
            open: true,
            target: Some(target.clone()),
            name: "feature/new".into(),
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(target.clone()),
                position: iced::Point::ORIGIN,
            }),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::Done {
        operation: branch_manage::Operation::Rename {
            target,
            new_name: "feature/new".into(),
        },
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(!app.branch_manage_rename.open);
    assert!(app.branch_manage_rename.target.is_none());
    assert!(app.selection.context_menu.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Renamed feature/old to feature/new")
    );
}

#[test]
fn branch_rename_error_preserves_form() {
    let target = local_branch("feature/old", false);
    let mut app = App {
        branch_manage_rename: BranchManageRenameState {
            open: true,
            target: Some(target.clone()),
            name: "feature/new".into(),
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::Done {
        operation: branch_manage::Operation::Rename {
            target,
            new_name: "feature/new".into(),
        },
        result: Err("rename failed".into()),
    }));

    assert!(!app.operation.loading);
    assert!(app.branch_manage_rename.open);
    assert_eq!(app.branch_manage_rename.name, "feature/new");
    assert_eq!(app.operation.error.as_deref(), Some("rename failed"));
}

#[test]
fn branch_delete_requested_uses_confirmation_prompt() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::LocalBranch(local_branch("feature/demo", false)),
    )));

    assert_eq!(
        app.selection
            .branch_delete_confirmation
            .as_ref()
            .map(|prompt| prompt.target.label()),
        Some("feature/demo")
    );

    let _ = app.update(Message::from(branch_manage::Message::DeleteCancelled));
    assert!(app.selection.branch_delete_confirmation.is_none());
}

#[test]
fn branch_delete_requested_ignores_current_branch() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::LocalBranch(local_branch("main", true)),
    )));

    assert!(app.selection.branch_delete_confirmation.is_none());
}

#[test]
fn local_folder_delete_requested_filters_current_branch() {
    let feature_a = local_branch("feature/a", false);
    let feature_current = local_branch("feature/current", true);
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::LocalBranches {
            label: "feature/".into(),
            branches: vec![feature_a.clone(), feature_current],
        },
    )));

    let prompt = app.selection.branch_delete_confirmation.as_ref().unwrap();
    match &prompt.target {
        BranchDeleteTarget::LocalBranches { label, branches } => {
            assert_eq!(label, "feature/");
            assert_eq!(branches, &[feature_a]);
        }
        other => panic!("expected local branch folder prompt, got {other:?}"),
    }
}

#[test]
fn local_branch_delete_prompt_tracks_linked_worktree_directory() {
    let target = local_branch("feature/linked", false);
    let linked = worktree_summary("/tmp/naite-linked", "feature/linked");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            worktrees: vec![linked.clone()],
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::LocalBranch(target),
    )));

    let prompt = app.selection.branch_delete_confirmation.as_ref().unwrap();
    assert!(!prompt.delete_linked_worktrees);
    assert_eq!(prompt.linked_worktrees.len(), 1);
    assert_eq!(prompt.linked_worktrees[0].branch, "feature/linked");
    assert_eq!(prompt.linked_worktrees[0].path, linked.path);
}

#[test]
fn local_branch_delete_requires_linked_worktree_toggle_before_confirming() {
    let target = local_branch("feature/linked", false);
    let linked = worktree_summary("/tmp/naite-linked", "feature/linked");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            worktrees: vec![linked],
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::LocalBranch(target),
    )));
    let _ = app.update(Message::from(branch_manage::Message::DeleteConfirmed));

    assert!(!app.operation.loading);
    assert!(app.selection.branch_delete_confirmation.is_some());
    assert_eq!(
        app.operation.error.as_deref(),
        Some("Enable linked worktree removal before deleting this branch.")
    );

    let _ = app.update(Message::from(
        branch_manage::Message::DeleteLinkedWorktreesToggled(true),
    ));

    assert!(
        app.selection
            .branch_delete_confirmation
            .as_ref()
            .unwrap()
            .delete_linked_worktrees
    );
}

#[test]
fn remote_branch_delete_requested_uses_confirmation_prompt() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                remote: vec![remote_branch("origin/claude/a")],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::RemoteBranches {
            label: "origin/claude/a".into(),
            branches: vec![remote_branch("origin/claude/a")],
        },
    )));

    let prompt = app.selection.branch_delete_confirmation.as_ref().unwrap();
    assert_eq!(prompt.target.label(), "origin/claude/a");
    assert_eq!(prompt.target.remote_branches().unwrap().len(), 1);
    assert!(!prompt.delete_matching_local_branches);
    assert!(prompt.matching_local_branches.is_empty());
}

#[test]
fn remote_folder_delete_prompt_counts_remote_only_branches() {
    let remote_a = remote_branch("origin/claude/a");
    let remote_b = remote_branch("origin/claude/b");
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                remote: vec![remote_a.clone(), remote_b.clone()],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::RemoteBranches {
            label: "origin/claude/".into(),
            branches: vec![remote_a, remote_b],
        },
    )));

    let prompt = app.selection.branch_delete_confirmation.as_ref().unwrap();
    assert_eq!(prompt.target.label(), "origin/claude/");
    assert_eq!(prompt.target.remote_branches().unwrap().len(), 2);
    assert!(prompt.matching_local_branches.is_empty());
}

#[test]
fn remote_branch_delete_toggle_updates_operation_payload_inputs() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            refs: Refs {
                local: vec![local_branch("claude/a", false)],
                remote: vec![remote_branch("origin/claude/a")],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteRequested(
        BranchDeleteTarget::RemoteBranches {
            label: "origin/claude/a".into(),
            branches: vec![remote_branch("origin/claude/a")],
        },
    )));
    let _ = app.update(Message::from(
        branch_manage::Message::DeleteMatchingLocalBranchesToggled(true),
    ));

    let prompt = app.selection.branch_delete_confirmation.as_ref().unwrap();
    assert!(prompt.delete_matching_local_branches);
    assert_eq!(prompt.matching_local_branches, vec!["claude/a"]);
}

#[test]
fn confirmed_branch_delete_starts_operation() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            branch_delete_confirmation: Some(BranchDeletePrompt {
                target: BranchDeleteTarget::LocalBranch(local_branch("feature/demo", false)),
                delete_matching_local_branches: false,
                matching_local_branches: Vec::new(),
                delete_linked_worktrees: false,
                linked_worktrees: Vec::new(),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::DeleteConfirmed));

    assert!(app.operation.loading);
    assert!(app.selection.branch_delete_confirmation.is_none());
}

#[test]
fn branch_delete_success_closes_prompt_context_and_starts_reload() {
    let target = local_branch("feature/demo", false);
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::Ref(target.clone()),
                position: iced::Point::ORIGIN,
            }),
            branch_delete_confirmation: Some(BranchDeletePrompt {
                target: BranchDeleteTarget::LocalBranch(target.clone()),
                delete_matching_local_branches: false,
                matching_local_branches: Vec::new(),
                delete_linked_worktrees: false,
                linked_worktrees: Vec::new(),
            }),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::Done {
        operation: branch_manage::Operation::Delete {
            target: BranchDeleteTarget::LocalBranch(target),
            delete_matching_local_branches: false,
            delete_linked_worktrees: false,
            linked_worktrees: Vec::new(),
        },
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.selection.branch_delete_confirmation.is_none());
    assert!(app.selection.context_menu.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Deleted feature/demo")
    );
}

#[test]
fn remote_branch_delete_success_closes_prompt_context_and_starts_reload() {
    let target = BranchDeleteTarget::RemoteBranches {
        label: "origin/claude/".into(),
        branches: vec![
            remote_branch("origin/claude/a"),
            remote_branch("origin/claude/b"),
        ],
    };
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            context_menu: Some(crate::state::ContextMenuState {
                kind: crate::state::ContextMenuKind::RemoteBranchFolder {
                    label: "origin/claude/".into(),
                    branches: target.remote_branches().unwrap().to_vec(),
                },
                position: iced::Point::ORIGIN,
            }),
            branch_delete_confirmation: Some(BranchDeletePrompt {
                target: target.clone(),
                delete_matching_local_branches: true,
                matching_local_branches: vec!["claude/a".into()],
                delete_linked_worktrees: false,
                linked_worktrees: Vec::new(),
            }),
            ..Default::default()
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(branch_manage::Message::Done {
        operation: branch_manage::Operation::Delete {
            target,
            delete_matching_local_branches: true,
            delete_linked_worktrees: false,
            linked_worktrees: Vec::new(),
        },
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(app.selection.branch_delete_confirmation.is_none());
    assert!(app.selection.context_menu.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Deleted origin/claude/ and matching local branches")
    );
}

#[test]
fn escape_closes_branch_delete_prompt_and_rename_form() {
    let mut app = App {
        selection: SelectionState {
            branch_delete_confirmation: Some(BranchDeletePrompt {
                target: BranchDeleteTarget::LocalBranch(local_branch("feature/demo", false)),
                delete_matching_local_branches: false,
                matching_local_branches: Vec::new(),
                delete_linked_worktrees: false,
                linked_worktrees: Vec::new(),
            }),
            ..Default::default()
        },
        branch_manage_rename: BranchManageRenameState {
            open: true,
            target: Some(local_branch("feature/old", false)),
            name: "feature/new".into(),
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert!(app.selection.branch_delete_confirmation.is_none());
    assert!(app.branch_manage_rename.open);

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert!(!app.branch_manage_rename.open);
    assert_eq!(app.branch_manage_rename.name, "feature/new");
}

#[test]
fn stash_create_submit_requires_untracked_toggle_for_untracked_only_worktree() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            status_detail: untracked_status_detail(),
            ..Default::default()
        },
        stash_create: crate::state::StashCreateState {
            open: true,
            include_untracked: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::Submitted));
    assert!(!app.operation.loading);
    assert!(app.stash_create.open);

    let _ = app.update(Message::from(stash::Message::IncludeUntrackedChanged(true)));
    let _ = app.update(Message::from(stash::Message::Submitted));
    assert!(app.operation.loading);
}

#[test]
fn stash_create_success_closes_form_and_starts_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        stash_create: crate::state::StashCreateState {
            open: true,
            message: "save work".into(),
            include_untracked: true,
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::Done {
        operation: stash::Operation::Create {
            message: "save work".into(),
            include_untracked: true,
        },
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(!app.stash_create.open);
    assert!(app.stash_create.message.is_empty());
    assert!(!app.stash_create.include_untracked);
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Stashed working tree changes")
    );
}

#[test]
fn stash_create_error_preserves_form() {
    let mut app = App {
        stash_create: crate::state::StashCreateState {
            open: true,
            message: "save work".into(),
            include_untracked: true,
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::Done {
        operation: stash::Operation::Create {
            message: "save work".into(),
            include_untracked: true,
        },
        result: Err("stash failed".into()),
    }));

    assert!(!app.operation.loading);
    assert!(app.stash_create.open);
    assert_eq!(app.stash_create.message, "save work");
    assert!(app.stash_create.include_untracked);
    assert_eq!(app.operation.error.as_deref(), Some("stash failed"));
}

#[test]
fn stash_branch_request_opens_form_with_default_name() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        stash_create: crate::state::StashCreateState {
            open: true,
            message: "save work".into(),
            ..Default::default()
        },
        selection: SelectionState {
            stash_confirmation: Some(crate::StashPrompt {
                action: StashPromptAction::Drop,
                stash: stash_summary("stash@{1}"),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::BranchRequested(
        stash_summary("stash@{2}"),
    )));

    assert!(app.stash_branch.open);
    assert_eq!(app.stash_branch.name, "stash/stash-2");
    assert_eq!(
        app.stash_branch
            .stash
            .as_ref()
            .map(|stash| stash.selector.as_str()),
        Some("stash@{2}")
    );
    assert!(!app.stash_create.open);
    assert!(app.selection.stash_confirmation.is_none());
}

#[test]
fn stash_branch_submit_requires_name_and_stash() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        stash_branch: StashBranchState {
            open: true,
            name: String::new(),
            stash: Some(stash_summary("stash@{0}")),
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::BranchSubmitted));
    assert!(!app.operation.loading);
    assert!(app.stash_branch.open);

    let _ = app.update(Message::from(stash::Message::BranchNameChanged(
        "feature/from-stash".into(),
    )));
    let _ = app.update(Message::from(stash::Message::BranchSubmitted));
    assert!(app.operation.loading);
}

#[test]
fn stash_branch_success_closes_form_and_starts_reload() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        stash_branch: StashBranchState {
            open: true,
            name: "feature/from-stash".into(),
            stash: Some(stash_summary("stash@{0}")),
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::Done {
        operation: stash::Operation::Branch {
            stash: stash_summary("stash@{0}"),
            branch_name: "feature/from-stash".into(),
        },
        result: Ok(()),
    }));

    assert!(app.operation.loading);
    assert!(!app.stash_branch.open);
    assert!(app.stash_branch.name.is_empty());
    assert!(app.stash_branch.stash.is_none());
    assert_eq!(
        app.operation
            .pending_transient_status_after_reload
            .as_deref(),
        Some("Created branch feature/from-stash from stash@{0}")
    );
}

#[test]
fn stash_branch_error_preserves_form() {
    let mut app = App {
        stash_branch: StashBranchState {
            open: true,
            name: "feature/from-stash".into(),
            stash: Some(stash_summary("stash@{0}")),
        },
        operation: OperationState {
            loading: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::Done {
        operation: stash::Operation::Branch {
            stash: stash_summary("stash@{0}"),
            branch_name: "feature/from-stash".into(),
        },
        result: Err("stash branch failed".into()),
    }));

    assert!(!app.operation.loading);
    assert!(app.stash_branch.open);
    assert_eq!(app.stash_branch.name, "feature/from-stash");
    assert_eq!(
        app.stash_branch
            .stash
            .as_ref()
            .map(|stash| stash.selector.as_str()),
        Some("stash@{0}")
    );
    assert_eq!(app.operation.error.as_deref(), Some("stash branch failed"));
}

#[test]
fn stash_pop_and_drop_use_confirmation_prompt() {
    let mut app = App::default();

    let _ = app.update(Message::from(stash::Message::PopRequested(stash_summary(
        "stash@{0}",
    ))));
    assert_eq!(
        app.selection
            .stash_confirmation
            .as_ref()
            .map(|prompt| prompt.action),
        Some(StashPromptAction::Pop)
    );

    let _ = app.update(Message::from(stash::Message::ConfirmationCancelled));
    assert!(app.selection.stash_confirmation.is_none());

    let _ = app.update(Message::from(stash::Message::DropRequested(stash_summary(
        "stash@{1}",
    ))));
    assert_eq!(
        app.selection
            .stash_confirmation
            .as_ref()
            .map(|prompt| (prompt.action, prompt.stash.selector.as_str())),
        Some((StashPromptAction::Drop, "stash@{1}"))
    );
}

#[test]
fn confirmed_stash_prompt_starts_operation() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        selection: SelectionState {
            stash_confirmation: Some(crate::StashPrompt {
                action: StashPromptAction::Pop,
                stash: stash_summary("stash@{0}"),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(stash::Message::Confirmed));

    assert!(app.operation.loading);
    assert!(app.selection.stash_confirmation.is_none());
}

#[test]
fn escape_closes_stash_prompt_and_form() {
    let mut app = App {
        selection: SelectionState {
            stash_confirmation: Some(crate::StashPrompt {
                action: StashPromptAction::Drop,
                stash: stash_summary("stash@{0}"),
            }),
            ..Default::default()
        },
        stash_create: crate::state::StashCreateState {
            open: true,
            message: "save work".into(),
            ..Default::default()
        },
        stash_branch: StashBranchState {
            open: true,
            name: "feature/from-stash".into(),
            stash: Some(stash_summary("stash@{0}")),
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert!(app.selection.stash_confirmation.is_none());
    assert!(app.stash_create.open);
    assert!(app.stash_branch.open);

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert!(!app.stash_create.open);
    assert_eq!(app.stash_create.message, "save work");
    assert!(app.stash_branch.open);

    let _ = app.update(Message::Keyboard(KeyAction::Escape));
    assert!(!app.stash_branch.open);
    assert_eq!(app.stash_branch.name, "feature/from-stash");
}

#[test]
fn select_wip_command_selects_dirty_worktree() {
    let mut app = App {
        command_palette: CommandPaletteState {
            open: true,
            ..Default::default()
        },
        repo: RepositoryState {
            status_detail: dirty_status_detail(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(command_palette::Message::Run(
        CommandId::SelectWip,
    )));

    assert!(!app.command_palette.open);
    assert!(app.selection.selected_wip);
    assert_eq!(
        app.selection.selected_wip_file,
        Some(wip_target(WorktreeDiffKind::Unstaged, "src/main.rs"))
    );
}

#[test]
fn worktree_selection_clears_git_selection_and_creates_terminal_session() {
    let target = worktree_summary("/tmp/naite-linked", "feature/linked");
    let mut app = App {
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            selected_wip: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(worktree::Message::Selected(target.clone())));

    assert_eq!(app.selection.selected_worktree, Some(target.clone()));
    assert!(app.selection.selected_commit_id.is_none());
    assert!(!app.selection.selected_wip);
    let active = app.terminal.active_session().unwrap();
    assert_eq!(active.target.cwd, target.path);
    assert_eq!(active.status, TerminalStatus::Idle);
    assert!(!active.pending_start);
}

#[test]
fn worktree_command_palette_items_require_selected_worktree() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(app
        .command_palette_items()
        .iter()
        .find(|item| item.id == CommandId::OpenSelectedWorktree)
        .unwrap()
        .disabled_reason
        .is_some());

    app.selection.selected_worktree = Some(worktree_summary("/tmp/naite-linked", "feature"));

    assert!(app
        .command_palette_items()
        .iter()
        .find(|item| item.id == CommandId::OpenSelectedWorktree)
        .unwrap()
        .enabled());
}

#[test]
fn worktree_remove_request_blocks_current_or_locked_worktrees() {
    let mut app = App::default();
    let mut current = worktree_summary("/tmp/current", "main");
    current.is_current = true;
    let mut locked = worktree_summary("/tmp/locked", "feature/locked");
    locked.locked = true;

    let _ = app.update(Message::from(worktree::Message::RemoveRequested(current)));
    assert!(app.selection.worktree_remove_confirmation.is_none());
    assert_eq!(
        app.operation.error.as_deref(),
        Some("Cannot remove the current worktree.")
    );

    app.operation.error = None;
    let _ = app.update(Message::from(worktree::Message::RemoveRequested(locked)));
    assert!(app.selection.worktree_remove_confirmation.is_none());
    assert_eq!(
        app.operation.error.as_deref(),
        Some("Unlock the worktree before removing it.")
    );
}

#[test]
fn terminal_idle_session_enables_start_command() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let id = app
        .terminal
        .ensure_session(PathBuf::from("/tmp/naite"), "Current repo".into());
    app.terminal.active = Some(id);
    app.terminal.open = true;

    assert!(app.terminal.open);
    assert!(app
        .command_palette_items()
        .iter()
        .find(|item| item.id == CommandId::RunTerminalCommand)
        .unwrap()
        .enabled());
}

#[test]
fn terminal_command_palette_item_shows_keyboard_shortcut() {
    let app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let item = app
        .command_palette_items()
        .into_iter()
        .find(|item| item.id == CommandId::OpenTerminal)
        .unwrap();

    assert_eq!(item.shortcut, "Cmd `");
    assert!(item.enabled());
}

#[test]
fn terminal_new_session_uses_active_shell_cwd() {
    let repo_path = PathBuf::from("/tmp/naite");
    let shell_cwd = repo_path.join("crates/naite-app");
    let mut app = App {
        repo: RepositoryState {
            path: Some(repo_path.clone()),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let first_id = app
        .terminal
        .ensure_session(repo_path.clone(), "main".into());
    app.terminal.session_mut(first_id).unwrap().shell_cwd = Some(shell_cwd.clone());

    let _ = app.update(Message::from(terminal::Message::NewSessionRequested));

    let session = app.terminal.active_session().unwrap();
    assert_ne!(session.id, first_id);
    assert_eq!(session.target.cwd, shell_cwd);
    assert_eq!(session.target.repo_tab, Some(repo_path));
    assert_eq!(session.status, TerminalStatus::Starting);
    assert!(session.pending_start);
}

#[test]
fn terminal_new_session_without_active_session_uses_repo_root() {
    let repo_path = PathBuf::from("/tmp/naite");
    let mut app = App {
        repo: RepositoryState {
            path: Some(repo_path.clone()),
            head_branch: Some("main".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(terminal::Message::NewSessionRequested));

    let session = app.terminal.active_session().unwrap();
    assert_eq!(session.target.cwd, repo_path.clone());
    assert_eq!(session.target.repo_tab, Some(repo_path));
    assert_eq!(session.label, "main");
}

#[test]
fn keyboard_shortcut_opens_terminal_even_when_text_input_captured() {
    let command_message = keyboard_shortcut(
        Key::Character("`".into()),
        Physical::Code(Code::Backquote),
        Modifiers::COMMAND,
        event::Status::Captured,
    );
    let control_message = keyboard_shortcut(
        Key::Character("`".into()),
        Physical::Code(Code::Backquote),
        Modifiers::CTRL,
        event::Status::Captured,
    );

    assert!(matches!(
        command_message,
        Some(Message::Keyboard(KeyAction::OpenTerminal))
    ));
    assert!(matches!(
        control_message,
        Some(Message::Keyboard(KeyAction::OpenTerminal))
    ));
}

#[test]
fn terminal_keyboard_capture_keeps_open_terminal_shortcut_available() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("`".into()),
            modified_key: Key::Character("`".into()),
            physical_key: Physical::Code(Code::Backquote),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: Some("`".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Keyboard(KeyAction::OpenTerminal))
    ));
}

#[test]
fn terminal_keyboard_capture_keeps_app_command_shortcuts_available() {
    let palette = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㅏ".into()),
            modified_key: Key::Character("ㅏ".into()),
            physical_key: Physical::Code(Code::KeyK),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: Some("ㅏ".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );
    let search = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("f".into()),
            modified_key: Key::Character("f".into()),
            physical_key: Physical::Code(Code::KeyF),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: Some("f".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );
    let release = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㄱ".into()),
            modified_key: Key::Character("ㄱ".into()),
            physical_key: Physical::Code(Code::KeyR),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND | Modifiers::SHIFT,
            text: Some("ㄱ".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        palette,
        Some(Message::Keyboard(KeyAction::OpenCommandPalette))
    ));
    assert!(matches!(
        search,
        Some(Message::Keyboard(KeyAction::FocusSearch))
    ));
    assert!(matches!(
        release,
        Some(Message::Keyboard(KeyAction::ReleasePromotion))
    ));
}

#[test]
fn terminal_keyboard_capture_preserves_control_chords_for_shell() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㅏ".into()),
            modified_key: Key::Character("ㅏ".into()),
            physical_key: Physical::Code(Code::KeyK),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::CTRL,
            text: Some("ㅏ".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Input(
            terminal::TerminalInput::Bytes(bytes)
        ))) if bytes == vec![0x0b]
    ));
}

#[test]
fn terminal_keyboard_capture_sends_korean_text_when_event_text_is_missing() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㅎ".into()),
            modified_key: Key::Character("한".into()),
            physical_key: Physical::Code(Code::KeyG),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::default(),
            text: None,
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Input(
            terminal::TerminalInput::Text(text)
        ))) if text == "한"
    ));
}

#[test]
fn terminal_keyboard_capture_composes_decomposed_hangul_text() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㄱ".into()),
            modified_key: Key::Character("ㄱ".into()),
            physical_key: Physical::Code(Code::KeyR),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::default(),
            text: Some("\u{1100}\u{1161}".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Input(
            terminal::TerminalInput::Text(text)
        ))) if text == "\u{1100}\u{1161}"
    ));
}

#[test]
fn terminal_keyboard_capture_keeps_compatibility_jamo_as_preedit_fallback() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㅎ".into()),
            modified_key: Key::Character("ㅎ".into()),
            physical_key: Physical::Code(Code::KeyG),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::default(),
            text: Some("ㅎ".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Ime(
            terminal::TerminalIme::FallbackPreedit(text)
        ))) if text == "ㅎ"
    ));
}

#[test]
fn terminal_keyboard_capture_keeps_korean_preedit_after_stale_command_modifier() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㅎ".into()),
            modified_key: Key::Character("ㅎ".into()),
            physical_key: Physical::Code(Code::KeyG),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: Some("ㅎ".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Ime(
            terminal::TerminalIme::FallbackPreedit(text)
        ))) if text == "ㅎ"
    ));
}

#[test]
fn terminal_ime_commit_writes_finalized_text_without_manual_jamo_patching() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        100,
        24,
    );
    app.terminal.open = true;
    app.terminal.active = Some(id);

    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: "하".into(),
            cursor: Some((3, 3)),
        },
    )));

    assert_eq!(
        app.terminal
            .session(id)
            .and_then(|session| session.ime_preedit.as_ref())
            .map(|preedit| preedit.text.as_str()),
        Some("하")
    );

    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Commit("하이".into()),
    )));

    assert!(app
        .terminal
        .session(id)
        .is_some_and(|session| session.ime_preedit.is_none()));
}

#[test]
fn terminal_ime_preedit_cursor_splits_after_composed_text() {
    let preedit = TerminalImePreedit {
        text: "안".into(),
        cursor: Some((3, 3)),
    };

    assert_eq!(
        crate::widgets::terminal_split_ime_preedit_at_cursor(&preedit),
        ("안", "")
    );
}

#[test]
fn terminal_keyboard_capture_keeps_captured_palette_navigation_available() {
    let down = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Named(Named::ArrowDown),
            modified_key: Key::Named(Named::ArrowDown),
            physical_key: Physical::Code(Code::ArrowDown),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::default(),
            text: None,
        }),
        event::Status::Captured,
        window::Id::unique(),
    );
    let enter = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Named(Named::Enter),
            modified_key: Key::Named(Named::Enter),
            physical_key: Physical::Code(Code::Enter),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::default(),
            text: None,
        }),
        event::Status::Captured,
        window::Id::unique(),
    );

    assert!(matches!(
        down,
        Some(Message::Keyboard(KeyAction::CommandPaletteNext))
    ));
    assert!(matches!(
        enter,
        Some(Message::Keyboard(KeyAction::CommandPaletteRun))
    ));
}

#[test]
fn terminal_keyboard_capture_sends_control_c_by_physical_key() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("ㅊ".into()),
            modified_key: Key::Character("ㅊ".into()),
            physical_key: Physical::Code(Code::KeyC),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::CTRL,
            text: Some("ㅊ".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Input(
            terminal::TerminalInput::Bytes(bytes)
        ))) if bytes == vec![0x03]
    ));
}

#[test]
fn terminal_keyboard_capture_maps_command_backspace_to_kill_line() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Named(Named::Backspace),
            modified_key: Key::Named(Named::Backspace),
            physical_key: Physical::Code(Code::Backspace),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: None,
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Input(
            terminal::TerminalInput::Bytes(bytes)
        ))) if bytes == vec![0x15]
    ));
}

#[test]
fn terminal_keyboard_capture_maps_captured_command_backspace_to_kill_line() {
    let message = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Named(Named::Backspace),
            modified_key: Key::Named(Named::Backspace),
            physical_key: Physical::Code(Code::Backspace),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: None,
        }),
        event::Status::Captured,
        window::Id::unique(),
    );

    assert!(matches!(
        message,
        Some(Message::Terminal(terminal::Message::Input(
            terminal::TerminalInput::Bytes(bytes)
        ))) if bytes == vec![0x15]
    ));
}

#[test]
fn terminal_ime_command_backspace_clears_preedit_and_kills_line_immediately() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        100,
        24,
    );
    app.terminal.open = true;
    app.terminal.active = Some(id);

    let _ = app.update(Message::from(terminal::Message::ModifiersChanged(
        Modifiers::COMMAND,
    )));
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: "안".into(),
            cursor: Some((3, 3)),
        },
    )));
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: String::new(),
            cursor: None,
        },
    )));

    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none() && session.ime_modified_delete_pending.is_none()
    }));
    assert!(matches!(
        app.operation.error.as_deref(),
        Some(message) if message.starts_with("terminal runtime")
    ));

    app.operation.error = None;
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Commit("안".into()),
    )));
    assert!(app.operation.error.is_none());
    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none()
            && session.ime_modified_delete_pending.is_none()
            && session.ime_suppressed_commit.is_none()
    }));

    let _ = app.update(Message::from(terminal::Message::KeyReleased {
        key: Key::Named(Named::Backspace),
        modifiers: Modifiers::COMMAND,
    }));

    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none() && session.ime_modified_delete_pending.is_none()
    }));
    assert!(app.operation.error.is_none());
}

#[test]
fn terminal_ime_command_backspace_release_handles_command_release_order() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        100,
        24,
    );
    app.terminal.open = true;
    app.terminal.active = Some(id);

    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: "안".into(),
            cursor: Some((3, 3)),
        },
    )));
    let _ = app.update(Message::from(terminal::Message::ModifiersChanged(
        Modifiers::COMMAND,
    )));
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: String::new(),
            cursor: None,
        },
    )));
    let _ = app.update(Message::from(terminal::Message::ModifiersChanged(
        Modifiers::default(),
    )));

    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none() && session.ime_modified_delete_pending.is_none()
    }));
    assert!(matches!(
        app.operation.error.as_deref(),
        Some(message) if message.starts_with("terminal runtime")
    ));

    let _ = app.update(Message::from(terminal::Message::KeyReleased {
        key: Key::Named(Named::Backspace),
        modifiers: Modifiers::default(),
    }));

    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none() && session.ime_modified_delete_pending.is_none()
    }));
    assert!(matches!(
        app.terminal
            .session(id)
            .and_then(|session| session.ime_suppressed_commit.as_deref()),
        Some("안")
    ));

    app.operation.error = None;
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Commit("안".into()),
    )));
    assert!(app.operation.error.is_none());
    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none()
            && session.ime_modified_delete_pending.is_none()
            && session.ime_suppressed_commit.is_none()
    }));
}

#[test]
fn terminal_ime_command_backspace_disabled_suppresses_late_commit() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        100,
        24,
    );
    app.terminal.open = true;
    app.terminal.active = Some(id);

    let _ = app.update(Message::from(terminal::Message::ModifiersChanged(
        Modifiers::COMMAND,
    )));
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: "안".into(),
            cursor: Some((3, 3)),
        },
    )));
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Disabled,
    )));

    assert!(matches!(
        app.operation.error.as_deref(),
        Some(message) if message.starts_with("terminal runtime")
    ));
    assert!(matches!(
        app.terminal
            .session(id)
            .and_then(|session| session.ime_suppressed_commit.as_deref()),
        Some("안")
    ));

    app.operation.error = None;
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Commit("안".into()),
    )));
    assert!(app.operation.error.is_none());
    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none()
            && session.ime_modified_delete_pending.is_none()
            && session.ime_suppressed_commit.is_none()
    }));
}

#[test]
fn terminal_ime_option_backspace_clears_preedit_and_kills_word_immediately() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        100,
        24,
    );
    app.terminal.open = true;
    app.terminal.active = Some(id);

    let _ = app.update(Message::from(terminal::Message::ModifiersChanged(
        Modifiers::ALT,
    )));
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: "안".into(),
            cursor: Some((3, 3)),
        },
    )));

    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_some()
            && session.ime_modified_delete_pending == Some(TerminalImeDeleteAction::KillWord)
    }));

    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Preedit {
            text: String::new(),
            cursor: None,
        },
    )));

    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none() && session.ime_modified_delete_pending.is_none()
    }));
    assert!(matches!(
        app.operation.error.as_deref(),
        Some(message) if message.starts_with("terminal runtime")
    ));

    app.operation.error = None;
    let _ = app.update(Message::from(terminal::Message::Ime(
        terminal::TerminalIme::Commit("안".into()),
    )));
    assert!(app.operation.error.is_none());
    assert!(app.terminal.session(id).is_some_and(|session| {
        session.ime_preedit.is_none()
            && session.ime_modified_delete_pending.is_none()
            && session.ime_suppressed_commit.is_none()
    }));
}

#[test]
fn terminal_keyboard_capture_maps_command_copy_and_paste() {
    let copy = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("c".into()),
            modified_key: Key::Character("c".into()),
            physical_key: Physical::Code(Code::KeyC),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: Some("c".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );
    let paste = terminal_app_event(
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character("v".into()),
            modified_key: Key::Character("v".into()),
            physical_key: Physical::Code(Code::KeyV),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            text: Some("v".into()),
        }),
        event::Status::Ignored,
        window::Id::unique(),
    );

    assert!(matches!(
        copy,
        Some(Message::Terminal(terminal::Message::CopySelectionRequested))
    ));
    assert!(matches!(
        paste,
        Some(Message::Terminal(terminal::Message::PasteRequested))
    ));
}

#[test]
fn terminal_selection_extracts_linear_multiline_text() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        80,
        24,
    );
    let session = app.terminal.session_mut(id).unwrap();
    session.screen = TerminalScreen {
        cols: 80,
        rows: 24,
        lines: vec![
            terminal_line("alpha beta"),
            terminal_line("gamma"),
            terminal_line("delta epsilon"),
        ],
        cursor: None,
        scrollback_len: 0,
    };
    session.selection = Some(TerminalSelection {
        anchor: TerminalGridPoint { row: 0, col: 6 },
        focus: TerminalGridPoint { row: 2, col: 5 },
        active: false,
    });

    assert_eq!(
        app.terminal
            .active_session()
            .unwrap()
            .selected_text()
            .as_deref(),
        Some("beta\ngamma\ndelta")
    );
}

#[test]
fn terminal_drag_selection_uses_terminal_cell_coordinates() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        80,
        24,
    );
    app.terminal.session_mut(id).unwrap().screen = TerminalScreen {
        cols: 80,
        rows: 24,
        lines: vec![terminal_line("alpha beta"), terminal_line("gamma")],
        cursor: None,
        scrollback_len: 0,
    };

    let _ = app.update(Message::from(terminal::Message::PointerMoved(
        iced::Point::new(
            crate::widgets::TERMINAL_CHAR_WIDTH,
            crate::widgets::TERMINAL_LINE_HEIGHT * 0.2,
        ),
    )));
    let _ = app.update(Message::from(terminal::Message::SelectionStarted));
    let _ = app.update(Message::from(terminal::Message::PointerMoved(
        iced::Point::new(
            crate::widgets::TERMINAL_CHAR_WIDTH * 5.0,
            crate::widgets::TERMINAL_LINE_HEIGHT * 0.2,
        ),
    )));
    let _ = app.update(Message::from(terminal::Message::SelectionEnded));

    assert_eq!(
        app.terminal
            .active_session()
            .unwrap()
            .selected_text()
            .as_deref(),
        Some("lpha")
    );
}

#[test]
fn terminal_click_without_drag_clears_existing_selection() {
    let mut app = App::default();
    let id = app.terminal.create_session(
        terminal::TerminalTarget::new(PathBuf::from("/tmp/naite"), None, None),
        "main".into(),
        "/bin/zsh".into(),
        80,
        24,
    );
    let session = app.terminal.session_mut(id).unwrap();
    session.screen = TerminalScreen {
        cols: 80,
        rows: 24,
        lines: vec![terminal_line("alpha beta")],
        cursor: None,
        scrollback_len: 0,
    };
    session.selection = Some(TerminalSelection {
        anchor: TerminalGridPoint { row: 0, col: 0 },
        focus: TerminalGridPoint { row: 0, col: 5 },
        active: false,
    });

    let _ = app.update(Message::from(terminal::Message::PointerMoved(
        iced::Point::new(
            crate::widgets::TERMINAL_CHAR_WIDTH * 20.0,
            crate::widgets::TERMINAL_LINE_HEIGHT * 8.0,
        ),
    )));
    let _ = app.update(Message::from(terminal::Message::SelectionStarted));
    let _ = app.update(Message::from(terminal::Message::SelectionEnded));

    assert!(app.terminal.active_session().unwrap().selection.is_none());
}

#[test]
fn open_terminal_shortcut_opens_repo_terminal_session() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::OpenTerminal));

    assert!(app.terminal.open);
    let session = app.terminal.active_session().unwrap();
    assert_eq!(session.target.cwd, PathBuf::from("/tmp/naite"));
    assert!(session.pending_start);
}

#[test]
fn terminal_stale_runtime_event_is_ignored() {
    let mut app = App::default();

    let _ = app.update(Message::from(terminal::Message::RuntimeEvent(
        terminal::TerminalEvent::ScreenUpdated {
            id: terminal::TerminalSessionId(404),
            screen: crate::state::TerminalScreen::default(),
        },
    )));

    assert!(app.terminal.sessions.is_empty());
}

#[test]
fn workspace_dashboard_toggle_is_state_only() {
    let mut app = App::default();

    let _ = app.update(Message::from(workspace::Message::DashboardToggled));
    assert!(app.workspace.dashboard_open);

    let _ = app.update(Message::from(workspace::Message::DashboardToggled));
    assert!(!app.workspace.dashboard_open);
}

#[test]
fn workspace_open_repo_closes_dashboard() {
    let mut app = App {
        workspace: crate::state::WorkspaceState {
            dashboard_open: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::from(workspace::Message::OpenRepo(PathBuf::from(
        "/tmp/naite",
    ))));

    assert!(!app.workspace.dashboard_open);
}

#[test]
fn clone_form_is_hidden_until_requested() {
    let mut app = App::default();

    assert!(!app.manager.clone_open);

    let _ = app.update(Message::from(repo_open::Message::CloneFormToggled));
    assert!(app.manager.clone_open);
    assert!(app.operation.error.is_none());

    let _ = app.update(Message::from(repo_open::Message::CloneFormToggled));
    assert!(!app.manager.clone_open);
}

#[test]
fn clone_url_input_keeps_clone_form_open() {
    let mut app = App::default();

    let _ = app.update(Message::from(repo_open::Message::CloneUrlChanged(
        "git@example.com:owner/repo.git".into(),
    )));

    assert!(app.manager.clone_open);
    assert_eq!(app.manager.clone_url, "git@example.com:owner/repo.git");
}

#[test]
fn phase3_session_tracks_three_repo_tabs_and_two_worktrees() {
    let mut app = App::default();
    for path in ["/tmp/repo-one", "/tmp/repo-two", "/tmp/repo-three"] {
        let path = PathBuf::from(path);
        app.tabs.remember(path.clone());
        app.workspace.summaries.push(WorkspaceRepoSummary {
            path: path.clone(),
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            current_branch: Some("main".into()),
            ..Default::default()
        });
    }
    app.repo.worktrees = vec![
        worktree_summary("/tmp/repo-one-linked-a", "feature/a"),
        worktree_summary("/tmp/repo-one-linked-b", "feature/b"),
    ];

    assert_eq!(app.tabs.open.len(), 3);
    assert_eq!(app.workspace.summaries.len(), 3);
    assert_eq!(app.repo.worktrees.len(), 2);
}

#[test]
fn arrow_keys_navigate_commit_graph_when_not_captured() {
    let down = keyboard_shortcut(
        Key::Named(Named::ArrowDown),
        Physical::Code(Code::ArrowDown),
        Modifiers::default(),
        event::Status::Ignored,
    );
    let up = keyboard_shortcut(
        Key::Named(Named::ArrowUp),
        Physical::Code(Code::ArrowUp),
        Modifiers::default(),
        event::Status::Ignored,
    );

    assert!(matches!(
        down,
        Some(Message::Keyboard(KeyAction::NextCommit))
    ));
    assert!(matches!(
        up,
        Some(Message::Keyboard(KeyAction::PreviousCommit))
    ));
}

#[test]
fn arrow_keys_keep_palette_navigation_when_text_input_captured() {
    let down = keyboard_shortcut(
        Key::Named(Named::ArrowDown),
        Physical::Code(Code::ArrowDown),
        Modifiers::default(),
        event::Status::Captured,
    );
    let up = keyboard_shortcut(
        Key::Named(Named::ArrowUp),
        Physical::Code(Code::ArrowUp),
        Modifiers::default(),
        event::Status::Captured,
    );

    assert!(matches!(
        down,
        Some(Message::Keyboard(KeyAction::CommandPaletteNext))
    ));
    assert!(matches!(
        up,
        Some(Message::Keyboard(KeyAction::CommandPalettePrevious))
    ));
}

#[test]
fn commit_keyboard_navigation_moves_selected_commit() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![
                commit("a111111", "add app shell", "june"),
                commit("b222222", "fix diff pane", "alex"),
                commit("c333333", "tune graph", "riley"),
            ],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(1),
            selected_commit_id: Some("b222222".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::NextCommit));
    assert_eq!(app.selection.selected, Some(2));
    assert_eq!(app.selection.selected_commit_id.as_deref(), Some("c333333"));

    let _ = app.update(Message::Keyboard(KeyAction::PreviousCommit));
    assert_eq!(app.selection.selected, Some(1));
    assert_eq!(app.selection.selected_commit_id.as_deref(), Some("b222222"));
}

#[test]
fn keyboard_shortcuts_ignore_captured_text_input_navigation() {
    let message = keyboard_shortcut(
        Key::Character("j".into()),
        Physical::Code(Code::KeyJ),
        Modifiers::default(),
        event::Status::Captured,
    );

    assert!(message.is_none());
}

#[test]
fn keyboard_shortcuts_keep_escape_for_captured_text_input() {
    let message = keyboard_shortcut(
        Key::Named(Named::Escape),
        Physical::Code(Code::Escape),
        Modifiers::default(),
        event::Status::Captured,
    );

    assert!(matches!(
        message,
        Some(Message::Keyboard(KeyAction::Escape))
    ));
}

#[test]
fn keyboard_shortcuts_open_palette_even_when_text_input_captured() {
    let command_message = keyboard_shortcut(
        Key::Character("ㅏ".into()),
        Physical::Code(Code::KeyK),
        Modifiers::COMMAND,
        event::Status::Captured,
    );
    let control_message = keyboard_shortcut(
        Key::Character("ㅏ".into()),
        Physical::Code(Code::KeyK),
        Modifiers::CTRL,
        event::Status::Captured,
    );

    assert!(matches!(
        command_message,
        Some(Message::Keyboard(KeyAction::OpenCommandPalette))
    ));
    assert!(matches!(
        control_message,
        Some(Message::Keyboard(KeyAction::OpenCommandPalette))
    ));
}

#[test]
fn keyboard_shortcut_opens_release_promotion_even_when_text_input_captured() {
    let message = keyboard_shortcut(
        Key::Character("ㄱ".into()),
        Physical::Code(Code::KeyR),
        Modifiers::COMMAND | Modifiers::SHIFT,
        event::Status::Captured,
    );

    assert!(matches!(
        message,
        Some(Message::Keyboard(KeyAction::ReleasePromotion))
    ));
}

#[test]
fn keyboard_shortcut_opens_tag_deployment_even_when_text_input_captured() {
    let message = keyboard_shortcut(
        Key::Character("ㅅ".into()),
        Physical::Code(Code::KeyT),
        Modifiers::COMMAND | Modifiers::SHIFT,
        event::Status::Captured,
    );

    assert!(matches!(
        message,
        Some(Message::Keyboard(KeyAction::CreateAndPushTag))
    ));
}

// --- Track C: commit action keyboard shortcuts ---

#[test]
fn commit_action_r_dispatches_reword_when_commit_selected() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![
                commit("a111111", "add app shell", "june"),
                commit("b222222", "fix diff pane", "alex"),
            ],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::RewordSelectedCommit));

    // The reword request opens the reword prompt
    assert!(app.history_reword.open);
    assert_eq!(
        app.history_reword.commit.as_ref().map(|c| c.id.as_str()),
        Some("a111111")
    );
}

#[test]
fn commit_action_r_is_blocked_when_history_confirmation_open() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("a111111", "add app shell", "june")],
            ..Default::default()
        },
        selection: SelectionState {
            selected: Some(0),
            selected_commit_id: Some("a111111".into()),
            history_confirmation: Some(crate::HistoryPrompt {
                operation: history::Operation::Drop(commit("a111111", "add app shell", "june")),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::RewordSelectedCommit));

    // Blocked — reword prompt must NOT have opened
    assert!(!app.history_reword.open);
}

#[test]
fn commit_action_r_is_noop_when_no_commit_selected() {
    let mut app = App {
        repo: RepositoryState {
            path: Some(PathBuf::from("/tmp/naite")),
            commits: vec![commit("a111111", "add app shell", "june")],
            ..Default::default()
        },
        selection: SelectionState {
            selected: None,
            ..Default::default()
        },
        ..Default::default()
    };

    let _ = app.update(Message::Keyboard(KeyAction::RewordSelectedCommit));

    assert!(!app.history_reword.open);
}

#[test]
fn commit_action_keys_are_suppressed_when_text_input_captured() {
    // keyboard_shortcut returns None for single-char keys when status is Captured
    let keys = [
        ("r", Code::KeyR),
        ("s", Code::KeyS),
        ("f", Code::KeyF),
        ("e", Code::KeyE),
        ("d", Code::KeyD),
        ("t", Code::KeyT),
        ("y", Code::KeyY),
    ];

    for (ch, code) in keys {
        let result = keyboard_shortcut(
            Key::Character(ch.into()),
            Physical::Code(code),
            Modifiers::default(),
            event::Status::Captured,
        );
        assert!(
            result.is_none(),
            "key '{ch}' should be suppressed when Captured"
        );
    }
}
