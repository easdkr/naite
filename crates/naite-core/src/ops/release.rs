use std::time::{SystemTime, UNIX_EPOCH};

use crate::repo::Repository;
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseProfile {
    pub remote: String,
    pub source_branch: String,
    pub target_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseProfileSuggestion {
    pub remotes: Vec<String>,
    pub source_candidates: Vec<String>,
    pub target_candidates: Vec<String>,
    pub default_profile: ReleaseProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseSyncCheck {
    pub profile: ReleaseProfile,
    pub source: ReleaseBranchSync,
    pub target: ReleaseBranchSync,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseBranchSync {
    pub branch: String,
    pub local_ref: String,
    pub remote_ref: String,
    pub local_oid: Option<String>,
    pub remote_oid: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

impl ReleaseSyncCheck {
    pub fn is_ready(&self) -> bool {
        self.source.is_ready() && self.target.is_ready()
    }
}

impl ReleaseBranchSync {
    pub fn is_ready(&self) -> bool {
        self.local_oid.is_some()
            && self.remote_oid.is_some()
            && self.local_oid == self.remote_oid
            && self.ahead == 0
            && self.behind == 0
    }
}

impl Repository {
    pub fn suggest_release_profile(&self) -> Result<ReleaseProfileSuggestion, Error> {
        let remotes = self.remote_names()?;
        let remote = remotes
            .iter()
            .find(|remote| remote.as_str() == "origin")
            .or_else(|| remotes.first())
            .cloned()
            .unwrap_or_else(|| "origin".to_string());

        let target_candidates = release_branch_candidates(
            self,
            &remote,
            remote_head_target(self, &remote)?
                .into_iter()
                .chain(["main".to_string(), "master".to_string()]),
        )?;
        let source_candidates = release_branch_candidates(
            self,
            &remote,
            ["staging", "develop", "development"]
                .into_iter()
                .map(str::to_string),
        )?;

        let target_branch = target_candidates
            .first()
            .cloned()
            .unwrap_or_else(|| "main".to_string());
        let source_branch = source_candidates
            .first()
            .cloned()
            .unwrap_or_else(|| "staging".to_string());

        Ok(ReleaseProfileSuggestion {
            remotes,
            source_candidates,
            target_candidates,
            default_profile: ReleaseProfile {
                remote,
                source_branch,
                target_branch,
            },
        })
    }

    pub fn fetch_release_remote(&self, remote: &str) -> Result<(), Error> {
        let remote = validate_ref_component(remote)?;
        let _ = self.git(&["fetch", remote])?;
        Ok(())
    }

    pub fn check_release_sync(&self, profile: &ReleaseProfile) -> Result<ReleaseSyncCheck, Error> {
        validate_release_profile(profile)?;
        Ok(ReleaseSyncCheck {
            profile: profile.clone(),
            source: self.branch_sync_against_remote(&profile.remote, &profile.source_branch)?,
            target: self.branch_sync_against_remote(&profile.remote, &profile.target_branch)?,
        })
    }

    pub fn sync_release_branches_with_remote(&self, profile: &ReleaseProfile) -> Result<(), Error> {
        validate_release_profile(profile)?;
        if self.status()?.is_dirty() {
            return Err(Error::DirtyWorkdir);
        }

        for branch in [&profile.target_branch, &profile.source_branch] {
            self.sync_release_branch_with_remote(&profile.remote, branch)?;
        }
        Ok(())
    }

    pub fn checkout_release_source(&self, profile: &ReleaseProfile) -> Result<(), Error> {
        validate_release_profile(profile)?;
        if self.status()?.is_dirty() {
            return Err(Error::DirtyWorkdir);
        }
        let _ = self.git(&[
            "show-ref",
            "--verify",
            &format!("refs/heads/{}", profile.source_branch),
        ])?;
        let _ = self.git(&["checkout", &profile.source_branch])?;
        Ok(())
    }

    pub fn create_release_backup_branch(&self, profile: &ReleaseProfile) -> Result<String, Error> {
        validate_release_profile(profile)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        let source = sanitize_branch_component(&profile.source_branch);
        let name = format!("naite/release-prep/{source}-{timestamp}");
        let _ = self.git(&["branch", &name, "HEAD"])?;
        Ok(name)
    }

    pub fn fast_forward_release_target(&self, profile: &ReleaseProfile) -> Result<(), Error> {
        validate_release_profile(profile)?;
        if self.status()?.is_dirty() {
            return Err(Error::DirtyWorkdir);
        }
        let _ = self.git(&["checkout", &profile.target_branch])?;
        let _ = self.git(&["merge", "--ff-only", &profile.source_branch])?;
        Ok(())
    }

    pub fn push_release_target(&self, profile: &ReleaseProfile) -> Result<(), Error> {
        validate_release_profile(profile)?;
        let _ = self.git(&["push", &profile.remote, &profile.target_branch])?;
        Ok(())
    }

    pub fn sync_release_source_from_target(&self, profile: &ReleaseProfile) -> Result<(), Error> {
        validate_release_profile(profile)?;
        if self.status()?.is_dirty() {
            return Err(Error::DirtyWorkdir);
        }
        let remote = validate_ref_component(&profile.remote)?;
        let remote_source = format!("{remote}/{}", profile.source_branch);
        let _ = self.git(&["fetch", remote])?;
        let _ = self.git(&["checkout", &profile.source_branch])?;
        let _ = self.git(&["reset", "--hard", &remote_source])?;
        let _ = self.git(&["rebase", &profile.target_branch])?;
        let _ = self.git(&[
            "push",
            "--force-with-lease",
            &profile.remote,
            &profile.source_branch,
        ])?;
        Ok(())
    }

    fn remote_names(&self) -> Result<Vec<String>, Error> {
        let output = self.git_allowing_exit_codes(&["remote"], &[1])?;
        Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    }

    fn branch_sync_against_remote(
        &self,
        remote: &str,
        branch: &str,
    ) -> Result<ReleaseBranchSync, Error> {
        let local_ref = format!("refs/heads/{branch}");
        let remote_ref = format!("refs/remotes/{remote}/{branch}");
        let local_oid = self.rev_parse_ref(&local_ref)?;
        let remote_oid = self.rev_parse_ref(&remote_ref)?;
        let (ahead, behind) = match (&local_oid, &remote_oid) {
            (Some(_), Some(_)) => self.rev_list_ahead_behind(&local_ref, &remote_ref)?,
            _ => (0, 0),
        };

        Ok(ReleaseBranchSync {
            branch: branch.to_string(),
            local_ref,
            remote_ref,
            local_oid,
            remote_oid,
            ahead,
            behind,
        })
    }

    fn sync_release_branch_with_remote(&self, remote: &str, branch: &str) -> Result<(), Error> {
        let sync = self.branch_sync_against_remote(remote, branch)?;
        if sync.is_ready() {
            return Ok(());
        }

        // Reuse the same force-sync operation exposed elsewhere in the app:
        // it checks out or creates the matching local branch, hard-resets it
        // to the remote tracking ref, and sets upstream metadata.
        self.force_sync_remote_branch(&sync.remote_ref)
    }

    fn rev_parse_ref(&self, ref_name: &str) -> Result<Option<String>, Error> {
        let output =
            self.git_allowing_exit_codes(&["rev-parse", "--verify", ref_name], &[1, 128])?;
        let oid = output.trim();
        Ok((!oid.is_empty()).then(|| oid.to_string()))
    }

    fn rev_list_ahead_behind(&self, left: &str, right: &str) -> Result<(u32, u32), Error> {
        let range = format!("{left}...{right}");
        let output = self.git(&["rev-list", "--left-right", "--count", &range])?;
        let mut parts = output.split_whitespace();
        let ahead = parts
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let behind = parts
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        Ok((ahead, behind))
    }
}

fn release_branch_candidates(
    repo: &Repository,
    remote: &str,
    candidates: impl Iterator<Item = String>,
) -> Result<Vec<String>, Error> {
    let mut unique = Vec::new();
    for candidate in candidates {
        if unique.iter().any(|existing| existing == &candidate) {
            continue;
        }
        if branch_exists(repo, remote, &candidate)? {
            unique.push(candidate);
        }
    }
    Ok(unique)
}

fn remote_head_target(repo: &Repository, remote: &str) -> Result<Option<String>, Error> {
    let ref_name = format!("refs/remotes/{remote}/HEAD");
    let output = repo.git_allowing_exit_codes(
        &["symbolic-ref", "--quiet", "--short", &ref_name],
        &[1, 128],
    )?;
    let short = output.trim();
    Ok(short
        .strip_prefix(&format!("{remote}/"))
        .filter(|branch| !branch.is_empty())
        .map(str::to_string))
}

fn branch_exists(repo: &Repository, remote: &str, branch: &str) -> Result<bool, Error> {
    let local_ref = format!("refs/heads/{branch}");
    let remote_ref = format!("refs/remotes/{remote}/{branch}");
    let local = repo.git_allowing_exit_codes(&["show-ref", "--verify", &local_ref], &[1, 128])?;
    if !local.trim().is_empty() {
        return Ok(true);
    }
    let remote = repo.git_allowing_exit_codes(&["show-ref", "--verify", &remote_ref], &[1, 128])?;
    Ok(!remote.trim().is_empty())
}

fn validate_release_profile(profile: &ReleaseProfile) -> Result<(), Error> {
    validate_ref_component(&profile.remote)?;
    validate_branch_name(&profile.source_branch)?;
    validate_branch_name(&profile.target_branch)?;
    if profile.source_branch == profile.target_branch {
        return Err(Error::InvalidRefName(profile.source_branch.clone()));
    }
    Ok(())
}

fn validate_ref_component(value: &str) -> Result<&str, Error> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('-')
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || value.contains(char::is_whitespace)
    {
        return Err(Error::InvalidRefName(value.to_string()));
    }
    Ok(value)
}

fn validate_branch_name(value: &str) -> Result<&str, Error> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') {
        return Err(Error::InvalidRefName(value.to_string()));
    }
    Ok(value)
}

fn sanitize_branch_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::process::Command;

    #[test]
    fn suggests_release_profile_candidates_from_remote_refs() {
        let remote = TempRepo::new("release-profile-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("release-profile-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        source.git(&["checkout", "-b", "staging"]);
        source.write("staging.txt", "staging\n");
        source.git(&["add", "staging.txt"]);
        source.git(&["commit", "-m", "staging"]);
        source.git(&["push", "-u", "origin", "staging"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let parent = TempRepo::new("release-profile-local-parent");
        let local_path = clone_main(&remote, &parent);
        let repo = Repository::open(&local_path).unwrap();

        let suggestion = repo.suggest_release_profile().unwrap();

        assert_eq!(suggestion.default_profile.remote, "origin");
        assert_eq!(suggestion.default_profile.target_branch, "main");
        assert_eq!(suggestion.default_profile.source_branch, "staging");
        assert!(suggestion.target_candidates.contains(&"main".into()));
        assert!(suggestion.source_candidates.contains(&"staging".into()));
    }

    #[test]
    fn release_sync_requires_local_and_remote_tips_to_match() {
        let remote = TempRepo::new("release-sync-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("release-sync-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        source.git(&["checkout", "-b", "staging"]);
        source.write("staging.txt", "staging\n");
        source.git(&["add", "staging.txt"]);
        source.git(&["commit", "-m", "staging"]);
        source.git(&["push", "-u", "origin", "staging"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let parent = TempRepo::new("release-sync-local-parent");
        let local_path = parent.path.join("local");
        let output = Command::new("git")
            .args([
                "clone",
                "--branch",
                "staging",
                remote.path.to_str().unwrap(),
                local_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        crate::command::run_git(&local_path, ["checkout", "main"]).unwrap();
        crate::command::run_git(&local_path, ["checkout", "staging"]).unwrap();

        let repo = Repository::open(&local_path).unwrap();
        let profile = ReleaseProfile {
            remote: "origin".into(),
            source_branch: "staging".into(),
            target_branch: "main".into(),
        };

        assert!(repo.check_release_sync(&profile).unwrap().is_ready());

        source.write("remote.txt", "remote\n");
        source.git(&["add", "remote.txt"]);
        source.git(&["commit", "-m", "remote"]);
        source.git(&["push", "origin", "staging"]);
        repo.fetch_release_remote("origin").unwrap();

        let sync = repo.check_release_sync(&profile).unwrap();
        assert!(!sync.is_ready());
        assert_eq!(sync.source.behind, 1);
    }

    #[test]
    fn release_sync_uses_existing_force_sync_to_match_remote_refs() {
        let remote = TempRepo::new("release-auto-sync-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("release-auto-sync-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        source.git(&["checkout", "-b", "staging"]);
        source.write("staging.txt", "staging\n");
        source.git(&["add", "staging.txt"]);
        source.git(&["commit", "-m", "staging"]);
        source.git(&["push", "-u", "origin", "staging"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let parent = TempRepo::new("release-auto-sync-local-parent");
        let local_path = parent.path.join("local");
        let output = Command::new("git")
            .args([
                "clone",
                "--branch",
                "staging",
                remote.path.to_str().unwrap(),
                local_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        crate::command::run_git(&local_path, ["config", "user.name", "naite test"]).unwrap();
        crate::command::run_git(&local_path, ["config", "user.email", "naite@example.com"])
            .unwrap();
        crate::command::run_git(&local_path, ["checkout", "main"]).unwrap();
        crate::command::run_git(&local_path, ["checkout", "staging"]).unwrap();

        std::fs::write(local_path.join("local-only.txt"), "local\n").unwrap();
        crate::command::run_git(&local_path, ["add", "local-only.txt"]).unwrap();
        crate::command::run_git(&local_path, ["commit", "-m", "local only"]).unwrap();
        source.git(&["checkout", "main"]);
        source.write("main-remote.txt", "main remote\n");
        source.git(&["add", "main-remote.txt"]);
        source.git(&["commit", "-m", "main remote"]);
        source.git(&["push", "origin", "main"]);
        source.git(&["checkout", "staging"]);
        source.write("staging-remote.txt", "staging remote\n");
        source.git(&["add", "staging-remote.txt"]);
        source.git(&["commit", "-m", "staging remote"]);
        source.git(&["push", "origin", "staging"]);

        let repo = Repository::open(&local_path).unwrap();
        let profile = ReleaseProfile {
            remote: "origin".into(),
            source_branch: "staging".into(),
            target_branch: "main".into(),
        };
        repo.fetch_release_remote("origin").unwrap();

        repo.sync_release_branches_with_remote(&profile).unwrap();

        assert!(repo.check_release_sync(&profile).unwrap().is_ready());
        assert_eq!(
            crate::command::run_git(&local_path, ["rev-parse", "staging"])
                .unwrap()
                .trim(),
            crate::command::run_git(&local_path, ["rev-parse", "origin/staging"])
                .unwrap()
                .trim()
        );
        assert_eq!(
            crate::command::run_git(&local_path, ["rev-parse", "main"])
                .unwrap()
                .trim(),
            crate::command::run_git(&local_path, ["rev-parse", "origin/main"])
                .unwrap()
                .trim()
        );
    }

    #[test]
    fn release_source_sync_rebases_remote_source_onto_target_before_force_push() {
        let remote = TempRepo::new("release-force-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("release-force-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        source.git(&["checkout", "-b", "staging"]);
        source.write("staging.txt", "staging\n");
        source.git(&["add", "staging.txt"]);
        source.git(&["commit", "-m", "release-ready"]);
        let release_ready = source.git_output(&["rev-parse", "HEAD"]);
        source.write("pending.txt", "pending\n");
        source.git(&["add", "pending.txt"]);
        source.git(&["commit", "-m", "pending"]);
        source.git(&["push", "-u", "origin", "staging"]);

        let repo = Repository::open(&source.path).unwrap();
        let profile = ReleaseProfile {
            remote: "origin".into(),
            source_branch: "staging".into(),
            target_branch: "main".into(),
        };

        // Simulate the release review dropping the pending commit locally before
        // updating and pushing the target branch.
        source.git(&["reset", "--hard", release_ready.trim()]);
        source.git(&["checkout", "main"]);
        source.git(&["merge", "--ff-only", "staging"]);
        source.git(&["push", "origin", "main"]);
        repo.sync_release_source_from_target(&profile).unwrap();

        let staging = remote.git_output(&["rev-parse", "refs/heads/staging"]);
        let main = remote.git_output(&["rev-parse", "refs/heads/main"]);
        assert_ne!(staging.trim(), main.trim());
        let pending_parent = remote.git_output(&["rev-parse", "refs/heads/staging^"]);
        assert_eq!(pending_parent.trim(), main.trim());
        assert_eq!(
            remote
                .git_output(&["log", "-1", "--format=%s", "refs/heads/staging"])
                .trim(),
            "pending"
        );
    }
}
