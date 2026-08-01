pub(super) const MIGRATE_V9_TO_V10_SQL: &str = "CREATE TABLE group_agent_graph_runs_v10 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_id TEXT NOT NULL REFERENCES group_agent_graphs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_id) = 'text'
      AND length(CAST(graph_id AS BLOB)) BETWEEN 1 AND 128),
  run_version INTEGER NOT NULL
    CHECK(typeof(run_version) = 'integer' AND run_version IN (1, 2)),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text'
      AND status IN ('awaiting_execution_contract', 'awaiting_core_dispatch')),
  source_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(source_snapshot_sha256) = 'blob'
      AND length(source_snapshot_sha256) = 32),
  graph_manifest_sha256 BLOB NOT NULL
    CHECK(typeof(graph_manifest_sha256) = 'blob'
      AND length(graph_manifest_sha256) = 32),
  scheduler_protocol_version INTEGER NOT NULL
    CHECK(typeof(scheduler_protocol_version) = 'integer'
      AND scheduler_protocol_version = 1),
  plan_blob BLOB NOT NULL
    CHECK(typeof(plan_blob) = 'blob'
      AND length(plan_blob) BETWEEN 1 AND 2097152),
  plan_bytes INTEGER NOT NULL
    CHECK(typeof(plan_bytes) = 'integer'
      AND plan_bytes BETWEEN 1 AND 2097152
      AND plan_bytes = length(plan_blob)),
  plan_sha256 BLOB NOT NULL
    CHECK(typeof(plan_sha256) = 'blob' AND length(plan_sha256) = 32),
  node_count INTEGER NOT NULL
    CHECK(typeof(node_count) = 'integer' AND node_count BETWEEN 1 AND 32),
  wave_count INTEGER NOT NULL
    CHECK(typeof(wave_count) = 'integer'
      AND wave_count BETWEEN 1 AND 32 AND wave_count <= node_count),
  execution_contract_present INTEGER NOT NULL
    CHECK(typeof(execution_contract_present) = 'integer'
      AND execution_contract_present IN (0, 1)),
  dispatch_authority_released INTEGER NOT NULL
    CHECK(typeof(dispatch_authority_released) = 'integer'
      AND dispatch_authority_released = 0),
  last_event_seq INTEGER NOT NULL
    CHECK(typeof(last_event_seq) = 'integer' AND last_event_seq IN (1, 2)),
  journal_bytes INTEGER NOT NULL
    CHECK(typeof(journal_bytes) = 'integer'
      AND journal_bytes BETWEEN 1 AND 131072),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  CHECK(
    (run_version = 1
      AND status = 'awaiting_execution_contract'
      AND execution_contract_present = 0
      AND dispatch_authority_released = 0
      AND last_event_seq = 1
      AND journal_bytes <= 65536)
    OR
    (run_version = 2
      AND status = 'awaiting_core_dispatch'
      AND execution_contract_present = 1
      AND dispatch_authority_released = 0
      AND last_event_seq = 2)
  )
);
CREATE TABLE group_agent_graph_run_events_v10 (
  graph_run_id TEXT NOT NULL
    REFERENCES group_agent_graph_runs_v10(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  seq INTEGER NOT NULL
    CHECK(typeof(seq) = 'integer' AND seq IN (1, 2)),
  event_version INTEGER NOT NULL
    CHECK(typeof(event_version) = 'integer' AND event_version IN (1, 2)),
  kind TEXT NOT NULL
    CHECK(typeof(kind) = 'text'
      AND kind IN ('graph_run_prepared', 'node_execution_contract_admitted')),
  event_blob BLOB NOT NULL
    CHECK(typeof(event_blob) = 'blob'
      AND length(event_blob) BETWEEN 1 AND 65536),
  event_bytes INTEGER NOT NULL
    CHECK(typeof(event_bytes) = 'integer'
      AND event_bytes BETWEEN 1 AND 65536
      AND event_bytes = length(event_blob)),
  event_sha256 BLOB NOT NULL
    CHECK(typeof(event_sha256) = 'blob' AND length(event_sha256) = 32),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  PRIMARY KEY(graph_run_id, seq),
  CHECK(
    (seq = 1 AND event_version = 1 AND kind = 'graph_run_prepared')
    OR
    (seq = 2 AND event_version = 2
      AND kind = 'node_execution_contract_admitted')
  )
);
INSERT INTO group_agent_graph_runs_v10
  SELECT * FROM group_agent_graph_runs;
INSERT INTO group_agent_graph_run_events_v10
  SELECT * FROM group_agent_graph_run_events;
DROP TABLE group_agent_graph_run_events;
DROP INDEX group_agent_graph_runs_graph;
DROP INDEX group_agent_graph_runs_created;
DROP TABLE group_agent_graph_runs;
ALTER TABLE group_agent_graph_runs_v10
  RENAME TO group_agent_graph_runs;
ALTER TABLE group_agent_graph_run_events_v10
  RENAME TO group_agent_graph_run_events;
CREATE INDEX group_agent_graph_runs_graph
  ON group_agent_graph_runs(graph_id, created_at_ms DESC, id DESC);
CREATE INDEX group_agent_graph_runs_created
  ON group_agent_graph_runs(created_at_ms DESC, id DESC);
CREATE TABLE group_agent_graph_node_execution_contracts (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  contract_version INTEGER NOT NULL
    CHECK(typeof(contract_version) = 'integer' AND contract_version = 1),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL
    CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  control_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(control_snapshot_sha256) = 'blob'
      AND length(control_snapshot_sha256) = 32),
  contract_blob BLOB NOT NULL
    CHECK(typeof(contract_blob) = 'blob'
      AND length(contract_blob) BETWEEN 1 AND 4194304),
  contract_bytes INTEGER NOT NULL
    CHECK(typeof(contract_bytes) = 'integer'
      AND contract_bytes BETWEEN 1 AND 4194304
      AND contract_bytes = length(contract_blob)),
  contract_sha256 BLOB NOT NULL
    CHECK(typeof(contract_sha256) = 'blob' AND length(contract_sha256) = 32),
  request_sha256 BLOB NOT NULL
    CHECK(typeof(request_sha256) = 'blob' AND length(request_sha256) = 32),
  project_lane_sha256 BLOB NOT NULL
    CHECK(typeof(project_lane_sha256) = 'blob'
      AND length(project_lane_sha256) = 32),
  expected_last_event_seq INTEGER NOT NULL
    CHECK(typeof(expected_last_event_seq) = 'integer'
      AND expected_last_event_seq = 1),
  expected_last_event_sha256 BLOB NOT NULL
    CHECK(typeof(expected_last_event_sha256) = 'blob'
      AND length(expected_last_event_sha256) = 32),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE INDEX group_agent_graph_node_contracts_project_lane
  ON group_agent_graph_node_execution_contracts(
    project_lane_sha256, created_at_ms DESC, id DESC
  );
CREATE INDEX group_agent_graph_node_contracts_created
  ON group_agent_graph_node_execution_contracts(created_at_ms DESC, id DESC);
PRAGMA user_version = 10;";
