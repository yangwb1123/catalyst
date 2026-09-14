use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "remote_credentials/checkpoint.rs"]
mod checkpoint;
#[path = "remote_token_claims.rs"]
mod token_claims;
pub(super) use checkpoint::ChangeCursorStore;
pub(super) use token_claims::{OwnerSelector, StoredCredential, credential_from_token};
use token_claims::{credential_key, validate_credential};

const MAX_CREDENTIAL_BYTES: usize = 16 * 1024;

pub(super) struct CredentialStore {
    config_root: PathBuf,
    directory: PathBuf,
}

impl CredentialStore {
    #[cfg(test)]
    pub(super) fn for_test(config_root: PathBuf) -> Self {
        Self {
            directory: config_root.join("forge-runtime").join("credentials"),
            config_root,
        }
    }

    pub(super) fn from_env() -> Result<Self, String> {
        let config_root = if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(path)
        } else {
            PathBuf::from(env::var_os("HOME").ok_or_else(|| {
                "cannot locate Forge credentials; set XDG_CONFIG_HOME or HOME".to_owned()
            })?)
            .join(".config")
        };
        if !config_root.is_absolute() {
            return Err("credential configuration directory must be an absolute path".into());
        }
        Ok(Self {
            directory: config_root.join("forge-runtime").join("credentials"),
            config_root,
        })
    }

    #[cfg(unix)]
    pub(super) fn save(&self, credential: &StoredCredential) -> Result<(), String> {
        let owner_uid = ensure_directory(&self.config_root, &self.directory, true)?;
        validate_credential(credential)?;
        let path = self.path_for(credential);
        reject_symlink_if_present(&path, owner_uid)?;
        let encoded = serde_json::to_vec(credential)
            .map_err(|_| "could not encode the Forge credential".to_owned())?;
        if encoded.len() > MAX_CREDENTIAL_BYTES {
            return Err("Forge credential exceeds the local size limit".into());
        }
        self.atomic_write(&path, &encoded, owner_uid)
    }

    #[cfg(not(unix))]
    pub(super) fn save(&self, _credential: &StoredCredential) -> Result<(), String> {
        Err("secure Forge credential storage is unavailable on this platform; set FORGE_ACCESS_TOKEN to continue".into())
    }

    pub(super) fn load(
        &self,
        issuer: &str,
        client_id: &str,
        selector: OwnerSelector<'_>,
    ) -> Result<StoredCredential, String> {
        let owner_uid = ensure_directory(&self.config_root, &self.directory, false)?;
        let entries = fs::read_dir(&self.directory)
            .map_err(|_| "could not read the Forge credential directory".to_owned())?;
        let now = unix_time()?;
        let mut matching = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| "could not inspect a Forge credential".to_owned())?;
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let credential = read_credential(&path, owner_uid)?;
            if credential.issuer == issuer
                && credential.client_id == client_id
                && selector
                    .subject
                    .is_none_or(|subject| credential.subject == subject)
                && selector
                    .tenant_id
                    .is_none_or(|tenant| credential.tenant_id == tenant)
            {
                validate_credential(&credential)?;
                if self.path_for(&credential) != path {
                    return Err("Forge credential binding does not match its file name".into());
                }
                if credential.expires_at_unix > now {
                    matching.push(credential);
                }
            }
        }
        match matching.len() {
            0 => Err("no unexpired Forge credential matches this issuer and client; run forge-runtime remote login or set FORGE_ACCESS_TOKEN".into()),
            1 => Ok(matching.remove(0)),
            _ => Err("multiple Forge accounts match this issuer and client; set SNAPLINK_SUBJECT and, if needed, SNAPLINK_TENANT_ID".into()),
        }
    }

    fn path_for(&self, credential: &StoredCredential) -> PathBuf {
        self.directory.join(format!(
            "{}.json",
            credential_key(
                &credential.issuer,
                &credential.client_id,
                &credential.tenant_id,
                &credential.subject,
            )
        ))
    }

    #[cfg(unix)]
    fn atomic_write(&self, path: &Path, encoded: &[u8], owner_uid: u32) -> Result<(), String> {
        use std::os::unix::fs::OpenOptionsExt;

        let nonce = format!(".credential-{}-{}.tmp", std::process::id(), unix_time()?);
        let temporary = self.directory.join(nonce);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)
            .map_err(|_| "could not create a private Forge credential file".to_owned())?;
        let result = (|| {
            file.write_all(encoded)
                .map_err(|_| "could not write the Forge credential".to_owned())?;
            file.sync_all()
                .map_err(|_| "could not flush the Forge credential".to_owned())?;
            check_file_metadata(
                &file
                    .metadata()
                    .map_err(|_| "could not inspect the Forge credential file".to_owned())?,
                owner_uid,
            )?;
            reject_symlink_if_present(path, owner_uid)?;
            fs::rename(&temporary, path)
                .map_err(|_| "could not atomically replace the Forge credential".to_owned())?;
            File::open(&self.directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| "could not flush the Forge credential directory".to_owned())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

fn unix_time() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock is before the Unix epoch".to_owned())
}

fn read_credential(path: &Path, owner_uid: u32) -> Result<StoredCredential, String> {
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
fn ensure_directory(config_root: &Path, directory: &Path, create: bool) -> Result<u32, String> {
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
fn ensure_directory(_config_root: &Path, _directory: &Path, _create: bool) -> Result<u32, String> {
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
fn check_file_metadata(metadata: &fs::Metadata, owner_uid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    if !metadata.is_file() || metadata.uid() != owner_uid || metadata.mode() & 0o777 != 0o600 {
        return Err(
            "Forge credential files must be owned by the current user with mode 0600".into(),
        );
    }
    Ok(())
}

#[cfg(unix)]
fn reject_symlink_if_present(path: &Path, owner_uid: u32) -> Result<(), String> {
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
fn reject_symlink_if_present(_path: &Path, _owner_uid: u32) -> Result<(), String> {
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

#[cfg(test)]
#[path = "remote_credentials_tests.rs"]
mod tests;
