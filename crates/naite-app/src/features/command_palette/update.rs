use iced::widget::text_input;
use iced::Task;
use naite_core::{GitHubIssueFilter, PullRequestFilter, ReorderDirection};

use crate::features::{
    branch_manage, checkout, cherry_pick, command_palette::Message as CommandPaletteMessage, fetch,
    file_inspect, github_issue, history, pull, pull_request, push, release_prep, repo_open, reset,
    revert, stage, stash, tag, terminal, workspace, worktree,
};
use crate::state::{DensityPreference, ThemePreference};
use crate::{App, CommandId, Message};

impl App {
    pub(crate) fn update_command_palette(
        &mut self,
        message: CommandPaletteMessage,
    ) -> Task<Message> {
        match message {
            CommandPaletteMessage::Opened => self.open_command_palette(),
            CommandPaletteMessage::Closed => {
                self.command_palette.open = false;
                Task::none()
            }
            CommandPaletteMessage::QueryChanged(query) => {
                self.command_palette.query = query;
                self.command_palette.selected = 0;
                Task::none()
            }
            CommandPaletteMessage::Selected(index) => {
                if index < self.filtered_command_palette_items().len() {
                    self.command_palette.selected = index;
                }
                Task::none()
            }
            CommandPaletteMessage::Run(command) => self.run_command_palette_command(command),
        }
    }

    pub(crate) fn open_command_palette(&mut self) -> Task<Message> {
        self.command_palette.open = true;
        self.command_palette.query.clear();
        self.command_palette.selected = 0;
        text_input::focus(self.command_palette_input_id.clone())
    }

    pub(crate) fn move_command_palette_selection(&mut self, delta: isize) -> Task<Message> {
        let count = self.filtered_command_palette_items().len();
        if count == 0 {
            self.command_palette.selected = 0;
            return Task::none();
        }

        let current = self.command_palette.selected.min(count.saturating_sub(1)) as isize;
        self.command_palette.selected =
            (current + delta).clamp(0, count.saturating_sub(1) as isize) as usize;
        Task::none()
    }

    pub(crate) fn run_selected_command_palette_command(&mut self) -> Task<Message> {
        let command = self
            .filtered_command_palette_items()
            .get(self.command_palette.selected)
            .map(|item| item.id);

        match command {
            Some(command) => self.run_command_palette_command(command),
            None => Task::none(),
        }
    }

