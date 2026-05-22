use crate::repo::Repository;
use crate::Error;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

/// UI-friendly view of a single ref (branch or tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSummary {
    pub kind: RefKind,
    /// Short, human-readable name. `main`, `origin/main`, `v1.0`.
    pub short_name: String,
    /// Full ref name as stored in the repository. `refs/heads/main` etc.
    pub full_name: String,
    /// 7-char short id of the commit this ref points to. Empty if the
    /// ref could not be peeled (rare: dangling tag etc.).
    pub target_short_id: String,
    /// True iff HEAD is symbolically pointing at this ref.
    pub is_head: bool,
    /// Local branch sync state against its configured upstream. Remote refs and
    /// tags do not carry this metadata.
    pub sync_status: Option<BranchSyncStatus>,
}

/// Refs grouped for sidebar consumption.
#[derive(Debug, Clone, Default)]
pub struct Refs {
    pub local: Vec<RefSummary>,
    pub remote: Vec<RefSummary>,
    pub tags: Vec<RefSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchSyncStatus {
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

impl Repository {
    /// Short name of the branch HEAD currently points at, or `None` if HEAD
    /// is detached or the repository has no HEAD yet.
    pub fn head_branch(&self) -> Option<String> {
        let head_name = self.inner.head_name().ok().flatten()?;
        let full = head_name.as_bstr().to_string();
        Some(strip_ref_prefix(&full).to_string())
    }

    /// All branches and tags grouped for sidebar display. Refs are sorted
    /// alphabetically within each group; the HEAD branch is hoisted to the
    /// front of the local group.
    pub fn refs(&self) -> Result<Refs, Error> {
        let head_full = self
            .inner
            .head_name()
            .ok()
            .flatten()
            .map(|n| n.as_bstr().to_string());
        let local_sync_statuses = self.local_branch_sync_statuses()?;

        let platform = self
            .inner
            .references()
            .map_err(|e| Error::Walk(Box::new(e)))?;
        let head_full_ref = head_full.as_deref();

        let mut local = Vec::new();
        for r in platform
            .local_branches()
            .map_err(|e| Error::Walk(Box::new(e)))?
        {
            let r = r.map_err(Error::Walk)?;
            local.push(ref_to_summary(
                r,
                RefKind::LocalBranch,
                head_full_ref,
                &local_sync_statuses,
            )?);
        }

        let mut remote = Vec::new();
        for r in platform
            .remote_branches()
            .map_err(|e| Error::Walk(Box::new(e)))?
        {
            let r = r.map_err(Error::Walk)?;
            remote.push(ref_to_summary(
                r,
                RefKind::RemoteBranch,
                head_full_ref,
                &HashMap::new(),
            )?);
        }

        let mut tags = Vec::new();
        for r in platform.tags().map_err(|e| Error::Walk(Box::new(e)))? {
            let r = r.map_err(Error::Walk)?;
            tags.push(ref_to_summary(
                r,
                RefKind::Tag,
                head_full_ref,
                &HashMap::new(),
            )?);
        }

        local.sort_by(|a, b| match (a.is_head, b.is_head) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.short_name.cmp(&b.short_name),
        });
        remote.sort_by(|a, b| a.short_name.cmp(&b.short_name));
        tags.sort_by(|a, b| b.short_name.cmp(&a.short_name));

        Ok(Refs {
            local,
            remote,
            tags,
        })
    }

    pub fn branch_sync_status(&self) -> Result<BranchSyncStatus, Error> {
        let Some(upstream) = self.current_upstream()? else {
            return Ok(BranchSyncStatus::default());
        };

        let output = self.git(&["rev-list", "--left-right", "--count", "@{upstream}...HEAD"])?;
        let (ahead, behind) = parse_rev_list_left_right_count(&output)?;

        Ok(BranchSyncStatus {
            upstream: Some(upstream),
            ahead,
            behind,
        })
    }

    pub(crate) fn current_upstream(&self) -> Result<Option<String>, Error> {
        if self.head_branch().is_none() {
            return Ok(None);
        }

        let output = self.git_allowing_exit_codes(
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
            &[128],
        )?;
        let upstream = output.trim();

        if upstream.is_empty() {
            Ok(None)
        } else {
            Ok(Some(upstream.to_string()))
        }
    }

    fn local_branch_sync_statuses(&self) -> Result<HashMap<String, BranchSyncStatus>, Error> {
        let output = self.git(&[
            "for-each-ref",
            "--format=%(refname)%00%(upstream:short)%00%(upstream:track)",
            "refs/heads",
        ])?;
        let mut statuses = HashMap::new();

        for line in output.lines() {
            let mut fields = line.split('\0');
            let Some(full_name) = fields.next().filter(|value| !value.is_empty()) else {
                continue;
            };
            let upstream = fields.next().unwrap_or_default().trim();
            let track = fields.next().unwrap_or_default().trim();

            if upstream.is_empty() {
                continue;
            }

            if let Some((ahead, behind)) = parse_upstream_track(track) {
                statuses.insert(
                    full_name.to_string(),
                    BranchSyncStatus {
                        upstream: Some(upstream.to_string()),
                        ahead,
                        behind,
                    },
                );
            }
        }

        Ok(statuses)
    }
}

fn parse_upstream_track(track: &str) -> Option<(u32, u32)> {
    let track = track.trim();
    if track.is_empty() {
        return Some((0, 0));
    }

    let inner = track.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty() {
        return Some((0, 0));
    }
    if inner == "gone" {
        return None;
    }

    let mut ahead = 0;
    let mut behind = 0;
    let mut parsed_any = false;

    for part in inner.split(',') {
        let mut words = part.split_whitespace();
        let direction = words.next()?;
        let count = words.next()?.parse::<u32>().ok()?;
        if words.next().is_some() {
            return None;
        }

        match direction {
            "ahead" => ahead = count,
            "behind" => behind = count,
            _ => return None,
        }
        parsed_any = true;
    }

    parsed_any.then_some((ahead, behind))
}

fn parse_rev_list_left_right_count(output: &str) -> Result<(u32, u32), Error> {
    let mut parts = output.split_whitespace();
    let Some(behind) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return Err(invalid_sync_status(output));
    };
    let Some(ahead) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return Err(invalid_sync_status(output));
    };

    if parts.next().is_some() {
        return Err(invalid_sync_status(output));
    }

    Ok((ahead, behind))
}

