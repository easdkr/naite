use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::command;
use crate::Error;

pub struct Repository {
    pub(crate) inner: gix::Repository,
    pub(crate) path: PathBuf,
}

impl Repository {
    /// Discover and open a repository starting from `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let inner = gix::discover(&path).map_err(|e| Error::Open {
            path: path.clone(),
            source: Box::new(e),
        })?;
        Ok(Self { inner, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn workdir(&self) -> Option<&Path> {
        self.inner.work_dir()
    }

    pub fn configured_user_email(&self) -> Result<Option<String>, Error> {
        let output = self.git_allowing_exit_codes(&["config", "--get", "user.email"], &[1])?;
        let email = output.trim();
        Ok((!email.is_empty()).then(|| email.to_string()))
    }

    pub fn init(path: impl AsRef<Path>) -> Result<PathBuf, Error> {
        let path = path.as_ref();
        let _ = command::run_git(path, ["init"])?;
        Ok(path.to_path_buf())
    }

    pub fn clone_from_url(url: &str, parent_dir: impl AsRef<Path>) -> Result<PathBuf, Error> {
        let url = url.trim();
        if url.is_empty() || url.starts_with('-') {
            return Err(Error::InvalidCloneUrl(url.to_string()));
        }

        let repo_dir =
            clone_directory_name(url).ok_or_else(|| Error::InvalidCloneUrl(url.to_string()))?;
        let destination = parent_dir.as_ref().join(repo_dir);

        let _ = command::run_git(
            parent_dir.as_ref(),
            [
                OsStr::new("clone"),
                OsStr::new(url),
                destination.as_os_str(),
            ],
        )?;
        Ok(destination)
    }

    pub(crate) fn git(&self, args: &[&str]) -> Result<String, Error> {
        let cwd = self.workdir().unwrap_or(self.path());
        command::run_git(cwd, args)
    }

    pub(crate) fn git_allowing_exit_codes(
        &self,
        args: &[&str],
        allowed_exit_codes: &[i32],
    ) -> Result<String, Error> {
        let cwd = self.workdir().unwrap_or(self.path());
        command::run_git_allowing_exit_codes(cwd, args, allowed_exit_codes)
    }

    pub(crate) fn git_with_env<K, V>(&self, args: &[&str], envs: &[(K, V)]) -> Result<String, Error>
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let cwd = self.workdir().unwrap_or(self.path());
        command::run_git_with_env(cwd, args, envs)
    }

    pub(crate) fn git_without_optional_locks(&self, args: &[&str]) -> Result<String, Error> {
        let cwd = self.workdir().unwrap_or(self.path());
        command::run_git_with_env(cwd, args, &[("GIT_OPTIONAL_LOCKS", "0")])
    }

    pub(crate) fn git_without_optional_locks_allowing_exit_codes(
        &self,
        args: &[&str],
        allowed_exit_codes: &[i32],
    ) -> Result<String, Error> {
        let cwd = self.workdir().unwrap_or(self.path());
        command::run_git_with_env_allowing_exit_codes(
            cwd,
            args,
            &[("GIT_OPTIONAL_LOCKS", "0")],
            allowed_exit_codes,
        )
    }
}

pub(crate) fn clone_directory_name(url: &str) -> Option<String> {
    let trimmed = url.trim_end_matches('/');
    let last_segment = trimmed
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(trimmed)
        .strip_suffix(".git")
        .unwrap_or_else(|| trimmed.rsplit(['/', ':']).next().unwrap_or(trimmed));

    let candidate = last_segment.trim();
    if candidate.is_empty() || candidate == "." || candidate == ".." {
        None
    } else {
        Some(candidate.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_directory_name_handles_common_urls() {
        assert_eq!(
            clone_directory_name("git@github.com:wisely/naite.git"),
            Some("naite".into())
        );
        assert_eq!(
            clone_directory_name("https://github.com/wisely/naite/"),
            Some("naite".into())
        );
        assert_eq!(clone_directory_name(""), None);
    }
}
