use std::path::PathBuf;

use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::runtime_domain::HubStoreError;

use super::{
    legacy_v1_database, open_database, restrict_fixture_root, schema_object_named, schema_version,
    table_columns,
};

type SchemaRow = (String, String, String, Option<String>);

const POST_PROJECT_OBJECTS: &[&str] = &[
    "groups",
    "conversations",
    "group_projects",
    "prompts",
    "conversations_scope",
    "prompts_conversation",
    "runs",
    "run_events",
    "run_assistant_prompts",
    "runs_conversation",
    "group_runs",
    "group_runs_group",
    "group_executions",
    "group_execution_events",
    "group_executions_group_run",
    "group_executions_created",
    "group_model_analyses",
    "group_model_analysis_events",
    "group_model_analysis_results",
    "group_model_analyses_group_run",
    "group_model_analyses_created",
    "group_analysis_panels",
    "group_analysis_panel_analyses",
    "group_analysis_panels_group_run",
    "group_analysis_panels_created",
    "group_panel_syntheses",
    "group_panel_synthesis_events",
    "group_panel_synthesis_results",
    "group_panel_syntheses_panel",
    "group_panel_syntheses_created",
    "group_agent_graphs",
    "group_agent_graphs_group_run",
    "group_agent_graphs_created",
    "group_agent_graph_runs",
    "group_agent_graph_run_events",
    "group_agent_graph_runs_graph",
    "group_agent_graph_runs_created",
    "group_agent_graph_node_execution_contracts",
    "group_agent_graph_node_contracts_project_lane",
    "group_agent_graph_node_contracts_created",
    "group_agent_graph_node_dispatch_requests",
    "group_agent_graph_node_dispatch_requests_project_lane",
    "group_agent_graph_node_dispatch_requests_created",
    "group_agent_graph_node_dispatch_claims",
    "group_agent_graph_node_dispatch_claims_created",
    "group_agent_project_lane_ownerships",
    "group_agent_project_lane_ownerships_claimed",
    "group_agent_graph_node_terminal_artifacts",
    "group_agent_graph_node_terminal_artifacts_created",
    "group_agent_graph_node_terminal_receipts",
    "group_agent_graph_node_terminal_receipts_created",
    "group_agent_graph_execution_schedules",
    "group_agent_graph_execution_schedules_created",
    "group_agent_graph_scheduled_node_contract_candidates",
    "group_agent_graph_scheduled_node_candidates_project_lane",
    "group_agent_graph_scheduled_node_candidates_created",
    "group_agent_graph_scheduled_node_provider_requests",
    "group_agent_graph_scheduled_node_provider_requests_project_lane",
    "group_agent_graph_scheduled_node_provider_requests_created",
];

const INDEX_ORIGIN_GOLDEN: &[(&str, (usize, usize, usize))] = &[
    ("projects", (1, 1, 0)),
    ("groups", (1, 2, 0)),
    ("conversations", (1, 1, 1)),
    ("group_projects", (1, 1, 0)),
    ("prompts", (1, 1, 1)),
    ("runs", (1, 1, 1)),
    ("run_events", (1, 0, 0)),
    ("run_assistant_prompts", (1, 1, 0)),
    ("group_runs", (1, 1, 1)),
    ("group_executions", (1, 1, 2)),
    ("group_execution_events", (1, 0, 0)),
    ("group_model_analyses", (1, 1, 2)),
    ("group_model_analysis_events", (1, 0, 0)),
    ("group_model_analysis_results", (1, 0, 0)),
    ("group_analysis_panels", (1, 1, 2)),
    ("group_analysis_panel_analyses", (1, 1, 0)),
    ("group_panel_syntheses", (1, 1, 2)),
    ("group_panel_synthesis_events", (1, 0, 0)),
    ("group_panel_synthesis_results", (1, 0, 0)),
    ("group_agent_graphs", (1, 1, 2)),
    ("group_agent_graph_runs", (1, 1, 2)),
    ("group_agent_graph_run_events", (1, 0, 0)),
    ("group_agent_graph_node_execution_contracts", (1, 2, 2)),
    ("group_agent_graph_node_dispatch_requests", (1, 3, 2)),
    ("group_agent_graph_node_dispatch_claims", (1, 4, 1)),
    ("group_agent_project_lane_ownerships", (1, 3, 1)),
    ("group_agent_graph_node_terminal_artifacts", (1, 2, 1)),
    ("group_agent_graph_node_terminal_receipts", (1, 3, 1)),
    ("group_agent_graph_execution_schedules", (1, 2, 1)),
    (
        "group_agent_graph_scheduled_node_contract_candidates",
        (1, 6, 2),
    ),
    (
        "group_agent_graph_scheduled_node_provider_requests",
        (1, 7, 2),
    ),
];

