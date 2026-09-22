use std::{fs, sync::Arc, time::SystemTime};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use super::{
    CredentialStore, MemoryRefreshTokenBackend, OwnerSelector, RefreshTokenBackend,
    StoredCredential, credential_from_token, credential_storage_capabilities,
};

#[test]
fn credential_storage_capability_contract_is_platform_truthful_and_side_effect_free() {
    let capabilities = credential_storage_capabilities();
    assert_eq!(
        capabilities.schema_version,
        "forge.remote-credential-capabilities/v1"
    );
    assert!(capabilities.access_token_env.available);
    assert_eq!(capabilities.access_token_env.backend, "FORGE_ACCESS_TOKEN");
    assert_eq!(capabilities.access_token_env.reason, None);

    #[cfg(target_os = "linux")]
    {
        assert_eq!(capabilities.platform, "linux");
        assert!(capabilities.refresh_token_os_keyring.available);
        assert!(capabilities.credential_metadata.available);
        assert!(capabilities.refresh_lock.available);
        assert!(capabilities.saved_login.available);
    }
    #[cfg(target_os = "macos")]
    {
        assert_eq!(capabilities.platform, "macos");
        assert!(capabilities.refresh_token_os_keyring.available);
        assert!(capabilities.credential_metadata.available);
        assert!(capabilities.refresh_lock.available);
        assert!(capabilities.saved_login.available);
    }
    #[cfg(target_os = "windows")]
    {
        assert_eq!(capabilities.platform, "windows");
        assert!(capabilities.refresh_token_os_keyring.available);
        assert!(!capabilities.credential_metadata.available);
        assert!(!capabilities.refresh_lock.available);
        assert!(!capabilities.saved_login.available);
        assert_eq!(capabilities.saved_login.backend, "environment token only");
    }
    #[cfg(target_os = "android")]
    {
        assert_eq!(capabilities.platform, "android");
        assert!(!capabilities.refresh_token_os_keyring.available);
        assert!(!capabilities.credential_metadata.available);
        assert!(!capabilities.refresh_lock.available);
        assert!(!capabilities.saved_login.available);
    }
}

#[cfg(unix)]
#[test]
fn owner_client_and_issuer_credentials_are_separate_and_selection_is_explicit() {
    let directory = secure_tempdir();
    let store = store(directory.path());
    let first = credential("https://id.example", "forge-cli", "user-a", "tenant-a");
    let second = credential("https://id.example", "forge-cli", "user-b", "tenant-a");
    store.save(&first).unwrap();
    store.save(&second).unwrap();

    assert_private_modes(&store, &first);
    assert!(
        store
            .load(
                "https://id.example",
                "forge-cli",
                OwnerSelector {
                    subject: None,
                    tenant_id: None,
                },
            )
            .is_err()
    );
    let loaded = store
        .load(
            "https://id.example",
            "forge-cli",
            OwnerSelector {
                subject: Some("user-b"),
                tenant_id: Some("tenant-a"),
            },
        )
        .unwrap();
    assert_eq!(loaded.access_token, second.access_token);
}

#[cfg(unix)]
#[test]
fn credential_lookup_is_bound_to_issuer_and_client() {
    let directory = secure_tempdir();
    let store = store(directory.path());
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");
    store.save(&saved).unwrap();
    for (issuer, client_id) in [
        ("https://other.example", "forge-cli"),
        ("https://id.example", "another-client"),
    ] {
        assert!(
            store
                .load(
                    issuer,
                    client_id,
                    OwnerSelector {
                        subject: Some("user-a"),
                        tenant_id: Some("tenant-a"),
                    },
                )
                .is_err()
        );
    }
}

#[cfg(unix)]
fn assert_private_modes(store: &CredentialStore, credential: &StoredCredential) {
    use std::os::unix::fs::MetadataExt;

    let directory = fs::metadata(&store.directory).unwrap();
    assert_eq!(directory.mode() & 0o777, 0o700);
    let file = fs::metadata(store.path_for(credential)).unwrap();
    assert_eq!(file.mode() & 0o777, 0o600);
    let current_uid = nix::unistd::Uid::effective().as_raw();
    assert_eq!(directory.uid(), current_uid);
    assert_eq!(file.uid(), current_uid);
}

