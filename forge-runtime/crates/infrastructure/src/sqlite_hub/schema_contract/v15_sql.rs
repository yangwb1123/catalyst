pub(super) const MIGRATE_V14_TO_V15_SQL: &str =
    "CREATE TABLE group_agent_graph_scheduled_node_provider_requests (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  schedule_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_execution_schedules(id) ON DELETE RESTRICT
    CHECK(typeof(schedule_id) = 'text'
      AND length(CAST(schedule_id AS BLOB)) BETWEEN 1 AND 128),
  scheduled_contract_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_scheduled_node_contract_candidates(id) ON DELETE RESTRICT
    CHECK(typeof(scheduled_contract_id) = 'text'
      AND length(CAST(scheduled_contract_id AS BLOB)) BETWEEN 1 AND 128),
  provider_request_version INTEGER NOT NULL
    CHECK(typeof(provider_request_version) = 'integer'
      AND provider_request_version = 1),
  codec_protocol_version INTEGER NOT NULL
    CHECK(typeof(codec_protocol_version) = 'integer'
      AND codec_protocol_version = 1),
  execution_ordinal INTEGER NOT NULL
    CHECK(typeof(execution_ordinal) = 'integer' AND execution_ordinal = 0),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text'
      AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL
    CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  scheduled_contract_sha256 BLOB NOT NULL
    CHECK(typeof(scheduled_contract_sha256) = 'blob'
      AND length(scheduled_contract_sha256) = 32),
  logical_request_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_scheduled_node_contract_candidates(request_id)
      ON DELETE RESTRICT
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
CREATE INDEX group_agent_graph_scheduled_node_provider_requests_project_lane
  ON group_agent_graph_scheduled_node_provider_requests(
    project_lane_sha256, created_at_ms DESC, id DESC
  );
CREATE INDEX group_agent_graph_scheduled_node_provider_requests_created
  ON group_agent_graph_scheduled_node_provider_requests(created_at_ms DESC, id DESC);
PRAGMA user_version = 15;";
