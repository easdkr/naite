use std::collections::HashMap;
use std::path::PathBuf;

use iced::Task;

use crate::features::repo_open::{self, Message as RepoOpenMessage};
use crate::message::TabsMessage;
use crate::state::ReleasePrepState;
use crate::{App, Message};

impl App {
    pub(crate) fn update_repo_open(&mut self, message: RepoOpenMessage) -> Task<Message> {
        match message {
            RepoOpenMessage::OpenClicked => {
                self.operation.error = None;
                self.manager.new_repo_menu_open = false;
                Task::perform(
                    repo_open::task::pick_folder("Choose a Git repository"),
                    |path| Message::from(RepoOpenMessage::PathPicked(path)),
                )
            }
            RepoOpenMessage::OpenRecent(path) => {
                self.operation.error = None;
                // Cache-hit path → instant tab swap; fall back to full load on miss.
                if self.tabs.cache.contains_key(&path) && self.tabs.active.as_ref() != Some(&path) {
                    return self.update(Message::from(TabsMessage::Activate(path)));
                }
                self.operation.loading = true;
                Task::perform(repo_open::task::load(path), |result| {
                    Message::from(RepoOpenMessage::Loaded(Box::new(result)))
                })
            }
            RepoOpenMessage::PathPicked(None) => Task::none(),
            RepoOpenMessage::PathPicked(Some(path)) => {
                self.operation.loading = true;
                Task::perform(repo_open::task::load(path), |result| {
                    Message::from(RepoOpenMessage::Loaded(Box::new(result)))
                })
            }
            RepoOpenMessage::Loaded(result) => match *result {
                Ok((
                    path,
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
                    let pending_status =
                        self.operation.pending_transient_status_after_reload.take();
                    let pending_error = self.operation.pending_error_after_reload.take();
                    let pending_force_push = self.operation.pending_force_push_after_reload;
                    self.operation.pending_force_push_after_reload = false;
                    let should_auto_fetch_after_load =
                        pending_status.is_none() && pending_error.is_none() && !pending_force_push;
                    let repo_changed = self.repo.path.as_ref() != Some(&path);
                    self.preserve_known_commit_avatar_urls(&path, &mut commits);

                    // Cache previous active state before replacing it. Only if
                    // it actually represents a different repo with a path.
                    if repo_changed {
                        if let Some(prev_path) = self.repo.path.clone() {
                            if prev_path != path {
                                let prev_state = std::mem::take(&mut self.repo);
                                self.tabs.cache.insert(prev_path, prev_state);
                            }
                        }
                    }

                    // Remove any pre-existing cache entry for the new path;
                    // we're loading fresh into self.repo.
                    self.tabs.cache.remove(&path);
                    self.tabs.refreshing.remove(&path);

                    self.repo.path = Some(path.clone());
                    self.repo.commits = commits;
                    self.repo.commit_page_cursor = commit_page_cursor;
                    self.repo.commits_loading_more = false;
                    self.repo.refs = refs;
                    self.repo.stashes = stashes;
                    self.repo.worktrees = worktrees;
                    self.repo.head_branch = head_branch;
                    self.repo.sync_status = sync_status;
                    self.repo.operation_state = operation_state;
                    self.repo.status_detail = status_detail;
                    self.operation.loading = false;
                    self.clear_repo_scoped_state();
                    if repo_changed {
                        self.release_prep = ReleasePrepState::default();
                        self.undo_stack.clear();
                        self.redo_stack.clear();
                    }
                    self.operation.error = pending_error;
                    if let Some(message) = pending_status {
                        self.set_transient_status(message);
                    }
                    if pending_force_push && self.operation.error.is_none() {
                        match self.force_push_prompt_for_current_branch() {
                            Ok(prompt) => {
                                self.selection.force_push_confirmation = Some(prompt);
                            }
                            Err(message) => {
                                self.operation.error = Some(message);
                            }
                        }
                    }
                    self.refresh_graph_layout();
                    self.catalog.remember(path.clone());
                    let evicted = self.tabs.remember(path.clone());
                    if let Some(evicted) = evicted {
                        self.tabs.cache.remove(&evicted);
                    }
                    self.tabs
                        .last_refreshed
                        .insert(path.clone(), std::time::Instant::now());
                    let terminal_task = self.ensure_repo_terminal_session(
                        path.clone(),
                        self.repo
                            .head_branch
                            .clone()
                            .unwrap_or_else(|| "Current repo".into()),
                    );
                    let save_task = self.save_catalog();
                    let save_tabs_task = self.save_open_tabs();
                    let workspace_task = self.refresh_workspace();
                    let pull_request_task = self.refresh_pull_requests();
                    let commit_avatar_task = self.prefetch_commit_avatars();
                    let provider_commit_avatar_task =
                        self.load_provider_commit_author_avatars(path.clone());
                    let auto_fetch_task = if should_auto_fetch_after_load {
                        self.start_auto_fetch()
                    } else {
                        Task::none()
                    };
                    let select_task = if self.repo.status_detail.is_dirty() {
                        self.select_wip()
                    } else {
                        Task::none()
                    };
                    let release_auto_task = if self.operation.error.is_none() {
                        self.continue_release_prep_auto()
                    } else {
                        Task::none()
                    };
                    Task::batch([
                        save_task,
                        save_tabs_task,
                        workspace_task,
                        pull_request_task,
                        commit_avatar_task,
                        provider_commit_avatar_task,
                        terminal_task,
                        auto_fetch_task,
                        select_task,
                        release_auto_task,
                    ])
                }
                Err(msg) => {
                    self.operation.pending_transient_status_after_reload = None;
                    self.operation.pending_error_after_reload = None;
                    self.operation.pending_force_push_after_reload = false;
                    self.release_prep.auto_running = false;
                    self.release_prep.auto_next_action = None;
                    self.operation.loading = false;
                    self.repo.commits_loading_more = false;
                    self.operation.error = Some(msg);
                    Task::none()
                }
            },
            RepoOpenMessage::LoadMoreCommitsRequested => self.load_more_commits(),
            RepoOpenMessage::MoreCommitsLoaded { path, result } => {
                if self.repo.path.as_ref() != Some(&path) {
                    return Task::none();
                }

                self.repo.commits_loading_more = false;
                match result {
                    Ok(mut page) => {
                        self.preserve_known_commit_avatar_urls(&path, &mut page.commits);
                        let existing: std::collections::HashSet<String> = self
                            .repo
                            .commits
                            .iter()
                            .map(|commit| commit.id.clone())
                            .collect();
                        self.repo.commits.extend(
                            page.commits
                                .into_iter()
                                .filter(|commit| !existing.contains(&commit.id)),
                        );
                        self.repo.commit_page_cursor = page.next_cursor;
                        self.refresh_graph_layout();
                        Task::batch([
                            self.prefetch_commit_avatars(),
                            self.load_provider_commit_author_avatars(path),
                        ])
                    }
                    Err(msg) => {
                        self.operation.error = Some(msg);
                        Task::none()
                    }
                }
            }
            RepoOpenMessage::CommitAuthorAvatarsLoaded { path, result } => {
                if self.repo.path.as_ref() != Some(&path) {
                    return Task::none();
                }

                let avatars = match result {
                    Ok(avatars) => avatars,
                    Err(err) => {
                        // The missing-CLI notice is one-shot per session (the
                        // `gh` binary's presence doesn't change mid-session, so
                        // every avatar load / pagination would otherwise re-toast).
                        // Other provider errors are transient and reported as-is.
                        if err.contains("could not find") {
                            if !self.provider_cli_notice_shown {
                                self.provider_cli_notice_shown = true;
                                self.set_transient_status(
                                    "GitHub CLI(gh)를 찾을 수 없어 아바타 대신 이니셜을 표시합니다. \
                                     gh를 설치하면 아바타가 표시됩니다."
                                        .to_string(),
                                );
                            }
                        } else {
                            self.set_transient_status(format!(
                                "GitHub 아바타를 불러오지 못했습니다: {err}"
                            ));
                        }
                        return Task::none();
                    }
                };
                let by_commit: HashMap<String, String> = avatars
                    .into_iter()
                    .filter_map(|avatar| {
                        avatar.author_avatar_url.map(|url| (avatar.commit_id, url))
                    })
                    .collect();
                if by_commit.is_empty() {
                    return Task::none();
                }

                for commit in &mut self.repo.commits {
                    if let Some(url) = by_commit.get(&commit.id) {
                        commit.author_avatar_url = Some(url.clone());
                    }
                }
                self.prefetch_commit_avatars()
            }
            RepoOpenMessage::ToggleFavorite(path) => {
                self.catalog.toggle_favorite(path);
                self.save_catalog()
            }
            RepoOpenMessage::RemoveFavorite(path) => {
                self.catalog.remove_favorite(&path);
                self.save_catalog()
            }
            RepoOpenMessage::RemoveRecent(path) => {
                self.catalog.remove_entry(&path);
                self.save_catalog()
            }
            RepoOpenMessage::CloneFormToggled => {
                self.manager.clone_open = !self.manager.clone_open;
                self.manager.new_repo_menu_open = false;
                self.operation.error = None;
                Task::none()
            }
            RepoOpenMessage::NewRepoMenuToggled => {
                self.manager.new_repo_menu_open = !self.manager.new_repo_menu_open;
                Task::none()
            }
            RepoOpenMessage::NewRepoMenuClosed => {
                self.manager.new_repo_menu_open = false;
                Task::none()
            }
            RepoOpenMessage::CloneUrlChanged(url) => {
                self.manager.clone_url = url;
                self.manager.clone_open = true;
                Task::none()
            }
            RepoOpenMessage::CloneClicked => {
                if self.manager.clone_url.trim().is_empty() {
                    self.manager.clone_open = true;
                    self.operation.error = Some("Enter a clone URL first.".into());
                    return Task::none();
                }

                self.operation.error = None;
                Task::perform(
                    repo_open::task::pick_folder("Choose where to clone the repository"),
                    |path| Message::from(RepoOpenMessage::CloneParentPicked(path)),
                )
            }
            RepoOpenMessage::CloneParentPicked(None) => Task::none(),
            RepoOpenMessage::CloneParentPicked(Some(parent)) => {
                self.operation.loading = true;
                let url = self.manager.clone_url.clone();
                Task::perform(repo_open::task::clone_repo(url, parent), |result| {
                    Message::from(RepoOpenMessage::CloneDone(result))
                })
            }
            RepoOpenMessage::CloneDone(Ok(path)) => {
                self.manager.clone_url.clear();
                self.manager.clone_open = false;
                self.operation.loading = true;
                Task::perform(repo_open::task::load(path), |result| {
                    Message::from(RepoOpenMessage::Loaded(Box::new(result)))
                })
            }
            RepoOpenMessage::CloneDone(Err(msg)) => {
                self.operation.loading = false;
                self.manager.clone_open = true;
                self.operation.error = Some(msg);
                Task::none()
            }
            RepoOpenMessage::InitClicked => {
                self.operation.error = None;
                self.manager.new_repo_menu_open = false;
                Task::perform(
                    repo_open::task::pick_folder("Choose an empty folder to initialize"),
                    |path| Message::from(RepoOpenMessage::InitPathPicked(path)),
                )
            }
            RepoOpenMessage::InitPathPicked(None) => Task::none(),
            RepoOpenMessage::InitPathPicked(Some(path)) => {
                self.operation.loading = true;
                Task::perform(repo_open::task::init(path), |result| {
                    Message::from(RepoOpenMessage::InitDone(result))
                })
            }
            RepoOpenMessage::InitDone(Ok(path)) => {
                self.operation.loading = true;
                Task::perform(repo_open::task::load(path), |result| {
                    Message::from(RepoOpenMessage::Loaded(Box::new(result)))
                })
            }
            RepoOpenMessage::InitDone(Err(msg)) => {
                self.operation.loading = false;
                self.operation.error = Some(msg);
                Task::none()
            }
        }
    }

    fn load_provider_commit_author_avatars(&self, path: PathBuf) -> Task<Message> {
        let commit_ids: Vec<String> = self
            .repo
            .commits
            .iter()
            .filter(|commit| commit.author_avatar_url.is_none())
            .map(|commit| commit.id.clone())
            .collect();
        if commit_ids.is_empty() {
            return Task::none();
        }

        Task::perform(
            repo_open::task::load_commit_author_avatars(path, commit_ids),
            |(path, result)| {
                Message::from(RepoOpenMessage::CommitAuthorAvatarsLoaded { path, result })
            },
        )
    }

    pub(crate) fn load_more_commits(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        let Some(cursor) = self.repo.commit_page_cursor else {
            return Task::none();
        };
        if self.repo.commits_loading_more || self.operation.loading || !self.search_query.is_empty()
        {
            return Task::none();
        }

        self.repo.commits_loading_more = true;
        Task::perform(
            repo_open::task::load_more_commits(path, cursor),
            |(path, result)| Message::from(RepoOpenMessage::MoreCommitsLoaded { path, result }),
        )
    }
}
