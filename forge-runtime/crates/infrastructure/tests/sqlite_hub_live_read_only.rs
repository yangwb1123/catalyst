use std::{fs, path::Path};

use forge_runtime_domain::{HubStore, HubStoreError};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn exact_current_live_reader_opens_clean_state_without_changing_logical_content() {
    let (_root, database) = fixture();
    let main_before = fs::read(&database).expect("read clean current main database");

    let live = SqliteHubStore::open_existing_current_live_read_only(&database)
        .expect("open clean exact-v28 live reader");
    assert!(live.list_groups().expect("read clean snapshot").is_empty());
    drop(live);

    assert_eq!(
        fs::read(&database).expect("read clean main after snapshot"),
        main_before,
        "live read must not change logical main-database content"
    );
    assert!(!sidecar(&database, "-journal").exists());
}

#[test]
fn exact_current_live_reader_includes_committed_hot_wal_without_logical_writes() {
    let (root, database) = fixture();
    let writer = hot_writer(&database);
    writer
        .execute(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms) VALUES(?1,?2,?3,?4)",
            ("hot-live", "Hot live group", "hot-live-key", 1_i64),
        )
        .expect("commit group into hot WAL");
    let main_before = fs::read(&database).expect("read main before live read");
    let wal_before = fs::read(sidecar(&database, "-wal")).expect("read WAL before live read");

    SqliteHubStore::open_existing_current_read_only(&database)
        .expect_err("immutable read contract still rejects hot WAL");
    let live = SqliteHubStore::open_existing_current_live_read_only(&database)
        .expect("open exact current live snapshot");
    let groups = live.list_groups().expect("read group through hot WAL");
    assert!(groups.iter().any(|group| group.id == "hot-live"));
    live.create_group("forbidden", "forbidden-live-write")
        .expect_err("mode=ro plus query_only rejects writes");
    drop(live);

    assert_eq!(fs::read(&database).expect("read main after"), main_before);
    assert_eq!(
        fs::read(sidecar(&database, "-wal")).expect("read WAL after"),
        wal_before
    );
    drop(writer);
    drop(root);
}

#[test]
fn exact_current_live_reader_rejects_old_and_future_hot_wal_versions() {
    let (_old_root, old_database) = fixture();
    let old_writer = hot_writer(&old_database);
    downgrade_empty_v28_to_v26(&old_writer);
    let old = SqliteHubStore::open_existing_current_live_read_only(&old_database)
        .expect_err("valid v26 requires an explicit upgrade");
    assert!(matches!(old, HubStoreError::Unavailable { .. }), "{old}");
    assert!(
        old.to_string().contains("valid v26; upgrade required"),
        "{old}"
    );

    let (_future_root, future_database) = fixture();
    let future_writer = hot_writer(&future_database);
    future_writer
        .pragma_update(None, "user_version", 29)
        .expect("commit future version into hot WAL");
    let future = SqliteHubStore::open_existing_current_live_read_only(&future_database)
        .expect_err("future schema is unsupported");
    assert!(matches!(future, HubStoreError::Corrupt { .. }), "{future}");
}

#[test]
fn exact_current_live_reader_rejects_incomplete_or_malformed_sidecars() {
    let cases = [
        ("wal_only", valid_wal_header().to_vec(), None),
        ("short_shm", valid_wal_header().to_vec(), Some(vec![0; 1])),
        ("bad_wal", vec![0; 32], Some(vec![0; 32_768])),
    ];
    for (name, wal, shm) in cases {
        let (_root, database) = fixture();
        write_private(&sidecar(&database, "-wal"), &wal);
        if let Some(bytes) = shm {
            write_private(&sidecar(&database, "-shm"), &bytes);
        }
        let error = SqliteHubStore::open_existing_current_live_read_only(&database)
            .expect_err("incomplete live sidecars must fail closed");
        assert!(
            matches!(error, HubStoreError::Corrupt { .. }),
            "{name}: {error}"
        );
    }
}

#[test]
fn exact_current_live_reader_rejects_rollback_state_and_missing_database() {
    let (root, database) = fixture();
    write_private(&sidecar(&database, "-journal"), b"not a WAL snapshot");
    let error = SqliteHubStore::open_existing_current_live_read_only(&database)
        .expect_err("rollback journal must fail closed");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error}");

    let missing = root.path().join("missing.sqlite3");
    let error = SqliteHubStore::open_existing_current_live_read_only(missing)
        .expect_err("missing database must not be created");
    assert!(
        matches!(error, HubStoreError::Unavailable { .. }),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn exact_current_live_reader_reports_unavailable_on_a_truly_read_only_directory() {
    use std::os::unix::fs::PermissionsExt;

    let (root, database) = fixture();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o500))
        .expect("make Hub directory read-only");
    if fs::write(root.path().join("permission-probe"), b"probe").is_ok() {
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("restore writable fixture directory");
        return;
    }

    let result = SqliteHubStore::open_existing_current_live_read_only(&database);
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
        .expect("restore writable fixture directory");
    let error = result.expect_err("live SQLite coordination needs writable sidecar storage");
    assert!(
        matches!(error, HubStoreError::Unavailable { .. }),
        "{error}"
    );
}

fn fixture() -> (TempDir, std::path::PathBuf) {
    let root = TempDir::new().expect("temporary Hub root");
    let database = root.path().join("hub.sqlite3");
    drop(SqliteHubStore::open(&database).expect("initialize current Hub"));
    (root, database)
}

fn hot_writer(database: &Path) -> Connection {
    let writer = Connection::open(database).expect("open WAL writer");
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint=0;")
        .expect("prepare hot WAL writer");
    writer
}

fn downgrade_empty_v28_to_v26(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TABLE run_lineages;
             DROP TABLE governance_claim_validation_jobs;
             DROP TABLE governance_claim_semantic_views;
             DROP TABLE governance_semantic_heads;
             PRAGMA user_version=26;",
        )
        .expect("restore valid empty v26 schema in hot WAL");
}

fn sidecar(database: &Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{suffix}", database.display()))
}

fn valid_wal_header() -> [u8; 32] {
    let mut header = [0_u8; 32];
    header[..4].copy_from_slice(&[0x37, 0x7f, 0x06, 0x82]);
    header
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write sidecar fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("restrict sidecar fixture");
    }
}