fn invalid_sync_status(output: &str) -> Error {
    Error::GitCommand {
        command: "git rev-list --left-right --count @{upstream}...HEAD".into(),
        stderr: format!("unexpected ahead/behind output: {}", output.trim()),
    }
}

fn ref_to_summary(
    mut r: gix::Reference<'_>,
    kind: RefKind,
    head_full: Option<&str>,
    sync_statuses: &HashMap<String, BranchSyncStatus>,
) -> Result<RefSummary, Error> {
    let full_name = r.name().as_bstr().to_string();
    let short_name = strip_ref_prefix(&full_name).to_string();
    let is_head = head_full.map(|h| h == full_name).unwrap_or(false);
    let sync_status = sync_statuses.get(&full_name).cloned();

    let target_short_id = match r.peel_to_id_in_place() {
        Ok(id) => id.to_hex_with_len(7).to_string(),
        Err(_) => String::new(),
    };

    Ok(RefSummary {
        kind,
        short_name,
        full_name,
        target_short_id,
        is_head,
        sync_status,
    })
}

/// Strip the conventional `refs/heads/`, `refs/remotes/`, `refs/tags/` prefix
/// from a full ref name. Returns the original string if no prefix matched.
fn strip_ref_prefix(full: &str) -> &str {
    for prefix in ["refs/heads/", "refs/remotes/", "refs/tags/"] {
        if let Some(rest) = full.strip_prefix(prefix) {
            return rest;
        }
    }
    full
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn parse_rev_list_left_right_count_maps_upstream_left_to_behind() {
        assert_eq!(parse_rev_list_left_right_count("0 0\n").unwrap(), (0, 0));
        assert_eq!(parse_rev_list_left_right_count("2 1\n").unwrap(), (1, 2));
        assert_eq!(
            parse_rev_list_left_right_count("  4\t3 \n").unwrap(),
            (3, 4)
        );
        assert!(matches!(
            parse_rev_list_left_right_count("1 2 3"),
            Err(Error::GitCommand { .. })
        ));
    }

    #[test]
    fn parse_upstream_track_handles_git_for_each_ref_track_output() {
        assert_eq!(parse_upstream_track(""), Some((0, 0)));
        assert_eq!(parse_upstream_track("[ahead 2]"), Some((2, 0)));
        assert_eq!(parse_upstream_track("[behind 3]"), Some((0, 3)));
        assert_eq!(parse_upstream_track("[ahead 2, behind 3]"), Some((2, 3)));
        assert_eq!(parse_upstream_track("[gone]"), None);
        assert_eq!(parse_upstream_track("[ahead nope]"), None);
        assert_eq!(parse_upstream_track("ahead 2"), None);
    }

    #[test]
    fn branch_sync_status_defaults_without_upstream() {
        let repo_dir = TempRepo::init_with_commit("sync-no-upstream");
        let repo = Repository::open(&repo_dir.path).unwrap();

        assert_eq!(
            repo.branch_sync_status().unwrap(),
            BranchSyncStatus::default()
        );
    }

    #[test]
    fn refs_include_sync_status_for_tracking_local_branches() {
        let remote = TempRepo::new("refs-sync-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("refs-sync-source");
        source.git(&["branch", "-M", "main"]);
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["push", "-u", "origin", "main"]);
        source.git(&["checkout", "-b", "feature/demo"]);
        source.git(&["push", "-u", "origin", "feature/demo"]);
        source.git(&["checkout", "main"]);
        remote.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        let parent = TempRepo::new("refs-sync-parent");
        let local_path = clone_main(&remote, &parent);
        let local = TempRepo {
            path: local_path.clone(),
        };
        local.git(&["config", "user.name", "naite test"]);
        local.git(&["config", "user.email", "naite@example.com"]);
        local.git(&[
            "checkout",
            "-b",
            "feature/demo",
            "--track",
            "origin/feature/demo",
        ]);
        local.write("local.txt", "local change\n");
        local.git(&["add", "local.txt"]);
        local.git(&["commit", "-m", "local change"]);

        source.write("remote.txt", "remote change\n");
        source.git(&["add", "remote.txt"]);
        source.git(&["commit", "-m", "remote change"]);
        source.git(&["push", "origin", "main"]);
        local.git(&["fetch", "origin"]);

        let refs = Repository::open(&local_path).unwrap().refs().unwrap();
        let main = refs
            .local
            .iter()
            .find(|ref_summary| ref_summary.short_name == "main")
            .unwrap();
        let feature = refs
            .local
            .iter()
            .find(|ref_summary| ref_summary.short_name == "feature/demo")
            .unwrap();
        let remote = refs
            .remote
            .iter()
            .find(|ref_summary| ref_summary.short_name == "origin/main")
            .unwrap();

        assert_eq!(
            main.sync_status,
            Some(BranchSyncStatus {
                upstream: Some("origin/main".into()),
                ahead: 0,
                behind: 1,
            })
        );
        assert_eq!(
            feature.sync_status,
            Some(BranchSyncStatus {
                upstream: Some("origin/feature/demo".into()),
                ahead: 1,
                behind: 0,
            })
        );
        assert_eq!(remote.sync_status, None);
    }

    #[test]
    fn repository_reads_empty_refs_for_empty_repo() {
        let repo_dir = TempRepo::new("empty-refs");

        Repository::init(&repo_dir.path).unwrap();
        let repo = Repository::open(&repo_dir.path).unwrap();
        let refs = repo.refs().unwrap();

        assert!(refs.local.is_empty());
        assert!(refs.remote.is_empty());
        assert!(refs.tags.is_empty());
    }
}
