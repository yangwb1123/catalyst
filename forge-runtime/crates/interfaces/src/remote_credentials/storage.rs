use super::{MAX_CREDENTIAL_BYTES, OpenOptions, Path, PathBuf, Read, StoredCredential, fs};

pub(super) fn read_credential(path: &Path, owner_uid: u32) -> Result<StoredCredential, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| "Forge credential is missing, unsafe, or unreadable".to_owned())?;
        check_file_metadata(
            &file
                .metadata()
                .map_err(|_| "could not inspect the Forge credential file".to_owned())?,
            owner_uid,
        )?;
        let mut bytes = Vec::new();
        file.take((MAX_CREDENTIAL_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "could not read the Forge credential".to_owned())?;
        if bytes.len() > MAX_CREDENTIAL_BYTES {
            return Err("Forge credential exceeds the local size limit".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Forge credential file is invalid".to_owned())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("secure Forge credential storage is unavailable on this platform; set FORGE_ACCESS_TOKEN to continue".into())
    }
}

#[cfg(unix)]
pub(super) fn ensure_directory(
    config_root: &Path,
    directory: &Path,
    create: bool,
) -> Result<u32, String> {
    let owner_uid = ensure_config_root(config_root, create)?;
    ensure_private_subdirectories(config_root, directory, create, owner_uid)?;
    Ok(owner_uid)
}

#[cfg(unix)]
fn ensure_config_root(config_root: &Path, create: bool) -> Result<u32, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    reject_symlink_components(config_root)?;
    if create && fs::symlink_metadata(config_root).is_err() {
        fs::create_dir(config_root).map_err(|_| {
            "could not create the Forge credential configuration directory".to_owned()
        })?;
        fs::set_permissions(config_root, fs::Permissions::from_mode(0o700)).map_err(|_| {
            "could not secure the Forge credential configuration directory".to_owned()
        })?;
    }
    let root_metadata = fs::metadata(config_root)
        .map_err(|_| "credential configuration directory does not exist".to_owned())?;
    if !root_metadata.is_dir() {
        return Err("credential configuration path is not a directory".into());
    }
    let owner_uid = nix::unistd::Uid::effective().as_raw();
    if root_metadata.uid() != owner_uid {
        return Err("credential configuration directory must be owned by the current user".into());
    }
    Ok(owner_uid)
}

#[cfg(unix)]
fn ensure_private_subdirectories(
    config_root: &Path,
    directory: &Path,
    create: bool,
    owner_uid: u32,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut current = config_root.to_path_buf();
    for component in directory
        .strip_prefix(config_root)
        .map_err(|_| "credential directory is outside the configuration root".to_owned())?
        .components()
    {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("credential directory contains a symlink or non-directory".into());
                }
                check_private_directory(&metadata, owner_uid)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                fs::create_dir(&current).map_err(|_| {
                    "could not create a private Forge credential directory".to_owned()
                })?;
                fs::set_permissions(&current, fs::Permissions::from_mode(0o700))
                    .map_err(|_| "could not secure the Forge credential directory".to_owned())?;
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|_| "could not inspect the Forge credential directory".to_owned())?;
                check_private_directory(&metadata, owner_uid)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(
                    "Forge credential directory does not exist; run forge-runtime remote login"
                        .into(),
                );
            }
            Err(_) => return Err("could not inspect the Forge credential directory".into()),
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn ensure_directory(
    _config_root: &Path,
    _directory: &Path,
    _create: bool,
) -> Result<u32, String> {
    Err("secure Forge credential storage is unavailable on this platform; set FORGE_ACCESS_TOKEN to continue".into())
}

#[cfg(unix)]
fn check_private_directory(metadata: &fs::Metadata, owner_uid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    if metadata.uid() != owner_uid || metadata.mode() & 0o777 != 0o700 {
        return Err(
            "Forge credential directories must be owned by the current user with mode 0700".into(),
        );
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn check_file_metadata(metadata: &fs::Metadata, owner_uid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    if !metadata.is_file() || metadata.uid() != owner_uid || metadata.mode() & 0o777 != 0o600 {
        return Err(
            "Forge credential files must be owned by the current user with mode 0600".into(),
        );
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn check_file_metadata(_metadata: &fs::Metadata, _owner_uid: u32) -> Result<(), String> {
    Err("secure Forge credential storage is unavailable on this platform; set FORGE_ACCESS_TOKEN to continue".into())
}

#[cfg(unix)]
pub(super) fn reject_symlink_if_present(path: &Path, owner_uid: u32) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("refusing a symlink at the Forge credential path".into())
        }
        Ok(metadata) if !metadata.is_file() => {
            Err("Forge credential path is not a regular file".into())
        }
        Ok(metadata) => check_file_metadata(&metadata, owner_uid),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("could not inspect the Forge credential path".into()),
    }
}

#[cfg(not(unix))]
pub(super) fn reject_symlink_if_present(_path: &Path, _owner_uid: u32) -> Result<(), String> {
    Err("secure Forge credential storage is unavailable on this platform; set FORGE_ACCESS_TOKEN to continue".into())
}

#[cfg(unix)]
fn reject_symlink_components(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err("credential path contains a symlink".into());
            }
            if metadata.is_dir() && metadata.mode() & 0o022 != 0 {
                return Err("credential path crosses a group- or world-writable directory".into());
            }
        }
    }
    Ok(())
}
