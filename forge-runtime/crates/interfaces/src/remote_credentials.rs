use serde::Serialize;
#[cfg(test)]
use std::{collections::HashMap, sync::Mutex};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "remote_credentials/checkpoint.rs"]
mod checkpoint;
#[path = "remote_credentials/run_timeline.rs"]
mod run_timeline;
#[path = "remote_credentials/storage.rs"]
mod storage;
#[path = "remote_token_claims.rs"]
mod token_claims;
pub(super) use checkpoint::ChangeCursorStore;
use storage::{check_file_metadata, ensure_directory, read_credential, reject_symlink_if_present};
pub(super) use token_claims::{OwnerSelector, StoredCredential, credential_from_token};
use token_claims::{credential_key, validate_credential};

const MAX_CREDENTIAL_BYTES: usize = 16 * 1024;
const MAX_REFRESH_TOKEN_BYTES: usize = 2048;
const KEYRING_SERVICE: &str = "forge-runtime-cli";
#[cfg(unix)]
static TEMP_FILE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone)]
pub(super) struct CredentialStore {
    config_root: PathBuf,
    directory: PathBuf,
    refresh_backend: Arc<dyn RefreshTokenBackend>,
}

pub(super) trait RefreshTokenBackend: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, token: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

struct PlatformRefreshTokenBackend;

#[cfg(all(unix, not(any(target_os = "redox", target_os = "solaris"))))]
pub(super) struct RefreshLock {
    _file: nix::fcntl::Flock<File>,
}

#[cfg(not(all(unix, not(any(target_os = "redox", target_os = "solaris")))))]
pub(super) struct RefreshLock {
    _file: File,
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct MemoryRefreshTokenBackend(Mutex<HashMap<String, String>>);

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl RefreshTokenBackend for PlatformRefreshTokenBackend {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(KEYRING_SERVICE, account).and_then(|entry| entry.get_password()) {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("OS credential store could not read the Forge refresh token".into()),
        }
    }

    fn set(&self, account: &str, token: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, account)
            .map_err(|_| "OS credential store is unavailable for Forge refresh tokens")?;
        entry
            .set_password(token)
            .map_err(|_| "OS credential store could not save the Forge refresh token")?;
        if entry.get_password().is_ok_and(|saved| saved == token) {
            Ok(())
        } else {
            Err("OS credential store did not verify the Forge refresh token".into())
        }
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, account)
            .map_err(|_| "OS credential store is unavailable for Forge refresh tokens")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("OS credential store could not remove the Forge refresh token".into()),
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl RefreshTokenBackend for PlatformRefreshTokenBackend {
    fn get(&self, _account: &str) -> Result<Option<String>, String> {
        Err("OS credential store is unsupported on this platform".into())
    }

    fn set(&self, _account: &str, _token: &str) -> Result<(), String> {
        Err("OS credential store is unsupported on this platform".into())
    }

    fn delete(&self, _account: &str) -> Result<(), String> {
        Err("OS credential store is unsupported on this platform".into())
    }
}

/// Describes which local credential persistence pieces are available on the
/// current target. This is deliberately a read-only contract: it never opens
/// the keyring, creates a credential directory, or contacts Snaplink.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct CredentialStorageCapabilities {
    pub schema_version: &'static str,
    pub platform: &'static str,
    pub access_token_env: CapabilityStatus,
    pub refresh_token_os_keyring: CapabilityStatus,
    pub credential_metadata: CapabilityStatus,
    pub refresh_lock: CapabilityStatus,
    pub saved_login: CapabilityStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct CapabilityStatus {
    pub available: bool,
    pub backend: &'static str,
    pub reason: Option<&'static str>,
}

const CREDENTIAL_STORAGE_CAPABILITIES_SCHEMA: &str = "forge.remote-credential-capabilities/v1";

pub(super) fn credential_storage_capabilities() -> CredentialStorageCapabilities {
    CredentialStorageCapabilities {
        schema_version: CREDENTIAL_STORAGE_CAPABILITIES_SCHEMA,
        platform: credential_platform(),
        access_token_env: CapabilityStatus {
            available: true,
            backend: "FORGE_ACCESS_TOKEN",
            reason: None,
        },
        refresh_token_os_keyring: refresh_token_keyring_capability(),
        credential_metadata: credential_metadata_capability(),
        refresh_lock: refresh_lock_capability(),
        saved_login: saved_login_capability(),
    }
}

