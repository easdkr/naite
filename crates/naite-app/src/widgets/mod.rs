//! Styled widget builders. Each panel lives in its own submodule so any one
//! file fits a single read; cross-panel primitives live in `common`.

mod command_palette;
mod commit_list;
pub mod common;
mod context_menu;
mod detail_pane;
mod forms;
mod modal;
mod pills;
mod preferences;
mod progress_overlay;
mod prompts;
mod rebase_editor;
mod release_prep;
mod repo_manager;
mod reset_prompt;
mod sidebar;
mod status;
mod status_bar;
mod tab_strip;
mod terminal;
pub mod toast;
mod toolbar;
mod workspace;

pub use command_palette::command_palette_overlay;
pub use commit_list::{commit_list, CommitListProps};
pub use common::{
    animated_dots, ease_in_out_sine, moving_progress_bar, spinner_frame, ErrorRecovery,
};
pub use context_menu::floating_context_menu;
pub use detail_pane::{detail_pane, DetailPaneProps};
pub use forms::{
    branch_create_prompt, branch_rename_prompt, history_reword_prompt, pull_request_create_prompt,
    pull_request_worktree_prompt, stash_branch_prompt, stash_create_prompt, tag_create_prompt,
    worktree_create_prompt,
};
pub use modal::{animated_modal, modal, wide_modal};
pub use preferences::{display_options_panel, shortcut_help_overlay};
pub use progress_overlay::progress_overlay;
pub use prompts::{
    branch_delete_prompt, checkout_prompt, discard_prompt, force_push_prompt, force_sync_prompt,
    history_prompt, rebase_prompt, stash_prompt, tag_delete_prompt, undo_prompt,
    worktree_remove_prompt,
};
pub use rebase_editor::{rebase_detail, rebase_editor, RebaseDetailProps};
pub use release_prep::{release_prep_actions, release_prep_config, release_prep_progress};
pub use reset_prompt::reset_prompt;
#[cfg(test)]
pub(crate) use sidebar::is_checkout_supported as sidebar_ref_checkout_supported;
pub use sidebar::{sidebar, SidebarProps};
pub use status_bar::{bottom_status_bar, top_status_bar};
#[cfg(test)]
pub(crate) use terminal::split_ime_preedit_at_cursor as terminal_split_ime_preedit_at_cursor;
pub use terminal::{
    panel_chrome, terminal_panel, TERMINAL_CHAR_WIDTH, TERMINAL_LINE_HEIGHT, TERMINAL_PANEL_HEIGHT,
    TERMINAL_PANEL_HEIGHT_MINIMIZED,
};
pub use toast::{toast_layer, Toast, ToastSeverity, MAX_VISIBLE as MAX_TOASTS_VISIBLE};
pub use toolbar::{toolbar, ToolbarProps};
pub use workspace::workspace_dashboard;

/// Height of one commit row. Kept here so the graph canvas, the row content,
/// and the scroll-into-view math in `update.rs` all agree — adjacent canvases
/// only render a continuous graph when this exactly matches the rendered row.
pub const ROW_HEIGHT: f32 = 32.0;
