use std::{fs, time::SystemTime};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use super::{CredentialStore, OwnerSelector, StoredCredential, credential_from_token};

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

fn store(config_root: &std::path::Path) -> CredentialStore {
    CredentialStore {
        config_root: config_root.to_path_buf(),
        directory: config_root.join("forge-runtime").join("credentials"),
    }
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
