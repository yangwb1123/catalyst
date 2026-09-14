use super::{
    schema::{
        SCHEMA_VERSION, migrate_with_before_final_fault_for_test, open_database,
        open_existing_dispatch_preflight_read_only_database,
        open_existing_dispatch_reentry_read_only_database,
    },
    schema_sql::{
        CREATE_V1_SCHEMA_SQL, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL, MIGRATE_V3_TO_V4_SQL,
        MIGRATE_V4_TO_V5_SQL, MIGRATE_V5_TO_V6_SQL, MIGRATE_V6_TO_V7_SQL, MIGRATE_V7_TO_V8_SQL,
    },
    schema_v9_sql::MIGRATE_V8_TO_V9_SQL,
    schema_v10_sql::MIGRATE_V9_TO_V10_SQL,
    schema_v11_sql::MIGRATE_V10_TO_V11_SQL,
    schema_v12_sql::MIGRATE_V11_TO_V12_SQL,
    schema_v13_sql::MIGRATE_V12_TO_V13_SQL,
    schema_v14_sql::MIGRATE_V13_TO_V14_SQL,
    schema_v15_sql::MIGRATE_V14_TO_V15_SQL,
    schema_v21_sql::MIGRATE_V20_TO_V21_SQL,
    schema_v22_sql::MIGRATE_V21_TO_V22_SQL,
    schema_v23_sql::MIGRATE_V22_TO_V23_SQL,
    schema_v24_sql::MIGRATE_V23_TO_V24_SQL,
    schema_v25_sql::MIGRATE_V24_TO_V25_SQL,
    schema_v28_sql::MIGRATE_V27_TO_V28_SQL,
    schema_v29_sql::MIGRATE_V28_TO_V29_SQL,
    schema_v30_sql::MIGRATE_V29_TO_V30_SQL,
    schema_v31_sql::MIGRATE_V30_TO_V31_SQL,
};
use crate::runtime_domain::HubStoreError;
use rusqlite::Connection;
#[path = "tests/schema_legacy_support.rs"]
mod legacy_support;
use legacy_support::*;

#[path = "tests/schema_full_validation.rs"]
mod schema_full_validation_tests;
pub(super) const RESTORE_HISTORICAL_ANALYSES_SQL: &str =
    include_str!("tests/restore_historical_analyses.sql");
pub(super) const DROP_V27_SEMANTIC_VIEW_SQL: &str = "DROP TABLE governance_claim_validation_jobs;
     DROP TABLE governance_claim_semantic_views;
     DROP TABLE governance_semantic_heads;";
