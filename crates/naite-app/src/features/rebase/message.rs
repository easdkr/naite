use naite_core::{RebaseAction, RefSummary};

use super::state::{RebaseApplyMode, RebasePlanPreset, RebasePlanRow};
use super::task::ApplyOutcome;

#[derive(Debug, Clone)]
pub enum Message {
    Started(RefSummary),
    Loaded {
        target: RefSummary,
        plan: Vec<RebasePlanRow>,
        current_branch: RefSummary,
        current_author_email: Option<String>,
    },
    LoadFailed(String),
    Cancelled,
    RowSelected(usize),
    RowSelectedRelative(isize),
    ActionSet(usize, RebaseAction),
    ActionSetSelected(RebaseAction),
    MoveUp(usize),
    MoveDown(usize),
    MoveSelected(isize),
    DragPressed(usize),
    DragEnded,
    DragCancelled,
    EscapePressed,
    Scrolled(f32),
    RewordOpened(usize),
    RewordChanged(usize, String),
    RewordCommitted(usize),
    PickMineRequested,
    PresetRequested(RebasePlanPreset),
    ApplyRequested(RebaseApplyMode),
    ApplyCancelled,
    ApplyConfirmed,
    Done {
        result: Result<ApplyOutcome, String>,
        apply_mode: RebaseApplyMode,
    },
}
