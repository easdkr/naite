use naite_core::{CommitMessage, CommitSummary, ConflictSide, RefSummary, ReorderDirection};

use crate::state::UndoCheckpoint;

#[derive(Debug, Clone)]
pub enum Message {
    Requested(Operation),
    Confirmed,
    Cancelled,
    RewordRequested(CommitSummary),
    RewordTitleChanged(String),
    RewordBodyAction(iced::widget::text_editor::Action),
    RewordFormLoaded(Result<CommitMessage, String>),
    RewordCancelled,
    RewordSubmitted,
    UndoRequested,
    RedoRequested,
    UndoConfirmed,
    UndoCancelled,
    Done {
        operation: Operation,
        checkpoint: Option<UndoCheckpoint>,
        head_before_reset: Option<UndoCheckpoint>,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
pub enum Operation {
    Merge(RefSummary),
    Rebase(RefSummary),
    AbortMerge,
    AbortRebase,
    ContinueRebase,
    ResolveWithSide {
        path: String,
        side: ConflictSide,
    },
    MarkResolved(String),
    Reword {
        commit: CommitSummary,
        message: String,
    },
    Drop(CommitSummary),
    Squash(CommitSummary),
    Fixup(CommitSummary),
    Edit(CommitSummary),
    Move {
        commit: CommitSummary,
        direction: ReorderDirection,
    },
    Undo(UndoCheckpoint),
    Redo(UndoCheckpoint),
}

impl Operation {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::Merge(target) => format!("Merge {}", target.short_name),
            Self::Rebase(target) => format!("Rebase onto {}", target.short_name),
            Self::AbortMerge => "Abort merge".into(),
            Self::AbortRebase => "Abort rebase".into(),
            Self::ContinueRebase => "Continue rebase".into(),
            Self::ResolveWithSide { path, side } => {
                format!("Use {} for {}", side_label(*side), path)
            }
            Self::MarkResolved(path) => format!("Mark {path} resolved"),
            Self::Reword { commit, .. } => format!("Reword {}", commit.short_id),
            Self::Drop(commit) => format!("Drop {}", commit.short_id),
            Self::Squash(commit) => format!("Squash {}", commit.short_id),
            Self::Fixup(commit) => format!("Fixup {}", commit.short_id),
            Self::Edit(commit) => format!("Edit {}", commit.short_id),
            Self::Move { commit, direction } => {
                let direction = match direction {
                    ReorderDirection::Earlier => "earlier",
                    ReorderDirection::Later => "later",
                };
                format!("Move {} {direction}", commit.short_id)
            }
            Self::Undo(checkpoint) => format!("Undo {}", checkpoint.label),
            Self::Redo(checkpoint) => format!("Redo {}", checkpoint.label),
        }
    }

    pub(crate) fn detail(&self) -> String {
        match self {
            Self::Merge(target) => format!(
                "Runs git merge --no-edit {} into the current branch.",
                target.short_name
            ),
            Self::Rebase(target) => {
                format!("Runs git rebase {} on the current branch.", target.short_name)
            }
            Self::AbortMerge => "Runs git merge --abort for the in-progress merge.".into(),
            Self::AbortRebase => "Runs git rebase --abort for the in-progress rebase.".into(),
            Self::ContinueRebase => "Runs git rebase --continue after conflicts are staged.".into(),
            Self::ResolveWithSide { side, .. } => format!(
                "Checks out the {} side for this conflicted file and stages it.",
                side_label(*side)
            ),
            Self::MarkResolved(_) => "Stages the file as the resolved conflict result.".into(),
            Self::Reword { commit, message } => format!(
                "Replays descendants of {} and replaces its commit message with \"{}\".",
                commit.short_id,
                message.lines().next().unwrap_or_default()
            ),
            Self::Drop(commit) => format!(
                "Interactive rebase will drop {} and replay later commits.",
                commit.short_id
            ),
            Self::Squash(commit) => format!(
                "Interactive rebase will squash {} into its parent.",
                commit.short_id
            ),
            Self::Fixup(commit) => format!(
                "Interactive rebase will fixup {} into its parent without keeping its message.",
                commit.short_id
            ),
            Self::Edit(commit) => format!(
                "Interactive rebase will stop at {} so you can amend it and continue.",
                commit.short_id
            ),
            Self::Move { commit, direction } => {
                let direction = match direction {
                    ReorderDirection::Earlier => "before its previous commit",
                    ReorderDirection::Later => "after its next commit",
                };
                format!("Interactive rebase will move {} {direction}.", commit.short_id)
            }
            Self::Undo(checkpoint) | Self::Redo(checkpoint) => format!(
                "Runs git reset --hard {}. Local unstaged work must be clean before using this safely.",
                checkpoint.head_id.chars().take(7).collect::<String>()
            ),
        }
    }

    pub(crate) fn button_label(&self) -> &'static str {
        match self {
            Self::Merge(_) => "Merge",
            Self::Rebase(_) => "Rebase",
            Self::AbortMerge | Self::AbortRebase => "Abort",
            Self::ContinueRebase => "Continue",
            Self::ResolveWithSide { .. } => "Resolve",
            Self::MarkResolved(_) => "Stage",
            Self::Reword { .. } => "Reword",
            Self::Drop(_) => "Drop",
            Self::Squash(_) => "Squash",
            Self::Fixup(_) => "Fixup",
            Self::Edit(_) => "Edit",
            Self::Move { .. } => "Move",
            Self::Undo(_) => "Undo",
            Self::Redo(_) => "Redo",
        }
    }

    pub(crate) fn success_message(&self) -> String {
        match self {
            Self::Merge(target) => format!("Merged {}", target.short_name),
            Self::Rebase(target) => format!("Rebased onto {}", target.short_name),
            Self::AbortMerge => "Merge aborted".into(),
            Self::AbortRebase => "Rebase aborted".into(),
            Self::ContinueRebase => "Rebase continued".into(),
            Self::ResolveWithSide { path, .. } => format!("Resolved {path}"),
            Self::MarkResolved(path) => format!("Staged resolved {path}"),
            Self::Reword { commit, .. } => format!("Reworded {}", commit.short_id),
            Self::Drop(commit) => format!("Dropped {}", commit.short_id),
            Self::Squash(commit) => format!("Squashed {}", commit.short_id),
            Self::Fixup(commit) => format!("Fixed up {}", commit.short_id),
            Self::Edit(commit) => format!("Stopped rebase at {}", commit.short_id),
            Self::Move { commit, .. } => format!("Moved {}", commit.short_id),
            Self::Undo(checkpoint) => format!("Undid {}", checkpoint.label),
            Self::Redo(checkpoint) => format!("Redid {}", checkpoint.label),
        }
    }

    pub(crate) fn undo_label(&self) -> Option<String> {
        match self {
            Self::Merge(target) => Some(format!("merge {}", target.short_name)),
            Self::Rebase(target) => Some(format!("rebase onto {}", target.short_name)),
            Self::Reword { commit, .. } => Some(format!("reword {}", commit.short_id)),
            Self::Drop(commit) => Some(format!("drop {}", commit.short_id)),
            Self::Squash(commit) => Some(format!("squash {}", commit.short_id)),
            Self::Fixup(commit) => Some(format!("fixup {}", commit.short_id)),
            Self::Edit(commit) => Some(format!("edit {}", commit.short_id)),
            Self::Move { commit, .. } => Some(format!("move {}", commit.short_id)),
            _ => None,
        }
    }
}

fn side_label(side: ConflictSide) -> &'static str {
    match side {
        ConflictSide::Ours => "ours",
        ConflictSide::Theirs => "theirs",
    }
}
