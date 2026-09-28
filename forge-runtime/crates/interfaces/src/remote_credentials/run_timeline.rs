use std::{fs, io::Read, path::PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{CredentialStore, check_file_metadata, ensure_directory, reject_symlink_if_present};

const MAX_CHECKPOINT_BYTES: usize = 8192;
const MAX_TIMELINE_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedRunTimelineCursor {
    schema_version: u8,
    coordinator: String,
    issuer: String,
    client_id: String,
    subject: String,
    tenant_id: String,
    conversation_id: String,
    run_id: String,
    cursor: u64,
}

/// Owner- and Run-bound checkpoint for metadata-only timeline replay.
///
/// The file contains no access token or event payload. It is intentionally a
/// separate namespace from the Conversation change cursor because each Run has
/// an independent dense sequence and each client needs its own resume point.
#[derive(Clone)]
pub struct RunTimelineCursorStore {
    config_root: PathBuf,
    directory: PathBuf,
    credential_store: CredentialStore,
    checkpoint: PersistedRunTimelineCursor,
    pub(super) path: PathBuf,
}

impl RunTimelineCursorStore {
    pub(super) fn from_binding(
        credential_store: CredentialStore,
        coordinator: &str,
        issuer: &str,
        client_id: &str,
        subject: &str,
        tenant_id: &str,
        run: (&str, &str),
    ) -> Self {
        let (conversation_id, run_id) = run;
        let checkpoint = PersistedRunTimelineCursor {
            schema_version: 1,
            coordinator: coordinator.to_owned(),
            issuer: issuer.to_owned(),
            client_id: client_id.to_owned(),
            subject: subject.to_owned(),
            tenant_id: tenant_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            run_id: run_id.to_owned(),
            cursor: 0,
        };
        let name = checkpoint_name(&checkpoint);
        let directory = credential_store.directory.clone();
        let config_root = credential_store.config_root.clone();
        Self {
            config_root,
            directory: directory.clone(),
            credential_store,
            checkpoint,
            path: directory.join(format!("{name}.run-timeline.cursor")),
        }
    }

    pub(in crate::remote_command) fn load(&self) -> Result<u64, String> {
        let owner_uid = ensure_directory(&self.config_root, &self.directory, false)?;
        match fs::symlink_metadata(&self.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err("could not inspect the Forge Run timeline checkpoint".into()),
            Ok(_) => {}
        }
        let bytes = read_checkpoint(&self.path, owner_uid)?;
        let saved: PersistedRunTimelineCursor = serde_json::from_slice(&bytes)
            .map_err(|_| "Forge Run timeline checkpoint is invalid".to_owned())?;
        if !same_binding(&saved, &self.checkpoint) {
            return Err("Forge Run timeline checkpoint binding is invalid".into());
        }
        if saved.cursor > MAX_TIMELINE_SEQUENCE {
            return Err("Forge Run timeline checkpoint cursor is invalid".into());
        }
        Ok(saved.cursor)
    }

    /// Saves a cursor monotonically. A stale page can therefore never move a
    /// persisted Run timeline backwards after a newer page has succeeded.
    pub(in crate::remote_command) fn save(&self, cursor: u64) -> Result<bool, String> {
        if cursor > MAX_TIMELINE_SEQUENCE {
            return Err("Forge Run timeline checkpoint cursor is invalid".into());
        }
        let owner_uid = ensure_directory(&self.config_root, &self.directory, false)?;
        reject_symlink_if_present(&self.path, owner_uid)?;
        let current = self.load()?;
        if cursor < current {
            return Ok(false);
        }
        let saved = PersistedRunTimelineCursor {
            cursor,
            ..self.checkpoint.clone()
        };
        let encoded = serde_json::to_vec(&saved)
            .map_err(|_| "could not encode the Forge Run timeline checkpoint".to_owned())?;
        if encoded.len() > MAX_CHECKPOINT_BYTES {
            return Err("Forge Run timeline checkpoint exceeds the local size limit".into());
        }
        self.credential_store
            .atomic_write(&self.path, &encoded, owner_uid)?;
        Ok(true)
    }
}

fn same_binding(saved: &PersistedRunTimelineCursor, expected: &PersistedRunTimelineCursor) -> bool {
    saved.schema_version == expected.schema_version
        && saved.coordinator == expected.coordinator
        && saved.issuer == expected.issuer
        && saved.client_id == expected.client_id
        && saved.subject == expected.subject
        && saved.tenant_id == expected.tenant_id
        && saved.conversation_id == expected.conversation_id
        && saved.run_id == expected.run_id
}

fn checkpoint_name(checkpoint: &PersistedRunTimelineCursor) -> String {
    let mut digest = Sha256::new();
    digest.update(b"forge-run-timeline-cursor/v1\0");
    for field in [
        checkpoint.coordinator.as_str(),
        checkpoint.issuer.as_str(),
        checkpoint.client_id.as_str(),
        checkpoint.tenant_id.as_str(),
        checkpoint.subject.as_str(),
        checkpoint.conversation_id.as_str(),
        checkpoint.run_id.as_str(),
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
            .map_err(|_| "Forge Run timeline checkpoint is unsafe or unreadable".to_owned())?;
        check_file_metadata(
            &file
                .metadata()
                .map_err(|_| "could not inspect the Forge Run timeline checkpoint".to_owned())?,
            owner_uid,
        )?;
        let mut bytes = Vec::new();
        file.take((MAX_CHECKPOINT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "could not read the Forge Run timeline checkpoint".to_owned())?;
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err("Forge Run timeline checkpoint exceeds the local size limit".into());
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
#[path = "run_timeline_tests.rs"]
mod tests;
