use forge_runtime_domain::GroupAgentScheduledNodeContractStore;
use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V13_TO_V14_SQL, MIGRATE_V14_TO_V15_SQL, migrate_with_before_final_fault_for_test,
    open_database, open_existing_dispatch_preflight_read_only_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_exists, schema_object_named,
    schema_v12_migration_tests::legacy_v11_database,
    schema_v13_migration_tests::legacy_active_v12_database,
    schema_v14_migration_tests::legacy_active_v13_database,
    schema_version, table_columns,
};

const REQUEST_TABLE: &str = "group_agent_graph_scheduled_node_provider_requests";
const PROJECT_LANE_INDEX: &str = "group_agent_graph_scheduled_node_provider_requests_project_lane";
const CREATED_INDEX: &str = "group_agent_graph_scheduled_node_provider_requests_created";
const V15_OBJECTS: &[&str] = &[REQUEST_TABLE, PROJECT_LANE_INDEX, CREATED_INDEX];
const V16_TABLE: &str = "group_agent_graph_scheduled_node_dispatch_lifecycles";
const V16_INDEXES: &[&str] = &[
    "group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active",
    "group_agent_graph_scheduled_node_dispatch_lifecycles_created",
];
const REQUEST_COLUMNS: &[&str] = &[
    "id",
    "graph_run_id",
    "schedule_id",
    "scheduled_contract_id",
    "provider_request_version",
    "codec_protocol_version",
    "execution_ordinal",
    "node_id",
    "attempt",
    "scheduled_contract_sha256",
    "logical_request_id",
    "logical_request_sha256",
    "schedule_sha256",
    "project_lane_sha256",
    "provider_kind",
    "endpoint",
    "model",
    "destination_sha256",
    "pricing_snapshot_sha256",
    "provider_request_blob",
    "provider_request_bytes",
    "provider_request_sha256",
    "prepared_request_sha256",
    "expected_last_event_seq",
    "expected_last_event_sha256",
    "provider_request_prepared",
    "provider_request_sent",
    "lifecycle_contract_admitted",
    "execution_authority_released",
    "dispatch_authority_released",
    "project_lane_claimed",
    "progress_observed",
    "successor_advance_authorized",
    "idempotency_key",
    "created_at_ms",
];

#[cfg(unix)]
#[test]
fn immutable_dispatch_preflight_accepts_clean_v11_through_v15_without_changes() {
    let fixtures = [
        legacy_v11_database(),
        legacy_active_v12_database(),
        legacy_active_v13_database(),
        legacy_active_v14_database(),
        legacy_active_v15_database(),
    ];
    for (expected_version, (root, database)) in (11..=15).zip(fixtures) {
        assert_clean_immutable_preflight(&database, expected_version);
        drop(root);
    }
}

#[cfg(unix)]
fn assert_clean_immutable_preflight(database: &std::path::Path, expected_version: i64) {
    use std::os::unix::fs::PermissionsExt;

    let wal = Connection::open(database).expect("open immutable matrix fixture");
    wal.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("persist immutable matrix WAL header");
    wal.close()
        .map_err(|(_, error)| error)
        .expect("close immutable matrix fixture");
    std::fs::set_permissions(database, std::fs::Permissions::from_mode(0o600))
        .expect("restrict immutable matrix fixture");
    let before = std::fs::read(database).expect("read immutable matrix fixture");

    let reader = open_existing_dispatch_preflight_read_only_database(database)
        .expect("open clean immutable dispatch preflight");
    assert_eq!(schema_version(&reader), expected_version);
    drop(reader);
    assert_eq!(std::fs::read(database).expect("reread fixture"), before);
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = std::path::PathBuf::from(format!("{}{suffix}", database.display()));
        assert!(!sidecar.exists(), "created sidecar {}", sidecar.display());
    }
}

#[test]
fn populated_v14_candidate_and_all_prior_schema_survive_v15_migration_and_reopen() {
    let (fixture, request) =
        super::sqlite_group_agent_scheduled_node_contract_support::prepared_fixture();
    fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("admit v14 candidate fixture");
    let database = fixture.database.clone();
    let legacy = fixture.connection();
    let before_candidate = candidate_row(&legacy);
    downgrade_current_to_v14(&legacy);
    let before_schema = schema_snapshot(&legacy);
    assert_eq!(schema_version(&legacy), 14);
    drop(legacy);

    let migrated = open_database(&database).expect("populated v14 Hub migrates to v16");
    assert_current_shape(&migrated);
    assert_eq!(
        without_v15_and_v16(&schema_snapshot(&migrated)),
        without_v15_and_v16(&before_schema)
    );
    assert_eq!(candidate_row(&migrated), before_candidate);
    assert_foreign_keys_clean(&migrated);
    drop(migrated);

    let reopened = open_database(&database).expect("migrated v16 Hub reopens");
    assert_current_shape(&reopened);
    assert_eq!(candidate_row(&reopened), before_candidate);
    drop((reopened, fixture));
}

