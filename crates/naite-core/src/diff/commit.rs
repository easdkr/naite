use crate::diff::parser::diff_from_outputs;
use crate::diff::CommitDiff;
use crate::repo::Repository;
use crate::Error;

impl Repository {
    /// First-parent diff for a commit. Git CLI is intentionally isolated here so
    /// the UI crate still consumes only structured core data.
    pub fn commit_diff(&self, commit_id: &str) -> Result<CommitDiff, Error> {
        let parents = self.commit_parent_ids(commit_id)?;
        let range_args = diff_range_args(commit_id, parents.first().map(String::as_str));

        let name_status = self.git(&name_status_args(&range_args))?;
        let patch = self.git(&patch_args(&range_args))?;
        Ok(diff_from_outputs(&name_status, &patch))
    }
}

fn diff_range_args<'a>(commit_id: &'a str, parent_id: Option<&'a str>) -> Vec<&'a str> {
    match parent_id {
        Some(parent_id) => vec![parent_id, commit_id],
        None => vec![commit_id],
    }
}

fn name_status_args<'a>(range_args: &'a [&'a str]) -> Vec<&'a str> {
    let mut args = if range_args.len() == 1 {
        vec![
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-status",
            "-r",
            "-M",
            "-C",
        ]
    } else {
        vec!["diff", "--name-status", "-M", "-C"]
    };
    args.extend_from_slice(range_args);
    args
}

pub(crate) fn patch_args<'a>(range_args: &'a [&'a str]) -> Vec<&'a str> {
    let mut args = if range_args.len() == 1 {
        vec![
            "diff-tree",
            "--root",
            "--no-commit-id",
            "-r",
            "-p",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
            "-M",
            "-C",
        ]
    } else {
        vec![
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
            "-M",
            "-C",
        ]
    };
    args.extend_from_slice(range_args);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_patch_args_recurse_into_nested_paths() {
        let args = patch_args(&["abc123"]);

        assert!(args.contains(&"diff-tree"));
        assert!(args.contains(&"--root"));
        assert!(args.contains(&"-r"));
    }
}
