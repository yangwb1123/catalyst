use std::{fs, time::SystemTime};

use super::super::{CredentialStore, StoredCredential, credential_from_token};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

#[cfg(unix)]
#[test]
fn cursor_checkpoint_round_trips_and_is_bound_to_coordinator_and_owner() {
    let root = private_tempdir();
    let credentials = store(root.path());
    let owner_a = credential("user-a", "tenant-a");
    credentials.save(&owner_a).unwrap();
    let cursor_a = credentials.change_cursor_store("https://forge-a.example/", &owner_a);
    assert_eq!(cursor_a.load().unwrap(), 0);
    cursor_a.save(42).unwrap();
    assert_eq!(cursor_a.load().unwrap(), 42);

    let other_coordinator = credentials.change_cursor_store("https://forge-b.example/", &owner_a);
    let other_owner = credentials.change_cursor_store(
        "https://forge-a.example/",
        &credential("user-b", "tenant-a"),
    );
    assert_eq!(other_coordinator.load().unwrap(), 0);
    assert_eq!(other_owner.load().unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn cursor_checkpoint_rejects_invalid_binding_cursor_modes_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = private_tempdir();
    let credentials = store(root.path());
    let owner = credential("user-a", "tenant-a");
    credentials.save(&owner).unwrap();
    let checkpoint = credentials.change_cursor_store("https://forge.example/", &owner);
    checkpoint.save(3).unwrap();
    let path = checkpoint.path.clone();

    let bytes = fs::read(&path).unwrap();
    let mut wrong_binding: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wrong_binding["subject"] = "user-b".into();
    fs::write(&path, serde_json::to_vec(&wrong_binding).unwrap()).unwrap();
    assert!(checkpoint.load().is_err());

    checkpoint.save(4).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(checkpoint.load().is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    let external = root.path().join("outside.cursor");
    fs::write(&external, bytes).unwrap();
    fs::remove_file(&path).unwrap();
    symlink(external, &path).unwrap();
    assert!(checkpoint.load().is_err());
    assert!(checkpoint.save(5).is_err());
}

#[cfg(unix)]
#[test]
fn cursor_checkpoint_rejects_fifo_paths_without_waiting_for_a_writer() {
    use nix::sys::stat::Mode;

    let root = private_tempdir();
    let credentials = store(root.path());
    let owner = credential("user-a", "tenant-a");
    credentials.save(&owner).unwrap();
    let checkpoint = credentials.change_cursor_store("https://forge.example/", &owner);
    nix::unistd::mkfifo(&checkpoint.path, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
    assert!(checkpoint.load().is_err());
}

#[cfg(unix)]
fn private_tempdir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

fn store(config_root: &std::path::Path) -> CredentialStore {
    CredentialStore::for_test(config_root.to_path_buf())
}

fn credential(subject: &str, tenant: &str) -> StoredCredential {
    let issuer = "https://id.example";
    let token = jwt(issuer, subject, tenant);
    let expires = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    credential_from_token(issuer, "forge-cli", token, expires).unwrap()
}

fn jwt(issuer: &str, subject: &str, tenant: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let expiry = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "iss": issuer,
            "sub": subject,
            "tenant_id": tenant,
            "client_id": "forge-cli",
            "aud": "forge-api",
            "scopes": ["forge:conversations:read", "forge:conversations:write"],
            "exp": expiry,
        }))
        .unwrap(),
    );
    format!("{header}.{payload}.inert-signature")
}
