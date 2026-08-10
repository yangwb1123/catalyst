use std::{fs, path::Path};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphCorePlan,
};
use serde_json::json;
use tempfile::TempDir;

use super::*;

const V11_MIGRATION_SOURCE: &str =
    include_str!("../../../infrastructure/src/sqlite_hub/schema_v11_sql.rs");

impl Fixture {
    pub(crate) fn new_v11() -> Self {
        let fixture = Self::new();
        downgrade_hub_to_v11(&fixture.state.path().join("hub.sqlite3"));
        fixture
    }

    pub(crate) fn new_multi_node() -> Self {
        let state = TempDir::new().expect("state directory");
        let cwd = TempDir::new().expect("current directory");
        let project = TempDir::new().expect("project directory");
        let sentinel = b"workspace-must-not-be-read-or-written".to_vec();
        fs::write(project.path().join("private.txt"), &sentinel).expect("workspace sentinel");
        let (graph_id, manifest) =
            prepare_multi_node_graph(state.path(), cwd.path(), project.path());
        let graph_run_id = prepare_multi_node_run(state.path(), cwd.path(), &graph_id, &manifest);
        let core_bin = build_go_core(cwd.path());
        let core_sha256 = file_sha256(&core_bin);
        let (authorization, pricing) =
            prepare_authority(state.path(), cwd.path(), &graph_run_id, &core_bin);
        let authorization_path = cwd.path().join("authorization.json");
        let pricing_path = cwd.path().join("pricing.json");
        fs::write(&authorization_path, authorization).expect("authorization fixture");
        fs::write(&pricing_path, pricing).expect("pricing fixture");
        downgrade_hub_to_v11(&state.path().join("hub.sqlite3"));
        Self {
            state,
            cwd,
            project,
            sentinel,
            graph_run_id,
            authorization_path,
            pricing_path,
            core_bin,
            core_sha256,
        }
    }

    pub(crate) fn schema_version(&self) -> u32 {
        let bytes = fs::read(self.state.path().join("hub.sqlite3")).expect("read SQLite header");
        u32::from_be_bytes(bytes[60..64].try_into().expect("SQLite user-version bytes"))
    }
}

const DOWNGRADE_V17_TO_V10_SQL: &str = "PRAGMA foreign_keys=OFF;
             BEGIN IMMEDIATE;
             CREATE TEMP TABLE saved_dispatch_request AS
               SELECT * FROM group_agent_graph_node_dispatch_requests;
             CREATE TEMP TABLE saved_seq3 AS
               SELECT * FROM group_agent_graph_run_events WHERE seq=3;
             DROP TABLE governance_structural_heads;
             DROP TABLE governance_records;
             DROP TABLE governance_record_append_batches;
             DROP INDEX group_agent_graph_scheduled_node_successor_candidates_created;
             DROP TABLE group_agent_graph_scheduled_node_successor_candidates;
             DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active;
             DROP INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_created;
             DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
             DROP TABLE group_agent_graph_scheduled_node_provider_requests;
             DROP TABLE group_agent_graph_scheduled_node_contract_candidates;
             DROP TABLE group_agent_graph_execution_schedules;
             DROP TABLE group_agent_graph_node_terminal_receipts;
             DROP TABLE group_agent_graph_node_terminal_artifacts;
             DROP TABLE group_agent_project_lane_ownerships;
             DROP TABLE group_agent_graph_node_dispatch_claims;
             DROP TABLE group_agent_graph_node_dispatch_requests;
             DELETE FROM group_agent_graph_run_events WHERE seq=3;
             UPDATE group_agent_graph_runs SET
               run_version=2,status='awaiting_core_dispatch',dispatch_request_present=0,
               last_event_seq=2,journal_bytes=(
                 SELECT SUM(event_bytes) FROM group_agent_graph_run_events
                 WHERE graph_run_id=group_agent_graph_runs.id
               );";
