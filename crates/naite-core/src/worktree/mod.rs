use std::path::{Component, Path};

use crate::Error;

pub(crate) mod diff;
pub(crate) mod status;

pub use diff::{WorktreeDiffKind, WorktreeDiffTarget};
pub use status::{StatusEntry, StatusKind, WorktreeStatus, WorktreeStatusDetail};

pub(crate) fn validate_status_path(path: &str) -> Result<(), Error> {
    let parsed = Path::new(path);
    if path.is_empty()
        || path.contains('\0')
        || parsed.is_absolute()
        || parsed.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::CurDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(Error::InvalidPath(path.to_string()));
    }

    Ok(())
}
