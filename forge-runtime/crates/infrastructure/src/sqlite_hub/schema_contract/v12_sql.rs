pub(super) const MIGRATE_V11_TO_V12_SQL: &str = "CREATE TABLE group_agent_graph_runs_v12 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_id TEXT NOT NULL REFERENCES group_agent_graphs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_id) = 'text'
      AND length(CAST(graph_id AS BLOB)) BETWEEN 1 AND 128),
  run_version INTEGER NOT NULL
    CHECK(typeof(run_version) = 'integer' AND run_version IN (1, 2, 3, 4, 5)),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text'
      AND status IN (
        'awaiting_execution_contract',
        'awaiting_core_dispatch',
        'awaiting_dispatch_authorization',
        'dispatch_unknown',
        'completed',
        'failed',
        'failed_uncertain'
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
      AND dispatch_authority_released IN (0, 1)),
  last_event_seq INTEGER NOT NULL
    CHECK(typeof(last_event_seq) = 'integer'
      AND last_event_seq IN (1, 2, 3, 4, 5)),
  journal_bytes INTEGER NOT NULL
    CHECK(typeof(journal_bytes) = 'integer'
      AND journal_bytes BETWEEN 1 AND 327680),
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
      AND last_event_seq = 3
      AND journal_bytes <= 196608)
    OR
    (run_version = 4
      AND status = 'dispatch_unknown'
      AND execution_contract_present = 1
      AND dispatch_request_present = 1
      AND dispatch_authority_released = 1
      AND last_event_seq = 4
      AND journal_bytes <= 262144)
    OR
    (run_version = 5
      AND status IN ('completed', 'failed', 'failed_uncertain')
      AND execution_contract_present = 1
      AND dispatch_request_present = 1
      AND dispatch_authority_released = 1
      AND last_event_seq = 5)
  )
);
CREATE TABLE group_agent_graph_run_events_v12 (
  graph_run_id TEXT NOT NULL
    REFERENCES group_agent_graph_runs_v12(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  seq INTEGER NOT NULL
    CHECK(typeof(seq) = 'integer' AND seq IN (1, 2, 3, 4, 5)),
  event_version INTEGER NOT NULL
    CHECK(typeof(event_version) = 'integer'
      AND event_version IN (1, 2, 3, 4, 5)),
  kind TEXT NOT NULL
    CHECK(typeof(kind) = 'text'
      AND kind IN (
        'graph_run_prepared',
        'node_execution_contract_admitted',
        'node_dispatch_request_prepared',
        'node_dispatch_released',
        'node_lifecycle_terminalized'
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
    OR
    (seq = 4 AND event_version = 4
      AND kind = 'node_dispatch_released')
    OR
    (seq = 5 AND event_version = 5
      AND kind = 'node_lifecycle_terminalized')
  )
);
CREATE TABLE group_agent_graph_node_execution_contracts_v12 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs_v12(id) ON DELETE RESTRICT
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
CREATE TABLE group_agent_graph_node_dispatch_requests_v12 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs_v12(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  contract_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_execution_contracts_v12(id) ON DELETE RESTRICT
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
INSERT INTO group_agent_graph_runs_v12 SELECT * FROM group_agent_graph_runs;
INSERT INTO group_agent_graph_run_events_v12 SELECT * FROM group_agent_graph_run_events;
INSERT INTO group_agent_graph_node_execution_contracts_v12
  SELECT * FROM group_agent_graph_node_execution_contracts;
INSERT INTO group_agent_graph_node_dispatch_requests_v12
  SELECT * FROM group_agent_graph_node_dispatch_requests;
DROP TABLE group_agent_graph_node_dispatch_requests;
DROP TABLE group_agent_graph_node_execution_contracts;
DROP TABLE group_agent_graph_run_events;
DROP INDEX group_agent_graph_runs_graph;
DROP INDEX group_agent_graph_runs_created;
DROP TABLE group_agent_graph_runs;
ALTER TABLE group_agent_graph_runs_v12 RENAME TO group_agent_graph_runs;
ALTER TABLE group_agent_graph_run_events_v12 RENAME TO group_agent_graph_run_events;
ALTER TABLE group_agent_graph_node_execution_contracts_v12
  RENAME TO group_agent_graph_node_execution_contracts;
ALTER TABLE group_agent_graph_node_dispatch_requests_v12
  RENAME TO group_agent_graph_node_dispatch_requests;
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
CREATE INDEX group_agent_graph_node_dispatch_requests_project_lane
  ON group_agent_graph_node_dispatch_requests(
    project_lane_sha256, created_at_ms DESC, id DESC
  );
CREATE INDEX group_agent_graph_node_dispatch_requests_created
  ON group_agent_graph_node_dispatch_requests(created_at_ms DESC, id DESC);
CREATE TABLE group_agent_graph_node_dispatch_claims (
  dispatch_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(dispatch_id) = 'text'
      AND length(CAST(dispatch_id AS BLOB)) BETWEEN 1 AND 128),
  claim_version INTEGER NOT NULL
    CHECK(typeof(claim_version) = 'integer' AND claim_version = 1),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT,
  authorization_id TEXT NOT NULL UNIQUE
    CHECK(typeof(authorization_id) = 'text'
      AND length(CAST(authorization_id AS BLOB)) BETWEEN 1 AND 128),
  authorization_sha256 BLOB NOT NULL
    CHECK(typeof(authorization_sha256) = 'blob' AND length(authorization_sha256) = 32),
  dispatch_request_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_dispatch_requests(id) ON DELETE RESTRICT,
  dispatch_request_sha256 BLOB NOT NULL
    CHECK(typeof(dispatch_request_sha256) = 'blob'
      AND length(dispatch_request_sha256) = 32),
  logical_request_sha256 BLOB NOT NULL
    CHECK(typeof(logical_request_sha256) = 'blob'
      AND length(logical_request_sha256) = 32),
  request_body_sha256 BLOB NOT NULL
    CHECK(typeof(request_body_sha256) = 'blob'
      AND length(request_body_sha256) = 32),
  request_body_bytes INTEGER NOT NULL
    CHECK(typeof(request_body_bytes) = 'integer'
      AND request_body_bytes BETWEEN 1 AND 16777216),
  pricing_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(pricing_snapshot_sha256) = 'blob'
      AND length(pricing_snapshot_sha256) = 32),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  max_cost_usd_micros INTEGER NOT NULL
    CHECK(typeof(max_cost_usd_micros) = 'integer'
      AND max_cost_usd_micros BETWEEN 1 AND 1000000000000),
  consent_contract_version INTEGER NOT NULL
    CHECK(typeof(consent_contract_version) = 'integer'
      AND consent_contract_version = 1),
  lane_ownership_id TEXT NOT NULL UNIQUE
    CHECK(typeof(lane_ownership_id) = 'text'
      AND length(CAST(lane_ownership_id AS BLOB)) BETWEEN 1 AND 128),
  project_lane_sha256 BLOB NOT NULL
    CHECK(typeof(project_lane_sha256) = 'blob'
      AND length(project_lane_sha256) = 32),
  expected_last_event_seq INTEGER NOT NULL
    CHECK(typeof(expected_last_event_seq) = 'integer'
      AND expected_last_event_seq = 3),
  expected_last_event_sha256 BLOB NOT NULL
    CHECK(typeof(expected_last_event_sha256) = 'blob'
      AND length(expected_last_event_sha256) = 32),
  claim_event_sha256 BLOB NOT NULL
    CHECK(typeof(claim_event_sha256) = 'blob'
      AND length(claim_event_sha256) = 32),
  claim_blob BLOB NOT NULL
    CHECK(typeof(claim_blob) = 'blob' AND length(claim_blob) BETWEEN 1 AND 65536),
  claim_bytes INTEGER NOT NULL
    CHECK(typeof(claim_bytes) = 'integer' AND claim_bytes = length(claim_blob)
      AND claim_bytes BETWEEN 1 AND 65536),
  released_at_ms INTEGER NOT NULL
    CHECK(typeof(released_at_ms) = 'integer' AND released_at_ms >= 0)
);
CREATE INDEX group_agent_graph_node_dispatch_claims_created
  ON group_agent_graph_node_dispatch_claims(released_at_ms DESC, dispatch_id DESC);
CREATE TABLE group_agent_project_lane_ownerships (
  lane_ownership_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(lane_ownership_id) = 'text'
      AND length(CAST(lane_ownership_id AS BLOB)) BETWEEN 1 AND 128),
  lane_version INTEGER NOT NULL
    CHECK(typeof(lane_version) = 'integer' AND lane_version = 1),
  project_lane_sha256 BLOB NOT NULL UNIQUE
    CHECK(typeof(project_lane_sha256) = 'blob'
      AND length(project_lane_sha256) = 32),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT,
  dispatch_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_dispatch_claims(dispatch_id) ON DELETE RESTRICT,
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  claim_event_sha256 BLOB NOT NULL
    CHECK(typeof(claim_event_sha256) = 'blob'
      AND length(claim_event_sha256) = 32),
  lane_blob BLOB NOT NULL
    CHECK(typeof(lane_blob) = 'blob' AND length(lane_blob) BETWEEN 1 AND 65536),
  lane_bytes INTEGER NOT NULL
    CHECK(typeof(lane_bytes) = 'integer' AND lane_bytes = length(lane_blob)
      AND lane_bytes BETWEEN 1 AND 65536),
  claimed_at_ms INTEGER NOT NULL
    CHECK(typeof(claimed_at_ms) = 'integer' AND claimed_at_ms >= 0)
);
CREATE INDEX group_agent_project_lane_ownerships_claimed
  ON group_agent_project_lane_ownerships(claimed_at_ms, lane_ownership_id);
