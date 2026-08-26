use forge_runtime_domain::{HubStore, HubStoreError};
use forge_runtime_infrastructure::SqliteHubStore;
use std::{collections::BTreeMap, fs, path::Path};
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
#[test]
fn dispatch_reentry_rejects_incomplete_or_malformed_wal_sidecars_without_changes() {
    let cases = [
        ("wal_without_shm", valid_wal_header().to_vec(), None),
        (
            "truncated_shm",
            valid_wal_header().to_vec(),
            Some(vec![0; 1]),
        ),
        ("invalid_wal", vec![0; 32], Some(vec![0; 32_768])),
    ];
    for (name, wal, shm) in cases {
        let (root, store) = fixture();
        drop(store);
        let database = root.path().join("hub.sqlite3");
        write_private(&root.path().join("hub.sqlite3-wal"), &wal);
        if let Some(bytes) = shm {
            write_private(&root.path().join("hub.sqlite3-shm"), &bytes);
        }
        let before = state_files(root.path());
        let result = SqliteHubStore::open_existing_dispatch_inspection_read_only(&database);
        assert!(result.is_err(), "accepted malformed sidecars: {name}");
        assert_eq!(state_files(root.path()), before, "changed state: {name}");
    }
}
#[test]
fn dispatch_reentry_preserves_corrupt_classification_from_hot_wal() {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let writer = rusqlite::Connection::open(&database).expect("open WAL writer");
    writer
        .execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA wal_autocheckpoint=0;
             CREATE TABLE rogue_hot_wal_schema(id TEXT PRIMARY KEY);",
        )
        .expect("commit rogue schema to hot WAL");
    let wal = root.path().join("hub.sqlite3-wal");
    let main_before = fs::read(&database).expect("read main before re-entry");
    let wal_before = fs::read(&wal).expect("read WAL before re-entry");
    let error = SqliteHubStore::open_existing_dispatch_inspection_read_only(&database)
        .expect_err("rogue hot-WAL schema must be corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error}");
    assert_eq!(fs::read(&database).expect("read main after"), main_before);
    assert_eq!(fs::read(&wal).expect("read WAL after"), wal_before);
    drop(writer);
}
#[test]
fn dispatch_reentry_reads_real_hot_v12_through_v16_wals_without_logical_changes() {
    for version in [12, 13, 14, 15, 16] {
        assert_hot_wal_reentry(version);
    }
}
fn assert_hot_wal_reentry(version: i64) {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let writer = rusqlite::Connection::open(&database).expect("open WAL writer");
    restore_schema_version(&writer, version);
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint=0;")
        .expect("checkpoint schema before hot write");
    writer
        .execute(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms)
             VALUES(?1,?2,?3,?4)",
            ("hot-group", "Hot WAL group", "hot-group-key", 1_i64),
        )
        .expect("commit one hot-WAL row");
    let wal = root.path().join("hub.sqlite3-wal");
    let shm = root.path().join("hub.sqlite3-shm");
    assert!(wal.exists() && shm.exists(), "missing hot SQLite sidecars");
    let main_before = fs::read(&database).expect("read main before re-entry");
    let wal_before = fs::read(&wal).expect("read WAL before re-entry");
    let reader = SqliteHubStore::open_existing_dispatch_inspection_read_only(&database)
        .expect("open exact hot-WAL dispatch re-entry");
    let groups = reader.list_groups().expect("read through hot WAL");
    assert!(groups.iter().any(|group| group.id == "hot-group"));
    drop(reader);
    assert_eq!(fs::read(&database).expect("read main after"), main_before);
    assert_eq!(fs::read(&wal).expect("read WAL after"), wal_before);
    drop(writer);
}
fn restore_schema_version(connection: &rusqlite::Connection, version: i64) {
    connection
        .execute_batch(RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore pre-v26 analyses definitions");
    restore_v24_schema(connection);
    if version < 17 {
        restore_v16_schema(connection);
    }
    if version < 16 {
        restore_v15_schema(connection);
    }
    if version < 15 {
        connection
            .execute_batch(
                "DROP TABLE group_agent_graph_scheduled_node_provider_requests;
                 PRAGMA user_version=14;",
            )
            .expect("restore exact v14 schema");
    }
    if version < 14 {
        connection
            .execute_batch(
                "DROP INDEX group_agent_graph_scheduled_node_candidates_project_lane;
                 DROP INDEX group_agent_graph_scheduled_node_candidates_created;
                 DROP TABLE group_agent_graph_scheduled_node_contract_candidates;
                 PRAGMA user_version=13;",
            )
            .expect("restore exact v13 schema");
    }
    if version < 13 {
        connection
            .execute_batch(
                "DROP INDEX group_agent_graph_execution_schedules_created;
                 DROP TABLE group_agent_graph_execution_schedules;
                 PRAGMA user_version=12;",
            )
            .expect("restore exact v12 schema");
    }
}
// v26 widened the endpoint CHECK on the analyses/syntheses tables; a
// downgraded fixture must restore the historical definitions.
const RESTORE_HISTORICAL_ANALYSES_SQL: &str = include_str!("restore_historical_analyses.sql");

fn restore_v24_schema(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "DROP TABLE run_lineages;
             DROP TABLE governance_claim_validation_jobs;
             DROP TABLE governance_claim_semantic_views;
             DROP TABLE governance_semantic_heads;
             DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             PRAGMA user_version=24;",
        )
        .expect("restore exact v24 schema");
}
fn valid_wal_header() -> [u8; 32] {
    let mut header = [0_u8; 32];
    header[..4].copy_from_slice(&[0x37, 0x7f, 0x06, 0x82]);
    header
}
fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write private sidecar fixture");
    restrict_file(path);
}
#[cfg(unix)]
fn restrict_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private sidecar mode");
}
#[cfg(not(unix))]
fn restrict_file(_path: &Path) {}
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
fn v16_lifecycle_sql() -> &'static str {
    const SOURCE: &str = include_str!("../src/sqlite_hub/schema_contract/v16_sql.rs");
    SOURCE
        .strip_prefix("pub(super) const MIGRATE_V15_TO_V16_SQL: &str =\n    \"")
        .and_then(|value| value.strip_suffix("\";\n"))
        .expect("embedded v16 lifecycle DDL")
}

