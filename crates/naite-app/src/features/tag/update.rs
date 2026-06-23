use std::time::{SystemTime, UNIX_EPOCH};

use iced::widget::text_input;
use iced::Task;
use naite_core::{CommitSummary, RefKind, RefSummary};

use crate::features::repo_open;
use crate::features::tag::{self, Message as TagMessage, Operation};
use crate::state::{OperationKind, TagCreateState, TagNameMode};
use crate::{App, Message, TagDeletePrompt};

impl App {
    pub(crate) fn update_tag(&mut self, message: TagMessage) -> Task<Message> {
        match message {
            TagMessage::CreateRequested(target_commit) => self.open_tag_create_form(target_commit),
            TagMessage::CreateAndPushRequested(target_commit) => {
                self.open_tag_create_form_with_options(target_commit, true)
            }
            TagMessage::CreateNameChanged(name) => {
                self.tag_create.name = name;
                Task::none()
            }
            TagMessage::CreateNameModeChanged(mode) => {
                let suggested = self.suggest_unique_tag_name(mode);
                self.tag_create.name_mode = mode;
                self.tag_create.name = suggested;
                Task::none()
            }
            TagMessage::CreatePushAfterChanged(push_after_create) => {
                self.tag_create.push_after_create = push_after_create;
                Task::none()
            }
            TagMessage::CreateCancelled => {
                self.tag_create = TagCreateState::default();
                Task::none()
            }
            TagMessage::CreateSubmitted => self.start_tag_create(),
            TagMessage::DeleteRequested(target) => self.open_tag_delete_prompt(target),
            TagMessage::DeleteCancelled => {
                self.selection.tag_delete_confirmation = None;
                Task::none()
            }
            TagMessage::DeleteConfirmed => {
                let Some(prompt) = self.selection.tag_delete_confirmation.take() else {
                    return Task::none();
                };
                self.start_tag_operation(Operation::Delete(prompt.target))
            }
            TagMessage::Done { operation, result } => {
                let completion = self.complete_manual_op(
                    &OperationKind::ManualAction("tag"),
                    result.as_ref().map(|_| ()).map_err(|e| e.clone()),
                );
                self.operation.loading = false;
                match result {
                    Ok(()) => {
                        self.tag_create = TagCreateState::default();
                        self.selection.tag_delete_confirmation = None;
                        self.selection.context_menu = None;
                        let status_message = operation.success_message();
                        if let Some(path) = self.repo.path.clone() {
                            self.operation.pending_transient_status_after_reload =
                                Some(status_message);
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
                            self.set_transient_status(status_message);
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

    pub(crate) fn open_tag_create_form(
        &mut self,
        target_commit: Option<CommitSummary>,
    ) -> Task<Message> {
        self.open_tag_create_form_with_options(target_commit, false)
    }

    pub(crate) fn open_tag_create_form_with_options(
        &mut self,
        target_commit: Option<CommitSummary>,
        push_after_create: bool,
    ) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading {
            return Task::none();
        }
        let name_mode = TagNameMode::default();
        self.selection.context_menu = None;
        self.tag_create = TagCreateState {
            open: true,
            target_commit,
            name: self.suggest_unique_tag_name(name_mode),
            name_mode,
            push_after_create,
        };
        text_input::focus(self.tag_create_input_id.clone())
    }

    pub(crate) fn open_tag_delete_prompt(&mut self, target: RefSummary) -> Task<Message> {
        if self.repo.path.is_none() || self.operation.loading || target.kind != RefKind::Tag {
            return Task::none();
        }
        self.selection.context_menu = None;
        self.selection.tag_delete_confirmation = Some(TagDeletePrompt { target });
        Task::none()
    }

    pub(crate) fn start_tag_create(&mut self) -> Task<Message> {
        if self.operation.loading || self.tag_create.name.trim().is_empty() {
            return Task::none();
        }
        self.start_tag_operation(Operation::Create {
            name: self.tag_create.name.clone(),
            push_after_create: self.tag_create.push_after_create,
            target_commit: self.tag_create.target_commit.clone(),
        })
    }

    fn start_tag_operation(&mut self, operation: Operation) -> Task<Message> {
        let Some(path) = self.repo.path.clone() else {
            return Task::none();
        };
        if self.operation.loading {
            return Task::none();
        }

        self.operation.error = None;
        self.operation.loading = true;
        let operation_for_message = operation.clone();
        let label = match &operation {
            Operation::Create { name, .. } => format!("Creating tag {name}…"),
            Operation::Delete(target) => format!("Deleting tag {}…", target.short_name),
        };
        let start = self.start_manual_op(OperationKind::ManualAction("tag"), label);
        start.chain(Task::perform(tag::task::run(path, operation), move |result| {
            Message::from(TagMessage::Done {
                operation: operation_for_message.clone(),
                result,
            })
        }))
    }

    pub(crate) fn suggest_unique_tag_name(&self, mode: TagNameMode) -> String {
        let base = match mode {
            TagNameMode::Timestamp => timestamp_tag_name(),
            TagNameMode::SemVerNext => self.next_semver_tag_name(),
            TagNameMode::BranchSlug => self
                .repo
                .head_branch
                .as_deref()
                .and_then(branch_slug_tag_name)
                .unwrap_or_else(timestamp_tag_name),
        };
        self.first_available_tag_name(&base)
    }

    fn next_semver_tag_name(&self) -> String {
        let next = self
            .repo
            .refs
            .tags
            .iter()
            .filter_map(|tag| parse_v_semver(&tag.short_name))
            .max()
            .map(|(major, minor, patch)| (major, minor, patch.saturating_add(1)))
            .unwrap_or((0, 1, 0));
        format!("v{}.{}.{}", next.0, next.1, next.2)
    }

    fn first_available_tag_name(&self, base: &str) -> String {
        if !self.tag_exists(base) {
            return base.to_string();
        }

        for suffix in 2.. {
            let candidate = format!("{base}-{suffix}");
            if !self.tag_exists(&candidate) {
                return candidate;
            }
        }
        unreachable!("unbounded suffix search should always find a free tag name")
    }

    fn tag_exists(&self, name: &str) -> bool {
        self.repo.refs.tags.iter().any(|tag| tag.short_name == name)
    }
}

fn timestamp_tag_name() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day) = unix_secs_to_utc_date(secs);
    format!("v{year}.{month}.{day}")
}

fn unix_secs_to_utc_date(secs: u64) -> (i32, u32, u32) {
    civil_from_days((secs / 86_400) as i64)
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn parse_v_semver(name: &str) -> Option<(u64, u64, u64)> {
    let version = name.strip_prefix('v')?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() || looks_like_date_tag(major, minor, patch) {
        None
    } else {
        Some((major, minor, patch))
    }
}

fn looks_like_date_tag(major: u64, minor: u64, patch: u64) -> bool {
    (1970..=9999).contains(&major) && (1..=12).contains(&minor) && (1..=31).contains(&patch)
}

fn branch_slug_tag_name(branch: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in branch.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_was_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_was_dash {
            last_was_dash = true;
            Some('-')
        } else {
            None
        };
        if let Some(ch) = next {
            slug.push(ch);
        }
    }

    let slug = slug.trim_matches('-').to_string();
    (!slug.is_empty()).then_some(slug)
}

#[cfg(test)]
mod tests {
    use super::{branch_slug_tag_name, parse_v_semver, unix_secs_to_utc_date};

    #[test]
    fn parse_v_semver_accepts_exact_v_major_minor_patch() {
        assert_eq!(parse_v_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_v_semver("1.2.3"), None);
        assert_eq!(parse_v_semver("v1.2"), None);
        assert_eq!(parse_v_semver("v1.2.3-2"), None);
        assert_eq!(parse_v_semver("v2026.3.30"), None);
    }

    #[test]
    fn branch_slug_tag_name_normalizes_unsafe_characters() {
        assert_eq!(
            branch_slug_tag_name("Feature/JIRA-123 Add tag UX"),
            Some("feature-jira-123-add-tag-ux".into())
        );
        assert_eq!(branch_slug_tag_name("///"), None);
    }

    #[test]
    fn unix_secs_to_utc_date_converts_calendar_dates() {
        assert_eq!(unix_secs_to_utc_date(0), (1970, 1, 1));
        assert_eq!(unix_secs_to_utc_date(1_779_148_800), (2026, 5, 19));
    }
}