CREATE TABLE group_agent_graph_node_terminal_artifacts (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT,
  dispatch_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_dispatch_claims(dispatch_id) ON DELETE RESTRICT,
  artifact_version INTEGER NOT NULL
    CHECK(typeof(artifact_version) = 'integer' AND artifact_version = 1),
  artifact_kind TEXT NOT NULL
    CHECK(typeof(artifact_kind) = 'text'
      AND artifact_kind IN ('result', 'uncertainty')),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  claim_event_sha256 BLOB NOT NULL
    CHECK(typeof(claim_event_sha256) = 'blob' AND length(claim_event_sha256) = 32),
  lane_ownership_id TEXT NOT NULL
    CHECK(typeof(lane_ownership_id) = 'text'
      AND length(CAST(lane_ownership_id AS BLOB)) BETWEEN 1 AND 128),
  provider_polling_began INTEGER NOT NULL
    CHECK(typeof(provider_polling_began) = 'integer'
      AND provider_polling_began IN (0, 1)),
  terminal_observed INTEGER NOT NULL
    CHECK(typeof(terminal_observed) = 'integer'
      AND terminal_observed IN (0, 1)),
  true_eof_observed INTEGER NOT NULL
    CHECK(typeof(true_eof_observed) = 'integer'
      AND true_eof_observed IN (0, 1)),
  retry_authorized INTEGER NOT NULL
    CHECK(typeof(retry_authorized) = 'integer' AND retry_authorized = 0),
  artifact_blob BLOB NOT NULL
    CHECK(typeof(artifact_blob) = 'blob'
      AND length(artifact_blob) BETWEEN 1 AND 1048576),
  artifact_blob_bytes INTEGER NOT NULL
    CHECK(typeof(artifact_blob_bytes) = 'integer'
      AND artifact_blob_bytes = length(artifact_blob)),
  artifact_bytes INTEGER NOT NULL
    CHECK(typeof(artifact_bytes) = 'integer'
      AND artifact_bytes BETWEEN 1 AND 1048576),
  artifact_sha256 BLOB NOT NULL
    CHECK(typeof(artifact_sha256) = 'blob' AND length(artifact_sha256) = 32),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  CHECK(artifact_kind = 'uncertainty'
    OR (provider_polling_began = 1
      AND terminal_observed = 1 AND true_eof_observed = 1))
);
CREATE INDEX group_agent_graph_node_terminal_artifacts_created
  ON group_agent_graph_node_terminal_artifacts(created_at_ms DESC, id DESC);
