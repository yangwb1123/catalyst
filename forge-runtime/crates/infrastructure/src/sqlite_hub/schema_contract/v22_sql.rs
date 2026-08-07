// SQLite v22 migration: scheduled multi-node dispatch.
//
// Removes the per-run one-shot walls from the effectful dispatch tables so
// a Graph Run can carry one provider request / dispatch lifecycle per node
// (wave-parallel, ADR-0035 + review Finding 1b):
//   - provider-requests: column-level UNIQUE(graph_run_id) / UNIQUE(schedule_id)
//     dropped; the table-level per-node slots UNIQUE(graph_run_id, node_id,
//     attempt) + UNIQUE(schedule_id, execution_ordinal, attempt) remain.
//   - dispatch lifecycles: column-level UNIQUE(graph_run_id) dropped; a
//     table-level UNIQUE(graph_run_id, node_id, attempt) slot is added.
pub(super) const MIGRATE_V21_TO_V22_SQL: &str = r"CREATE TABLE group_agent_graph_scheduled_node_provider_requests_v22 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  schedule_id TEXT NOT NULL
    REFERENCES group_agent_graph_execution_schedules(id) ON DELETE RESTRICT
    CHECK(typeof(schedule_id) = 'text'
      AND length(CAST(schedule_id AS BLOB)) BETWEEN 1 AND 128),
  scheduled_contract_id TEXT NOT NULL UNIQUE
    CHECK(typeof(scheduled_contract_id) = 'text'
      AND length(CAST(scheduled_contract_id AS BLOB)) BETWEEN 1 AND 128),
  provider_request_version INTEGER NOT NULL
    CHECK(typeof(provider_request_version) = 'integer'
      AND provider_request_version = 1),
  codec_protocol_version INTEGER NOT NULL
    CHECK(typeof(codec_protocol_version) = 'integer'
      AND codec_protocol_version = 1),
  execution_ordinal INTEGER NOT NULL
    CHECK(typeof(execution_ordinal) = 'integer'
      AND execution_ordinal BETWEEN 0 AND 31),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL
    CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  scheduled_contract_sha256 BLOB NOT NULL
    CHECK(typeof(scheduled_contract_sha256) = 'blob'
      AND length(scheduled_contract_sha256) = 32),
  logical_request_id TEXT NOT NULL UNIQUE
    CHECK(typeof(logical_request_id) = 'text'
      AND length(CAST(logical_request_id AS BLOB)) BETWEEN 1 AND 128),
  logical_request_sha256 BLOB NOT NULL
    CHECK(typeof(logical_request_sha256) = 'blob'
      AND length(logical_request_sha256) = 32),
  schedule_sha256 BLOB NOT NULL
    CHECK(typeof(schedule_sha256) = 'blob' AND length(schedule_sha256) = 32),
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
    CHECK(typeof(destination_sha256) = 'blob'
      AND length(destination_sha256) = 32),
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
  prepared_request_sha256 BLOB NOT NULL
    CHECK(typeof(prepared_request_sha256) = 'blob'
      AND length(prepared_request_sha256) = 32),
  expected_last_event_seq INTEGER NOT NULL
    CHECK(typeof(expected_last_event_seq) = 'integer'
      AND expected_last_event_seq = 1),
  expected_last_event_sha256 BLOB NOT NULL
    CHECK(typeof(expected_last_event_sha256) = 'blob'
      AND length(expected_last_event_sha256) = 32),
  provider_request_prepared INTEGER NOT NULL
    CHECK(typeof(provider_request_prepared) = 'integer'
      AND provider_request_prepared = 1),
  provider_request_sent INTEGER NOT NULL
    CHECK(typeof(provider_request_sent) = 'integer' AND provider_request_sent = 0),
  lifecycle_contract_admitted INTEGER NOT NULL
    CHECK(typeof(lifecycle_contract_admitted) = 'integer'
      AND lifecycle_contract_admitted = 0),
  execution_authority_released INTEGER NOT NULL
    CHECK(typeof(execution_authority_released) = 'integer'
      AND execution_authority_released = 0),
  dispatch_authority_released INTEGER NOT NULL
    CHECK(typeof(dispatch_authority_released) = 'integer'
      AND dispatch_authority_released = 0),
  project_lane_claimed INTEGER NOT NULL
    CHECK(typeof(project_lane_claimed) = 'integer' AND project_lane_claimed = 0),
  progress_observed INTEGER NOT NULL
    CHECK(typeof(progress_observed) = 'integer' AND progress_observed = 0),
  successor_advance_authorized INTEGER NOT NULL
    CHECK(typeof(successor_advance_authorized) = 'integer'
      AND successor_advance_authorized = 0),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  UNIQUE(graph_run_id, node_id, attempt),
  UNIQUE(schedule_id, execution_ordinal, attempt)
);
INSERT INTO group_agent_graph_scheduled_node_provider_requests_v22
  SELECT id,graph_run_id,schedule_id,scheduled_contract_id,
         provider_request_version,codec_protocol_version,execution_ordinal,node_id,attempt,
         scheduled_contract_sha256,logical_request_id,logical_request_sha256,schedule_sha256,
         project_lane_sha256,provider_kind,endpoint,model,destination_sha256,
         pricing_snapshot_sha256,provider_request_blob,provider_request_bytes,
         provider_request_sha256,prepared_request_sha256,expected_last_event_seq,
         expected_last_event_sha256,provider_request_prepared,provider_request_sent,
         lifecycle_contract_admitted,execution_authority_released,dispatch_authority_released,
         project_lane_claimed,progress_observed,successor_advance_authorized,
         idempotency_key,created_at_ms
  FROM group_agent_graph_scheduled_node_provider_requests;
