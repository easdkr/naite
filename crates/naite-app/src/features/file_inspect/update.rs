use iced::Task;

use crate::features::file_inspect::{self, FileInsightResult, Message as FileInspectMessage};
use crate::state::FileInsightMode;
use crate::{App, Message};

impl App {
    pub(crate) fn update_file_inspect(&mut self, message: FileInspectMessage) -> Task<Message> {
        match message {
            FileInspectMessage::HistoryRequested(path) => {
                self.start_file_inspect(path, FileInsightMode::History)
            }
            FileInspectMessage::BlameRequested(path) => {
                self.start_file_inspect(path, FileInsightMode::Blame)
            }
            FileInspectMessage::Done { path, mode, result } => {
                if self.file_insight.path.as_deref() != Some(path.as_str())
                    || self.file_insight.mode != mode
                {
                    return Task::none();
                }
                self.file_insight.loading = false;
                match result {
                    Ok(FileInsightResult::History(history)) => {
                        self.file_insight.history = history;
                        self.file_insight.blame.clear();
                        self.file_insight.error = None;
                    }
                    Ok(FileInsightResult::Blame(blame)) => {
                        self.file_insight.blame = blame;
                        self.file_insight.history.clear();
                        self.file_insight.error = None;
                    }
                    Err(msg) => {
                        self.file_insight.error = Some(msg);
                        self.file_insight.history.clear();
                        self.file_insight.blame.clear();
                    }
                }
                Task::none()
            }
        }
    }

    fn start_file_inspect(&mut self, path: String, mode: FileInsightMode) -> Task<Message> {
        let Some(repo_path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.file_insight.path = Some(path.clone());
        self.file_insight.mode = mode;
        self.file_insight.loading = true;
        self.file_insight.error = None;
        self.file_insight.history.clear();
        self.file_insight.blame.clear();
        Task::perform(
            file_inspect::task::load(repo_path, path, mode),
            |(path, mode, result)| Message::from(FileInspectMessage::Done { path, mode, result }),
        )
    }
}
