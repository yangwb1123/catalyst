use std::{fs, time::SystemTime};

use super::super::{CredentialStore, StoredCredential, credential_from_token};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

#[cfg(unix)]
#[test]
fn run_timeline_checkpoint_round_trips_and_partitions_each_run() {
    let root = private_tempdir();
    let credentials = store(root.path());
    let owner = credential("user-a", "tenant-a");
    credentials.save(&owner).unwrap();
    let first = credentials.change_cursor_store("https://forge.example/", &owner);
    let first_run = first.run_timeline_cursor_store("conversation-1", "run-1");
    assert_eq!(first_run.load().unwrap(), 0);
    assert!(first_run.save(42).unwrap());
    assert_eq!(first_run.load().unwrap(), 42);

    let second_run = first.run_timeline_cursor_store("conversation-1", "run-2");
    let second_conversation = first.run_timeline_cursor_store("conversation-2", "run-1");
    let other_owner = credentials
        .change_cursor_store("https://forge.example/", &credential("user-b", "tenant-a"))
        .run_timeline_cursor_store("conversation-1", "run-1");
    let other_coordinator = credentials
        .change_cursor_store("https://other.example/", &owner)
        .run_timeline_cursor_store("conversation-1", "run-1");
    assert_eq!(second_run.load().unwrap(), 0);
    assert_eq!(second_conversation.load().unwrap(), 0);
    assert_eq!(other_owner.load().unwrap(), 0);
    assert_eq!(other_coordinator.load().unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn run_timeline_checkpoint_rejects_regression_modes_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = private_tempdir();
    let credentials = store(root.path());
    let owner = credential("user-a", "tenant-a");
    credentials.save(&owner).unwrap();
    let checkpoint = credentials
        .change_cursor_store("https://forge.example/", &owner)
        .run_timeline_cursor_store("conversation-1", "run-1");
    assert!(checkpoint.save(3).unwrap());
    assert!(!checkpoint.save(2).unwrap());
    assert_eq!(checkpoint.load().unwrap(), 3);
    let path = checkpoint.path.clone();

    let bytes = fs::read(&path).unwrap();
    let mut wrong_binding: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wrong_binding["run_id"] = "run-2".into();
    fs::write(&path, serde_json::to_vec(&wrong_binding).unwrap()).unwrap();
    assert!(checkpoint.load().is_err());

    fs::remove_file(&path).unwrap();
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
fn run_timeline_checkpoint_uses_json_safe_integer_boundary() {
    let root = private_tempdir();
    let credentials = store(root.path());
    let owner = credential("user-a", "tenant-a");
    credentials.save(&owner).unwrap();
    let checkpoint = credentials
        .change_cursor_store("https://forge.example/", &owner)
        .run_timeline_cursor_store("conversation-1", "run-1");
    assert!(checkpoint.save(9_007_199_254_740_991).unwrap());
    assert_eq!(checkpoint.load().unwrap(), 9_007_199_254_740_991);
    assert!(checkpoint.save(9_007_199_254_740_992).is_err());
}

#[cfg(unix)]
#[test]
fn run_timeline_checkpoint_rejects_fifo_paths_without_waiting_for_writer() {
    use nix::sys::stat::Mode;

    let root = private_tempdir();
    let credentials = store(root.path());
    let owner = credential("user-a", "tenant-a");
    credentials.save(&owner).unwrap();
    let checkpoint = credentials
        .change_cursor_store("https://forge.example/", &owner)
        .run_timeline_cursor_store("conversation-1", "run-1");
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
