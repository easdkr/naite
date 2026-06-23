use iced::Task;
use naite_core::CommitOptions;

use crate::features::commit::{self, Message as CommitMessage};
use crate::state::{CommitFormState, OperationKind};
use crate::{features::repo_open, App, Message};

impl App {
    pub(crate) fn update_commit(&mut self, message: CommitMessage) -> Task<Message> {
        match message {
            CommitMessage::TitleChanged(title) => {
                self.commit_form.title = title;
                Task::none()
            }
            CommitMessage::BodyChanged(body) => {
                self.commit_form.body = body;
                Task::none()
            }
            CommitMessage::CoAuthorsChanged(co_authors) => {
                self.commit_form.co_authors = co_authors;
                Task::none()
            }
            CommitMessage::AmendChanged(amend) => {
                self.commit_form.amend = amend;
                Task::none()
            }
            CommitMessage::SkipHooksChanged(skip_hooks) => {
                self.commit_form.skip_hooks = skip_hooks;
                Task::none()
            }
            CommitMessage::PushAfterChanged(push_after) => {
                self.commit_form.push_after = push_after;
                Task::none()
            }
            CommitMessage::Requested => self.start_commit(),
            CommitMessage::Done(result) => {
                let completion = self.complete_manual_op(
                    &OperationKind::ManualAction("commit"),
                    result.as_ref().map(|_| ()).map_err(|e| e.clone()),
                );
                self.operation.loading = false;
                match result {
                    Ok(outcome) => {
                        self.commit_form = CommitFormState::default();
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.pending_transient_status_after_reload =
                                Some(commit_success_message(outcome.pushed));
                            self.operation.loading = true;
                            let reload_start = self.start_manual_op(
                                OperationKind::Custom("repo_open".to_string()),
                                "Reloading repository…".to_string(),
                            );
                            completion.chain(
                                reload_start.chain(Task::perform(
                                    repo_open::task::load(path),
                                    |result| {
                                        Message::from(repo_open::Message::Loaded(Box::new(result)))
                                    },
                                )),
                            )
                        } else {
                            completion
                        }
                    }
                    Err(msg) => {
                        self.operation.error = Some(msg);
                        completion
                    }
                }
            }
        }
    }

    pub(crate) fn start_commit(&mut self) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading
            || self.commit_form.title.trim().is_empty()
            || self.repo.status_detail.staged.is_empty()
            || (self.commit_form.push_after && self.repo.head_branch.is_none())
        {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.loading = true;
        let options = CommitOptions {
            title: self.commit_form.title.clone(),
            body: self.commit_form.body.clone(),
            co_authors: parse_co_authors(&self.commit_form.co_authors),
            amend: self.commit_form.amend,
            skip_hooks: self.commit_form.skip_hooks,
        };
        let start = self.start_manual_op(
            OperationKind::ManualAction("commit"),
            "Committing staged changes…".to_string(),
        );
        start.chain(Task::perform(
            commit::task::run(path, options, self.commit_form.push_after),
            |result| Message::from(CommitMessage::Done(result)),
        ))
    }
}

fn parse_co_authors(raw: &str) -> Vec<String> {
    raw.split(['\n', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn commit_success_message(pushed: bool) -> String {
    if pushed {
        "Committed and pushed current branch".into()
    } else {
        "Committed staged changes".into()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_co_authors;

    #[test]
    fn parse_co_authors_accepts_semicolon_and_newline_separated_values() {
        assert_eq!(
            parse_co_authors(
                "Ada <ada@example.com>; Grace <grace@example.com>\n Linus <linus@example.com> "
            ),
            vec![
                "Ada <ada@example.com>".to_string(),
                "Grace <grace@example.com>".to_string(),
                "Linus <linus@example.com>".to_string(),
            ]
        );
    }
}