#[cfg(unix)]
#[test]
fn credential_store_rejects_symlinks_and_over_permissive_modes() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let directory = secure_tempdir();
    let store = store(directory.path());
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");
    store.save(&saved).unwrap();
    let path = store.path_for(&saved);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        store
            .load(
                "https://id.example",
                "forge-cli",
                OwnerSelector {
                    subject: None,
                    tenant_id: None,
                },
            )
            .is_err()
    );

    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let target = directory.path().join("outside.json");
    fs::write(&target, b"{}").unwrap();
    fs::remove_file(&path).unwrap();
    symlink(&target, &path).unwrap();
    assert!(store.save(&saved).is_err());
    assert!(
        store
            .load(
                "https://id.example",
                "forge-cli",
                OwnerSelector {
                    subject: None,
                    tenant_id: None,
                },
            )
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn credential_store_refuses_over_permissive_directories() {
    use std::os::unix::fs::PermissionsExt;

    let directory = secure_tempdir();
    let store = store(directory.path());
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");
    store.save(&saved).unwrap();
    fs::set_permissions(&store.directory, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(store.save(&saved).is_err());
    assert!(
        store
            .load(
                "https://id.example",
                "forge-cli",
                OwnerSelector {
                    subject: None,
                    tenant_id: None,
                },
            )
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn credential_store_rejects_world_writable_config_root() {
    use std::os::unix::fs::PermissionsExt;

    let directory = secure_tempdir();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o1777)).unwrap();
    let store = store(directory.path());
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");
    assert!(store.save(&saved).is_err());
}

#[cfg(unix)]
#[test]
fn login_keeps_refresh_token_out_of_the_credential_file_and_verifies_keyring_write() {
    let directory = secure_tempdir();
    let backend = Arc::new(MemoryRefreshTokenBackend::default());
    let store = CredentialStore::with_test_backend(directory.path().to_path_buf(), backend);
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");

    store.save_login(&saved, Some("refresh-secret-01")).unwrap();

    assert_eq!(
        store.load_refresh_token(&saved).unwrap().as_deref(),
        Some("refresh-secret-01")
    );
    let bytes = fs::read(store.path_for(&saved)).unwrap();
    let serialized = String::from_utf8(bytes).unwrap();
    assert!(serialized.contains(&saved.access_token));
    assert!(!serialized.contains("refresh-secret-01"));
}

#[cfg(unix)]
#[test]
fn login_fails_closed_when_the_secure_refresh_store_is_unavailable() {
    struct FailedBackend;

    impl RefreshTokenBackend for FailedBackend {
        fn get(&self, _account: &str) -> Result<Option<String>, String> {
            Err("test keyring read failure".into())
        }

        fn set(&self, _account: &str, _token: &str) -> Result<(), String> {
            Err("test keyring write failure".into())
        }

        fn delete(&self, _account: &str) -> Result<(), String> {
            Err("test keyring delete failure".into())
        }
    }

    let directory = secure_tempdir();
    let store =
        CredentialStore::with_test_backend(directory.path().to_path_buf(), Arc::new(FailedBackend));
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");

    assert!(store.save_login(&saved, Some("refresh-secret-01")).is_err());
    assert!(!store.path_for(&saved).exists());
}

#[cfg(unix)]
#[test]
fn failed_login_restores_the_previous_refresh_token() {
    use std::os::unix::fs::PermissionsExt;

    let directory = secure_tempdir();
    let store = CredentialStore::with_test_backend(
        directory.path().to_path_buf(),
        Arc::new(MemoryRefreshTokenBackend::default()),
    );
    let saved = credential("https://id.example", "forge-cli", "user-a", "tenant-a");
    store.save_login(&saved, Some("refresh-original")).unwrap();

    fs::set_permissions(store.path_for(&saved), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        store
            .save_login(&saved, Some("refresh-replacement"))
            .is_err()
    );

    assert_eq!(
        store.load_refresh_token(&saved).unwrap().as_deref(),
        Some("refresh-original")
    );
}

fn store(config_root: &std::path::Path) -> CredentialStore {
    CredentialStore::for_test(config_root.to_path_buf())
}

#[cfg(unix)]
fn secure_tempdir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

fn credential(issuer: &str, client: &str, subject: &str, tenant: &str) -> StoredCredential {
    credential_from_token(issuer, client, jwt(issuer, subject, tenant), future_time()).unwrap()
}

fn jwt(issuer: &str, subject: &str, tenant: &str) -> String {
    jwt_with(
        issuer,
        subject,
        tenant,
        "forge-api",
        "forge:conversations:read forge:conversations:write",
    )
}

fn jwt_with(issuer: &str, subject: &str, tenant: &str, audience: &str, scopes: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "iss": issuer,
            "sub": subject,
            "tenant_id": tenant,
            "client_id": "forge-cli",
            "aud": audience,
            "scopes": scopes.split_ascii_whitespace().collect::<Vec<_>>(),
            "exp": future_time(),
        }))
        .unwrap(),
    );
    format!("{header}.{payload}.inert-signature")
}

fn future_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600
}
