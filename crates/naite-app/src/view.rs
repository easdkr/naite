use iced::widget::{column, container, pane_grid, stack, Space};
use iced::{Element, Length, Size};

use crate::features::{
    checkout, discard, history, pull_request, push, rebase, release_prep, reset, stash, tag,
    worktree,
};
use crate::message::Message;
use crate::state::ReleasePrepPhase;
use crate::widgets;
use crate::{App, PaneId};

impl App {
    pub(crate) fn view(&self) -> Element<'_, Message> {
        let visible_indices = self.visible_commit_indices();
        let command_palette_items = self.filtered_command_palette_items();

        let toolbar = widgets::toolbar(widgets::ToolbarProps {
            repo_path: self.repo.path.as_deref(),
            head_branch: self.repo.head_branch.as_deref(),
            sync_status: &self.repo.sync_status,
            status_detail: &self.repo.status_detail,
            stashes: &self.repo.stashes,
            transient_status: self
                .operation
                .transient_status
                .as_ref()
                .map(|status| status.message.as_str()),
            loading: self.operation.loading,
            search_query: &self.search_query,
            visible_count: visible_indices.len(),
            total_count: self.repo.commits.len(),
            search_input_id: &self.search_input_id,
            window_width: self.window_width,
        });

        let pane_grid = pane_grid::PaneGrid::new(&self.panes, |_pane, pane_id, _| {
            let content: Element<'_, Message> = match pane_id {
                PaneId::Sidebar => widgets::sidebar(widgets::SidebarProps {
                    repo_path: self.repo.path.as_deref(),
                    refs: &self.repo.refs,
                    pull_requests: &self.repo.pull_requests,
                    github_issues: &self.repo.github_issues,
                    pull_request_filter: self.pull_requests.filter,
                    pull_request_search_query: &self.pull_requests.search_query,
                    pull_request_loading: self.pull_requests.loading,
                    pull_request_error: self.pull_requests.error.as_deref(),
                    pull_request_search_input_id: &self.pull_request_search_input_id,
                    github_issue_filter: self.github_issues.filter,
                    github_issue_search_query: &self.github_issues.search_query,
                    github_issue_loading: self.github_issues.loading,
                    github_issue_error: self.github_issues.error.as_deref(),
                    actions_disabled: self.operation.loading,
                    head_branch: self.repo.head_branch.as_deref(),
                    stashes: &self.repo.stashes,
                    worktrees: &self.repo.worktrees,
                    selected_pull_request_number: self
                        .selection
                        .selected_pull_request
                        .as_ref()
                        .map(|pull_request| pull_request.number),
                    selected_github_issue_number: self
                        .selection
                        .selected_github_issue
                        .as_ref()
                        .map(|issue| issue.number),
                    selected_stash_selector: self
                        .selection
                        .selected_stash
                        .as_ref()
                        .map(|stash| stash.selector.as_str()),
                    selected_worktree_path: self
                        .selection
                        .selected_worktree
                        .as_ref()
                        .map(|worktree| worktree.path.as_path()),
                    catalog: &self.catalog,
                    tabs: &self.tabs,
                    sidebar_state: &self.sidebar,
                    clone_url: &self.manager.clone_url,
                    clone_open: self.manager.clone_open,
                    new_repo_menu_open: self.manager.new_repo_menu_open,
                }),
                PaneId::List => {
                    if let Some(session) = &self.rebase {
                        widgets::rebase_editor(
                            session,
                            self.selection.cursor_position,
                            self.release_prep.active_profile.is_some(),
                            &self.avatars,
                        )
                    } else if self.workspace.dashboard_open {
                        widgets::workspace_dashboard(
                            &self.workspace,
                            &self.tabs,
                            self.operation.loading,
                            &self.preferences,
                        )
                    } else {
                        let visible_indices = self.visible_commit_indices();
                        let list_width = self.window_width
                            * (1.0 - self.preferences.sidebar_ratio.clamp(0.14, 0.36))
                            * self.preferences.detail_ratio.clamp(0.50, 0.78);
                        let error_recovery = self.error_recovery_action();
                        let commit_list = widgets::commit_list(widgets::CommitListProps {
                            commits: &self.repo.commits,
                            visible_indices,
                            selected: self.selected_index(),
                            wip_selected: self.selection.selected_wip,
                            status_detail: &self.repo.status_detail,
                            fatal_error: self.operation.fatal_error.as_deref(),
                            error_recovery,
                            graph_layout: &self.repo.graph_layout,
                            refs: &self.repo.refs,
                            scroll_id: &self.commit_list_id,
                            preferences: &self.preferences,
                            avatars: &self.avatars,
                            list_width,
                            has_more_commits: self.repo.commit_page_cursor.is_some(),
                            loading_more_commits: self.repo.commits_loading_more,
                        });
                        // When the terminal panel is open, embed it inline
                        // at the bottom of the list pane so it is flush with
                        // the pane boundary. (An overlay-based approach left
                        // a hairline gap at the bottom in iced 0.13.)
                        if self.terminal.open {
                            let panel_height = if self
                                .terminal
                                .active_session()
                                .is_some_and(|session| session.minimized)
                            {
                                widgets::TERMINAL_PANEL_HEIGHT_MINIMIZED
                            } else {
                                widgets::TERMINAL_PANEL_HEIGHT
                            };
                            let terminal_panel = container(widgets::terminal_panel(
                                &self.terminal,
                                self.repo.path.as_deref(),
                                self.repo.head_branch.as_deref(),
                                &self.repo.worktrees,
                                &self.terminal_input_id,
                            ))
                            .width(Length::Fill)
                            .height(Length::Fixed(panel_height));
                            column![container(commit_list).height(Length::Fill), terminal_panel,]
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .into()
                        } else {
                            commit_list
                        }
                    }
                }
                PaneId::Detail => {
                    if let Some(session) = &self.rebase {
                        widgets::rebase_detail(widgets::RebaseDetailProps {
                            session,
                            diff: self.operation.current_diff.as_ref(),
                            diff_loading: self.operation.diff_loading,
                            diff_error: self.operation.diff_error.as_deref(),
                            selected_file: self.selection.selected_file,
                            selected_hunk: self.selection.selected_hunk,
                            diff_view_mode: self.selection.diff_view_mode,
                            selected_wip_file: self.selection.selected_wip_file.as_ref(),
                            file_insight: &self.file_insight,
                        })
                    } else {
                        widgets::detail_pane(widgets::DetailPaneProps {
                            commits: &self.repo.commits,
                            selected: self.selected_index(),
                            wip_selected: self.selection.selected_wip,
                            selected_pull_request: self.selection.selected_pull_request.as_ref(),
                            selected_github_issue: self.selection.selected_github_issue.as_ref(),
                            selected_stash: self.selection.selected_stash.as_ref(),
                            status_detail: &self.repo.status_detail,
                            diff: self.operation.current_diff.as_ref(),
                            diff_highlight: self.operation.current_diff_highlight.as_ref(),
                            diff_loading: self.operation.diff_loading,
                            diff_error: self.operation.diff_error.as_deref(),
                            selected_file: self.selection.selected_file,
                            selected_hunk: self.selection.selected_hunk,
                            diff_view_mode: self.selection.diff_view_mode,
                            selected_wip_file: self.selection.selected_wip_file.as_ref(),
                            commit_form: &self.commit_form,
                            head_branch: self.repo.head_branch.as_deref(),
                            actions_disabled: self.operation.loading,
                            operation_state: self.repo.operation_state,
                            file_insight: &self.file_insight,
                            avatars: &self.avatars,
                            preferences: &self.preferences,
                        })
                    }
                }
            };
            pane_grid::Content::new(content)
        })
        .on_resize(8, Message::PaneResized)
        .width(Length::Fill)
        .height(Length::Fill);

        let mut root = column![toolbar].width(Length::Fill).height(Length::Fill);
        root = root.push(widgets::top_status_bar(
            &self.operation_tracker,
            self.status_animation_frame,
        ));

        if self.preferences.display_options_open {
            root = root.push(widgets::display_options_panel(&self.preferences));
        }

        if self.preferences.shortcuts_open {
            root = root.push(widgets::shortcut_help_overlay());
        }

        if self.branch_create.open {
            root = root.push(widgets::branch_create_prompt(
                &self.branch_create,
                self.operation.loading,
                &self.branch_create_input_id,
            ));
        }

        if self.worktree_create.open {
            root = root.push(widgets::worktree_create_prompt(
                &self.worktree_create,
                self.operation.loading,
                &self.worktree_path_input_id,
                &self.worktree_start_input_id,
                &self.worktree_branch_input_id,
            ));
        }

        if self.branch_manage_rename.open {
            root = root.push(widgets::branch_rename_prompt(
                &self.branch_manage_rename,
                self.operation.loading,
                &self.branch_manage_input_id,
            ));
        }

        if self.stash_create.open {
            root = root.push(widgets::stash_create_prompt(
                &self.stash_create,
                &self.repo.status_detail,
                self.operation.loading,
                self.can_submit_stash_create(),
                &self.stash_create_input_id,
            ));
        }

        if self.stash_branch.open {
            root = root.push(widgets::stash_branch_prompt(
                &self.stash_branch,
                self.operation.loading,
                &self.stash_branch_input_id,
            ));
        }

        if self.history_reword.open {
            root = root.push(widgets::history_reword_prompt(
                &self.history_reword,
                self.operation.loading,
                &self.history_reword_input_id,
            ));
        }

        root = root.push(container(pane_grid).height(Length::Fill));
        root = root.push(widgets::bottom_status_bar(&self.operation_tracker));

        let base: Element<'_, Message> = root.into();

        let mut overlays: Vec<Element<'_, Message>> = Vec::new();

        if self.command_palette.open {
            overlays.push(widgets::command_palette_overlay(
                &self.command_palette.query,
                command_palette_items,
                self.command_palette.selected,
                &self.command_palette_input_id,
                self.window_height,
            ));
        }

        if self.pull_requests.create.open {
            overlays.push(widgets::modal(
                widgets::pull_request_create_prompt(
                    &self.pull_requests.create,
                    self.operation.loading,
                    &self.pull_request_create_base_input_id,
                ),
                Message::from(pull_request::Message::CreateCancelled),
            ));
        }

        if self.pull_requests.checkout_worktree.open {
            overlays.push(widgets::modal(
                widgets::pull_request_worktree_prompt(
                    &self.pull_requests.checkout_worktree,
                    self.operation.loading,
                    &self.pull_request_worktree_path_input_id,
                    &self.pull_request_worktree_branch_input_id,
                ),
                Message::from(pull_request::Message::CheckoutWorktreeCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.force_sync_confirmation {
            overlays.push(widgets::modal(
                widgets::force_sync_prompt(prompt, self.operation.loading),
                Message::from(checkout::Message::Cancelled),
            ));
        }

        if let Some(prompt) = &self.selection.force_push_confirmation {
            overlays.push(widgets::modal(
                widgets::force_push_prompt(prompt, self.operation.loading),
                Message::from(push::Message::ForceWithLeaseCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.branch_delete_confirmation {
            overlays.push(widgets::modal(
                widgets::branch_delete_prompt(prompt, self.operation.loading),
                Message::from(crate::features::branch_manage::Message::DeleteCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.checkout_confirmation {
            overlays.push(widgets::modal(
                widgets::checkout_prompt(prompt),
                Message::from(checkout::Message::Cancelled),
            ));
        }

        if let Some(prompt) = &self.selection.discard_confirmation {
            overlays.push(widgets::modal(
                widgets::discard_prompt(prompt, self.operation.loading),
                Message::from(discard::Message::Cancelled),
            ));
        }

        if let Some(prompt) = &self.selection.stash_confirmation {
            overlays.push(widgets::modal(
                widgets::stash_prompt(prompt, self.operation.loading),
                Message::from(stash::Message::ConfirmationCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.history_confirmation {
            overlays.push(widgets::modal(
                widgets::history_prompt(prompt, self.operation.loading),
                Message::from(history::Message::Cancelled),
            ));
        }

        if let Some(prompt) = &self.selection.rebase_confirmation {
            overlays.push(widgets::wide_modal(
                widgets::rebase_prompt(prompt, &self.avatars, self.operation.loading),
                Message::from(rebase::Message::ApplyCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.reset_confirmation {
            overlays.push(widgets::modal(
                widgets::reset_prompt(prompt, self.operation.loading),
                Message::from(reset::Message::Cancelled),
            ));
        }

        if let Some(prompt) = &self.selection.tag_delete_confirmation {
            overlays.push(widgets::modal(
                widgets::tag_delete_prompt(prompt, self.operation.loading),
                Message::from(tag::Message::DeleteCancelled),
            ));
        }

        if self.tag_create.open {
            overlays.push(widgets::modal(
                widgets::tag_create_prompt(
                    &self.tag_create,
                    self.operation.loading,
                    &self.tag_create_input_id,
                ),
                Message::from(tag::Message::CreateCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.undo_confirmation {
            overlays.push(widgets::modal(
                widgets::undo_prompt(prompt, self.operation.loading),
                Message::from(history::Message::UndoCancelled),
            ));
        }

        if let Some(prompt) = &self.selection.worktree_remove_confirmation {
            overlays.push(widgets::modal(
                widgets::worktree_remove_prompt(prompt, self.operation.loading),
                Message::from(worktree::Message::RemoveCancelled),
            ));
        }

        if self.release_prep.phase == ReleasePrepPhase::Configuring {
            overlays.push(widgets::animated_modal(
                widgets::release_prep_config(&self.release_prep, self.operation.loading),
                Message::from(release_prep::Message::Cancelled),
                self.release_prep.animation_frame,
            ));
        }

        if self.release_prep.phase == ReleasePrepPhase::Preparing {
            overlays.push(widgets::animated_modal(
                widgets::release_prep_progress(&self.release_prep),
                Message::from(release_prep::Message::Cancelled),
                self.release_prep.animation_frame,
            ));
        }

        if matches!(
            self.release_prep.phase,
            ReleasePrepPhase::Actions | ReleasePrepPhase::RunningAction
        ) {
            overlays.push(widgets::animated_modal(
                widgets::release_prep_actions(&self.release_prep, self.operation.loading),
                Message::from(release_prep::Message::Cancelled),
                self.release_prep.animation_frame,
            ));
        }

        let context_menu_overlay: Element<'_, Message> = match &self.selection.context_menu {
            Some(menu) => {
                let window_size = Size::new(self.window_width, self.window_height);
                let force_sync_target = menu
                    .kind
                    .as_ref()
                    .and_then(|ref_summary| self.force_sync_target_for_ref(ref_summary));
                widgets::floating_context_menu(menu, window_size, force_sync_target)
            }
            None => Space::new(0, 0).into(),
        };

        let mut layered = stack![base];
        for overlay in overlays {
            layered = layered.push(overlay);
        }
        layered = layered.push(context_menu_overlay);
        // Toast layer + progress overlay are the topmost surfaces; they
        // sit above modals/context menus so users always see completion
        // feedback regardless of what modal is open. Overlay visibility
        // is computed in update.rs on every TransientStatusTick
        // (ReleasePrep shows immediately, everything else waits
        // OVERLAY_TRIGGER_SECS). The id is looked up by
        // active().iter().find() so a stale id (op completed between
        // tick and render) silently resolves to None rather than
        // dereferencing a freed reference.
        layered = layered.push(widgets::toast_layer(&self.toasts));
        if let Some(op) = self
            .overlay_visible
            .and_then(|id| self.operation_tracker.active().iter().find(|op| op.id == id))
        {
            layered = layered.push(widgets::progress_overlay(op, self.status_animation_frame));
        }
        layered.into()
    }
}