// v26 widened the endpoint CHECK on the analyses/syntheses tables; a
// downgraded fixture must restore the historical definitions so the
// pre-migration contract check matches the recorded catalogs.
const RESTORE_HISTORICAL_ANALYSES_SQL: &str =
    include_str!("../../../infrastructure/tests/restore_historical_analyses.sql");

fn restore_historical_analyses(connection: &rusqlite::Connection) {
    connection
        .execute_batch(RESTORE_HISTORICAL_ANALYSES_SQL)
        .expect("restore historical analyses definitions");
}

fn downgrade_hub_to_v11(database: &Path) {
    let connection = rusqlite::Connection::open(database).expect("open current Hub for fixture");
    connection
        .execute_batch(DOWNGRADE_V17_TO_V10_SQL)
        .expect("prepare v10-shaped fixture");
    restore_historical_analyses(&connection);
    connection
        .execute_batch(v11_migration_sql())
        .expect("rebuild exact v11 schema");
    connection
        .execute_batch(
            "INSERT INTO group_agent_graph_node_dispatch_requests
               SELECT * FROM saved_dispatch_request;
             INSERT INTO group_agent_graph_run_events SELECT * FROM saved_seq3;
             UPDATE group_agent_graph_runs SET
               run_version=3,status='awaiting_dispatch_authorization',
               dispatch_request_present=1,last_event_seq=3,journal_bytes=(
                 SELECT SUM(event_bytes) FROM group_agent_graph_run_events
                 WHERE graph_run_id=group_agent_graph_runs.id
               );
             COMMIT;
             PRAGMA foreign_keys=ON;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .expect("restore exact v11 dispatch state");
}

fn v11_migration_sql() -> &'static str {
    V11_MIGRATION_SOURCE
        .strip_prefix("pub(super) const MIGRATE_V10_TO_V11_SQL: &str = \"")
        .and_then(|value| value.strip_suffix("\";\n"))
        .expect("embedded v11 migration literal")
}

fn prepare_multi_node_graph(state: &Path, cwd: &Path, project: &Path) -> (String, String) {
    let (group_run_id, project_id) = prepare_source(state, cwd, project);
    let spec = multi_node_spec(&project_id);
    let graph = successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "prepare",
            &group_run_id,
            "--spec",
            "-",
            "--idempotency-key",
            "multi-node-graph",
        ],
        &serde_json::to_vec(&spec).unwrap(),
    ));
    (
        text(&graph["inspection"]["graph"]["graph_id"]),
        text(&graph["inspection"]["graph"]["manifest_sha256"]),
    )
}

fn multi_node_spec(project_id: &str) -> serde_json::Value {
    json!({
        "v": 1,
        "manager": {
            "agent_profile": "multi-node-manager",
            "instruction": "Coordinate two nodes without executing either."
        },
        "nodes": [
            node_spec("worker", project_id),
            node_spec("worker-two", project_id)
        ],
        "edges": []
    })
}

fn node_spec(node_id: &str, project_id: &str) -> serde_json::Value {
    json!({
        "node_id": node_id,
        "project_id": project_id,
        "member_role": "worker",
        "agent_profile": "implementer",
        "task": TASK_SECRET,
        "acceptance": "return one bounded result"
    })
}

fn prepare_multi_node_run(state: &Path, cwd: &Path, graph_id: &str, manifest: &str) -> String {
    let nodes = vec!["worker".to_owned(), "worker-two".to_owned()];
    let mut plan = GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: 1,
        graph_id: graph_id.into(),
        graph_manifest_sha256: manifest.into(),
        authored_node_ids: nodes.clone(),
        edges: Vec::new(),
        waves: vec![nodes],
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: "0".repeat(64),
    };
    plan.plan_sha256 = plan.expected_sha256().expect("plan digest");
    let output = successful_json(&invoke_with_stdin(
        state,
        cwd,
        &[
            "group",
            "graph",
            "run",
            "prepare",
            graph_id,
            "--plan",
            "-",
            "--idempotency-key",
            "multi-node-run",
        ],
        plan.canonical_json().unwrap().as_bytes(),
    ));
    text(&output["inspection"]["run"]["graph_run_id"])
}
