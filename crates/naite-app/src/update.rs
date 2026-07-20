use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::widget::{scrollable, text_input};
use iced::Task;
use naite_core::{
    CommitSummary, RefKind, RefSummary, StashSummary, WorktreeDiffKind, WorktreeDiffTarget,
    WorktreeStatusDetail,
};

use crate::features::repo_open;
use crate::message::{KeyAction, Message, TabsMessage};
use crate::persistence::{self, OpenTabsSnapshot};
use crate::state::{
    BranchCreateState, CommitFormState, FileInsightState, HistoryRewordState, OpResult, OpSeverity,
    ReleasePrepPhase, RepositoryState, SidebarClickState, StashBranchState, StashCreateState,
    TagCreateState, TransientStatus,
};
use crate::tasks;
use crate::theme::OVERLAY_TRIGGER_SECS;
use crate::widgets::{Toast, ROW_HEIGHT as COMMIT_ROW_HEIGHT};
use crate::App;

const TRANSIENT_STATUS_DURATION: Duration = Duration::from_secs(3);
const SIDEBAR_DOUBLE_CLICK_DURATION: Duration = Duration::from_millis(300);
const TAB_REFRESH_STALE_AFTER: Duration = Duration::from_secs(30);
/// Pixels of cursor motion required after a mousedown on a rebase row before
/// it is reinterpreted as a drag (rather than a plain click-to-select).
const DRAG_THRESHOLD: f32 = 5.0;

