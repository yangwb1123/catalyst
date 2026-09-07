use cap_std::fs::Dir;

use crate::runtime_domain::ToolError;

#[cfg(target_os = "linux")]
const MAX_XATTR_LIST_BYTES: usize = 64 * 1024;
#[cfg(target_os = "linux")]
const DEFAULT_ACL: &[u8] = b"system.posix_acl_default";

pub(super) fn ensure_parent_acl_safe(parent: &Dir) -> Result<(), ToolError> {
    #[cfg(target_os = "linux")]
    {
        let names = list_directory_xattr_names(parent)?;
        if contains_name(&names, DEFAULT_ACL) {
            return Err(metadata_not_preserved(
                "parent directory has a default POSIX ACL",
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn list_directory_xattr_names(parent: &Dir) -> Result<Vec<u8>, ToolError> {
    let descriptor = rustix::fs::openat(
        parent,
        ".",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| metadata_not_preserved(&format!("parent metadata is unreadable: {error}")))?;
    list_xattr_names(descriptor)
}

pub(super) fn ensure_target_metadata_safe(file: &std::fs::File) -> Result<(), ToolError> {
    #[cfg(target_os = "linux")]
    {
        if !list_xattr_names(file)?.is_empty() {
            return Err(metadata_not_preserved(
                "target has extended attributes that replacement cannot preserve",
            ));
        }
    }
    Ok(())
}

pub(super) fn ensure_staged_metadata_safe(file: &cap_std::fs::File) -> Result<(), ToolError> {
    #[cfg(target_os = "linux")]
    {
        if !list_xattr_names(file)?.is_empty() {
            return Err(metadata_not_preserved(
                "staged file has extended metadata that replacement cannot preserve",
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_target_metadata_unchanged(file: &std::fs::File) -> Result<(), ToolError> {
    ensure_target_metadata_safe(file).map_err(|_| metadata_conflict())
}

pub(super) fn verify_parent_acl_unchanged(parent: &Dir) -> Result<(), ToolError> {
    ensure_parent_acl_safe(parent).map_err(|_| metadata_conflict())
}

#[cfg(target_os = "linux")]
fn list_xattr_names<Fd: std::os::fd::AsFd>(fd: Fd) -> Result<Vec<u8>, ToolError> {
    let mut empty: [u8; 0] = [];
    let size = match rustix::fs::flistxattr(fd.as_fd(), &mut empty) {
        Ok(size) => size,
        Err(error) if error == rustix::io::Errno::OPNOTSUPP => return Ok(Vec::new()),
        Err(error) => {
            return Err(metadata_not_preserved(&format!(
                "extended metadata is unreadable: {error}"
            )));
        }
    };
    if size > MAX_XATTR_LIST_BYTES {
        return Err(metadata_not_preserved(
            "extended metadata name list exceeds the safety bound",
        ));
    }
    let mut names = vec![0_u8; size];
    let length = rustix::fs::flistxattr(fd.as_fd(), &mut names)
        .map_err(|_| metadata_not_preserved("extended metadata changed while being inspected"))?;
    finish_xattr_names(names, length)
}

#[cfg(target_os = "linux")]
fn finish_xattr_names(mut names: Vec<u8>, length: usize) -> Result<Vec<u8>, ToolError> {
    if length > names.len() {
        return Err(metadata_not_preserved(
            "extended metadata changed while being inspected",
        ));
    }
    names.truncate(length);
    Ok(names)
}

#[cfg(target_os = "linux")]
fn contains_name(names: &[u8], expected: &[u8]) -> bool {
    names.split(|byte| *byte == 0).any(|name| name == expected)
}

fn metadata_not_preserved(reason: &str) -> ToolError {
    ToolError::new("metadata_not_preserved", reason)
}

fn metadata_conflict() -> ToolError {
    ToolError::new(
        "edit_conflict",
        "target or parent security metadata changed; edit was not committed",
    )
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::finish_xattr_names;

    #[test]
    fn grown_xattr_list_cannot_be_mistaken_for_an_empty_snapshot() {
        let error = finish_xattr_names(Vec::new(), 8).expect_err("growth must fail closed");
        assert_eq!(error.code, "metadata_not_preserved");
    }
}
