pub(super) const MIGRATE_V10_TO_V11_SQL: &str = "CREATE TABLE group_agent_graph_runs_v11 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_id TEXT NOT NULL REFERENCES group_agent_graphs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_id) = 'text'
      AND length(CAST(graph_id AS BLOB)) BETWEEN 1 AND 128),
  run_version INTEGER NOT NULL
    CHECK(typeof(run_version) = 'integer' AND run_version IN (1, 2, 3)),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text'
      AND status IN (
        'awaiting_execution_contract',
        'awaiting_core_dispatch',
        'awaiting_dispatch_authorization'
      )),
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
  dispatch_request_present INTEGER NOT NULL
    CHECK(typeof(dispatch_request_present) = 'integer'
      AND dispatch_request_present IN (0, 1)),
  dispatch_authority_released INTEGER NOT NULL
    CHECK(typeof(dispatch_authority_released) = 'integer'
      AND dispatch_authority_released = 0),
  last_event_seq INTEGER NOT NULL
    CHECK(typeof(last_event_seq) = 'integer' AND last_event_seq IN (1, 2, 3)),
  journal_bytes INTEGER NOT NULL
    CHECK(typeof(journal_bytes) = 'integer'
      AND journal_bytes BETWEEN 1 AND 196608),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  CHECK(
    (run_version = 1
      AND status = 'awaiting_execution_contract'
      AND execution_contract_present = 0
      AND dispatch_request_present = 0
      AND dispatch_authority_released = 0
      AND last_event_seq = 1
      AND journal_bytes <= 65536)
    OR
    (run_version = 2
      AND status = 'awaiting_core_dispatch'
      AND execution_contract_present = 1
      AND dispatch_request_present = 0
      AND dispatch_authority_released = 0
      AND last_event_seq = 2
      AND journal_bytes <= 131072)
    OR
    (run_version = 3
      AND status = 'awaiting_dispatch_authorization'
      AND execution_contract_present = 1
      AND dispatch_request_present = 1
      AND dispatch_authority_released = 0
      AND last_event_seq = 3)
  )
);
CREATE TABLE group_agent_graph_run_events_v11 (
  graph_run_id TEXT NOT NULL
    REFERENCES group_agent_graph_runs_v11(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  seq INTEGER NOT NULL
    CHECK(typeof(seq) = 'integer' AND seq IN (1, 2, 3)),
  event_version INTEGER NOT NULL
    CHECK(typeof(event_version) = 'integer' AND event_version IN (1, 2, 3)),
  kind TEXT NOT NULL
    CHECK(typeof(kind) = 'text'
      AND kind IN (
        'graph_run_prepared',
        'node_execution_contract_admitted',
        'node_dispatch_request_prepared'
      )),
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
    OR
    (seq = 3 AND event_version = 3
      AND kind = 'node_dispatch_request_prepared')
  )
);
CREATE TABLE group_agent_graph_node_execution_contracts_v11 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs_v11(id) ON DELETE RESTRICT
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
INSERT INTO group_agent_graph_runs_v11(
  id,graph_id,run_version,status,source_snapshot_sha256,
  graph_manifest_sha256,scheduler_protocol_version,plan_blob,plan_bytes,
  plan_sha256,node_count,wave_count,execution_contract_present,
  dispatch_request_present,dispatch_authority_released,last_event_seq,
  journal_bytes,idempotency_key,created_at_ms
)
SELECT
  id,graph_id,run_version,status,source_snapshot_sha256,
  graph_manifest_sha256,scheduler_protocol_version,plan_blob,plan_bytes,
  plan_sha256,node_count,wave_count,execution_contract_present,
  0,dispatch_authority_released,last_event_seq,journal_bytes,
  idempotency_key,created_at_ms
FROM group_agent_graph_runs;
INSERT INTO group_agent_graph_run_events_v11
  SELECT * FROM group_agent_graph_run_events;
INSERT INTO group_agent_graph_node_execution_contracts_v11
  SELECT * FROM group_agent_graph_node_execution_contracts;
DROP TABLE group_agent_graph_node_execution_contracts;
DROP TABLE group_agent_graph_run_events;
DROP INDEX group_agent_graph_runs_graph;
DROP INDEX group_agent_graph_runs_created;
DROP TABLE group_agent_graph_runs;
ALTER TABLE group_agent_graph_runs_v11
  RENAME TO group_agent_graph_runs;