CREATE TABLE group_agent_graph_node_terminal_receipts (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT,
  dispatch_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_dispatch_claims(dispatch_id) ON DELETE RESTRICT,
  artifact_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_node_terminal_artifacts(id) ON DELETE RESTRICT,
  receipt_version INTEGER NOT NULL
    CHECK(typeof(receipt_version) = 'integer' AND receipt_version = 1),
  graph_status TEXT NOT NULL
    CHECK(typeof(graph_status) = 'text'
      AND graph_status IN ('completed', 'failed', 'failed_uncertain')),
  claim_event_sha256 BLOB NOT NULL
    CHECK(typeof(claim_event_sha256) = 'blob' AND length(claim_event_sha256) = 32),
  lane_ownership_id TEXT NOT NULL
    CHECK(typeof(lane_ownership_id) = 'text'
      AND length(CAST(lane_ownership_id AS BLOB)) BETWEEN 1 AND 128),
  artifact_sha256 BLOB NOT NULL
    CHECK(typeof(artifact_sha256) = 'blob' AND length(artifact_sha256) = 32),
  retry_authorized INTEGER NOT NULL
    CHECK(typeof(retry_authorized) = 'integer' AND retry_authorized = 0),
  lane_release_authorized INTEGER NOT NULL
    CHECK(typeof(lane_release_authorized) = 'integer'
      AND lane_release_authorized = 1),
  receipt_blob BLOB NOT NULL
    CHECK(typeof(receipt_blob) = 'blob'
      AND length(receipt_blob) BETWEEN 1 AND 65536),
  receipt_bytes INTEGER NOT NULL
    CHECK(typeof(receipt_bytes) = 'integer'
      AND receipt_bytes = length(receipt_blob)
      AND receipt_bytes BETWEEN 1 AND 65536),
  receipt_sha256 BLOB NOT NULL
    CHECK(typeof(receipt_sha256) = 'blob' AND length(receipt_sha256) = 32),
  terminal_at_ms INTEGER NOT NULL
    CHECK(typeof(terminal_at_ms) = 'integer' AND terminal_at_ms >= 0)
);
CREATE INDEX group_agent_graph_node_terminal_receipts_created
  ON group_agent_graph_node_terminal_receipts(terminal_at_ms DESC, id DESC);
PRAGMA user_version = 12;";
