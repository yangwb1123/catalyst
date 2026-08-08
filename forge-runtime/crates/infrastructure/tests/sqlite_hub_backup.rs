use tempfile::TempDir;

use forge_runtime_infrastructure::SqliteHubStore;

fn fixture() -> (TempDir, SqliteHubStore) {
    let root = TempDir::new().expect("hub root");
    let store = SqliteHubStore::open(root.path().join("hub.sqlite3").as_path()).expect("open hub");
    (root, store)
}

#[test]
fn upgrade_backs_up_the_old_hub_before_migration() {
    // Production-readiness condition (stage-06 High): an irreversible
    // migration must snapshot the pre-upgrade hub first. Downgrade a hub
    // to v16, then open it and prove a backups/ snapshot exists at the old
    // version.
    let (root, store) = fixture();
    drop(store);
    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
             DROP TABLE group_agent_graph_scheduled_node_successor_candidates;
             DROP TABLE group_agent_graph_scheduled_node_provider_requests;
             PRAGMA user_version=14;",
        )
        .expect("downgrade to v14");
    drop(connection);

    SqliteHubStore::open(&database).expect("migrate hub to current");

    let backups = root.path().join("backups");
    let entries: Vec<_> = std::fs::read_dir(&backups)
        .expect("backups directory")
        .map(|entry| entry.expect("backup entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one pre-upgrade backup: {entries:?}");
    assert!(
        entries[0].contains("v14-before-upgrade"),
        "backup must pin the pre-upgrade version: {entries:?}"
    );
    let backup_path = backups.join(&entries[0]);
    let version: i64 = rusqlite::Connection::open(&backup_path)
        .expect("open backup")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("backup version");
    assert_eq!(version, 14, "backup must preserve the old schema version");
}
