use std::{collections::BTreeMap, fs, path::Path};

use forge_runtime_domain::{HubStore, HubStoreError};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn shared_database_file_is_rejected_without_chmod_or_other_changes() {
    use std::os::unix::fs::PermissionsExt;

    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    fs::set_permissions(&database, fs::Permissions::from_mode(0o644))
        .expect("make database shared");
    let before = state_files(root.path());

    let error = SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("shared database file is rejected without chmod");

    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    assert_eq!(state_files(root.path()), before);
    assert_eq!(
        fs::metadata(&database)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
}

#[test]
fn clean_rollback_mode_is_rejected_without_changes() {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .expect("switch to rollback journal mode");
    drop(connection);
    assert!(!root.path().join("hub.sqlite3-journal").exists());
    let before = state_files(root.path());

    let error = SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("clean rollback-mode Hub is rejected");

    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(state_files(root.path()), before);
}

#[test]
fn hot_rollback_journal_is_rejected_before_immutable_read() {
    let (root, store) = fixture();
    store
        .create_group("rollback fixture", "rollback-group")
        .expect("create rollback fixture");
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let committed = fs::read(&database).expect("read committed database");
    let writer = rusqlite::Connection::open(&database).expect("open rollback writer");
    writer
        .pragma_update(None, "journal_mode", "DELETE")
        .expect("switch to rollback journal mode");
    writer
        .execute_batch("PRAGMA cache_size = 1; PRAGMA cache_spill = ON; BEGIN EXCLUSIVE")
        .expect("begin spilling rollback transaction");
    let dirty_name = "x".repeat(2 * 1024 * 1024);
    writer
        .execute("UPDATE groups SET name = ?1", [&dirty_name])
        .expect("spill uncommitted pages to main database");
    assert!(root.path().join("hub.sqlite3-journal").exists());
    assert_ne!(fs::read(&database).expect("read dirty main"), committed);
    let before = state_files(root.path());

    let error = SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("hot rollback journal is rejected before immutable open");

    assert!(matches!(error, HubStoreError::Unavailable { .. }));
    assert!(error.to_string().contains("journal"));
    assert_eq!(state_files(root.path()), before);
    writer.execute_batch("ROLLBACK").expect("rollback fixture");
}

fn fixture() -> (TempDir, SqliteHubStore) {
    let root = TempDir::new().expect("temporary root");
    let store = SqliteHubStore::open(root.path().join("hub.sqlite3")).expect("open Hub");
    (root, store)
}

fn state_files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("read state directory")
        .map(|entry| {
            let entry = entry.expect("state entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            let bytes = fs::read(entry.path()).expect("read state file");
            (name, bytes)
        })
        .collect()
}