fn v15_provider_request_sql() -> &'static str {
    const SOURCE: &str = include_str!("../src/sqlite_hub/schema_contract/v15_sql.rs");
    SOURCE
        .strip_prefix("pub(super) const MIGRATE_V14_TO_V15_SQL: &str =\n    \"")
        .and_then(|value| value.strip_suffix("\";\n"))
        .expect("embedded v15 provider request DDL")
}

fn restore_v16_schema(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "DROP INDEX group_agent_graph_scheduled_node_successor_candidates_created;
             DROP TABLE group_agent_graph_scheduled_node_successor_candidates;
             DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active;
             DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_created;
             DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
             DROP TABLE group_agent_graph_scheduled_node_provider_requests;",
        )
        .expect("drop v19/v18-shaped tables for v16");
    connection
        .execute_batch(v16_lifecycle_sql())
        .expect("rebuild exact v16 lifecycle table for v16");
    connection
        .execute_batch(v15_provider_request_sql())
        .expect("rebuild exact v15 provider request table for v16");
    connection
        .pragma_update(None, "user_version", 16)
        .expect("mark v16 schema");
}
fn restore_v15_schema(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active;
             DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_created;
             DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
             DROP TABLE group_agent_graph_scheduled_node_provider_requests;",
        )
        .expect("drop v18-shaped tables");
    connection
        .execute_batch(v15_provider_request_sql())
        .expect("rebuild exact v15 provider request table");
    connection
        .pragma_update(None, "user_version", 15)
        .expect("mark v15 schema");
}
