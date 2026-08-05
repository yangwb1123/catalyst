pub(super) const MIGRATE_V16_TO_V17_SQL: &str =
    "CREATE TABLE group_agent_graph_scheduled_node_successor_candidates (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  graph_id TEXT NOT NULL REFERENCES group_agent_graphs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_id) = 'text'
      AND length(CAST(graph_id AS BLOB)) BETWEEN 1 AND 128),
  schedule_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_execution_schedules(id) ON DELETE RESTRICT
    CHECK(typeof(schedule_id) = 'text'
      AND length(CAST(schedule_id AS BLOB)) BETWEEN 1 AND 128),
  contract_version INTEGER NOT NULL
    CHECK(typeof(contract_version) = 'integer' AND contract_version = 2),
  scheduler_protocol_version INTEGER NOT NULL
    CHECK(typeof(scheduler_protocol_version) = 'integer'
      AND scheduler_protocol_version = 1),
  node_execution_protocol_version INTEGER NOT NULL
    CHECK(typeof(node_execution_protocol_version) = 'integer'
      AND node_execution_protocol_version = 2),
  execution_schedule_protocol_version INTEGER NOT NULL
    CHECK(typeof(execution_schedule_protocol_version) = 'integer'
      AND execution_schedule_protocol_version = 1),
  contract_scope TEXT NOT NULL
    CHECK(typeof(contract_scope) = 'text'
      AND contract_scope = 'schedule_successor_only'),
  control_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(control_snapshot_sha256) = 'blob'
      AND length(control_snapshot_sha256) = 32),
  schedule_sha256 BLOB NOT NULL
    CHECK(typeof(schedule_sha256) = 'blob' AND length(schedule_sha256) = 32),
  expected_last_event_seq INTEGER NOT NULL
    CHECK(typeof(expected_last_event_seq) = 'integer'
      AND expected_last_event_seq = 1),
  expected_last_event_sha256 BLOB NOT NULL
    CHECK(typeof(expected_last_event_sha256) = 'blob'
      AND length(expected_last_event_sha256) = 32),
  execution_ordinal INTEGER NOT NULL
    CHECK(typeof(execution_ordinal) = 'integer' AND execution_ordinal >= 1),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  authored_node_index INTEGER NOT NULL
    CHECK(typeof(authored_node_index) = 'integer'
      AND authored_node_index BETWEEN 0 AND 31),
  topology_wave_index INTEGER NOT NULL
    CHECK(typeof(topology_wave_index) = 'integer'
      AND topology_wave_index BETWEEN 0 AND 31),
  attempt INTEGER NOT NULL
    CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  project_lane_sha256 BLOB NOT NULL
    CHECK(typeof(project_lane_sha256) = 'blob'
      AND length(project_lane_sha256) = 32),
  request_id TEXT NOT NULL UNIQUE
    CHECK(typeof(request_id) = 'text'
      AND length(CAST(request_id AS BLOB)) BETWEEN 1 AND 128),
  request_sha256 BLOB NOT NULL
    CHECK(typeof(request_sha256) = 'blob' AND length(request_sha256) = 32),
  required_predecessor_node_count INTEGER NOT NULL
    CHECK(typeof(required_predecessor_node_count) = 'integer'
      AND required_predecessor_node_count BETWEEN 0 AND 31),
  predecessor_receipt_count INTEGER NOT NULL
    CHECK(typeof(predecessor_receipt_count) = 'integer'
      AND predecessor_receipt_count BETWEEN 1 AND 31),
  lifecycle_contract_admitted INTEGER NOT NULL
    CHECK(typeof(lifecycle_contract_admitted) = 'integer'
      AND lifecycle_contract_admitted = 0),
  provider_request_present INTEGER NOT NULL
    CHECK(typeof(provider_request_present) = 'integer'
      AND provider_request_present = 0),
  execution_authority_released INTEGER NOT NULL
    CHECK(typeof(execution_authority_released) = 'integer'
      AND execution_authority_released = 0),
  dispatch_authority_released INTEGER NOT NULL
    CHECK(typeof(dispatch_authority_released) = 'integer'
      AND dispatch_authority_released = 0),
  progress_observed INTEGER NOT NULL
    CHECK(typeof(progress_observed) = 'integer' AND progress_observed = 0),
  successor_advance_authorized INTEGER NOT NULL
    CHECK(typeof(successor_advance_authorized) = 'integer'
      AND successor_advance_authorized = 0),
  contract_blob BLOB NOT NULL
    CHECK(typeof(contract_blob) = 'blob'
      AND length(contract_blob) BETWEEN 1 AND 4194304),
  contract_bytes INTEGER NOT NULL
    CHECK(typeof(contract_bytes) = 'integer'
      AND contract_bytes BETWEEN 1 AND 4194304
      AND contract_bytes = length(contract_blob)),
  contract_sha256 BLOB NOT NULL
    CHECK(typeof(contract_sha256) = 'blob' AND length(contract_sha256) = 32),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  UNIQUE(graph_run_id, node_id, attempt),
  UNIQUE(schedule_id, execution_ordinal, attempt)
);
CREATE INDEX group_agent_graph_scheduled_node_successor_candidates_created
  ON group_agent_graph_scheduled_node_successor_candidates(created_at_ms DESC, id DESC);
PRAGMA user_version = 17;";
