use std::path::Path;

use tempfile::TempDir;

use forge_runtime_infrastructure::SqliteHubStore;

#[allow(dead_code)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code)]
mod sqlite_group_agent_scheduled_node_contract_support;
#[allow(dead_code)]
mod sqlite_group_agent_scheduled_node_provider_request_support;

// v26 widened the endpoint CHECK on the analyses/syntheses tables; a
// downgraded fixture must restore the historical definitions.
const RESTORE_HISTORICAL_ANALYSES_SQL: &str = include_str!("restore_historical_analyses.sql");
fn fixture() -> (TempDir, SqliteHubStore) {
    let root = TempDir::new().expect("hub root");
    let store = SqliteHubStore::open(root.path().join("hub.sqlite3").as_path()).expect("open hub");
    (root, store)
}

#[test]
fn upgrade_backs_up_the_old_hub_before_migration() {
    // Production-readiness condition (stage-06 High): an irreversible
    // migration must snapshot the pre-upgrade hub first. Downgrade a hub
    // to the exact additive predecessor, then open it and prove a backups/
    // snapshot exists at the old version.
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    downgrade_to_v24(&database);

    SqliteHubStore::open(&database).expect("migrate hub to current");

    let backup_path = single_backup(root.path(), 24);
    let version: i64 = rusqlite::Connection::open(&backup_path)
        .expect("open backup")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("backup version");
    assert_eq!(version, 24, "backup must preserve the old schema version");
}

#[test]
fn upgrade_backup_includes_committed_hot_wal_pages() {
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let writer = rusqlite::Connection::open(&database).expect("open raw Hub");
    downgrade_to_v26(&writer);
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA wal_autocheckpoint=0;")
        .expect("checkpoint exact v26 schema");
    writer
        .execute(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms)
             VALUES(?1,?2,?3,?4)",
            (
                "hot-backup-group",
                "Hot backup group",
                "hot-backup-key",
                1_i64,
            ),
        )
        .expect("commit marker into hot WAL");
    let wal = database.with_file_name("hub.sqlite3-wal");
    assert!(
        std::fs::metadata(&wal).expect("hot WAL metadata").len() > 32,
        "committed marker must remain in the WAL before migration"
    );

    SqliteHubStore::open(&database).expect("migrate hot-WAL v26 hub");

    let backup =
        rusqlite::Connection::open(single_backup(root.path(), 26)).expect("open consistent backup");
    let marker: String = backup
        .query_row(
            "SELECT id FROM groups WHERE id='hot-backup-group'",
            [],
            |row| row.get(0),
        )
        .expect("backup contains committed WAL marker");
    assert_eq!(marker, "hot-backup-group");
    drop(writer);
}

fn single_backup(root: &Path, version: i64) -> std::path::PathBuf {
    let backups = root.join("backups");
    let entries: Vec<_> = std::fs::read_dir(&backups)
        .expect("backups directory")
        .map(|entry| entry.expect("backup entry").path())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one pre-upgrade backup");
    assert!(
        entries[0]
            .file_name()
            .expect("backup name")
            .to_string_lossy()
            .contains(&format!("v{version}-before-upgrade")),
        "backup must pin pre-upgrade v{version}: {entries:?}"
    );
    entries.into_iter().next().expect("one backup")
}

fn downgrade_to_v26(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DROP TABLE run_lineages;
             DROP TABLE governance_claim_validation_jobs;
             DROP TABLE governance_claim_semantic_views;
             DROP TABLE governance_semantic_heads;
             PRAGMA user_version=26;",
        )
        .expect("downgrade semantic view schema to v26");
}

fn downgrade_to_v24(database: &Path) {
    let connection = rusqlite::Connection::open(database).expect("open raw Hub");
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DROP TABLE run_lineages;
             DROP TABLE governance_claim_validation_jobs;
             DROP TABLE governance_claim_semantic_views;
             DROP TABLE governance_semantic_heads;
             DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             PRAGMA user_version=24;",
        )
        .expect("downgrade journal schema to v24");
    connection
        .execute_batch(RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore historical analyses definitions");
}