ALTER TABLE group_agent_graph_run_events_v11
  RENAME TO group_agent_graph_run_events;
ALTER TABLE group_agent_graph_node_execution_contracts_v11
  RENAME TO group_agent_graph_node_execution_contracts;
CREATE INDEX group_agent_graph_runs_graph
  ON group_agent_graph_runs(graph_id, created_at_ms DESC, id DESC);
CREATE INDEX group_agent_graph_runs_created
  ON group_agent_graph_runs(created_at_ms DESC, id DESC);
CREATE INDEX group_agent_graph_node_contracts_project_lane
  ON group_agent_graph_node_execution_contracts(
    project_lane_sha256, created_at_ms DESC, id DESC
  );
CREATE INDEX group_agent_graph_node_contracts_created
  ON group_agent_graph_node_execution_contracts(created_at_ms DESC, id DESC);
CREATE TABLE group_agent_graph_node_dispatch_requests (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  contract_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_execution_contracts(id) ON DELETE RESTRICT
    CHECK(typeof(contract_id) = 'text'
      AND length(CAST(contract_id AS BLOB)) BETWEEN 1 AND 128),
  request_version INTEGER NOT NULL
    CHECK(typeof(request_version) = 'integer' AND request_version = 1),
  codec_protocol_version INTEGER NOT NULL
    CHECK(typeof(codec_protocol_version) = 'integer'
      AND codec_protocol_version = 1),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL
    CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  contract_sha256 BLOB NOT NULL
    CHECK(typeof(contract_sha256) = 'blob' AND length(contract_sha256) = 32),
  request_sha256 BLOB NOT NULL
    CHECK(typeof(request_sha256) = 'blob' AND length(request_sha256) = 32),
  project_lane_sha256 BLOB NOT NULL
    CHECK(typeof(project_lane_sha256) = 'blob'
      AND length(project_lane_sha256) = 32),
  provider_kind TEXT NOT NULL
    CHECK(typeof(provider_kind) = 'text' AND provider_kind = 'openai_responses'),
  endpoint TEXT NOT NULL
    CHECK(typeof(endpoint) = 'text'
      AND length(CAST(endpoint AS BLOB)) BETWEEN 1 AND 2048),
  model TEXT NOT NULL
    CHECK(typeof(model) = 'text'
      AND length(CAST(model AS BLOB)) BETWEEN 1 AND 128),
  destination_sha256 BLOB NOT NULL
    CHECK(typeof(destination_sha256) = 'blob' AND length(destination_sha256) = 32),
  pricing_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(pricing_snapshot_sha256) = 'blob'
      AND length(pricing_snapshot_sha256) = 32),
  provider_request_blob BLOB NOT NULL
    CHECK(typeof(provider_request_blob) = 'blob'
      AND length(provider_request_blob) BETWEEN 1 AND 16777216),
  provider_request_bytes INTEGER NOT NULL
    CHECK(typeof(provider_request_bytes) = 'integer'
      AND provider_request_bytes BETWEEN 1 AND 16777216
      AND provider_request_bytes = length(provider_request_blob)),
  provider_request_sha256 BLOB NOT NULL
    CHECK(typeof(provider_request_sha256) = 'blob'
      AND length(provider_request_sha256) = 32),
  dispatch_request_sha256 BLOB NOT NULL
    CHECK(typeof(dispatch_request_sha256) = 'blob'
      AND length(dispatch_request_sha256) = 32),
  expected_last_event_seq INTEGER NOT NULL
    CHECK(typeof(expected_last_event_seq) = 'integer'
      AND expected_last_event_seq = 2),
  expected_last_event_sha256 BLOB NOT NULL
    CHECK(typeof(expected_last_event_sha256) = 'blob'
      AND length(expected_last_event_sha256) = 32),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE INDEX group_agent_graph_node_dispatch_requests_project_lane
  ON group_agent_graph_node_dispatch_requests(
    project_lane_sha256, created_at_ms DESC, id DESC
  );
CREATE INDEX group_agent_graph_node_dispatch_requests_created
  ON group_agent_graph_node_dispatch_requests(created_at_ms DESC, id DESC);
PRAGMA user_version = 11;";
