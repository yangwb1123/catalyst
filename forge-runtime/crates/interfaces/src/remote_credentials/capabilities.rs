use super::{CapabilityStatus, CredentialStorageCapabilities};

const CREDENTIAL_STORAGE_CAPABILITIES_SCHEMA: &str = "forge.remote-credential-capabilities/v1";

pub(in crate::remote_command) fn credential_storage_capabilities() -> CredentialStorageCapabilities
{
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
