use std::collections::HashMap;

use iced::Point;
use naite_core::{HistoryCommit, RebaseAction, RefSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseApplyMode {
    RebaseOnly,
    RebaseThenForcePush,
    ReleasePromotionAuto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebasePlanPreset {
    KeepMine,
    SquashMine,
    SquashAll,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractiveRebaseSession {
    pub current_branch: RefSummary,
    pub target: RefSummary,
    pub current_author_email: Option<String>,
    pub plan: Vec<RebasePlanRow>,
    pub selected: usize,
    pub drag: Option<DragState>,
    pub reword_drafts: HashMap<String, String>,
    pub applying: bool,
    pub scroll_offset: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePlanRow {
    pub action: RebaseAction,
    pub commit: HistoryCommit,
    /// Author avatar resolved after the session loads: from the persisted
    /// avatar cache, the loaded commit list, a noreply-email fallback, or a
    /// GitHub GraphQL lookup for commits none of those cover.
    pub author_avatar_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragState {
    pub source_index: usize,
    pub hover_index: usize,
    pub press_origin: Point,
    pub started: bool,
}

impl InteractiveRebaseSession {
    pub fn selected_row(&self) -> Option<&RebasePlanRow> {
        self.plan.get(self.selected)
    }
}