fn downgrade_current_to_v14(legacy: &Connection) {
    legacy
        .execute_batch(super::DROP_V29_CONTROLLER_SQL)
        .expect("drop v29 controller journal");
    legacy
        .execute_batch(&format!(
            "DROP TABLE {REQUEST_TABLE};
             DROP INDEX {};
             DROP INDEX {};
             DROP TABLE {V16_TABLE};
             DROP TABLE group_agent_graph_scheduled_node_successor_candidates;
             {}
             {}
             DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             PRAGMA user_version=14;",
            V16_INDEXES[0],
            V16_INDEXES[1],
            super::DROP_V28_LINEAGE_SQL,
            super::DROP_V27_SEMANTIC_VIEW_SQL,
        ))
        .expect("downgrade empty v15 suffix to exact v14");
    legacy
        .execute_batch(super::RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore historical analyses definitions for downgraded fixture");
}

#[test]
fn active_v14_data_survive_v15_migration_without_rebuilding_old_tables() {
    let (root, database) = legacy_active_v14_database();
    let legacy = Connection::open(&database).expect("open active v14 fixture");
    let before_schema = schema_snapshot(&legacy);
    let before_run = active_run(&legacy);
    drop(legacy);

    let migrated = open_database(&database).expect("active v14 Hub migrates to v16");
    assert_current_shape(&migrated);
    assert_eq!(
        without_v15_and_v16(&schema_snapshot(&migrated)),
        without_v15_and_v16(&before_schema)
    );
    assert_eq!(active_run(&migrated), before_run);
    assert_foreign_keys_clean(&migrated);
    drop((migrated, root));
}

#[test]
fn v14_future_request_table_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_active_v14_database();
    let blocker = Connection::open(&database).expect("open v14 blocker fixture");
    let before_run = active_run(&blocker);
    blocker
        .execute_batch(&format!("CREATE TABLE {REQUEST_TABLE}(blocker TEXT)"))
        .expect("install future v15 table blocker");
    drop(blocker);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen rejected v14 fixture");
    assert_eq!(schema_version(&unchanged), 14);
    assert_eq!(table_columns(&unchanged, REQUEST_TABLE), ["blocker"]);
    assert!(!schema_object_named(&unchanged, PROJECT_LANE_INDEX));
    assert!(!schema_object_named(&unchanged, CREATED_INDEX));
    assert_eq!(active_run(&unchanged), before_run);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v14_to_v15_atomically() {
    let (root, database) = legacy_active_v14_database();
    let connection = Connection::open(&database).expect("open v14 rollback fixture");
    let before_schema = schema_snapshot(&connection);
    let before_run = active_run(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v15_final_fault(id TEXT)")
    })
    .expect_err("final v15 validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(schema_version(&connection), 14);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(active_run(&connection), before_run);
    for object in V15_OBJECTS {
        assert!(!schema_object_named(&connection, object));
    }
    assert!(!schema_object_named(&connection, "rogue_v15_final_fault"));
    drop((connection, root));
}

#[test]
fn malformed_v15_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v15_cases() {
        let (root, database) = legacy_active_v14_database();
        let connection = Connection::open(&database).expect("open malformed v15 fixture");
        let before_run = active_run(&connection);
        connection.execute_batch(&sql).expect("forge malformed v15");
        drop(connection);

        assert_open_corrupt(&database);
        let unchanged = Connection::open(&database).expect("reopen malformed v15");
        assert_eq!(schema_version(&unchanged), 15);
        assert_eq!(active_run(&unchanged), before_run);
        drop((unchanged, root));
    }
}

#[test]
fn current_physical_columns_and_catalog_counts_are_locked() {
    let (root, database) = legacy_active_v14_database();
    let connection = open_database(&database).expect("migrate contract fixture");
    assert_current_shape(&connection);
    assert_eq!(table_columns(&connection, REQUEST_TABLE), REQUEST_COLUMNS);
    let (tables, implicit_indexes, explicit_indexes): (i64, i64, i64) = connection
        .query_row(
            "SELECT
               SUM(type='table'),
               SUM(type='index' AND sql IS NULL),
               SUM(type='index' AND sql IS NOT NULL)
             FROM sqlite_schema",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("catalog counts");
    assert_eq!((tables, implicit_indexes, explicit_indexes), (42, 107, 40));
    drop((connection, root));
}

fn legacy_active_v14_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_active_v13_database();
    let connection = Connection::open(&database).expect("open v13 fixture");
    connection
        .execute_batch(MIGRATE_V13_TO_V14_SQL)
        .expect("migrate fixture to v14");
    drop(connection);
    (root, database)
}