    pub(crate) fn run_command_palette_command(&mut self, command: CommandId) -> Task<Message> {
        let Some(item) = self
            .command_palette_items()
            .into_iter()
            .find(|item| item.id == command)
        else {
            return Task::none();
        };

        if !item.enabled() {
            return Task::none();
        }

        self.command_palette.open = false;
        match command {
            CommandId::OpenRepository => self.update(repo_open::Message::OpenClicked.into()),
            CommandId::InitRepository => self.update(repo_open::Message::InitClicked.into()),
            CommandId::CloneRepository => self.update(repo_open::Message::CloneClicked.into()),
            CommandId::ToggleWorkspaceDashboard => {
                self.update(workspace::Message::DashboardToggled.into())
            }
            CommandId::RefreshWorkspace => self.update(workspace::Message::RefreshRequested.into()),
            CommandId::WorkspaceFetchAll => {
                self.update(workspace::Message::FetchAllRequested.into())
            }
            CommandId::WorkspacePullAll => self.update(workspace::Message::PullAllRequested.into()),
            CommandId::RefreshPullRequests => {
                self.update(pull_request::Message::RefreshRequested.into())
            }
            CommandId::FilterAllPullRequests => {
                self.update(pull_request::Message::FilterChanged(PullRequestFilter::All).into())
            }
            CommandId::FilterMyPullRequests => {
                self.update(pull_request::Message::FilterChanged(PullRequestFilter::Mine).into())
            }
            CommandId::FilterNeedsReviewPullRequests => self.update(
                pull_request::Message::FilterChanged(PullRequestFilter::NeedsReview).into(),
            ),
            CommandId::FilterDraftPullRequests => {
                self.update(pull_request::Message::FilterChanged(PullRequestFilter::Draft).into())
            }
            CommandId::FilterFailingPullRequests => self.update(
                pull_request::Message::FilterChanged(PullRequestFilter::FailingChecks).into(),
            ),
            CommandId::FilterCurrentBranchPullRequests => self.update(
                pull_request::Message::FilterChanged(PullRequestFilter::CurrentBranch).into(),
            ),
            CommandId::SearchPullRequests => {
                self.update(pull_request::Message::SearchSubmitted.into())
            }
            CommandId::CreatePullRequest => {
                self.update(pull_request::Message::CreateRequested.into())
            }
            CommandId::CheckoutSelectedPullRequest => self
                .selection
                .selected_pull_request
                .clone()
                .map(|pull_request| {
                    self.update(pull_request::Message::CheckoutRequested(pull_request).into())
                })
                .unwrap_or_else(Task::none),
            CommandId::CheckoutSelectedPullRequestIntoWorktree => self
                .selection
                .selected_pull_request
                .clone()
                .map(|pull_request| {
                    self.update(
                        pull_request::Message::CheckoutWorktreeRequested(pull_request).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::OpenSelectedPullRequest => self
                .selection
                .selected_pull_request
                .clone()
                .map(|pull_request| {
                    self.update(pull_request::Message::OpenInBrowserRequested(pull_request).into())
                })
                .unwrap_or_else(Task::none),
            CommandId::RefreshGitHubIssues => {
                self.update(github_issue::Message::RefreshRequested.into())
            }
            CommandId::FilterOpenGitHubIssues => {
                self.update(github_issue::Message::FilterChanged(GitHubIssueFilter::Open).into())
            }
            CommandId::FilterAssignedGitHubIssues => self
                .update(github_issue::Message::FilterChanged(GitHubIssueFilter::Assigned).into()),
            CommandId::FilterMentionedGitHubIssues => self
                .update(github_issue::Message::FilterChanged(GitHubIssueFilter::Mentioned).into()),
            CommandId::FilterClosedGitHubIssues => {
                self.update(github_issue::Message::FilterChanged(GitHubIssueFilter::Closed).into())
            }
            CommandId::SearchGitHubIssues => {
                self.update(github_issue::Message::SearchSubmitted.into())
            }
            CommandId::OpenSelectedGitHubIssue => self
                .selection
                .selected_github_issue
                .clone()
                .map(|issue| {
                    self.update(github_issue::Message::OpenInBrowserRequested(issue).into())
                })
                .unwrap_or_else(Task::none),
            CommandId::ToggleDisplayOptions => self.update(Message::ToggleDisplayOptions),
            CommandId::ToggleShortcutHelp => self.update(Message::ToggleShortcutOverlay),
            CommandId::ToggleTheme => {
                let next = match self.preferences.theme {
                    ThemePreference::Dark => ThemePreference::HighContrast,
                    ThemePreference::HighContrast => ThemePreference::Dark,
                };
                self.update(Message::ThemePreferenceChanged(next))
            }
            CommandId::SetDensityComfortable => self.update(Message::DensityPreferenceChanged(
                DensityPreference::Comfortable,
            )),
            CommandId::SetDensityCompact => self.update(Message::DensityPreferenceChanged(
                DensityPreference::Compact,
            )),
            CommandId::SetDensityDense => {
                self.update(Message::DensityPreferenceChanged(DensityPreference::Dense))
            }
            CommandId::ToggleCommitAuthorDisplay => {
                self.update(Message::DisplayCommitAuthorToggled)
            }
            CommandId::ToggleFileInspectionDisplay => {
                self.update(Message::DisplayFileInspectionToggled)
            }
            CommandId::TogglePullRequestMetadataDisplay => {
                self.update(Message::DisplayPrMetadataToggled)
            }
            CommandId::ToggleWorkspaceDetailsDisplay => {
                self.update(Message::DisplayWorkspaceDetailsToggled)
            }
            CommandId::FocusSearch => text_input::focus(self.search_input_id.clone()),
            CommandId::SelectWip => self.select_wip(),
            CommandId::CreateBranch => self.open_branch_create_form(),
            CommandId::CheckoutSelectedRef => self
                .selected_context_checkoutable_ref()
                .cloned()
                .map(|target| self.update(checkout::Message::Requested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::ForceSyncSelectedRef => self
                .selected_context_force_sync_target()
                .map(|target| self.update(checkout::Message::ForceSyncRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::RenameSelectedBranch => self
                .selected_context_local_branch()
                .cloned()
                .map(|target| self.update(branch_manage::Message::RenameRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::DeleteSelectedBranch => self
                .selected_context_local_branch()
                .cloned()
                .map(|target| {
                    self.update(
                        branch_manage::Message::DeleteRequested(
                            crate::BranchDeleteTarget::LocalBranch(target),
                        )
                        .into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::CreateWorktree => self.update(worktree::Message::CreateRequested.into()),
            CommandId::OpenSelectedWorktree => self
                .selection
                .selected_worktree
                .clone()
                .map(|target| self.update(worktree::Message::OpenRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::LockSelectedWorktree => self
                .selection
                .selected_worktree
                .clone()
                .map(|target| self.update(worktree::Message::LockRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::UnlockSelectedWorktree => self
                .selection
                .selected_worktree
                .clone()
                .map(|target| self.update(worktree::Message::UnlockRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::RemoveSelectedWorktree => self
                .selection
                .selected_worktree
                .clone()
                .map(|target| self.update(worktree::Message::RemoveRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::OpenTerminal => self.update(terminal::Message::OpenRequested.into()),
            CommandId::RunTerminalCommand => self.update(terminal::Message::StartRequested.into()),
            CommandId::RestartTerminalCommand => {
                self.update(terminal::Message::RestartRequested.into())
            }
            CommandId::KillTerminalCommand => self.update(terminal::Message::KillRequested.into()),
            CommandId::Fetch => {
                self.update(fetch::Message::Requested(fetch::FetchScope::CurrentRemote).into())
            }
            CommandId::FetchAll => {
                self.update(fetch::Message::Requested(fetch::FetchScope::AllRemotes).into())
            }
            CommandId::PullFastForwardOnly => {
                self.update(pull::Message::Requested(pull::PullMode::FastForwardOnly).into())
            }
            CommandId::PullFastForward => {
                self.update(pull::Message::Requested(pull::PullMode::FastForward).into())
            }
            CommandId::PullRebase => {
                self.update(pull::Message::Requested(pull::PullMode::Rebase).into())
            }
            CommandId::Push => self.update(push::Message::Requested(push::PushMode::Normal).into()),
            CommandId::PushForceWithLease => {
                self.update(push::Message::ForceWithLeaseConfirmationRequested.into())
            }
            CommandId::PrepareProductionRelease => {
                self.update(release_prep::Message::Requested.into())
            }
            CommandId::ReleaseUpdateTargetFromSource => self.update(
                release_prep::Message::ActionRequested(
                    release_prep::ReleasePrepAction::UpdateTargetFromSource,
                )
                .into(),
            ),
            CommandId::ReleasePushTarget => self.update(
                release_prep::Message::ActionRequested(release_prep::ReleasePrepAction::PushTarget)
                    .into(),
            ),
            CommandId::ReleaseSyncSourceFromTarget => self.update(
                release_prep::Message::ActionRequested(
                    release_prep::ReleasePrepAction::SyncSourceFromTarget,
                )
                .into(),
            ),
            CommandId::MergeSelectedRef => self
                .selected_context_mergeable_ref()
                .cloned()
                .map(|target| {
                    self.update(
                        history::Message::Requested(history::Operation::Merge(target)).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::RebaseOntoSelectedRef => self
                .selected_context_mergeable_ref()
                .cloned()
                .map(|target| {
                    self.update(
                        history::Message::Requested(history::Operation::Rebase(target)).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::AbortMerge => {
                self.update(history::Message::Requested(history::Operation::AbortMerge).into())
            }
            CommandId::AbortRebase => {
                self.update(history::Message::Requested(history::Operation::AbortRebase).into())
            }
            CommandId::ContinueRebase => {
                self.update(history::Message::Requested(history::Operation::ContinueRebase).into())
            }
            CommandId::RewordSelectedCommit => self
                .selected_commit()
                .map(|commit| self.update(history::Message::RewordRequested(commit).into()))
                .unwrap_or_else(Task::none),
            CommandId::DropSelectedCommit => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        history::Message::Requested(history::Operation::Drop(commit)).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::SquashSelectedCommit => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        history::Message::Requested(history::Operation::Squash(commit)).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::FixupSelectedCommit => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        history::Message::Requested(history::Operation::Fixup(commit)).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::EditSelectedCommit => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        history::Message::Requested(history::Operation::Edit(commit)).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::MoveSelectedCommitEarlier => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        history::Message::Requested(history::Operation::Move {
                            commit,
                            direction: ReorderDirection::Earlier,
                        })
                        .into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::MoveSelectedCommitLater => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        history::Message::Requested(history::Operation::Move {
                            commit,
                            direction: ReorderDirection::Later,
                        })
                        .into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::CherryPickSelectedCommit => self
                .selected_commit()
                .map(|commit| self.update(cherry_pick::Message::Requested(commit).into()))
                .unwrap_or_else(Task::none),
            CommandId::RevertSelectedCommit => self
                .selected_commit()
                .map(|commit| self.update(revert::Message::Requested(commit).into()))
                .unwrap_or_else(Task::none),
            CommandId::ResetToSelectedCommit => self
                .selected_commit()
                .map(|commit| self.update(reset::Message::Requested(commit).into()))
                .unwrap_or_else(Task::none),
            CommandId::CreateBranchFromSelectedCommit => self
                .selected_commit()
                .map(|commit| {
                    self.update(
                        crate::features::branch_create::Message::RequestedFromCommit(commit).into(),
                    )
                })
                .unwrap_or_else(Task::none),
            CommandId::CreateTag => {
                self.update(tag::Message::CreateRequested(self.selected_commit()).into())
            }
            CommandId::CreateAndPushTag => {
                self.update(tag::Message::CreateAndPushRequested(self.selected_commit()).into())
            }
            CommandId::DeleteSelectedTag => self
                .selected_context_tag()
                .cloned()
                .map(|target| self.update(tag::Message::DeleteRequested(target).into()))
                .unwrap_or_else(Task::none),
            CommandId::Undo => self.update(history::Message::UndoRequested.into()),
            CommandId::Redo => self.update(history::Message::RedoRequested.into()),
            CommandId::ShowFileHistory => self
                .selected_detail_file_path()
                .map(|path| self.update(file_inspect::Message::HistoryRequested(path).into()))
                .unwrap_or_else(Task::none),
            CommandId::ShowFileBlame => self
                .selected_detail_file_path()
                .map(|path| self.update(file_inspect::Message::BlameRequested(path).into()))
                .unwrap_or_else(Task::none),
            CommandId::StashChanges => self.update(stash::Message::CreateRequested.into()),
            CommandId::PopLatestStash => self
                .repo
                .stashes
                .first()
                .cloned()
                .map(|summary| self.update(stash::Message::PopRequested(summary).into()))
                .unwrap_or_else(Task::none),
            CommandId::CreateBranchFromSelectedStash => self
                .selection
                .selected_stash
                .clone()
                .map(|summary| self.update(stash::Message::BranchRequested(summary).into()))
                .unwrap_or_else(Task::none),
            CommandId::StageAll => self.start_stage_operation(stage::Operation::StageAll),
            CommandId::UnstageAll => self.start_stage_operation(stage::Operation::UnstageAll),
            CommandId::Commit => self.start_commit(),
        }
    }
}