impl App {
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NoOp => Task::none(),
            Message::Catalog(message) => self.update_catalog(message),
            Message::RepoOpen(message) => self.update_repo_open(message),
            Message::Checkout(message) => self.update_checkout(message),
            Message::Fetch(message) => self.update_fetch(message),
            Message::Pull(message) => self.update_pull(message),
            Message::PullRequest(message) => self.update_pull_request(message),
            Message::GitHubIssue(message) => self.update_github_issue(message),
            Message::Push(message) => self.update_push(message),
            Message::Rebase(message) => self.update_rebase(message),
            Message::ReleasePrep(message) => self.update_release_prep(message),
            Message::Stage(message) => self.update_stage(message),
            Message::Discard(message) => self.update_discard(message),
            Message::Commit(message) => self.update_commit(message),
            Message::History(message) => self.update_history(message),
            Message::BranchCreate(message) => self.update_branch_create(message),
            Message::BranchManage(message) => self.update_branch_manage(message),
            Message::Tag(message) => self.update_tag(message),
            Message::FileInspect(message) => self.update_file_inspect(message),
            Message::Stash(message) => self.update_stash(message),
            Message::CherryPick(message) => self.update_cherry_pick(message),
            Message::Revert(message) => self.update_revert(message),
            Message::Reset(message) => self.update_reset(message),
            Message::Worktree(message) => self.update_worktree(message),
            Message::Workspace(message) => self.update_workspace(message),
            Message::Terminal(message) => self.update_terminal(message),
            Message::CommandPalette(message) => self.update_command_palette(message),
            Message::Tabs(message) => self.update_tabs(message),
            Message::WindowFocused => {
                Task::batch([self.start_auto_fetch(), self.refresh_worktree_status()])
            }
            Message::WorktreeStatusDetailLoaded(result) => {
                self.operation.loading = false;
                match result {
                    Ok(status_detail) => self.apply_refreshed_status_detail(status_detail),
                    Err(msg) => {
                        self.toasts.push(Toast::failure(msg.as_str()));
                        self.operation.error = Some(msg);
                        Task::none()
                    }
                }
            }
            Message::WipSelected => self.select_wip(),
            Message::WipStatusPathSelected(target) => self.select_wip_file(target),
            Message::WipDiffLoaded { target, result } => {
                if self.operation.pending_wip_diff_target.as_ref() != Some(&target)
                    || self.selection.selected_wip_file.as_ref() != Some(&target)
                {
                    return Task::none();
                }

                self.operation.pending_wip_diff_target = None;
                self.operation.diff_loading = false;
                match result {
                    Ok((diff, hl)) => {
                        self.selection.selected_file = (!diff.files.is_empty()).then_some(0);
                        self.operation.current_diff = Some(diff);
                        self.operation.current_diff_highlight = Some(hl);
                        self.operation.diff_error = None;
                        self.reset_selected_hunk();
                    }
                    Err(msg) => {
                        self.selection.selected_file = None;
                        self.selection.selected_hunk = None;
                        self.operation.current_diff = None;
                        self.operation.current_diff_highlight = None;
                        self.operation.diff_error = Some(msg);
                    }
                }
                Task::none()
            }
            Message::StashSelected(stash) => self.select_stash(stash),
            Message::StashDiffLoaded { selector, result } => {
                if self.operation.pending_stash_diff_selector.as_deref() != Some(selector.as_str())
                    || self
                        .selection
                        .selected_stash
                        .as_ref()
                        .map(|stash| stash.selector.as_str())
                        != Some(selector.as_str())
                {
                    return Task::none();
                }

                self.operation.pending_stash_diff_selector = None;
                self.operation.diff_loading = false;
                match result {
                    Ok((diff, hl)) => {
                        self.selection.selected_file = (!diff.files.is_empty()).then_some(0);
                        self.operation.current_diff = Some(diff);
                        self.operation.current_diff_highlight = Some(hl);
                        self.operation.diff_error = None;
                        self.reset_selected_hunk();
                    }
                    Err(msg) => {
                        self.selection.selected_file = None;
                        self.selection.selected_hunk = None;
                        self.operation.current_diff = None;
                        self.operation.current_diff_highlight = None;
                        self.operation.diff_error = Some(msg);
                    }
                }
                Task::none()
            }
            Message::CommitSelected(i) => self.select_commit_index(i),
            Message::CommitListScrolled(viewport) => {
                self.commit_list_scroll_y = viewport.absolute_offset().y;
                self.commit_list_viewport_height = viewport.bounds().height;
                self.load_more_commits_if_near_end()
            }
            Message::DiffLoaded { commit_id, result } => {
                if self.operation.pending_diff_commit_id.as_deref() != Some(commit_id.as_str())
                    || self.selection.selected_commit_id.as_deref() != Some(commit_id.as_str())
                {
                    return Task::none();
                }

                self.operation.pending_diff_commit_id = None;
                self.operation.diff_loading = false;
                match result {
                    Ok((diff, hl)) => {
                        self.selection.selected_file = (!diff.files.is_empty()).then_some(0);
                        self.operation.current_diff = Some(diff);
                        self.operation.current_diff_highlight = Some(hl);
                        self.operation.diff_error = None;
                        self.reset_selected_hunk();
                    }
                    Err(msg) => {
                        self.operation.current_diff = None;
                        self.operation.current_diff_highlight = None;
                        self.selection.selected_file = None;
                        self.selection.selected_hunk = None;
                        self.operation.diff_error = Some(msg);
                    }
                }
                Task::none()
            }
            Message::DetailFileSelected(i) => {
                self.selection.selected_file = Some(i);
                self.reset_selected_hunk();
                Task::none()
            }
            Message::DetailHunkSelected(i) => {
                self.select_hunk(i);
                Task::none()
            }
            Message::DetailNextHunk => self.select_relative_hunk(1),
            Message::DetailPreviousHunk => self.select_relative_hunk(-1),
            Message::DiffViewModeChanged(mode) => {
                self.selection.diff_view_mode = mode;
                self.clamp_selected_hunk();
                Task::none()
            }
            Message::Keyboard(action) => self.handle_key_action(action),
            Message::SearchChanged(query) => {
                self.search_query = query;
                self.refresh_graph_layout();
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window_width = size.width;
                self.window_height = size.height;
                self.selection.context_menu = None;
                self.resize_active_terminal_to_window();
                Task::none()
            }
            Message::PaneResized(event) => {
                // Clamp before applying so pane_grid, the persisted preferences,
                // and the terminal-panel overlay all read the same value — the
                // terminal panel position is derived from the clamped ratio,
                // and would otherwise drift away from the sidebar/list edge
                // when the user drags past the clamp.
                let clamped = if event.ratio < 0.45 {
                    let r = event.ratio.clamp(0.14, 0.36);
                    self.preferences.sidebar_ratio = r;
                    r
                } else {
                    let r = event.ratio.clamp(0.50, 0.78);
                    self.preferences.detail_ratio = r;
                    r
                };
                self.panes.resize(event.split, clamped);
                self.selection.context_menu = None;
                self.save_preferences()
            }
            Message::ToggleDisplayOptions => {
                self.preferences.display_options_open = !self.preferences.display_options_open;
                if self.preferences.display_options_open {
                    self.preferences.shortcuts_open = false;
                }
                Task::none()
            }
            Message::ToggleShortcutOverlay => {
                self.preferences.shortcuts_open = !self.preferences.shortcuts_open;
                if self.preferences.shortcuts_open {
                    self.preferences.display_options_open = false;
                }
                Task::none()
            }
            Message::ThemePreferenceChanged(theme) => {
                self.preferences.theme = theme;
                self.save_preferences()
            }
            Message::DensityPreferenceChanged(density) => {
                self.preferences.density = density;
                self.save_preferences()
            }
            Message::DisplayCommitAuthorToggled => {
                self.preferences.display.show_commit_author =
                    !self.preferences.display.show_commit_author;
                self.save_preferences()
            }
            Message::DisplayFileInspectionToggled => {
                self.preferences.display.show_file_inspection =
                    !self.preferences.display.show_file_inspection;
                self.save_preferences()
            }
            Message::DisplayPrMetadataToggled => {
                self.preferences.display.show_pr_metadata =
                    !self.preferences.display.show_pr_metadata;
                self.save_preferences()
            }
            Message::DisplayWorkspaceDetailsToggled => {
                self.preferences.display.show_workspace_details =
                    !self.preferences.display.show_workspace_details;
                self.save_preferences()
            }
            Message::PreferencesSaved(Ok(())) => Task::none(),
            Message::PreferencesSaved(Err(msg)) => {
                self.set_transient_status(format!("Preferences save failed: {msg}"));
                Task::none()
            }
            Message::CursorMoved(position) => {
                self.selection.cursor_position = Some(position);
                if let Some(session) = self.rebase.as_mut() {
                    if let Some(drag) = session.drag.as_mut() {
                        let dx = position.x - drag.press_origin.x;
                        let dy = position.y - drag.press_origin.y;
                        if !drag.started && (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD {
                            drag.started = true;
                        }
                        if drag.started && !session.plan.is_empty() {
                            let last = session.plan.len() as isize - 1;
                            let delta_rows = (dy / COMMIT_ROW_HEIGHT).round() as isize;
                            let target =
                                (drag.source_index as isize + delta_rows).clamp(0, last) as usize;
                            drag.hover_index = target;
                        }
                    }
                }
                Task::none()
            }
            Message::ContextMenuOpened(kind) => {
                let position = self
                    .selection
                    .cursor_position
                    .unwrap_or(iced::Point::new(0.0, 0.0));
                self.selection.context_menu =
                    Some(crate::state::ContextMenuState { kind, position });
                self.selection.last_sidebar_click = None;
                Task::none()
            }
            Message::ContextMenuClosed => {
                self.selection.context_menu = None;
                self.selection.last_sidebar_click = None;
                Task::none()
            }
            Message::SidebarRefPressed(ref_summary) => self.handle_sidebar_ref_pressed(ref_summary),
            Message::SidebarRefHovered(ref_summary) => {
                self.sidebar.hover_ref(&ref_summary);
                Task::none()
            }
            Message::SidebarRefUnhovered(ref_summary) => {
                self.sidebar.clear_hovered_ref(&ref_summary);
                Task::none()
            }
            Message::SidebarSectionToggled(section) => {
                self.sidebar.toggle(section);
                Task::none()
            }
            Message::SidebarTreeFolderToggled { section, path } => {
                self.sidebar.toggle_tree_folder(section, path);
                Task::none()
            }
            Message::AutoFetchTick => self.start_auto_fetch(),
            Message::StatusBarTick => Task::none(),
            Message::TransientStatusTick => {
                if self
                    .operation
                    .transient_status
                    .as_ref()
                    .is_some_and(|status| Instant::now() >= status.expires_at)
                {
                    self.operation.transient_status = None;
                }
                self.status_animation_frame = self.status_animation_frame.wrapping_add(1);
                // Reuse this 250ms tick for toast TTL bookkeeping so the
                // subscription list stays small. Failure toasts are filtered
                // out by `is_expired` because they never auto-dismiss.
                if !self.toasts.is_empty() {
                    let now = Instant::now();
                    self.toasts.retain(|toast| !toast.is_expired(now));
                }
                // Reuse the same tick for overlay visibility bookkeeping.
                // AutoFetch stays in the status bar and ReleasePrep uses its
                // dedicated progress modal. Foreground operations wait for
                // OVERLAY_TRIGGER_SECS so fast operations never flash the card.
                self.overlay_visible = self
                    .operation_tracker
                    .should_show_overlay(OVERLAY_TRIGGER_SECS);
                Task::none()
            }
            Message::ToastDismissed { index } => {
                if index < self.toasts.len() {
                    self.toasts.remove(index);
                }
                Task::none()
            }
            Message::Operation(event) => match event {
                crate::message::OperationEvent::Started { id, kind, label } => {
                    let _ = self.operation_tracker.start_with_id(id, kind, label);
                    Task::none()
                }
                crate::message::OperationEvent::StepProgressed {
                    id,
                    label,
                    current,
                    total,
                } => {
                    let _ = self
                        .operation_tracker
                        .update_step(id, label, current, total);
                    Task::none()
                }
                crate::message::OperationEvent::Completed {
                    id,
                    result,
                    severity,
                } => {
                    if let OpResult::Failed(ref msg) = result {
                        match severity {
                            OpSeverity::Recoverable => {
                                self.toasts.push(Toast::failure(msg.as_str()));
                            }
                            OpSeverity::Fatal => {
                                self.operation.fatal_error = Some(msg.clone());
                            }
                        }
                    }
                    let _ = self.operation_tracker.complete(id, result, severity);
                    Task::none()
                }
                crate::message::OperationEvent::Cancelled { id } => {
                    let _ = self.operation_tracker.cancel(id);
                    Task::none()
                }
                crate::message::OperationEvent::Dismissed { id } => {
                    // Errors from the tracker (stale ids, double-dismiss)
                    // are intentionally swallowed: the UI event is the
                    // source of truth and a stale dismiss is harmless.
                    let _ = self.operation_tracker.dismiss(id);
                    Task::none()
                }
            },
            Message::ReleasePrepTick => {
                if self.release_prep.phase == crate::state::ReleasePrepPhase::Idle {
                    return Task::none();
                }
                let release_prep_loading = matches!(
                    self.release_prep.phase,
                    crate::state::ReleasePrepPhase::Preparing
                        | crate::state::ReleasePrepPhase::RunningAction
                );
                if release_prep_loading
                    || self.release_prep.animation_frame
                        < crate::state::RELEASE_PREP_MODAL_ANIMATION_FRAMES
                {
                    self.release_prep.animation_frame =
                        self.release_prep.animation_frame.wrapping_add(1);
                }
                Task::none()
            }
            Message::CopyText(text) => iced::clipboard::write(text),
            Message::ClearError => {
                self.operation.error = None;
                self.operation.fatal_error = None;
                self.operation.transient_status = None;
                Task::none()
            }
            Message::AvatarFetched { url, bytes } => {
                self.avatars.in_flight.remove(&url);
                match bytes {
                    Ok(data) => {
                        let handle = circular_avatar_handle(&data)
                            .unwrap_or_else(|| iced::widget::image::Handle::from_bytes(data));
                        self.avatars.handles.insert(url, handle);
                    }
                    Err(err) => {
                        if is_permanent_avatar_fetch_failure(&err) {
                            self.avatars.failed.insert(url);
                        }
                    }
                }
                Task::none()
            }
        }
    }

    /// Kick off an avatar fetch for `url` if it hasn't been cached,
    /// permanently failed, or already requested. Returns `Task::none()` when
    /// no work is needed so callers can compose it with other tasks freely.
    pub(crate) fn maybe_fetch_avatar(&mut self, url: Option<&str>) -> Task<Message> {
        let Some(url) = url else {
            return Task::none();
        };
        let url = url.trim();
        if url.is_empty() || !self.avatars.needs_fetch(url) {
            return Task::none();
        }
        let url = url.to_string();
        self.avatars.in_flight.insert(url.clone());
        Task::perform(
            crate::features::pull_request::task::fetch_avatar(url),
            |(url, bytes)| Message::AvatarFetched { url, bytes },
        )
    }

    pub(crate) fn prefetch_commit_avatars(&mut self) -> Task<Message> {
        let urls: Vec<String> = self
            .repo
            .commits
            .iter()
            .filter_map(|commit| commit.author_avatar_url.clone())
            .collect();

        Task::batch(
            urls.iter()
                .map(|url| self.maybe_fetch_avatar(Some(url.as_str()))),
        )
    }

    pub(crate) fn preserve_known_commit_avatar_urls(
        &self,
        path: &Path,
        commits: &mut [CommitSummary],
    ) {
        let known = self.known_author_avatar_urls_for_path(path);
        if known.is_empty() {
            return;
        }

        for commit in commits {
            if let Some(key) = Self::commit_author_avatar_key(commit) {
                if let Some(url) = known.get(&key) {
                    commit.author_avatar_url = Some(url.clone());
                }
            }
        }
    }

    pub(crate) fn known_author_avatar_urls_for_path(&self, path: &Path) -> HashMap<String, String> {
        let mut known = persistence::load_avatar_urls().unwrap_or_default();
        let source = if self.repo.path.as_deref() == Some(path) {
            Some(&self.repo)
        } else {
            self.tabs.cache.get(path)
        };

        for commit in source.into_iter().flat_map(|repo| repo.commits.iter()) {
            if let (Some(key), Some(url)) = (
                Self::commit_author_avatar_key(commit),
                commit.author_avatar_url.as_ref(),
            ) {
                known.insert(key, url.clone());
            }
        }
        known
    }

    pub(crate) fn commit_author_avatar_key(commit: &CommitSummary) -> Option<String> {
        Self::author_avatar_key(&commit.author_email, &commit.author_name)
    }

    /// Cache key for the persisted author→avatar-URL map, shared by every
    /// surface that resolves avatars (commit list, rebase plan).
    pub(crate) fn author_avatar_key(author_email: &str, author_name: &str) -> Option<String> {
        let email = author_email.trim();
        if !email.is_empty() {
            return Some(format!("email:{}", email.to_ascii_lowercase()));
        }

        let name = author_name.trim();
        (!name.is_empty()).then(|| format!("name:{}", name.to_ascii_lowercase()))
    }

    pub(crate) fn save_preferences(&self) -> Task<Message> {
        Task::perform(
            persistence::save_preferences_task(self.preferences.clone()),
            Message::PreferencesSaved,
        )
    }

    pub(crate) fn clear_repo_scoped_state(&mut self) {
        self.selection.selected = None;
        self.selection.selected_commit_id = None;
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_stash = None;
        self.selection.selected_worktree = None;
        self.selection.selected_pull_request = None;
        self.selection.selected_github_issue = None;
        self.pull_requests.loading = false;
        self.pull_requests.error = None;
        self.github_issues.loading = false;
        self.github_issues.error = None;
        self.pull_requests.create.open = false;
        self.pull_requests.checkout_worktree.open = false;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_loading = false;
        self.operation.diff_error = None;
        self.operation.pending_diff_commit_id = None;
        self.operation.pending_wip_diff_target = None;
        self.operation.pending_stash_diff_selector = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;
        self.search_query.clear();
        self.commit_form = CommitFormState::default();
        self.branch_create = BranchCreateState::default();
        self.branch_manage_rename = Default::default();
        self.stash_create = StashCreateState::default();
        self.stash_branch = StashBranchState::default();
        self.history_reword = HistoryRewordState::default();
        self.tag_create = TagCreateState::default();
        self.worktree_create = Default::default();
        self.file_insight = FileInsightState::default();
        self.rebase = None;
        self.selection.context_menu = None;
        self.selection.last_sidebar_click = None;
        self.selection.checkout_confirmation = None;
        self.selection.force_sync_confirmation = None;
        self.selection.force_push_confirmation = None;
        self.selection.branch_delete_confirmation = None;
        self.selection.discard_confirmation = None;
        self.selection.stash_confirmation = None;
        self.selection.history_confirmation = None;
        self.selection.rebase_confirmation = None;
        self.selection.reset_confirmation = None;
        self.selection.tag_delete_confirmation = None;
        self.selection.undo_confirmation = None;
        self.selection.worktree_remove_confirmation = None;
    }

    pub(crate) fn refresh_graph_layout(&mut self) {
        self.repo.graph_layout = naite_core::compute_graph_layout(&self.visible_commits());
    }

    pub(crate) fn load_more_commits_if_near_end(&mut self) -> Task<Message> {
        if self.repo.commit_page_cursor.is_none()
            || self.repo.commits_loading_more
            || !self.search_query.trim().is_empty()
            || self.commit_list_viewport_height <= 0.0
        {
            return Task::none();
        }

        let row_count = self.repo.commits.len() + usize::from(self.repo.status_detail.is_dirty());
        let content_height = COMMIT_ROW_HEIGHT + row_count as f32 * COMMIT_ROW_HEIGHT;
        let viewport_bottom = self.commit_list_scroll_y + self.commit_list_viewport_height;
        if viewport_bottom + COMMIT_ROW_HEIGHT * 8.0 >= content_height {
            self.load_more_commits()
        } else {
            Task::none()
        }
    }

    pub(crate) fn select_commit_index(&mut self, index: usize) -> Task<Message> {
        self.select_commit_index_inner(index, false)
    }

    pub(crate) fn select_commit_index_with_scroll(&mut self, index: usize) -> Task<Message> {
        self.select_commit_index_inner(index, true)
    }

    fn select_commit_index_inner(&mut self, index: usize, auto_scroll: bool) -> Task<Message> {
        let Some(commit) = self.repo.commits.get(index) else {
            return Task::none();
        };
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        let commit_id = commit.id.clone();
        self.selection.selected = Some(index);
        self.selection.selected_commit_id = Some(commit_id.clone());
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_stash = None;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_error = None;
        self.operation.diff_loading = true;
        self.operation.pending_diff_commit_id = Some(commit_id.clone());
        self.operation.pending_wip_diff_target = None;
        self.operation.pending_stash_diff_selector = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;

        let scroll_task = if auto_scroll {
            self.scroll_commit_into_view_task(index)
        } else {
            None
        };

        let diff_task = Task::perform(tasks::load_diff(path, commit_id), |(commit_id, result)| {
            Message::DiffLoaded { commit_id, result }
        });

        match scroll_task {
            Some(scroll_task) => Task::batch([diff_task, scroll_task]),
            None => diff_task,
        }
    }

    /// Compute a scroll-to task that keeps `index` inside the visible viewport
    /// with minimal motion. Returns `None` when the row is already fully
    /// visible (no scroll needed).
    fn scroll_commit_into_view_task(&mut self, index: usize) -> Option<Task<Message>> {
        let visible_indices = self.visible_commit_indices();
        let visible_pos = visible_indices.iter().position(|i| *i == index)?;
        let wip_offset = if self.repo.status_detail.is_dirty() {
            COMMIT_ROW_HEIGHT
        } else {
            0.0
        };
        let target_top = wip_offset + visible_pos as f32 * COMMIT_ROW_HEIGHT;
        let target_bottom = target_top + COMMIT_ROW_HEIGHT;
        let viewport_top = self.commit_list_scroll_y;
        let viewport_height = self.commit_list_viewport_height;

        let new_offset_y = if viewport_height <= 0.0 {
            // No viewport info yet (no on_scroll has fired). Fall back to the
            // legacy behavior of anchoring the row at the top so the first
            // keyboard nav still works before any scroll event arrives.
            target_top
        } else {
            let viewport_bottom = viewport_top + viewport_height;
            if target_top < viewport_top {
                target_top
            } else if target_bottom > viewport_bottom {
                target_bottom - viewport_height
            } else {
                return None;
            }
        };

        self.commit_list_scroll_y = new_offset_y;
        Some(scrollable::scroll_to(
            self.commit_list_id.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: new_offset_y,
            },
        ))
    }

    pub(crate) fn handle_key_action(&mut self, action: KeyAction) -> Task<Message> {
        match action {
            KeyAction::NextCommit if self.command_palette.open => {
                self.move_command_palette_selection(1)
            }
            KeyAction::PreviousCommit if self.command_palette.open => {
                self.move_command_palette_selection(-1)
            }
            KeyAction::NextCommit => self.select_relative_commit(1),
            KeyAction::PreviousCommit => self.select_relative_commit(-1),
            KeyAction::OpenRepository => {
                self.update(crate::features::repo_open::Message::OpenClicked.into())
            }
            KeyAction::OpenTerminal => {
                self.update(crate::features::terminal::Message::OpenRequested.into())
            }
            KeyAction::FocusSearch => text_input::focus(self.search_input_id.clone()),
            KeyAction::OpenCommandPalette => self.open_command_palette(),
            KeyAction::ToggleShortcutOverlay => {
                self.preferences.shortcuts_open = !self.preferences.shortcuts_open;
                if self.preferences.shortcuts_open {
                    self.preferences.display_options_open = false;
                }
                Task::none()
            }
            KeyAction::CommandPaletteNext => self.move_command_palette_selection(1),
            KeyAction::CommandPalettePrevious => self.move_command_palette_selection(-1),
            KeyAction::CommandPaletteRun => self.run_selected_command_palette_command(),
            KeyAction::ReleasePromotion => {
                self.command_palette.open = false;
                self.update(crate::features::release_prep::Message::Requested.into())
            }
            KeyAction::CreateAndPushTag => {
                self.command_palette.open = false;
                self.update(
                    crate::features::tag::Message::CreateAndPushRequested(self.selected_commit())
                        .into(),
                )
            }
            KeyAction::NextHunk => self.select_relative_hunk(1),
            KeyAction::PreviousHunk => self.select_relative_hunk(-1),
            KeyAction::Push if self.command_palette.open => Task::none(),
            KeyAction::Push => self.update(
                crate::features::push::Message::Requested(crate::features::push::PushMode::Normal)
                    .into(),
            ),
            KeyAction::RewordSelectedCommit => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::from(
                    crate::features::history::Message::RewordRequested(commit),
                ))
            }
            KeyAction::SquashSelectedCommit => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::from(crate::features::history::Message::Requested(
                    crate::features::history::Operation::Squash(commit),
                )))
            }
            KeyAction::FixupSelectedCommit => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::from(crate::features::history::Message::Requested(
                    crate::features::history::Operation::Fixup(commit),
                )))
            }
            KeyAction::EditSelectedCommit => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::from(crate::features::history::Message::Requested(
                    crate::features::history::Operation::Edit(commit),
                )))
            }
            KeyAction::DropSelectedCommit => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::from(crate::features::history::Message::Requested(
                    crate::features::history::Operation::Drop(commit),
                )))
            }
            KeyAction::TagSelectedCommit => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::from(
                    crate::features::tag::Message::CreateRequested(Some(commit)),
                ))
            }
            KeyAction::CopySelectedCommitHash => {
                if self.is_commit_action_blocked() {
                    return Task::none();
                }
                let Some(commit) = self.selected_commit() else {
                    return Task::none();
                };
                self.update(Message::CopyText(commit.id))
            }
            KeyAction::Enter => {
                if self.command_palette.open {
                    return self.run_selected_command_palette_command();
                }
                let visible = self.visible_commit_indices();
                if visible.len() == 1 {
                    self.select_commit_index_with_scroll(visible[0])
                } else {
                    Task::none()
                }
            }
            KeyAction::Escape => {
                if self.selection.context_menu.is_some() {
                    self.selection.context_menu = None;
                    return Task::none();
                }
                if self.release_prep.phase != ReleasePrepPhase::Idle {
                    if self.release_prep.auto_running {
                        return Task::none();
                    }
                    let closing_actions = self.release_prep.phase == ReleasePrepPhase::Actions;
                    self.release_prep.phase = ReleasePrepPhase::Idle;
                    if closing_actions {
                        // Keep script edits made in the actions modal.
                        return self.persist_active_release_profile_if_changed();
                    }
                    return Task::none();
                }
                if self.preferences.shortcuts_open {
                    self.preferences.shortcuts_open = false;
                    return Task::none();
                }
                if self.preferences.display_options_open {
                    self.preferences.display_options_open = false;
                    return Task::none();
                }
                if self.selection.checkout_confirmation.is_some() {
                    self.selection.checkout_confirmation = None;
                    return Task::none();
                }
                if self.selection.reset_confirmation.is_some() {
                    self.selection.reset_confirmation = None;
                    return Task::none();
                }
                if self.selection.discard_confirmation.is_some() {
                    self.selection.discard_confirmation = None;
                    return Task::none();
                }
                if self.selection.branch_delete_confirmation.is_some() {
                    self.selection.branch_delete_confirmation = None;
                    return Task::none();
                }
                if self.selection.force_sync_confirmation.is_some() {
                    self.selection.force_sync_confirmation = None;
                    return Task::none();
                }
                if self.selection.force_push_confirmation.is_some() {
                    self.selection.force_push_confirmation = None;
                    return Task::none();
                }
                if self.selection.stash_confirmation.is_some() {
                    self.selection.stash_confirmation = None;
                    return Task::none();
                }
                if self.selection.history_confirmation.is_some() {
                    self.selection.history_confirmation = None;
                    return Task::none();
                }
                if self.selection.rebase_confirmation.is_some() {
                    self.selection.rebase_confirmation = None;
                    return Task::none();
                }
                if self.selection.tag_delete_confirmation.is_some() {
                    self.selection.tag_delete_confirmation = None;
                    return Task::none();
                }
                if self.selection.undo_confirmation.is_some() {
                    self.selection.undo_confirmation = None;
                    return Task::none();
                }
                if self.selection.worktree_remove_confirmation.is_some() {
                    self.selection.worktree_remove_confirmation = None;
                    return Task::none();
                }
                if self.command_palette.open {
                    self.command_palette.open = false;
                    return Task::none();
                }
                if self.manager.new_repo_menu_open {
                    self.manager.new_repo_menu_open = false;
                    return Task::none();
                }
                if self.manager.clone_open {
                    self.manager.clone_open = false;
                    return Task::none();
                }
                if self.worktree_create.open {
                    self.worktree_create.open = false;
                    return Task::none();
                }
                if self.pull_requests.create.open {
                    self.pull_requests.create.open = false;
                    return Task::none();
                }
                if self.pull_requests.checkout_worktree.open {
                    self.pull_requests.checkout_worktree.open = false;
                    return Task::none();
                }
                if self.terminal.open && !self.terminal.captures_keyboard() {
                    self.terminal.open = false;
                    return Task::none();
                }
                if self.history_reword.open {
                    self.history_reword.open = false;
                    return Task::none();
                }
                if self.tag_create.open {
                    self.tag_create.open = false;
                    return Task::none();
                }
                if self.stash_create.open {
                    self.stash_create.open = false;
                    return Task::none();
                }
                if self.stash_branch.open {
                    self.stash_branch.open = false;
                    return Task::none();
                }
                if self.branch_create.open {
                    self.branch_create.open = false;
                    return Task::none();
                }
                if self.branch_manage_rename.open {
                    self.branch_manage_rename.open = false;
                    return Task::none();
                }
                if self.workspace.dashboard_open {
                    self.workspace.dashboard_open = false;
                    return Task::none();
                }
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.refresh_graph_layout();
                } else {
                    self.selection.selected = None;
                    self.selection.selected_commit_id = None;
                    self.operation.current_diff = None;
                    self.operation.current_diff_highlight = None;
                    self.operation.diff_error = None;
                    self.operation.diff_loading = false;
                    self.operation.pending_diff_commit_id = None;
                    self.operation.pending_wip_diff_target = None;
                    self.operation.pending_stash_diff_selector = None;
                    self.selection.selected_file = None;
                    self.selection.selected_hunk = None;
                    self.selection.selected_wip_file = None;
                    self.selection.selected_stash = None;
                    self.selection.selected_worktree = None;
                    self.selection.selected_pull_request = None;
                    self.selection.selected_github_issue = None;
                    self.selection.selected_wip = false;
                }
                self.selection.context_menu = None;
                self.selection.last_sidebar_click = None;
                self.selection.checkout_confirmation = None;
                self.selection.force_sync_confirmation = None;
                self.selection.force_push_confirmation = None;
                self.selection.branch_delete_confirmation = None;
                self.selection.discard_confirmation = None;
                self.selection.stash_confirmation = None;
                self.selection.history_confirmation = None;
                self.selection.rebase_confirmation = None;
                self.selection.reset_confirmation = None;
                self.selection.tag_delete_confirmation = None;
                self.selection.undo_confirmation = None;
                self.selection.worktree_remove_confirmation = None;
                Task::none()
            }
        }
    }

    fn is_commit_action_blocked(&self) -> bool {
        let s = &self.selection;
        s.history_confirmation.is_some()
            || s.rebase_confirmation.is_some()
            || s.checkout_confirmation.is_some()
            || s.force_sync_confirmation.is_some()
            || s.force_push_confirmation.is_some()
            || s.branch_delete_confirmation.is_some()
            || s.discard_confirmation.is_some()
            || s.stash_confirmation.is_some()
            || s.reset_confirmation.is_some()
            || s.tag_delete_confirmation.is_some()
            || s.undo_confirmation.is_some()
            || s.worktree_remove_confirmation.is_some()
            || s.context_menu.is_some()
    }

    pub(crate) fn select_relative_commit(&mut self, delta: isize) -> Task<Message> {
        let visible = self.visible_commit_indices();
        if visible.is_empty() {
            return Task::none();
        }

        let current_visible = self
            .selected_index()
            .and_then(|selected| visible.iter().position(|i| *i == selected))
            .map(|i| i as isize)
            .unwrap_or(if self.selection.selected_wip { -1 } else { 0 });
        if self.repo.status_detail.is_dirty() && current_visible == 0 && delta < 0 {
            return self.select_wip();
        }
        let next_visible =
            (current_visible + delta).clamp(0, visible.len().saturating_sub(1) as isize);

        self.select_commit_index_with_scroll(visible[next_visible as usize])
    }

    fn handle_sidebar_ref_pressed(&mut self, ref_summary: RefSummary) -> Task<Message> {
        let now = Instant::now();
        let double_clicked = self
            .selection
            .last_sidebar_click
            .as_ref()
            .is_some_and(|last| {
                last.ref_summary.kind == ref_summary.kind
                    && last.ref_summary.full_name == ref_summary.full_name
                    && now.duration_since(last.at) <= SIDEBAR_DOUBLE_CLICK_DURATION
            });

        self.selection.last_sidebar_click = Some(SidebarClickState {
            ref_summary: ref_summary.clone(),
            at: now,
        });

        if double_clicked && is_checkoutable_ref(&ref_summary) {
            self.selection.last_sidebar_click = None;
            return self.update(crate::features::checkout::Message::Requested(ref_summary).into());
        }

        Task::none()
    }

    pub(crate) fn select_wip(&mut self) -> Task<Message> {
        self.selection.selected = None;
        self.selection.selected_commit_id = None;
        self.selection.selected_wip = true;
        self.selection.selected_stash = None;
        self.selection.selected_worktree = None;
        self.selection.selected_pull_request = None;
        self.selection.selected_github_issue = None;
        self.operation.pending_diff_commit_id = None;
        self.operation.pending_stash_diff_selector = None;
        let target = first_worktree_diff_target(&self.repo.status_detail);
        let diff_task = match target {
            Some(target) => self.select_wip_file(target),
            None => {
                self.selection.selected_wip_file = None;
                self.selection.selected_file = None;
                self.selection.selected_hunk = None;
                self.operation.current_diff = None;
                self.operation.current_diff_highlight = None;
                self.operation.diff_error = None;
                self.operation.diff_loading = false;
                self.operation.pending_wip_diff_target = None;
                Task::none()
            }
        };
        let scroll_task = scrollable::scroll_to(
            self.commit_list_id.clone(),
            scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
        );
        Task::batch([diff_task, scroll_task])
    }

    pub(crate) fn select_wip_after_status_update(
        &mut self,
        previous_target: Option<&WorktreeDiffTarget>,
    ) -> Task<Message> {
        let target = previous_target
            .and_then(|target| matching_worktree_diff_target(&self.repo.status_detail, target))
            .or_else(|| first_worktree_diff_target(&self.repo.status_detail));

        match target {
            Some(target) => self.select_wip_file(target),
            None => {
                self.selection.selected_wip_file = None;
                self.selection.selected_file = None;
                self.selection.selected_hunk = None;
                self.operation.current_diff = None;
                self.operation.current_diff_highlight = None;
                self.operation.diff_error = None;
                self.operation.diff_loading = false;
                self.operation.pending_wip_diff_target = None;
                Task::none()
            }
        }
    }

    pub(crate) fn select_wip_file(&mut self, target: WorktreeDiffTarget) -> Task<Message> {
        self.selection.selected = None;
        self.selection.selected_commit_id = None;
        self.selection.selected_wip = true;
        self.selection.selected_wip_file = Some(target.clone());
        self.selection.selected_stash = None;
        self.selection.selected_worktree = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_error = None;
        self.operation.diff_loading = true;
        self.operation.pending_diff_commit_id = None;
        self.operation.pending_wip_diff_target = Some(target.clone());
        self.operation.pending_stash_diff_selector = None;

        let Some(path) = self.repo.path.clone() else {
            self.operation.diff_loading = false;
            self.operation.pending_wip_diff_target = None;
            return Task::none();
        };

        Task::perform(tasks::load_wip_diff(path, target), |(target, result)| {
            Message::WipDiffLoaded { target, result }
        })
    }

    pub(crate) fn select_stash(&mut self, stash: StashSummary) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };

        let selector = stash.selector.clone();
        self.selection.selected = None;
        self.selection.selected_commit_id = None;
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_stash = Some(stash);
        self.selection.selected_worktree = None;
        self.selection.selected_pull_request = None;
        self.selection.selected_github_issue = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_error = None;
        self.operation.diff_loading = true;
        self.operation.pending_diff_commit_id = None;
        self.operation.pending_wip_diff_target = None;
        self.operation.pending_stash_diff_selector = Some(selector.clone());

        Task::perform(
            crate::features::stash::task::load_diff(path, selector),
            |(selector, result)| Message::StashDiffLoaded { selector, result },
        )
    }

    pub(crate) fn refresh_worktree_status(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.loading = true;
        Task::perform(
            tasks::load_status_detail(path),
            Message::WorktreeStatusDetailLoaded,
        )
    }

    pub(crate) fn apply_refreshed_status_detail(
        &mut self,
        status_detail: WorktreeStatusDetail,
    ) -> Task<Message> {
        let previous_target = self.selection.selected_wip_file.clone();
        let was_wip_selected = self.selection.selected_wip;
        self.repo.status_detail = status_detail;

        if !self.repo.status_detail.is_dirty() {
            if was_wip_selected {
                self.selection.selected_wip = false;
                self.selection.selected_wip_file = None;
                self.selection.selected_file = None;
                self.selection.selected_hunk = None;
                self.operation.current_diff = None;
                self.operation.current_diff_highlight = None;
                self.operation.diff_error = None;
                self.operation.diff_loading = false;
                self.operation.pending_wip_diff_target = None;
                self.operation.pending_diff_commit_id = None;
                self.operation.pending_stash_diff_selector = None;
            }
            self.operation.error = None;
            return Task::none();
        }

        self.operation.error = None;
        if was_wip_selected {
            self.select_wip_after_status_update(previous_target.as_ref())
        } else {
            Task::none()
        }
    }

    pub(crate) fn set_transient_status(&mut self, message: String) {
        self.operation.transient_status = Some(TransientStatus {
            message,
            expires_at: Instant::now() + TRANSIENT_STATUS_DURATION,
        });
    }

    pub(crate) fn clear_dirty_selection(&mut self) {
        self.selection.selected_wip = false;
        self.selection.selected_wip_file = None;
        self.selection.selected_file = None;
        self.selection.selected_hunk = None;
        self.operation.current_diff = None;
        self.operation.current_diff_highlight = None;
        self.operation.diff_loading = false;
        self.operation.diff_error = None;
        self.operation.pending_wip_diff_target = None;
        self.operation.pending_diff_commit_id = None;
        self.operation.pending_stash_diff_selector = None;
    }

    pub(crate) fn selected_file_hunk_count(&self) -> usize {
        let Some(diff) = self.operation.current_diff.as_ref() else {
            return 0;
        };
        if diff.files.is_empty() {
            return 0;
        }

        let file_index = self
            .selection
            .selected_file
            .unwrap_or(0)
            .min(diff.files.len() - 1);

        diff.hunks_by_file
            .get(&diff.files[file_index].path)
            .map(Vec::len)
            .unwrap_or(0)
    }

    pub(crate) fn reset_selected_hunk(&mut self) {
        self.selection.selected_hunk = (self.selected_file_hunk_count() > 0).then_some(0);
    }

    pub(crate) fn clamp_selected_hunk(&mut self) {
        let count = self.selected_file_hunk_count();
        self.selection.selected_hunk = if count == 0 {
            None
        } else {
            Some(self.selection.selected_hunk.unwrap_or(0).min(count - 1))
        };
    }

    pub(crate) fn select_hunk(&mut self, index: usize) {
        let count = self.selected_file_hunk_count();
        self.selection.selected_hunk = if count == 0 {
            None
        } else {
            Some(index.min(count - 1))
        };
    }

    pub(crate) fn select_relative_hunk(&mut self, delta: isize) -> Task<Message> {
        let count = self.selected_file_hunk_count();
        if count == 0 {
            self.selection.selected_hunk = None;
            return Task::none();
        }

        let current = self.selection.selected_hunk.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, count.saturating_sub(1) as isize);
        self.selection.selected_hunk = Some(next as usize);
        Task::none()
    }

    pub(crate) fn update_tabs(&mut self, message: TabsMessage) -> Task<Message> {
        match message {
            TabsMessage::Activate(target) => {
                if self.tabs.active.as_ref() == Some(&target) {
                    if self.should_refresh_tab(&target) {
                        return self.spawn_tab_refresh(target);
                    }
                    return Task::none();
                }

                if self.tabs.cache.contains_key(&target) {
                    self.swap_active_tab(target.clone(), true);
                    self.clear_repo_scoped_state();
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.operation.error = None;
                    self.operation.loading = false;
                    self.refresh_graph_layout();
                    let terminal_task = if let Some(path) = self.repo.path.clone() {
                        self.ensure_repo_terminal_session(
                            path,
                            self.repo
                                .head_branch
                                .clone()
                                .unwrap_or_else(|| "Current repo".into()),
                        )
                    } else {
                        Task::none()
                    };

                    let refresh_task = if self.should_refresh_tab(&target) {
                        self.spawn_tab_refresh(target.clone())
                    } else {
                        Task::none()
                    };
                    let select_task = if self.repo.status_detail.is_dirty() {
                        self.select_wip()
                    } else {
                        Task::none()
                    };
                    let commit_avatar_task = self.prefetch_commit_avatars();
                    let provider_commit_avatar_task =
                        self.load_provider_commit_author_avatars(target);
                    Task::batch([
                        self.save_open_tabs(),
                        terminal_task,
                        refresh_task,
                        select_task,
                        commit_avatar_task,
                        provider_commit_avatar_task,
                    ])
                } else {
                    self.update(Message::from(repo_open::Message::OpenRecent(target)))
                }
            }
            TabsMessage::Close(target) => {
                let outcome = self.tabs.close(&target);
                if !outcome.was_active {
                    return self.save_open_tabs();
                }

                if let Some(new_active) = outcome.new_active {
                    if self.tabs.cache.contains_key(&new_active) {
                        self.swap_active_tab(new_active.clone(), false);
                        self.clear_repo_scoped_state();
                        self.undo_stack.clear();
                        self.redo_stack.clear();
                        self.operation.error = None;
                        self.operation.loading = false;
                        self.refresh_graph_layout();
                        let terminal_task = if let Some(path) = self.repo.path.clone() {
                            self.ensure_repo_terminal_session(
                                path,
                                self.repo
                                    .head_branch
                                    .clone()
                                    .unwrap_or_else(|| "Current repo".into()),
                            )
                        } else {
                            Task::none()
                        };

                        let refresh_task = if self.should_refresh_tab(&new_active) {
                            self.spawn_tab_refresh(new_active.clone())
                        } else {
                            Task::none()
                        };
                        let select_task = if self.repo.status_detail.is_dirty() {
                            self.select_wip()
                        } else {
                            Task::none()
                        };
                        let commit_avatar_task = self.prefetch_commit_avatars();
                        let provider_commit_avatar_task =
                            self.load_provider_commit_author_avatars(new_active);
                        Task::batch([
                            self.save_open_tabs(),
                            terminal_task,
                            refresh_task,
                            select_task,
                            commit_avatar_task,
                            provider_commit_avatar_task,
                        ])
                    } else {
                        self.repo = RepositoryState::default();
                        self.update(Message::from(repo_open::Message::OpenRecent(new_active)))
                    }
                } else {
                    self.repo = RepositoryState::default();
                    self.clear_repo_scoped_state();
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.operation.error = None;
                    self.operation.loading = false;
                    self.refresh_graph_layout();
                    self.save_open_tabs()
                }
            }
            TabsMessage::Restored(Ok(paths)) => {
                if paths.is_empty() {
                    return Task::none();
                }
                self.tabs.open = paths.clone();
                self.tabs.active = paths.first().cloned();
                if let Some(first) = paths.into_iter().next() {
                    self.update(Message::from(repo_open::Message::OpenRecent(first)))
                } else {
                    Task::none()
                }
            }
            TabsMessage::Restored(Err(msg)) => {
                self.toasts.push(Toast::failure(msg.as_str()));
                self.operation.error = Some(msg);
                Task::none()
            }
            TabsMessage::Saved(Ok(())) => Task::none(),
            TabsMessage::Saved(Err(msg)) => {
                self.set_transient_status(format!("Open tabs save failed: {msg}"));
                Task::none()
            }
            TabsMessage::RefreshDone { path, result } => {
                self.tabs.refreshing.remove(&path);
                self.tabs
                    .last_refreshed
                    .insert(path.clone(), Instant::now());

                match *result {
                    Ok((
                        loaded_path,
                        mut commits,
                        commit_page_cursor,
                        refs,
                        stashes,
                        worktrees,
                        head_branch,
                        status_detail,
                        sync_status,
                        operation_state,
                    )) => {
                        let is_active = self
                            .tabs
                            .active
                            .as_deref()
                            .map(|active| active == loaded_path.as_path())
                            .unwrap_or(false);
                        self.preserve_known_commit_avatar_urls(&loaded_path, &mut commits);
                        let provider_commit_avatar_task = self
                            .load_provider_commit_author_avatars_for_commits(
                                loaded_path.clone(),
                                &commits,
                            );

                        let target_state: Option<&mut RepositoryState> = if is_active {
                            Some(&mut self.repo)
                        } else {
                            self.tabs.cache.get_mut(&loaded_path)
                        };

                        if let Some(state) = target_state {
                            state.path = Some(loaded_path);
                            state.commits = commits;
                            state.commit_page_cursor = commit_page_cursor;
                            state.commits_loading_more = false;
                            state.refs = refs;
                            state.stashes = stashes;
                            state.worktrees = worktrees;
                            state.head_branch = head_branch;
                            state.sync_status = sync_status;
                            state.operation_state = operation_state;
                            state.status_detail = status_detail;
                        }

                        if is_active {
                            self.refresh_graph_layout();
                            return Task::batch([
                                self.prefetch_commit_avatars(),
                                provider_commit_avatar_task,
                            ]);
                        }
                        provider_commit_avatar_task
                    }
                    Err(msg) => {
                        self.set_transient_status(format!("Tab refresh failed: {msg}"));
                        Task::none()
                    }
                }
            }
        }
    }

    /// Move `target` into `self.repo`, optionally caching the outgoing active
    /// repo state. Used by both tab activation (cache_outgoing = true) and
    /// post-close fallback (cache_outgoing = false; closed tab is dropped).
    pub(crate) fn swap_active_tab(&mut self, target: PathBuf, cache_outgoing: bool) {
        let old_active = self.tabs.active.replace(target.clone());
        let new_state = self.tabs.cache.remove(&target).unwrap_or_default();
        let old_state = std::mem::replace(&mut self.repo, new_state);
        if cache_outgoing {
            if let Some(old_active) = old_active.filter(|p| p != &target) {
                self.tabs.cache.insert(old_active, old_state);
            }
        }
        self.tabs.open.retain(|p| p != &target);
        self.tabs.open.insert(0, target);
    }

    pub(crate) fn should_refresh_tab(&self, path: &Path) -> bool {
        !self.tabs.refreshing.contains(path)
            && self
                .tabs
                .last_refreshed
                .get(path)
                .map(|t| t.elapsed() >= TAB_REFRESH_STALE_AFTER)
                .unwrap_or(true)
    }

    pub(crate) fn spawn_tab_refresh(&mut self, path: PathBuf) -> Task<Message> {
        self.tabs.refreshing.insert(path.clone());
        let path_for_msg = path.clone();
        Task::perform(repo_open::task::load(path), move |result| {
            Message::from(TabsMessage::RefreshDone {
                path: path_for_msg.clone(),
                result: Box::new(result),
            })
        })
    }

    pub(crate) fn save_open_tabs(&self) -> Task<Message> {
        let snapshot = OpenTabsSnapshot {
            open: self.tabs.open.clone(),
        };
        Task::perform(save_open_tabs_task(snapshot), |result| {
            Message::from(TabsMessage::Saved(result))
        })
    }
}

