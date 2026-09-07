use std::{io::Read, path::Path};

use cap_std::fs::Dir;
#[cfg(unix)]
use cap_std::fs::MetadataExt as _;

use crate::runtime_domain::ToolError;

use super::{EditPlan, ensure_regular, sha256};

#[derive(Clone, Copy)]
pub(super) struct TargetIdentity {
    length: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    links: u64,
    #[cfg(unix)]
    owner: u32,
    #[cfg(unix)]
    group: u32,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanos: i64,
}

pub(super) fn identity_from_cap(metadata: &cap_std::fs::Metadata) -> TargetIdentity {
    TargetIdentity {
        length: metadata.len(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(unix)]
        mode: metadata.mode(),
        #[cfg(unix)]
        links: metadata.nlink(),
        #[cfg(unix)]
        owner: metadata.uid(),
        #[cfg(unix)]
        group: metadata.gid(),
        #[cfg(unix)]
        changed_seconds: metadata.ctime(),
        #[cfg(unix)]
        changed_nanos: metadata.ctime_nsec(),
    }
}

#[cfg(unix)]
pub(super) fn verify_staged_ownership(
    staged_file: &cap_std::fs::File,
    expected: TargetIdentity,
) -> Result<(), ToolError> {
    let metadata = staged_file
        .metadata()
        .map_err(|_| authorization_mismatch())?;
    if metadata.uid() != expected.owner || metadata.gid() != expected.group {
        return Err(authorization_mismatch());
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_staged_ownership(
    _staged_file: &cap_std::fs::File,
    _expected: TargetIdentity,
) -> Result<(), ToolError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn verify_replacement_metadata(
    staged_file: &cap_std::fs::File,
    expected: TargetIdentity,
) -> Result<(), ToolError> {
    let metadata = staged_file
        .metadata()
        .map_err(|_| authorization_mismatch())?;
    if metadata.uid() != expected.owner
        || metadata.gid() != expected.group
        || metadata.mode() != expected.mode
    {
        return Err(authorization_mismatch());
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_replacement_metadata(
    _staged_file: &cap_std::fs::File,
    _expected: TargetIdentity,
) -> Result<(), ToolError> {
    Ok(())
}

pub(super) fn verify_target_unchanged(
    parent: &Dir,
    target: &Path,
    plan: &EditPlan,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<(), ToolError> {
    super::metadata::verify_parent_acl_unchanged(parent)?;
    let expected_identity = plan.before_identity.expect("replacement identity");
    let expected_hash = plan.before_sha256.as_deref().expect("replacement hash");
    let metadata = parent
        .symlink_metadata(target)
        .map_err(|_| edit_conflict())?;
    ensure_regular(&metadata).map_err(|_| edit_conflict())?;
    if !identity_matches_cap(expected_identity, &metadata) {
        return Err(edit_conflict());
    }
    if current_target_hash(parent, target, expected_identity, cancellation)? != expected_hash {
        return Err(edit_conflict());
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn current_target_hash(
    parent: &Dir,
    target: &Path,
    expected: TargetIdentity,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<String, ToolError> {
    use std::os::unix::fs::MetadataExt as _;

    let descriptor = rustix::fs::openat(
        parent,
        target,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| edit_conflict())?;
    let file = std::fs::File::from(descriptor);
    let metadata = file.metadata().map_err(|_| edit_conflict())?;
    if !metadata.is_file()
        || metadata.len() != expected.length
        || metadata.dev() != expected.device
        || metadata.ino() != expected.inode
        || metadata.mode() != expected.mode
        || metadata.nlink() != expected.links
        || metadata.uid() != expected.owner
        || metadata.gid() != expected.group
        || metadata.ctime() != expected.changed_seconds
        || metadata.ctime_nsec() != expected.changed_nanos
    {
        return Err(edit_conflict());
    }
    super::metadata::verify_target_metadata_unchanged(&file)?;
    hash_exact_reader(file, expected.length, cancellation)
}

#[cfg(not(unix))]
pub(super) fn current_target_hash(
    parent: &Dir,
    target: &Path,
    expected: TargetIdentity,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<String, ToolError> {
    let file = parent.open(target).map_err(|_| edit_conflict())?;
    let metadata = file.metadata().map_err(|_| edit_conflict())?;
    if !identity_matches_cap(expected, &metadata) {
        return Err(edit_conflict());
    }
    hash_exact_reader(file, expected.length, cancellation)
}

fn hash_exact_reader(
    mut reader: impl Read,
    expected_length: u64,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<String, ToolError> {
    let capacity = usize::try_from(expected_length).map_err(|_| edit_conflict())?;
    let mut bytes = Vec::with_capacity(capacity);
    let target = capacity.saturating_add(1);
    let mut buffer = [0_u8; 8 * 1024];
    while bytes.len() < target {
        ensure_active(cancellation)?;
        let length = (target - bytes.len()).min(buffer.len());
        let read = reader
            .read(&mut buffer[..length])
            .map_err(|_| edit_conflict())?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    ensure_active(cancellation)?;
    if u64::try_from(bytes.len()).ok() != Some(expected_length) {
        return Err(edit_conflict());
    }
    Ok(sha256(&bytes))
}

pub(super) fn identity_matches_cap(
    expected: TargetIdentity,
    metadata: &cap_std::fs::Metadata,
) -> bool {
    if metadata.len() != expected.length {
        return false;
    }
    #[cfg(unix)]
    if metadata.dev() != expected.device
        || metadata.ino() != expected.inode
        || metadata.mode() != expected.mode
        || metadata.nlink() != expected.links
        || metadata.uid() != expected.owner
        || metadata.gid() != expected.group
        || metadata.ctime() != expected.changed_seconds
        || metadata.ctime_nsec() != expected.changed_nanos
    {
        return false;
    }
    true
}

pub(super) fn identity_matches_std(expected: TargetIdentity, metadata: &std::fs::Metadata) -> bool {
    if metadata.len() != expected.length {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.dev() != expected.device
            || metadata.ino() != expected.inode
            || metadata.mode() != expected.mode
            || metadata.nlink() != expected.links
            || metadata.uid() != expected.owner
            || metadata.gid() != expected.group
            || metadata.ctime() != expected.changed_seconds
            || metadata.ctime_nsec() != expected.changed_nanos
        {
            return false;
        }
    }
    true
}

fn ensure_active(cancellation: &forge_runtime_domain::Cancellation) -> Result<(), ToolError> {
    if cancellation.is_cancelled() {
        Err(ToolError::new("cancelled", "run was cancelled"))
    } else {
        Ok(())
    }
}

fn edit_conflict() -> ToolError {
    ToolError::new(
        "edit_conflict",
        "target changed after it was read; edit was not committed",
    )
}

#[cfg(unix)]
fn authorization_mismatch() -> ToolError {
    ToolError::new(
        "metadata_not_preserved",
        "replacement cannot preserve the target owner, group, and POSIX mode",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use cap_std::{ambient_authority, fs::Dir};

    use super::*;

    #[test]
    fn staged_owner_and_restored_mode_must_match_target() {
        let root = tempfile::TempDir::new().expect("workspace");
        std::fs::write(root.path().join("stage"), "content").expect("fixture");
        let parent = Dir::open_ambient_dir(root.path(), ambient_authority()).expect("parent");
        let file = parent.open("stage").expect("staged file");
        let expected = identity_from_cap(&file.metadata().expect("metadata"));
        verify_staged_ownership(&file, expected).expect("matching owner");
        verify_replacement_metadata(&file, expected).expect("matching metadata");

        let mut wrong_owner = expected;
        wrong_owner.owner = wrong_owner.owner.wrapping_add(1);
        assert_eq!(
            verify_staged_ownership(&file, wrong_owner)
                .expect_err("owner drift")
                .code,
            "metadata_not_preserved"
        );
        let mut wrong_mode = expected;
        wrong_mode.mode ^= 0o100;
        assert_eq!(
            verify_replacement_metadata(&file, wrong_mode)
                .expect_err("mode drift")
                .code,
            "metadata_not_preserved"
        );
    }
}
