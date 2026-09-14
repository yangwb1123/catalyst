use std::{fs, io::Read, path::PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    CredentialStore, StoredCredential, check_file_metadata, ensure_directory,
    reject_symlink_if_present,
};

const MAX_CHECKPOINT_BYTES: usize = 8192;
const MAX_CHANGE_CURSOR: u64 = i64::MAX as u64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedCursor {
    schema_version: u8,
    coordinator: String,
    issuer: String,
    client_id: String,
    subject: String,
    tenant_id: String,
    cursor: u64,
}

#[derive(Clone)]
pub struct ChangeCursorStore {
    config_root: PathBuf,
    directory: PathBuf,
    checkpoint: PersistedCursor,
    path: PathBuf,
}

impl CredentialStore {
    pub fn change_cursor_store(
        &self,
        coordinator: &str,
        credential: &StoredCredential,
    ) -> ChangeCursorStore {
        let checkpoint = PersistedCursor {
            schema_version: 1,
            coordinator: coordinator.to_owned(),
            issuer: credential.issuer.clone(),
            client_id: credential.client_id.clone(),
            subject: credential.subject.clone(),
            tenant_id: credential.tenant_id.clone(),
            cursor: 0,
        };
        let name = checkpoint_name(&checkpoint);
        ChangeCursorStore {
            config_root: self.config_root.clone(),
            directory: self.directory.clone(),
            checkpoint,
            path: self.directory.join(format!("{name}.cursor")),
        }
    }
}

impl ChangeCursorStore {
    pub fn load(&self) -> Result<u64, String> {
        let owner_uid = ensure_directory(&self.config_root, &self.directory, false)?;
        match fs::symlink_metadata(&self.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err("could not inspect the Forge change checkpoint".into()),
            Ok(_) => {}
        }
        let bytes = read_checkpoint(&self.path, owner_uid)?;
        let saved: PersistedCursor = serde_json::from_slice(&bytes)
            .map_err(|_| "Forge change checkpoint is invalid".to_owned())?;
        if !same_binding(&saved, &self.checkpoint) {
            return Err("Forge change checkpoint binding is invalid".into());
        }
        if saved.cursor > MAX_CHANGE_CURSOR {
            return Err("Forge change checkpoint cursor is invalid".into());
        }
        Ok(saved.cursor)
    }

    pub fn save(&self, cursor: u64) -> Result<(), String> {
        if cursor > MAX_CHANGE_CURSOR {
            return Err("Forge change checkpoint cursor is invalid".into());
        }
        let owner_uid = ensure_directory(&self.config_root, &self.directory, false)?;
        reject_symlink_if_present(&self.path, owner_uid)?;
        let saved = PersistedCursor {
            cursor,
            ..self.checkpoint.clone()
        };
        let encoded = serde_json::to_vec(&saved)
            .map_err(|_| "could not encode the Forge change checkpoint".to_owned())?;
        if encoded.len() > MAX_CHECKPOINT_BYTES {
            return Err("Forge change checkpoint exceeds the local size limit".into());
        }
        CredentialStore {
            config_root: self.config_root.clone(),
            directory: self.directory.clone(),
        }
        .atomic_write(&self.path, &encoded, owner_uid)
    }
}

fn same_binding(saved: &PersistedCursor, expected: &PersistedCursor) -> bool {
    saved.schema_version == expected.schema_version
        && saved.coordinator == expected.coordinator
        && saved.issuer == expected.issuer
        && saved.client_id == expected.client_id
        && saved.subject == expected.subject
        && saved.tenant_id == expected.tenant_id
}

fn checkpoint_name(checkpoint: &PersistedCursor) -> String {
    let mut digest = Sha256::new();
    digest.update(b"forge-conversation-change-cursor/v1\0");
    for field in [
        checkpoint.coordinator.as_str(),
        checkpoint.issuer.as_str(),
        checkpoint.client_id.as_str(),
        checkpoint.tenant_id.as_str(),
        checkpoint.subject.as_str(),
    ] {
        digest.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn read_checkpoint(path: &std::path::Path, owner_uid: u32) -> Result<Vec<u8>, String> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| "Forge change checkpoint is unsafe or unreadable".to_owned())?;
        check_file_metadata(
            &file
                .metadata()
                .map_err(|_| "could not inspect the Forge change checkpoint".to_owned())?,
            owner_uid,
        )?;
        let mut bytes = Vec::new();
        file.take((MAX_CHECKPOINT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "could not read the Forge change checkpoint".to_owned())?;
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err("Forge change checkpoint exceeds the local size limit".into());
        }
        Ok(bytes)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, owner_uid);
        Err("secure Forge checkpoint storage is unavailable on this platform".into())
    }
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