#[test]
fn nonempty_v0_catalog_is_corrupt_and_unchanged() {
    let (root, database) = empty_database();
    let connection = Connection::open(&database).expect("open v0 fixture");
    connection
        .execute_batch(
            "CREATE TABLE projects(blocker TEXT);
             INSERT INTO projects VALUES('preserve-v0');",
        )
        .expect("install rogue v0 table");
    let before = schema_snapshot(&connection);
    drop(connection);

    assert_open_is_corrupt(&database, "nonempty v0 catalog");
    let unchanged = Connection::open(&database).expect("reopen rejected v0");
    assert_eq!(schema_version(&unchanged), 0);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert_eq!(
        single_text(&unchanged, "SELECT blocker FROM projects"),
        "preserve-v0"
    );
    assert_eq!(table_columns(&unchanged, "projects"), vec!["blocker"]);
    assert_objects_absent(&unchanged, POST_PROJECT_OBJECTS);
    drop((unchanged, root));
}

#[test]
fn v1_future_table_blocker_is_corrupt_before_migration() {
    let (root, database) = legacy_v1_database();
    let connection = Connection::open(&database).expect("open v1 blocker fixture");
    connection
        .execute_batch(
            "CREATE TABLE runs(blocker TEXT);
             INSERT INTO runs VALUES('preserve-blocker');",
        )
        .expect("install future v2 blocker");
    let before = schema_snapshot(&connection);
    let prompt = single_text(
        &connection,
        "SELECT content FROM prompts WHERE id='prompt-1'",
    );
    drop(connection);

    assert_open_is_corrupt(&database, "v1 future table blocker");
    let unchanged = Connection::open(&database).expect("reopen rejected v1");
    assert_eq!(schema_version(&unchanged), 1);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert_eq!(
        single_text(&unchanged, "SELECT blocker FROM runs"),
        "preserve-blocker"
    );
    assert_eq!(
        single_text(
            &unchanged,
            "SELECT content FROM prompts WHERE id='prompt-1'"
        ),
        prompt
    );
    assert_post_v1_migrations_absent(&unchanged);
    drop((unchanged, root));
}

#[test]
fn raw_autoindex_owner_corruption_is_rejected_without_repair() {
    let (root, database) = empty_database();
    let connection = open_database(&database).expect("create valid v15 fixture");
    let table_sql = table_definition(&connection, "groups");
    corrupt_unique_index_owner(&connection);
    let before = schema_snapshot(&connection);
    drop(connection);

    assert_open_is_corrupt(&database, "raw autoindex owner corruption");
    let unchanged = Connection::open(&database).expect("reopen raw-corrupt fixture");
    assert_eq!(schema_version(&unchanged), 16);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert_eq!(table_definition(&unchanged, "groups"), table_sql);
    drop((unchanged, root));
}

#[test]
fn sqlite_prefixed_trigger_is_rejected_without_repair() {
    let (root, database) = empty_database();
    let connection = open_database(&database).expect("create valid v15 fixture");
    install_hidden_panel_trigger(&connection);
    let before = schema_snapshot(&connection);
    drop(connection);

    assert_open_is_corrupt(&database, "sqlite-prefixed trigger");
    let unchanged = Connection::open(&database).expect("reopen rejected trigger fixture");
    assert_eq!(schema_version(&unchanged), 16);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert!(schema_object_named(&unchanged, "sqlite_hidden_panel_child"));
    drop((unchanged, root));
}

#[test]
fn v15_structural_index_inventory_matches_the_release_golden() {
    let (root, database) = empty_database();
    let connection = open_database(&database).expect("create valid v15 fixture");
    let mut totals = (0, 0, 0);
    for &(table, expected) in INDEX_ORIGIN_GOLDEN {
        let actual = index_origin_counts(&connection, table);
        assert_eq!(actual, expected, "{table}");
        totals.0 += actual.0;
        totals.1 += actual.1;
        totals.2 += actual.2;
    }
    assert_eq!(totals, (31, 48, 29));
    drop((connection, root));
}

