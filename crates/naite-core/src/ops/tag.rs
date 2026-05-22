use crate::repo::Repository;
use crate::Error;

impl Repository {
    pub fn create_tag(&self, tag_name: &str, target: Option<&str>) -> Result<(), Error> {
        let tag_name = validate_tag_name(tag_name)?;
        let ref_name = format!("refs/tags/{tag_name}");
        let _ = self.git(&["check-ref-format", &ref_name])?;
        if self
            .refs()?
            .tags
            .iter()
            .any(|tag| tag.short_name == tag_name)
        {
            return Err(Error::TagAlreadyExists(tag_name.to_string()));
        }

        let mut args = vec!["tag", tag_name];
        if let Some(target) = target {
            args.push(validate_commitish(target)?);
        }

        let _ = self.git(&args)?;
        Ok(())
    }

    pub fn delete_tag(&self, tag_name: &str) -> Result<(), Error> {
        let tag_name = validate_tag_name(tag_name)?;
        let _ = self.git(&["tag", "--delete", tag_name])?;
        Ok(())
    }

    pub fn push_tag(&self, tag_name: &str) -> Result<(), Error> {
        let tag_name = validate_tag_name(tag_name)?;
        let ref_name = format!("refs/tags/{tag_name}");
        let _ = self.git(&["check-ref-format", &ref_name])?;
        let refspec = format!("{ref_name}:{ref_name}");
        let _ = self.git(&["push", "origin", &refspec])?;
        Ok(())
    }
}

fn validate_tag_name(tag_name: &str) -> Result<&str, Error> {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() || tag_name.starts_with('-') {
        return Err(Error::InvalidTagName(tag_name.to_string()));
    }
    Ok(tag_name)
}

fn validate_commitish(commit: &str) -> Result<&str, Error> {
    let commit = commit.trim();
    if commit.is_empty() || commit.starts_with('-') {
        return Err(Error::InvalidCommit(commit.to_string()));
    }
    Ok(commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn create_tag_targets_selected_commit() {
        let repo_dir = TempRepo::init_with_commit("tag-create");
        let commit = {
            let repo = Repository::open(&repo_dir.path).unwrap();
            repo.list_commits(1).unwrap()[0].id.clone()
        };

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.create_tag("v1.0.0", Some(&commit)).unwrap();

        let refs = repo.refs().unwrap();
        assert!(refs.tags.iter().any(|tag| tag.short_name == "v1.0.0"));
    }

    #[test]
    fn delete_tag_removes_existing_tag() {
        let repo_dir = TempRepo::init_with_commit("tag-delete");
        repo_dir.git(&["tag", "v1.0.0"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        repo.delete_tag("v1.0.0").unwrap();

        let refs = repo.refs().unwrap();
        assert!(refs.tags.is_empty());
    }

    #[test]
    fn create_tag_rejects_duplicate_name() {
        let repo_dir = TempRepo::init_with_commit("tag-duplicate");
        repo_dir.git(&["tag", "v1.0.0"]);

        let repo = Repository::open(&repo_dir.path).unwrap();
        let err = repo.create_tag("v1.0.0", None).unwrap_err();

        assert!(matches!(err, Error::TagAlreadyExists(name) if name == "v1.0.0"));
    }

    #[test]
    fn push_tag_pushes_only_requested_tag() {
        let remote = TempRepo::new("tag-push-remote");
        remote.git(&["init", "--bare"]);

        let source = TempRepo::init_with_commit("tag-push-source");
        source.git(&["remote", "add", "origin", remote.path.to_str().unwrap()]);
        source.git(&["tag", "v1.0.0"]);
        source.git(&["tag", "v1.0.1"]);

        let repo = Repository::open(&source.path).unwrap();
        repo.push_tag("v1.0.0").unwrap();

        let pushed = remote.git_output(&["tag", "--list"]);
        assert!(pushed.lines().any(|tag| tag == "v1.0.0"));
        assert!(!pushed.lines().any(|tag| tag == "v1.0.1"));
    }

    #[test]
    fn tag_operations_reject_option_like_names() {
        let repo_dir = TempRepo::init_with_commit("tag-invalid");
        let repo = Repository::open(&repo_dir.path).unwrap();

        assert!(matches!(
            repo.create_tag("-bad", None),
            Err(Error::InvalidTagName(_))
        ));
        assert!(matches!(
            repo.delete_tag("-bad"),
            Err(Error::InvalidTagName(_))
        ));
        assert!(matches!(
            repo.push_tag("-bad"),
            Err(Error::InvalidTagName(_))
        ));
    }
}
