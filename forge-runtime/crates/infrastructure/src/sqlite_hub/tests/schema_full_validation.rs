use rusqlite::Connection;
use tempfile::TempDir;

use crate::runtime_domain::HubStoreError;

use super::{
    CREATE_V1_SCHEMA_SQL, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL, MIGRATE_V3_TO_V4_SQL,
    MIGRATE_V4_TO_V5_SQL, assert_legacy_run, legacy_v1_database, legacy_v2_database,
    legacy_v3_database, legacy_v4_database, legacy_v5_database, legacy_v6_database, open_database,
    restrict_fixture_root, schema_object_named, schema_v8_migration_tests::legacy_v7_database,
    schema_v9_migration_tests::legacy_v8_database, schema_version, seed_v1_records,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    V1,
    V2,
    V3,
    V4,
}

#[derive(Clone, Copy)]
enum Mutation {
    Replace {
        stage: Stage,
        original: &'static str,
        replacement: &'static str,
    },
    Append(&'static str),
}

struct MalformedCase {
    name: &'static str,
    mutation: Mutation,
}

pub(super) type SchemaRow = (String, String, String, Option<String>);

#[test]
fn valid_fresh_and_legacy_schemas_reach_and_reopen_as_current() {
    assert_valid_migration(fresh_database(), 0);
    assert_valid_migration(legacy_v1_database(), 1);
    assert_valid_migration(legacy_v2_database(), 2);
    assert_valid_migration(legacy_v3_database(), 3);
    assert_valid_migration(legacy_v4_database(), 4);
    assert_valid_migration(legacy_v5_database(), 5);
    assert_valid_migration(legacy_v6_database(), 6);
    assert_valid_migration(legacy_v7_database(), 7);
    assert_valid_migration(legacy_v8_database(), 8);
}

#[test]
fn malformed_owned_v1_through_v4_objects_are_rejected_without_repair() {
    for case in malformed_cases() {
        assert_malformed_case_is_rejected(&case);
    }
}

#[test]
fn rogue_ordinary_table_is_rejected_without_repair() {
    let case = appended(
        "rogue ordinary table",
        "CREATE TABLE rogue_catalog_table(id TEXT);",
    );
    assert_malformed_case_is_rejected(&case);
}

#[test]
fn rogue_view_is_rejected_without_repair() {
    let case = appended(
        "rogue view",
        "CREATE VIEW rogue_catalog_view AS SELECT id FROM projects;",
    );
    assert_malformed_case_is_rejected(&case);
}

#[test]
fn shadowed_pragma_index_list_is_rejected_without_repair() {
    let case = appended(
        "shadowed pragma_index_list virtual table",
        "CREATE VIRTUAL TABLE pragma_index_list
         USING fts5(seq,name,\"unique\",origin,partial);",
    );
    assert_malformed_case_is_rejected(&case);
}

fn assert_valid_migration(fixture: (TempDir, std::path::PathBuf), seed_level: u8) {
    let (root, database) = fixture;
    let connection = open_database(&database).expect("valid schema migrates to v11");
    assert_eq!(schema_version(&connection), 11);
    assert_seed_data(&connection, seed_level);
    drop(connection);

    let reopened = open_database(&database).expect("valid v11 schema reopens");
    assert_eq!(schema_version(&reopened), 11);
    assert_seed_data(&reopened, seed_level);
    drop((reopened, root));
}

fn assert_seed_data(connection: &Connection, seed_level: u8) {
    if seed_level >= 1 {
        let content: String = connection
            .query_row(
                "SELECT content FROM prompts WHERE id = 'prompt-1'",
                [],
                |row| row.get(0),
            )
            .expect("legacy v1 Prompt survives");
        assert_eq!(content, "preserve me");
    }
    if seed_level >= 2 {
        assert_legacy_run(connection);
    }
    if seed_level >= 3 {
        assert_row_exists(connection, "group_runs", "group-run-legacy");
    }
    if seed_level >= 4 {
        assert_row_exists(connection, "group_executions", "group-execution-legacy");
        assert_event_exists(connection);
    }
    if seed_level >= 5 {
        assert_row_exists(connection, "group_model_analyses", "analysis-legacy");
    }
    if seed_level >= 6 {
        assert_row_exists(connection, "group_analysis_panels", "panel-legacy");
        let members: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM group_analysis_panel_analyses
                 WHERE panel_id = 'panel-legacy'",
                [],
                |row| row.get(0),
            )
            .expect("legacy panel members survive");
        assert_eq!(members, 2);
    }
}

fn assert_row_exists(connection: &Connection, table: &str, id: &str) {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)");
    let exists: bool = connection
        .query_row(&sql, [id], |row| row.get(0))
        .expect("query preserved legacy row");
    assert!(exists, "missing preserved row {table}.{id}");
}