#[test]
fn exhausted_writer_lock_is_classified_as_unavailable() {
    let (root, database) = empty_database();
    let writer = open_database(&database).expect("create lock fixture");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("hold deterministic writer lock");

    let error = open_database(&database).expect_err("writer lock must time out");
    assert!(matches!(error, HubStoreError::Unavailable { .. }));

    writer
        .execute_batch("ROLLBACK")
        .expect("release writer lock");
    let reopened = open_database(&database).expect("database reopens after lock release");
    drop((reopened, writer, root));
}

fn empty_database() -> (TempDir, PathBuf) {
    let root = TempDir::new().expect("private Hub root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    (root, database)
}

fn assert_open_is_corrupt(database: &std::path::Path, subject: &str) {
    let error = open_database(database)
        .err()
        .unwrap_or_else(|| panic!("{subject} unexpectedly opened"));
    assert!(
        matches!(error, HubStoreError::Corrupt { .. }),
        "{subject} returned {error:?}"
    );
}

fn schema_snapshot(connection: &Connection) -> Vec<SchemaRow> {
    let mut statement = connection
        .prepare(
            "SELECT type,name,tbl_name,sql FROM sqlite_schema
             ORDER BY type,name",
        )
        .expect("prepare schema snapshot");
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query schema snapshot")
        .collect::<Result<_, _>>()
        .expect("read schema snapshot")
}

fn single_text(connection: &Connection, sql: &str) -> String {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("query fixture text")
}

fn assert_objects_absent(connection: &Connection, names: &[&str]) {
    for name in names {
        assert!(!schema_object_named(connection, name), "unexpected {name}");
    }
}

fn assert_post_v1_migrations_absent(connection: &Connection) {
    assert_eq!(table_columns(connection, "runs"), vec!["blocker"]);
    assert_objects_absent(connection, &POST_PROJECT_OBJECTS[7..]);
}

fn table_definition(connection: &Connection, table: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )
        .expect("read table definition")
}

fn corrupt_unique_index_owner(connection: &Connection) {
    let index: String = connection
        .query_row(
            "SELECT name FROM pragma_index_list('groups')
             WHERE origin='u' ORDER BY name LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("select implicit unique index");
    connection
        .execute_batch("PRAGMA writable_schema=ON")
        .expect("enable raw schema fixture");
    connection
        .execute(
            "UPDATE sqlite_schema SET tbl_name='projects' WHERE name=?1",
            [index],
        )
        .expect("corrupt implicit index owner");
    connection
        .execute_batch("PRAGMA schema_version=99; PRAGMA writable_schema=OFF")
        .expect("publish raw schema fixture");
}

fn install_hidden_panel_trigger(connection: &Connection) {
    let definition = "CREATE TRIGGER sqlite_hidden_panel_child
        AFTER INSERT ON group_analysis_panel_analyses
        BEGIN
          DELETE FROM group_analysis_panel_analyses
          WHERE panel_id=NEW.panel_id AND position=NEW.position;
        END";
    connection
        .execute_batch("PRAGMA writable_schema=ON")
        .expect("enable raw schema fixture");
    connection
        .execute(
            "INSERT INTO sqlite_schema(type,name,tbl_name,rootpage,sql)
             VALUES('trigger',?1,'group_analysis_panel_analyses',0,?2)",
            params!["sqlite_hidden_panel_child", definition],
        )
        .expect("inject sqlite-prefixed trigger");
    connection
        .execute_batch("PRAGMA schema_version=100; PRAGMA writable_schema=OFF")
        .expect("publish hidden trigger fixture");
}

fn index_origin_counts(connection: &Connection, table: &str) -> (usize, usize, usize) {
    let escaped = table.replace('\'', "''");
    let mut statement = connection
        .prepare(&format!("PRAGMA main.index_list('{escaped}')"))
        .expect("prepare index inventory");
    let origins = statement
        .query_map([], |row| row.get::<_, String>(3))
        .expect("query index inventory")
        .collect::<Result<Vec<_>, _>>()
        .expect("read index inventory");
    (
        origins.iter().filter(|origin| *origin == "pk").count(),
        origins.iter().filter(|origin| *origin == "u").count(),
        origins.iter().filter(|origin| *origin == "c").count(),
    )
}
