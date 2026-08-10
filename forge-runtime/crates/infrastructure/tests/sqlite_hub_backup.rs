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

    let backups = root.path().join("backups");
    let entries: Vec<_> = std::fs::read_dir(&backups)
        .expect("backups directory")
        .map(|entry| {
            entry
                .expect("backup entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "exactly one pre-upgrade backup: {entries:?}"
    );
    assert!(
        entries[0].contains("v24-before-upgrade"),
        "backup must pin the pre-upgrade version: {entries:?}"
    );
    let backup_path = backups.join(&entries[0]);
    let version: i64 = rusqlite::Connection::open(&backup_path)
        .expect("open backup")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("backup version");
    assert_eq!(version, 24, "backup must preserve the old schema version");
}

fn downgrade_to_v24(database: &Path) {
    let connection = rusqlite::Connection::open(database).expect("open raw Hub");
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
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
