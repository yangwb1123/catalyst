use std::{fs, path::Path};

use super::HubStoreError;

#[cfg(unix)]
pub(super) fn verify_unique_link(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() == 1 {
        Ok(())
    } else {
        Err(HubStoreError::Unavailable {
            message: format!(
                "Hub database must have exactly one hard link: {}",
                path.display()
            ),
        })
    }
}

#[cfg(not(unix))]
pub(super) fn verify_unique_link(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    Ok(())
}
