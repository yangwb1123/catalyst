use std::cell::Cell;

use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    legacy_v1_database, legacy_v4_database, migrate_with_before_final_fault_for_test,
    open_database,
    schema_full_validation_tests::{
        assert_post_v1_objects_absent, data_snapshot, schema_snapshot, v1_data_snapshot,
    },
    schema_object_exists, schema_object_named, schema_version,
};

const FINAL_VALIDATION_ROGUE: &str = "rogue_before_final_validation";
const FINAL_VALIDATION_FAULT_SQL: &str = "CREATE TABLE rogue_before_final_validation(id TEXT)";
const FINAL_TABLES: &[&str] = &[
    "runs",
    "group_runs",
    "group_executions",
    "group_model_analyses",
    "group_analysis_panels",
    "group_analysis_panel_analyses",
    "group_panel_syntheses",
    "group_panel_synthesis_events",
    "group_panel_synthesis_results",
    "group_agent_graphs",
    "group_agent_graph_runs",
    "group_agent_graph_run_events",
    "group_agent_graph_node_execution_contracts",
    "group_agent_graph_node_dispatch_requests",
    "group_agent_graph_node_dispatch_claims",
    "group_agent_project_lane_ownerships",
    "group_agent_graph_node_terminal_artifacts",
    "group_agent_graph_node_terminal_receipts",
    "group_agent_graph_execution_schedules",
    "group_agent_graph_scheduled_node_contract_candidates",
    "group_agent_graph_scheduled_node_provider_requests",
    "governance_record_append_batches",
    "governance_records",
    "governance_structural_heads",
];

#[test]
fn malformed_v4_prefix_is_rejected_before_migration() {
    let (root, database) = legacy_v4_database();
    let connection = Connection::open(&database).expect("open v4 prefix fixture");
    malform_conversations_scope(&connection);
    let before_schema = schema_snapshot(&connection);
    let before_data = data_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("v4 prefix validation must fail");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    let unchanged = Connection::open(&database).expect("reopen rejected v4 fixture");
    assert_eq!(schema_version(&unchanged), 4);
    assert_eq!(schema_snapshot(&unchanged), before_schema);
    assert_eq!(data_snapshot(&unchanged), before_data);
    assert!(!schema_object_exists(
        &unchanged,
        "table",
        "group_model_analyses"
    ));
    drop((unchanged, root));
}

#[test]
fn malformed_v1_prefix_is_rejected_before_migration_chain() {
    let (root, database) = legacy_v1_database();
    let connection = Connection::open(&database).expect("open v1 prefix fixture");
    malform_conversations_scope(&connection);
    let before_schema = schema_snapshot(&connection);
    let before_data = v1_data_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("v1 prefix validation must fail");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    let unchanged = Connection::open(&database).expect("reopen rejected v1 fixture");
    assert_v1_unchanged(&unchanged, &before_schema, &before_data);
    drop((unchanged, root));
}

#[test]
fn injected_final_validation_failure_rolls_back_complete_v1_migration_chain() {
    let (root, database) = legacy_v1_database();
    let connection = Connection::open(&database).expect("open valid v1 rollback fixture");
    let before_schema = schema_snapshot(&connection);
    let before_data = v1_data_snapshot(&connection);
    let reached_final = Cell::new(false);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        reached_final.set(true);
        assert_eq!(schema_version(migrated), 25);
        for table in FINAL_TABLES {
            assert!(
                schema_object_exists(migrated, "table", table),
                "{table} must exist before the final-validation fault"
            );
        }
        migrated.execute_batch(FINAL_VALIDATION_FAULT_SQL)
    })
    .expect_err("real final v25 validation must reject the injected rogue table");
    assert!(
        reached_final.get(),
        "before-final fault hook was not reached"
    );
    let HubStoreError::Corrupt { message } = error else {
        panic!("final validator returned the wrong error class: {error:?}");
    };
    assert_eq!(
        message, "Hub v25 main catalog has invalid object inventory",
        "error must originate from the real final v25 catalog validator"
    );

    assert_v1_unchanged(&connection, &before_schema, &before_data);
    drop(connection);

    let reopened = Connection::open(&database).expect("reopen rolled-back v1 fixture");
    assert_v1_unchanged(&reopened, &before_schema, &before_data);
    drop((reopened, root));
}

fn assert_v1_unchanged(
    connection: &Connection,
    before_schema: &[super::schema_full_validation_tests::SchemaRow],
    before_data: &[String],
) {
    assert_eq!(schema_version(connection), 1);
    assert_eq!(schema_snapshot(connection), before_schema);
    assert_eq!(v1_data_snapshot(connection), before_data);
    assert_post_v1_objects_absent(connection);
    assert!(!schema_object_named(connection, FINAL_VALIDATION_ROGUE));
}

fn malform_conversations_scope(connection: &Connection) {
    connection
        .execute_batch(
            "DROP INDEX conversations_scope;
             CREATE INDEX conversations_scope
               ON conversations(scope_kind, updated_at_ms DESC);",
        )
        .expect("malform a legacy v1 index");
}

#[test]
fn defer_foreign_keys_allows_parent_rebuild_with_live_children_in_one_batch() {
    // Stage-02 Finding 1 mechanism test: the v17->v18 / v21->v22 rebuilds
    // DROP the parent provider-requests table while dispatch lifecycles
    // still reference it. Under FK enforcement the implicit DELETE violates
    // ON DELETE RESTRICT; PRAGMA defer_foreign_keys moves the check to the
    // COMMIT, where the rebuilt parent makes the final state consistent.
    let connection = Connection::open_in_memory().expect("memory connection");
    connection
        .execute_batch(
            "CREATE TABLE parent (id TEXT PRIMARY KEY);
             CREATE TABLE child (
               id TEXT PRIMARY KEY,
               parent_id TEXT NOT NULL REFERENCES parent(id) ON DELETE RESTRICT
             );
             INSERT INTO parent VALUES ('p1');
             INSERT INTO child VALUES ('c1', 'p1');",
        )
        .expect("seed FK fixture");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             PRAGMA defer_foreign_keys = ON;
             DROP TABLE parent;
             CREATE TABLE parent (id TEXT PRIMARY KEY);
             INSERT INTO parent VALUES ('p1');
             COMMIT;",
        )
        .expect("parent rebuild with live children must commit under defer");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM child", [], |row| row.get(0))
        .expect("count children");
    assert_eq!(count, 1, "child row must survive the rebuild");
    // Without defer, the same batch fails at the DROP.
    connection
        .execute_batch(
            "BEGIN;
             DROP TABLE parent;",
        )
        .expect_err("DROP parent with live children must fail under FK enforcement");
    connection.execute_batch("ROLLBACK").expect("rollback");
}