fn is_permanent_avatar_fetch_failure(err: &str) -> bool {
    matches!(err.trim(), "HTTP 404" | "HTTP 410")
}

fn circular_avatar_handle(data: &[u8]) -> Option<iced::widget::image::Handle> {
    // Mask resolution. Sized to comfortably oversample the 16-logical-pixel
    // display target (32 physical pixels at 2× DPR) so the edge survives iced's
    // bilinear downscale.
    const SIZE: u32 = 128;
    // Sub-pixel samples per axis used to compute the alpha at the disc edge.
    // 4×4 = 16 samples per pixel gives a visibly smooth edge without burning
    // measurable CPU on a one-shot decode.
    const SUBSAMPLES: u32 = 4;

    let img = image::load_from_memory(data).ok()?;
    let rgba = if img.width() == SIZE && img.height() == SIZE {
        img.into_rgba8()
    } else {
        img.resize_to_fill(SIZE, SIZE, image::imageops::FilterType::Lanczos3)
            .into_rgba8()
    };
    let mut pixels = rgba.into_raw();

    let center = SIZE as f32 / 2.0;
    let radius = SIZE as f32 / 2.0;
    let radius_sq = radius * radius;
    let sub_step = 1.0 / SUBSAMPLES as f32;
    let sub_total = (SUBSAMPLES * SUBSAMPLES) as f32;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut hits = 0u32;
            for sy in 0..SUBSAMPLES {
                for sx in 0..SUBSAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) * sub_step;
                    let py = y as f32 + (sy as f32 + 0.5) * sub_step;
                    let dx = px - center;
                    let dy = py - center;
                    if dx * dx + dy * dy <= radius_sq {
                        hits += 1;
                    }
                }
            }
            let coverage = hits as f32 / sub_total;
            let alpha_index = ((y * SIZE + x) * 4 + 3) as usize;
            pixels[alpha_index] = (pixels[alpha_index] as f32 * coverage).round() as u8;
        }
    }

    Some(iced::widget::image::Handle::from_rgba(SIZE, SIZE, pixels))
}