#[cfg(target_os = "linux")]
fn credential_platform() -> &'static str {
    "linux"
}

#[cfg(target_os = "macos")]
fn credential_platform() -> &'static str {
    "macos"
}

#[cfg(target_os = "windows")]
fn credential_platform() -> &'static str {
    "windows"
}

#[cfg(target_os = "android")]
fn credential_platform() -> &'static str {
    "android"
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
    target_os = "android"
)))]
fn credential_platform() -> &'static str {
    "other"
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn refresh_token_keyring_capability() -> CapabilityStatus {
    CapabilityStatus {
        available: true,
        backend: "OS keyring",
        reason: None,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn refresh_token_keyring_capability() -> CapabilityStatus {
    CapabilityStatus {
        available: false,
        backend: "none",
        reason: Some("this target has no supported Forge refresh-token keyring backend"),
    }
}

#[cfg(all(unix, not(target_os = "android")))]
fn credential_metadata_capability() -> CapabilityStatus {
    CapabilityStatus {
        available: true,
        backend: "private user-owned credential file",
        reason: None,
    }
}

#[cfg(not(all(unix, not(target_os = "android"))))]
fn credential_metadata_capability() -> CapabilityStatus {
    CapabilityStatus {
        available: false,
        backend: "none",
        reason: Some("secure persistent credential metadata is not implemented on this target"),
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "redox", target_os = "solaris", target_os = "android"))
))]
fn refresh_lock_capability() -> CapabilityStatus {
    CapabilityStatus {
        available: true,
        backend: "private file lock",
        reason: None,
    }
}

#[cfg(not(all(
    unix,
    not(any(target_os = "redox", target_os = "solaris", target_os = "android"))
)))]
fn refresh_lock_capability() -> CapabilityStatus {
    CapabilityStatus {
        available: false,
        backend: "none",
        reason: Some("secure concurrent refresh locking is not implemented on this target"),
    }
}

fn saved_login_capability() -> CapabilityStatus {
    let metadata = credential_metadata_capability();
    let lock = refresh_lock_capability();
    if metadata.available && lock.available {
        return CapabilityStatus {
            available: true,
            backend: "credential metadata + OS keyring + refresh lock",
            reason: None,
        };
    }
    CapabilityStatus {
        available: false,
        backend: "environment token only",
        reason: Some(
            "use FORGE_ACCESS_TOKEN until secure metadata and refresh locking are implemented",
        ),
    }
}

#[cfg(test)]
impl RefreshTokenBackend for MemoryRefreshTokenBackend {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, token: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .insert(account.to_owned(), token.to_owned());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

impl CredentialStore {
    #[cfg(test)]
    pub(super) fn for_test(config_root: PathBuf) -> Self {
        Self {
            directory: config_root.join("forge-runtime").join("credentials"),
            config_root,
            refresh_backend: Arc::new(MemoryRefreshTokenBackend::default()),
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
            refresh_backend: Arc::new(PlatformRefreshTokenBackend),
        })
    }

    #[cfg(test)]
    pub(super) fn with_test_backend(
        config_root: PathBuf,
        refresh_backend: Arc<dyn RefreshTokenBackend>,
    ) -> Self {
        Self {
            directory: config_root.join("forge-runtime").join("credentials"),
            config_root,
            refresh_backend,
        }
    }

    pub(super) fn load_refresh_token(
        &self,
        credential: &StoredCredential,
    ) -> Result<Option<String>, String> {
        let token = self
            .refresh_backend
            .get(&Self::refresh_account(credential))?;
        if let Some(token) = &token {
            validate_refresh_token(token)?;
        }
        Ok(token)
    }

    pub(super) fn save_refresh_token(
        &self,
        credential: &StoredCredential,
        refresh_token: &str,
    ) -> Result<(), String> {
        validate_refresh_token(refresh_token)?;
        self.refresh_backend
            .set(&Self::refresh_account(credential), refresh_token)?;
        if self.load_refresh_token(credential)?.as_deref() != Some(refresh_token) {
            return Err("OS credential store did not verify the Forge refresh token".into());
        }
        Ok(())
    }

    pub(super) fn clear_refresh_token(&self, credential: &StoredCredential) -> Result<(), String> {
        self.refresh_backend
            .delete(&Self::refresh_account(credential))?;
        if self.load_refresh_token(credential)?.is_some() {
            return Err("OS credential store did not remove the Forge refresh token".into());
        }
        Ok(())
    }

