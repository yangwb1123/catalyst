pub(super) const MIGRATE_V12_TO_V13_SQL: &str =
    "CREATE TABLE group_agent_graph_execution_schedules (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  graph_id TEXT NOT NULL REFERENCES group_agent_graphs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_id) = 'text'
      AND length(CAST(graph_id AS BLOB)) BETWEEN 1 AND 128),
  schedule_version INTEGER NOT NULL
    CHECK(typeof(schedule_version) = 'integer' AND schedule_version = 1),
  scheduler_protocol_version INTEGER NOT NULL
    CHECK(typeof(scheduler_protocol_version) = 'integer'
      AND scheduler_protocol_version = 1),
  execution_schedule_protocol_version INTEGER NOT NULL
    CHECK(typeof(execution_schedule_protocol_version) = 'integer'
      AND execution_schedule_protocol_version = 1),
  control_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(control_snapshot_sha256) = 'blob'
      AND length(control_snapshot_sha256) = 32),
  expected_last_event_seq INTEGER NOT NULL
    CHECK(typeof(expected_last_event_seq) = 'integer'
      AND expected_last_event_seq = 1),
  expected_last_event_sha256 BLOB NOT NULL
    CHECK(typeof(expected_last_event_sha256) = 'blob'
      AND length(expected_last_event_sha256) = 32),
  initial_node TEXT NOT NULL
    CHECK(typeof(initial_node) = 'text'
      AND length(CAST(initial_node AS BLOB)) BETWEEN 1 AND 128),
  node_count INTEGER NOT NULL
    CHECK(typeof(node_count) = 'integer' AND node_count BETWEEN 2 AND 32),
  wave_count INTEGER NOT NULL
    CHECK(typeof(wave_count) = 'integer'
      AND wave_count BETWEEN 1 AND 32 AND wave_count <= node_count),
  execution_contract_present INTEGER NOT NULL
    CHECK(typeof(execution_contract_present) = 'integer'
      AND execution_contract_present = 0),
  dispatch_authority_released INTEGER NOT NULL
    CHECK(typeof(dispatch_authority_released) = 'integer'
      AND dispatch_authority_released = 0),
  progress_observed INTEGER NOT NULL
    CHECK(typeof(progress_observed) = 'integer' AND progress_observed = 0),
  successor_advanced INTEGER NOT NULL
    CHECK(typeof(successor_advanced) = 'integer' AND successor_advanced = 0),
  schedule_blob BLOB NOT NULL
    CHECK(typeof(schedule_blob) = 'blob'
      AND length(schedule_blob) BETWEEN 1 AND 1048576),
  schedule_bytes INTEGER NOT NULL
    CHECK(typeof(schedule_bytes) = 'integer'
      AND schedule_bytes BETWEEN 1 AND 1048576
      AND schedule_bytes = length(schedule_blob)),
  schedule_sha256 BLOB NOT NULL
    CHECK(typeof(schedule_sha256) = 'blob' AND length(schedule_sha256) = 32),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE INDEX group_agent_graph_execution_schedules_created
  ON group_agent_graph_execution_schedules(created_at_ms DESC, id DESC);
PRAGMA user_version = 13;";