fn assert_event_exists(connection: &Connection) {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM group_execution_events
               WHERE execution_id = 'group-execution-legacy' AND seq = 1
             )",
            [],
            |row| row.get(0),
        )
        .expect("query preserved execution event");
    assert!(exists, "missing preserved Group Execution event");
}

fn fresh_database() -> (TempDir, std::path::PathBuf) {
    let root = TempDir::new().expect("fresh Hub root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    (root, database)
}

fn assert_malformed_case_is_rejected(case: &MalformedCase) {
    let (root, database) = malformed_v5_database(case);
    let connection = Connection::open(&database).expect("open malformed v5 fixture");
    let before_schema = schema_snapshot(&connection);
    let before_data = data_snapshot(&connection);
    drop(connection);

    let error = open_database(&database)
        .err()
        .unwrap_or_else(|| panic!("{} unexpectedly opened", case.name));
    assert!(
        matches!(error, HubStoreError::Corrupt { .. }),
        "{} returned {error:?}",
        case.name
    );

    let unchanged = Connection::open(&database).expect("reopen rejected v5 fixture");
    assert_eq!(schema_version(&unchanged), 5, "{}", case.name);
    assert_eq!(schema_snapshot(&unchanged), before_schema, "{}", case.name);
    assert_eq!(data_snapshot(&unchanged), before_data, "{}", case.name);
    drop((unchanged, root));
}

fn malformed_v5_database(case: &MalformedCase) -> (TempDir, std::path::PathBuf) {
    let (root, database) = fresh_database();
    let connection = Connection::open(&database).expect("create malformed v5 fixture");
    execute_stage(&connection, case, Stage::V1, CREATE_V1_SCHEMA_SQL);
    seed_v1_records(&connection);
    execute_stage(&connection, case, Stage::V2, MIGRATE_V1_TO_V2_SQL);
    execute_stage(&connection, case, Stage::V3, MIGRATE_V2_TO_V3_SQL);
    execute_stage(&connection, case, Stage::V4, MIGRATE_V3_TO_V4_SQL);
    connection
        .execute_batch(MIGRATE_V4_TO_V5_SQL)
        .expect("create v5-owned schema");
    if let Mutation::Append(sql) = case.mutation {
        connection.execute_batch(sql).expect("append rogue object");
    }
    drop(connection);
    (root, database)
}

fn execute_stage(connection: &Connection, case: &MalformedCase, stage: Stage, source: &str) {
    let sql = match case.mutation {
        Mutation::Replace {
            stage: target,
            original,
            replacement,
        } if stage == target => {
            let changed = source.replacen(original, replacement, 1);
            assert_ne!(changed, source, "{} fixture replacement missed", case.name);
            changed
        }
        _ => source.to_owned(),
    };
    connection
        .execute_batch(&sql)
        .unwrap_or_else(|error| panic!("create {} fixture: {error}", case.name));
}

pub(super) fn schema_snapshot(connection: &Connection) -> Vec<SchemaRow> {
    let mut statement = connection
        .prepare(
            "SELECT type,name,tbl_name,sql FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
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

pub(super) fn data_snapshot(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT 'prompt|' || id || '|' || content FROM prompts
             UNION ALL
             SELECT 'run|' || id || '|' || idempotency_key FROM runs
             UNION ALL
             SELECT 'group_run|' || id || '|' || status FROM group_runs
             UNION ALL
             SELECT 'execution|' || id || '|' || status FROM group_executions
             ORDER BY 1",
        )
        .expect("prepare data snapshot");
    statement
        .query_map([], |row| row.get(0))
        .expect("query data snapshot")
        .collect::<Result<_, _>>()
        .expect("read data snapshot")
}

pub(super) fn v1_data_snapshot(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT 'project|' || id || '|' || name FROM projects
             UNION ALL
             SELECT 'conversation|' || id || '|' || title FROM conversations
             UNION ALL
             SELECT 'prompt|' || id || '|' || content FROM prompts
             ORDER BY 1",
        )
        .expect("prepare v1 data snapshot");
    statement
        .query_map([], |row| row.get(0))
        .expect("query v1 data snapshot")
        .collect::<Result<_, _>>()
        .expect("read v1 data snapshot")
}

pub(super) fn assert_post_v1_objects_absent(connection: &Connection) {
    for name in [
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
    ] {
        assert!(!schema_object_named(connection, name), "unexpected {name}");
    }
}

fn malformed_cases() -> Vec<MalformedCase> {
    malformed_v1_cases()
        .into_iter()
        .chain(malformed_v2_cases())
        .chain(malformed_v3_cases())
        .chain(malformed_v4_cases())
        .chain(rogue_owned_object_cases())
        .collect()
}

fn malformed_v1_cases() -> [MalformedCase; 2] {
    [
        replacement(
            "v1 conversations CHECK",
            Stage::V1,
            "scope_kind TEXT NOT NULL CHECK(scope_kind IN ('global','project','group'))",
            "scope_kind TEXT NOT NULL",
        ),
        replacement(
            "v1 conversations scope index",
            Stage::V1,
            "ON conversations(scope_kind, scope_id, updated_at_ms DESC)",
            "ON conversations(scope_kind, updated_at_ms DESC)",
        ),
    ]
}

fn malformed_v2_cases() -> [MalformedCase; 2] {
    [
        replacement(
            "v2 runs foreign key",
            Stage::V2,
            "conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT",
            "conversation_id TEXT NOT NULL",
        ),
        replacement(
            "v2 run event primary key",
            Stage::V2,
            "PRIMARY KEY(run_id, seq)",
            "PRIMARY KEY(seq, run_id)",
        ),
    ]
}

fn malformed_v3_cases() -> [MalformedCase; 2] {
    [
        replacement(
            "v3 group_runs CHECK",
            Stage::V3,
            "CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128)",
            "CHECK(typeof(id) = 'text')",
        ),
        replacement(
            "v3 group_runs index",
            Stage::V3,
            "ON group_runs(group_id, created_at_ms DESC, id DESC)",
            "ON group_runs(group_id, created_at_ms, id DESC)",
        ),
    ]
}

fn malformed_v4_cases() -> [MalformedCase; 2] {
    [
        replacement(
            "v4 execution event primary key",
            Stage::V4,
            "PRIMARY KEY(execution_id, seq)",
            "PRIMARY KEY(seq, execution_id)",
        ),
        replacement(
            "v4 execution event foreign key",
            Stage::V4,
            "execution_id TEXT NOT NULL REFERENCES group_executions(id) ON DELETE RESTRICT",
            "execution_id TEXT NOT NULL",
        ),
    ]
}

fn rogue_owned_object_cases() -> [MalformedCase; 2] {
    [
        appended(
            "rogue legacy index",
            "CREATE INDEX rogue_runs_index ON runs(created_at_ms);",
        ),
        appended(
            "rogue legacy trigger",
            "CREATE TRIGGER rogue_conversations_trigger
             AFTER INSERT ON conversations BEGIN SELECT 1; END;",
        ),
    ]
}

fn replacement(
    name: &'static str,
    stage: Stage,
    original: &'static str,
    replacement: &'static str,
) -> MalformedCase {
    MalformedCase {
        name,
        mutation: Mutation::Replace {
            stage,
            original,
            replacement,
        },
    }
}

fn appended(name: &'static str, sql: &'static str) -> MalformedCase {
    MalformedCase {
        name,
        mutation: Mutation::Append(sql),
    }
}