DROP TABLE group_agent_graph_scheduled_node_provider_requests;
ALTER TABLE group_agent_graph_scheduled_node_provider_requests_v22
  RENAME TO group_agent_graph_scheduled_node_provider_requests;
CREATE INDEX group_agent_graph_scheduled_node_provider_requests_project_lane
  ON group_agent_graph_scheduled_node_provider_requests(
    project_lane_sha256, created_at_ms DESC, id DESC
  );
CREATE INDEX group_agent_graph_scheduled_node_provider_requests_created
  ON group_agent_graph_scheduled_node_provider_requests(created_at_ms DESC, id DESC);
PRAGMA user_version = 22;
CREATE TABLE group_agent_graph_scheduled_node_dispatch_lifecycles_v22 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  provider_request_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_scheduled_node_provider_requests(id) ON DELETE RESTRICT
    CHECK(typeof(provider_request_id) = 'text'
      AND length(CAST(provider_request_id AS BLOB)) BETWEEN 1 AND 128),
  authorization_id TEXT NOT NULL UNIQUE
    CHECK(typeof(authorization_id) = 'text'
      AND length(CAST(authorization_id AS BLOB)) BETWEEN 1 AND 256),
  authorization_sha256 BLOB NOT NULL
    CHECK(typeof(authorization_sha256) = 'blob' AND length(authorization_sha256) = 32),
  provider_request_sha256 BLOB NOT NULL
    CHECK(typeof(provider_request_sha256) = 'blob' AND length(provider_request_sha256) = 32),
  request_body_blob BLOB NOT NULL
    CHECK(typeof(request_body_blob) = 'blob' AND length(request_body_blob) BETWEEN 1 AND 16777216),
  request_body_bytes INTEGER NOT NULL
    CHECK(typeof(request_body_bytes) = 'integer' AND request_body_bytes = length(request_body_blob)),
  project_lane_sha256 BLOB NOT NULL
    CHECK(typeof(project_lane_sha256) = 'blob' AND length(project_lane_sha256) = 32),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text' AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  claim_json BLOB NOT NULL
    CHECK(typeof(claim_json) = 'blob' AND length(claim_json) BETWEEN 1 AND 65536),
  claim_json_bytes INTEGER NOT NULL
    CHECK(typeof(claim_json_bytes) = 'integer' AND claim_json_bytes = length(claim_json)),
  active_lane_json BLOB NOT NULL
    CHECK(typeof(active_lane_json) = 'blob' AND length(active_lane_json) BETWEEN 1 AND 65536),
  active_lane_json_bytes INTEGER NOT NULL
    CHECK(typeof(active_lane_json_bytes) = 'integer' AND active_lane_json_bytes = length(active_lane_json)),
  release_control_json BLOB NOT NULL
    CHECK(typeof(release_control_json) = 'blob' AND length(release_control_json) BETWEEN 1 AND 67108864),
  release_control_json_bytes INTEGER NOT NULL
    CHECK(typeof(release_control_json_bytes) = 'integer' AND release_control_json_bytes = length(release_control_json)),
  authorization_json BLOB NOT NULL
    CHECK(typeof(authorization_json) = 'blob' AND length(authorization_json) BETWEEN 1 AND 1048576),
  authorization_json_bytes INTEGER NOT NULL
    CHECK(typeof(authorization_json_bytes) = 'integer' AND authorization_json_bytes = length(authorization_json)),
  pricing_json BLOB NOT NULL
    CHECK(typeof(pricing_json) = 'blob' AND length(pricing_json) BETWEEN 1 AND 16384),
  pricing_json_bytes INTEGER NOT NULL
    CHECK(typeof(pricing_json_bytes) = 'integer' AND pricing_json_bytes = length(pricing_json)),
  claim_event_json BLOB NOT NULL
    CHECK(typeof(claim_event_json) = 'blob' AND length(claim_event_json) BETWEEN 1 AND 65536),
  claim_event_json_bytes INTEGER NOT NULL
    CHECK(typeof(claim_event_json_bytes) = 'integer' AND claim_event_json_bytes = length(claim_event_json)),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text' AND status IN ('claimed','terminalized','quarantined')),
  lane_active INTEGER NOT NULL
    CHECK(typeof(lane_active) = 'integer'
      AND ((status = 'claimed' AND lane_active = 1)
        OR (status IN ('terminalized','quarantined') AND lane_active = 0))),
  artifact_json BLOB
    CHECK(artifact_json IS NULL OR (typeof(artifact_json) = 'blob' AND length(artifact_json) BETWEEN 1 AND 1048576)),
  artifact_json_bytes INTEGER
    CHECK((artifact_json IS NULL AND artifact_json_bytes IS NULL)
      OR (typeof(artifact_json_bytes) = 'integer' AND artifact_json_bytes = length(artifact_json))),
  terminal_control_json BLOB
    CHECK(terminal_control_json IS NULL OR (typeof(terminal_control_json) = 'blob' AND length(terminal_control_json) BETWEEN 1 AND 67108864)),
  terminal_control_json_bytes INTEGER
    CHECK((terminal_control_json IS NULL AND terminal_control_json_bytes IS NULL)
      OR (typeof(terminal_control_json_bytes) = 'integer' AND terminal_control_json_bytes = length(terminal_control_json))),
  terminal_receipt_json BLOB
    CHECK(terminal_receipt_json IS NULL OR (typeof(terminal_receipt_json) = 'blob' AND length(terminal_receipt_json) BETWEEN 1 AND 65536)),
  terminal_receipt_json_bytes INTEGER
    CHECK((terminal_receipt_json IS NULL AND terminal_receipt_json_bytes IS NULL)
      OR (typeof(terminal_receipt_json_bytes) = 'integer' AND terminal_receipt_json_bytes = length(terminal_receipt_json))),
  created_at_ms INTEGER NOT NULL CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  terminalized_at_ms INTEGER
    CHECK(terminalized_at_ms IS NULL OR (typeof(terminalized_at_ms) = 'integer' AND terminalized_at_ms >= created_at_ms)),
  UNIQUE(graph_run_id, node_id, attempt)
);
INSERT INTO group_agent_graph_scheduled_node_dispatch_lifecycles_v22
  SELECT id,graph_run_id,provider_request_id,authorization_id,authorization_sha256,provider_request_sha256,request_body_blob,request_body_bytes,project_lane_sha256,node_id,attempt,claim_json,claim_json_bytes,active_lane_json,active_lane_json_bytes,release_control_json,release_control_json_bytes,authorization_json,authorization_json_bytes,pricing_json,pricing_json_bytes,claim_event_json,claim_event_json_bytes,status,lane_active,artifact_json,artifact_json_bytes,terminal_control_json,terminal_control_json_bytes,terminal_receipt_json,terminal_receipt_json_bytes,created_at_ms,terminalized_at_ms
  FROM group_agent_graph_scheduled_node_dispatch_lifecycles;
DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
ALTER TABLE group_agent_graph_scheduled_node_dispatch_lifecycles_v22
  RENAME TO group_agent_graph_scheduled_node_dispatch_lifecycles;
CREATE UNIQUE INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active
  ON group_agent_graph_scheduled_node_dispatch_lifecycles(project_lane_sha256)
  WHERE lane_active = 1;
CREATE INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_created
  ON group_agent_graph_scheduled_node_dispatch_lifecycles(created_at_ms DESC, id DESC);
PRAGMA user_version = 22;";