pub(super) const DROP_V28_LINEAGE_SQL: &str = "DROP TABLE run_lineages;";
#[path = "tests/schema_version_object_support.rs"]
mod schema_version_object_support;
use schema_version_object_support::{
    DROP_V29_CONTROLLER_SQL, DROP_V30_CONVERSATION_CHANGES_SQL, DROP_V31_CONVERSATION_OWNERS_SQL,
    DROP_V32_OWNER_CURSOR_OBJECTS_SQL, DROP_V33_PROJECT_CONSENT_OBJECTS_SQL,
    DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL, V29_CONTROLLER_OBJECTS, V30_CHANGE_OBJECTS,
    V31_OWNER_OBJECTS, V32_OWNER_CURSOR_OBJECTS, V33_PROJECT_CONSENT_OBJECTS,
    V34_PENDING_RUN_INTENT_OBJECTS,
};
#[path = "tests/schema_migration_support.rs"]
mod schema_migration_support;
#[path = "tests/schema_open_adversarial.rs"]
mod schema_open_adversarial_tests;
#[path = "tests/schema_release_golden.rs"]
mod schema_release_golden_tests;
#[path = "tests/schema_transaction_rollback.rs"]
mod schema_transaction_rollback_tests;
#[path = "tests/schema_v10_migration.rs"]
mod schema_v10_migration_tests;
#[path = "tests/schema_v11_migration.rs"]
mod schema_v11_migration_tests;
#[path = "tests/schema_v12_migration.rs"]
mod schema_v12_migration_tests;
#[path = "tests/schema_v13_migration.rs"]
mod schema_v13_migration_tests;
#[path = "tests/schema_v14_migration.rs"]
mod schema_v14_migration_tests;
#[path = "tests/schema_v15_migration.rs"]
mod schema_v15_migration_tests;
#[path = "tests/schema_v24_migration.rs"]
mod schema_v24_migration_tests;
#[path = "tests/schema_v25_migration.rs"]
mod schema_v25_migration_tests;
#[path = "tests/schema_v26_migration.rs"]
mod schema_v26_migration_tests;
#[path = "tests/schema_v27_migration.rs"]
mod schema_v27_migration_tests;
#[path = "tests/schema_v28_migration.rs"]
mod schema_v28_migration_tests;
#[path = "tests/schema_v29_migration.rs"]
mod schema_v29_migration_tests;
#[path = "tests/schema_v30_migration.rs"]
mod schema_v30_migration_tests;
#[path = "tests/schema_v31_migration.rs"]
mod schema_v31_migration_tests;
#[path = "tests/schema_v32_migration.rs"]
mod schema_v32_migration_tests;
#[path = "tests/schema_v33_migration.rs"]
mod schema_v33_migration_tests;
#[path = "tests/schema_v34_migration.rs"]
mod schema_v34_migration_tests;
#[path = "tests/schema_v5_migration.rs"]
mod schema_v5_migration_tests;
#[path = "tests/schema_v6_migration.rs"]
mod schema_v6_migration_tests;
#[path = "tests/schema_v7_migration.rs"]
mod schema_v7_migration_tests;
#[path = "tests/schema_v8_migration.rs"]
mod schema_v8_migration_tests;
#[path = "tests/schema_v9_migration.rs"]
mod schema_v9_migration_tests;
#[path = "../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[path = "../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_run_support;
#[path = "../../tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_scheduled_node_contract_support;
use schema_migration_support::{
    restrict_fixture_root, schema_object_exists, schema_version, table_columns,
};
#[test]
fn v1_future_group_runs_blocker_is_rejected_before_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open v1 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_runs(blocker TEXT)")
        .expect("install future v3 table blocker");
    drop(blocker);
    let error = open_database(&database).expect_err("v1 prefix rejects future v3 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    assert_eq!(schema_version(&unchanged), 1);
    for table in ["runs", "run_events", "run_assistant_prompts"] {
        assert!(
            !schema_object_exists(&unchanged, "table", table),
            "unexpected post-v1 table exists: {table}"
        );
    }
    assert_eq!(table_columns(&unchanged, "group_runs"), vec!["blocker"]);
    let prompt: String = unchanged
        .query_row(
            "SELECT content FROM prompts WHERE id='prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("v1 Prompt remains after prefix rejection");
    assert_eq!(prompt, "preserve me");
    drop((unchanged, root));
}
#[test]
fn v1_future_group_executions_blocker_is_rejected_before_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open v1 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_executions(blocker TEXT)")
        .expect("install future v4 table blocker");
    drop(blocker);
    let error = open_database(&database).expect_err("v1 prefix rejects future v4 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    assert_eq!(schema_version(&unchanged), 1);
    for table in [
        "runs",
        "run_events",
        "run_assistant_prompts",
        "group_runs",
        "group_execution_events",
    ] {
        assert!(
            !schema_object_exists(&unchanged, "table", table),
            "unexpected post-v1 table exists: {table}"
        );
    }
    assert_eq!(
        table_columns(&unchanged, "group_executions"),
        vec!["blocker"]
    );
    for index in [
        "group_runs_group",
        "group_executions_group_run",
        "group_executions_created",
    ] {
        assert!(!schema_object_named(&unchanged, index));
    }
    let prompt: String = unchanged
        .query_row(
            "SELECT content FROM prompts WHERE id='prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("v1 Prompt remains after prefix rejection");
    assert_eq!(prompt, "preserve me");
    drop((unchanged, root));
}
#[test]
fn v3_future_group_executions_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v3_database();
    let blocker = Connection::open(&database).expect("open v3 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_executions(blocker TEXT)")
        .expect("install future v4 table blocker");
    drop(blocker);
    let error = open_database(&database).expect_err("v3 prefix rejects future v4 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v3 database");
    assert_v3_schema(&unchanged);
    assert_eq!(
        table_columns(&unchanged, "group_executions"),
        vec!["blocker"]
    );
    for object in ["group_execution_events", "group_executions_group_run"] {
        assert!(!schema_object_named(&unchanged, object));
    }
    drop((unchanged, root));
}