async fn save_open_tabs_task(snapshot: OpenTabsSnapshot) -> Result<(), String> {
    tokio::task::spawn_blocking(move || persistence::save_open_tabs(&snapshot))
        .await
        .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn load_open_tabs_task() -> Result<Vec<PathBuf>, String> {
    tokio::task::spawn_blocking(persistence::load_open_tabs)
        .await
        .map_err(|e| format!("worker join error: {e}"))?
        .map(|snapshot| snapshot.open)
}

pub(crate) fn first_worktree_diff_target(
    status_detail: &WorktreeStatusDetail,
) -> Option<WorktreeDiffTarget> {
    status_detail
        .staged
        .first()
        .map(|entry| WorktreeDiffTarget {
            kind: WorktreeDiffKind::Staged,
            path: entry.path.clone(),
        })
        .or_else(|| {
            status_detail
                .unstaged
                .first()
                .map(|entry| WorktreeDiffTarget {
                    kind: WorktreeDiffKind::Unstaged,
                    path: entry.path.clone(),
                })
        })
        .or_else(|| {
            status_detail
                .conflicted
                .first()
                .map(|entry| WorktreeDiffTarget {
                    kind: WorktreeDiffKind::Conflict,
                    path: entry.path.clone(),
                })
        })
        .or_else(|| {
            status_detail
                .untracked
                .first()
                .map(|entry| WorktreeDiffTarget {
                    kind: WorktreeDiffKind::Untracked,
                    path: entry.path.clone(),
                })
        })
}

pub(crate) fn matching_worktree_diff_target(
    status_detail: &WorktreeStatusDetail,
    previous: &WorktreeDiffTarget,
) -> Option<WorktreeDiffTarget> {
    let exact_entries = match previous.kind {
        WorktreeDiffKind::Staged => status_detail.staged.as_slice(),
        WorktreeDiffKind::Unstaged => status_detail.unstaged.as_slice(),
        WorktreeDiffKind::Untracked => status_detail.untracked.as_slice(),
        WorktreeDiffKind::Conflict => status_detail.conflicted.as_slice(),
    };
    if exact_entries
        .iter()
        .any(|entry| entry.path == previous.path)
    {
        return Some(previous.clone());
    }

    [
        (WorktreeDiffKind::Staged, status_detail.staged.as_slice()),
        (
            WorktreeDiffKind::Unstaged,
            status_detail.unstaged.as_slice(),
        ),
        (
            WorktreeDiffKind::Untracked,
            status_detail.untracked.as_slice(),
        ),
        (
            WorktreeDiffKind::Conflict,
            status_detail.conflicted.as_slice(),
        ),
    ]
    .into_iter()
    .find_map(|(kind, entries)| {
        entries
            .iter()
            .find(|entry| entry.path == previous.path)
            .map(|entry| WorktreeDiffTarget {
                kind,
                path: entry.path.clone(),
            })
    })
}

fn is_checkoutable_ref(ref_summary: &RefSummary) -> bool {
    matches!(
        ref_summary.kind,
        RefKind::LocalBranch | RefKind::RemoteBranch
    )
}