    pub(super) fn save_login(
        &self,
        credential: &StoredCredential,
        refresh_token: Option<&str>,
    ) -> Result<(), String> {
        let _refresh_lock = self.lock_refresh_account(credential)?;
        let previous_refresh_token = self.load_refresh_token(credential)?;
        let token_update = if let Some(refresh_token) = refresh_token {
            self.save_refresh_token(credential, refresh_token)
        } else {
            self.clear_refresh_token(credential)
        };
        if let Err(error) = token_update {
            if self
                .restore_login_refresh_token(credential, previous_refresh_token.as_deref())
                .is_err()
            {
                return Err(
                    "could not update the Forge credential or restore its previous refresh token"
                        .into(),
                );
            }
            return Err(error);
        }
        if let Err(error) = self.save(credential) {
            if self
                .restore_login_refresh_token(credential, previous_refresh_token.as_deref())
                .is_err()
            {
                return Err(
                    "could not save the Forge credential or restore its previous refresh token"
                        .into(),
                );
            }
            return Err(error);
        }
        Ok(())
    }

    fn restore_login_refresh_token(
        &self,
        credential: &StoredCredential,
        previous: Option<&str>,
    ) -> Result<(), String> {
        match previous {
            Some(token) => self.save_refresh_token(credential, token),
            None => self.clear_refresh_token(credential),
        }
    }

    pub(super) fn lock_and_load_refresh_credential(
        &self,
        credential: &StoredCredential,
    ) -> Result<(RefreshLock, StoredCredential), String> {
        let lock = self.lock_refresh_account(credential)?;
        let latest = self.load(
            &credential.issuer,
            &credential.client_id,
            OwnerSelector {
                subject: Some(&credential.subject),
                tenant_id: Some(&credential.tenant_id),
            },
        )?;
        Ok((lock, latest))
    }

    #[cfg(all(unix, not(any(target_os = "redox", target_os = "solaris"))))]
    fn lock_refresh_account(&self, credential: &StoredCredential) -> Result<RefreshLock, String> {
        use std::os::unix::fs::OpenOptionsExt;

        let owner_uid = ensure_directory(&self.config_root, &self.directory, true)?;
        let path = self.directory.join(format!(
            "{}.lock",
            credential_key(
                &credential.issuer,
                &credential.client_id,
                &credential.tenant_id,
                &credential.subject,
            )
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| "could not open the private Forge credential lock".to_owned())?;
        check_file_metadata(
            &file
                .metadata()
                .map_err(|_| "could not inspect the Forge credential lock".to_owned())?,
            owner_uid,
        )?;
        let file = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive)
            .map_err(|(_, _)| "could not lock the Forge credential for refresh".to_owned())?;
        Ok(RefreshLock { _file: file })
    }

    #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "solaris")))))]
    fn lock_refresh_account(&self, _credential: &StoredCredential) -> Result<RefreshLock, String> {
        Err("secure Forge credential locking is unavailable on this platform".into())
    }

    fn refresh_account(credential: &StoredCredential) -> String {
        format!(
            "refresh-{}",
            credential_key(
                &credential.issuer,
                &credential.client_id,
                &credential.tenant_id,
                &credential.subject,
            )
        )
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
        let mut expired = Vec::new();
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
                } else {
                    expired.push(credential);
                }
            }
        }
        match matching.len() {
            0 if expired.len() == 1 => Ok(expired.remove(0)),
            0 if expired.len() > 1 => Err("multiple expired Forge accounts match this issuer and client; set SNAPLINK_SUBJECT and, if needed, SNAPLINK_TENANT_ID".into()),
            0 => Err("no Forge credential matches this issuer and client; run forge-runtime remote login or set FORGE_ACCESS_TOKEN".into()),
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

        use std::sync::atomic::Ordering;

        let nonce = format!(
            ".credential-{}-{}-{}.tmp",
            std::process::id(),
            unix_time()?,
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed),
        );
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

pub(super) fn validate_refresh_token(token: &str) -> Result<(), String> {
    if token.is_empty()
        || token.len() > MAX_REFRESH_TOKEN_BYTES
        || token.trim() != token
        || !token.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err("Snaplink returned an invalid refresh token".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "remote_credentials_tests.rs"]
mod tests;