fn legacy_active_v15_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_active_v14_database();
    let connection = Connection::open(&database).expect("open v14 fixture");
    connection
        .execute_batch(MIGRATE_V14_TO_V15_SQL)
        .expect("migrate fixture to v15");
    drop(connection);
    (root, database)
}

fn malformed_v15_cases() -> Vec<String> {
    vec![
        malformed(
            "scheduled_contract_id TEXT NOT NULL UNIQUE",
            "scheduled_contract_id TEXT NOT NULL",
        ),
        malformed(
            "REFERENCES group_agent_graph_scheduled_node_contract_candidates(request_id)",
            "REFERENCES group_agent_graph_scheduled_node_contract_candidates(id)",
        ),
        malformed(
            "provider_request_version = 1",
            "provider_request_version BETWEEN 1 AND 2",
        ),
        malformed("execution_ordinal = 0", "execution_ordinal >= 0"),
        malformed("attempt = 1", "attempt BETWEEN 1 AND 2"),
        malformed(
            "provider_request_prepared = 1",
            "provider_request_prepared IN (0, 1)",
        ),
        malformed(
            "successor_advance_authorized = 0",
            "successor_advance_authorized IN (0, 1)",
        ),
        malformed(
            "ON group_agent_graph_scheduled_node_provider_requests(created_at_ms DESC, id DESC)",
            "ON group_agent_graph_scheduled_node_provider_requests(created_at_ms, id DESC)",
        ),
        format!("{MIGRATE_V14_TO_V15_SQL}\nCREATE TABLE rogue_v15_table(id TEXT);"),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V14_TO_V15_SQL.replacen(original, replacement, 1);
    assert_ne!(
        sql, MIGRATE_V14_TO_V15_SQL,
        "fixture replacement must match"
    );
    sql
}

fn assert_current_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), super::SCHEMA_VERSION);
    assert!(schema_object_exists(connection, "table", REQUEST_TABLE));
    assert!(schema_object_exists(
        connection,
        "index",
        PROJECT_LANE_INDEX
    ));
    assert!(schema_object_exists(connection, "index", CREATED_INDEX));
    assert_eq!(row_count(connection, REQUEST_TABLE), 0);
    assert!(schema_object_exists(connection, "table", V16_TABLE));
    for index in V16_INDEXES {
        assert!(schema_object_exists(connection, "index", index));
    }
}

fn without_v15_and_v16(snapshot: &[SchemaRow]) -> Vec<SchemaRow> {
    snapshot
        .iter()
        .filter(|(_, name, _, _)| {
            !V15_OBJECTS.contains(&name.as_str())
                && (*name != "group_model_analyses"
                    && *name != "group_model_analysis_events"
                    && *name != "group_model_analysis_results"
                    && *name != "group_panel_syntheses"
                    && *name != "group_panel_synthesis_events"
                    && *name != "group_panel_synthesis_results"
                    && *name != "group_model_analyses_group_run"
                    && *name != "group_model_analyses_created"
                    && *name != "group_panel_syntheses_panel"
                    && *name != "group_panel_syntheses_created")
                && *name != V16_TABLE
                && !V16_INDEXES.contains(&name.as_str())
                && *name != "group_agent_graph_scheduled_node_successor_candidates"
                && *name != "group_agent_graph_scheduled_node_successor_candidates_created"
                && !super::V29_CONTROLLER_OBJECTS.contains(&name.as_str())
                && !matches!(
                    name.as_str(),
                    "governance_record_append_batches"
                        | "governance_records"
                        | "governance_records_aggregate_appended"
                        | "governance_records_appended"
                        | "governance_records_kind_appended"
                        | "governance_structural_heads"
                        | "governance_semantic_heads"
                        | "governance_claim_semantic_views"
                        | "governance_claim_validation_jobs"
                        | "governance_semantic_heads_state_validity"
                        | "governance_claim_semantic_conflicts"
                        | "governance_claim_validation_jobs_due"
                        | "run_lineages"
                        | "run_lineages_parent"
                )
        })
        .cloned()
        .collect()
}

fn active_run(connection: &Connection) -> (i64, String, i64, i64) {
    connection
        .query_row(
            "SELECT run_version,status,dispatch_authority_released,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("active Graph Run survives")
}

fn candidate_row(connection: &Connection) -> (String, Vec<u8>, Vec<u8>, String, i64) {
    connection
        .query_row(
            "SELECT id,contract_blob,contract_sha256,idempotency_key,created_at_ms
             FROM group_agent_graph_scheduled_node_contract_candidates",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("candidate row")
}

fn assert_foreign_keys_clean(connection: &Connection) {
    assert_eq!(row_count(connection, "pragma_foreign_key_check"), 0);
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn assert_open_corrupt(database: &std::path::Path) {
    assert!(matches!(
        open_database(database),
        Err(HubStoreError::Corrupt { .. })
    ));
}
