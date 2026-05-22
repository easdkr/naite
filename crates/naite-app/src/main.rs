#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod app_icon;
mod features;
mod icons;
mod message;
mod persistence;
mod state;
mod styles;
mod subscription;
mod tasks;
mod theme;
mod update;
mod view;
mod widgets;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use iced::{window, Size, Task};
use state::ThemePreference;

pub use app::{
    App, BranchDeletePrompt, BranchDeleteTarget, CheckoutPrompt, CommandId, CommandPaletteItem,
    DiscardPrompt, DiscardTarget, ForcePushPrompt, ForceSyncPrompt, HistoryPrompt,
    LinkedWorktreeDeleteTarget, PaneId, RebasePrompt, ResetPrompt, StashPrompt, StashPromptAction,
    TagDeletePrompt, UndoPrompt, UndoPromptAction, WorktreeRemovePrompt,
};
pub use message::Message;

fn main() -> iced::Result {
    let initial_repo = initial_repository_path();
    let preferences = persistence::load_preferences().unwrap_or_default();

    iced::application("naite", App::update, App::view)
        .subscription(App::subscription)
        .theme(|app| match app.preferences.theme {
            ThemePreference::Dark => theme::naite_dark(),
            ThemePreference::HighContrast => theme::naite_high_contrast(),
        })
        .default_font(theme::font_regular())
        .window(window::Settings {
            size: Size::new(1200.0, 760.0),
            min_size: Some(Size::new(900.0, 600.0)),
            icon: app_icon::window_icon(),
            ..window::Settings::default()
        })
        .run_with(move || {
            (
                App::with_preferences(preferences),
                initial_task(initial_repo),
            )
        })
}

fn initial_repository_path() -> Option<PathBuf> {
    std::env::args_os()
        .skip(1)
        .find(|arg| !arg.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
}

fn initial_task(initial_repo: Option<PathBuf>) -> Task<Message> {
    let catalog = Task::perform(features::catalog::task::load(), |result| {
        features::catalog::Message::Loaded(result).into()
    });
    match initial_repo {
        Some(path) => Task::batch([
            catalog,
            Task::perform(features::repo_open::task::load(path), |result| {
                features::repo_open::Message::Loaded(Box::new(result)).into()
            }),
        ]),
        None => {
            let tabs = Task::perform(update::load_open_tabs_task(), |result| {
                message::TabsMessage::Restored(result).into()
            });
            Task::batch([catalog, tabs])
        }
    }
}
