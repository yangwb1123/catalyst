// SQLite v29 migration: bounded append-only controller state for serial
// scheduled whole-Graph progression. The journal is independent from the
// immutable seq-1 Graph Run control journal.
pub(super) const MIGRATE_V28_TO_V29_SQL: &str = r"CREATE TABLE group_agent_scheduled_graph_controllers (
  controller_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(controller_id) = 'text'
      AND length(CAST(controller_id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  schedule_id TEXT NOT NULL
    REFERENCES group_agent_graph_execution_schedules(id) ON DELETE RESTRICT
    CHECK(typeof(schedule_id) = 'text'
      AND length(CAST(schedule_id AS BLOB)) BETWEEN 1 AND 128),
  version INTEGER NOT NULL
    CHECK(typeof(version) = 'integer' AND version = 1),
  controller_protocol_version INTEGER NOT NULL
    CHECK(typeof(controller_protocol_version) = 'integer'
      AND controller_protocol_version = 1),
  schedule_version INTEGER NOT NULL
    CHECK(typeof(schedule_version) = 'integer' AND schedule_version = 1),
  progress_protocol_version INTEGER NOT NULL
    CHECK(typeof(progress_protocol_version) = 'integer'
      AND progress_protocol_version = 1),
  schedule_sha256 BLOB NOT NULL
    CHECK(typeof(schedule_sha256) = 'blob' AND length(schedule_sha256) = 32),
  core_bin_sha256 BLOB NOT NULL
    CHECK(typeof(core_bin_sha256) = 'blob' AND length(core_bin_sha256) = 32),
  execution_profile_sha256 BLOB NOT NULL
    CHECK(typeof(execution_profile_sha256) = 'blob'
      AND length(execution_profile_sha256) = 32),
  node_count INTEGER NOT NULL
    CHECK(typeof(node_count) = 'integer' AND node_count BETWEEN 1 AND 32),
  max_effectful_steps INTEGER NOT NULL
    CHECK(typeof(max_effectful_steps) = 'integer'
      AND max_effectful_steps BETWEEN 1 AND 32
      AND max_effectful_steps <= node_count),
  max_total_cost_usd_micros INTEGER NOT NULL
    CHECK(typeof(max_total_cost_usd_micros) = 'integer'
      AND max_total_cost_usd_micros > 0),
  controller_sha256 BLOB NOT NULL UNIQUE
    CHECK(typeof(controller_sha256) = 'blob' AND length(controller_sha256) = 32),
  header_blob BLOB NOT NULL
    CHECK(typeof(header_blob) = 'blob' AND length(header_blob) BETWEEN 1 AND 65536),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE INDEX group_agent_scheduled_graph_controllers_schedule
  ON group_agent_scheduled_graph_controllers(schedule_id, created_at_ms, controller_id);
CREATE TABLE group_agent_scheduled_graph_controller_events (
  controller_id TEXT NOT NULL
    REFERENCES group_agent_scheduled_graph_controllers(controller_id) ON DELETE RESTRICT,
  sequence INTEGER NOT NULL
    CHECK(typeof(sequence) = 'integer' AND sequence BETWEEN 1 AND 512),
  previous_event_sha256 BLOB
    CHECK(previous_event_sha256 IS NULL
      OR (typeof(previous_event_sha256) = 'blob' AND length(previous_event_sha256) = 32)),
  event_sha256 BLOB NOT NULL UNIQUE
    CHECK(typeof(event_sha256) = 'blob' AND length(event_sha256) = 32),
  event_kind TEXT NOT NULL
    CHECK(typeof(event_kind) = 'text' AND event_kind IN (
      'started','materialize_planned','materialize_observed','prepare_planned',
      'prepare_observed','awaiting_fresh_consent','dispatch_planned',
      'node_completed','retryable_preclaim_failure','stopped','completed')),
  effectful_step_reservation INTEGER
    CHECK(effectful_step_reservation IS NULL
      OR (typeof(effectful_step_reservation) = 'integer'
        AND effectful_step_reservation BETWEEN 1 AND 32)),
  reserved_cost_usd_micros INTEGER
    CHECK(reserved_cost_usd_micros IS NULL
      OR (typeof(reserved_cost_usd_micros) = 'integer'
        AND reserved_cost_usd_micros > 0)),
  event_blob BLOB NOT NULL
    CHECK(typeof(event_blob) = 'blob' AND length(event_blob) BETWEEN 1 AND 65536),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  PRIMARY KEY(controller_id, sequence),
  CHECK((sequence = 1 AND previous_event_sha256 IS NULL)
    OR (sequence > 1 AND previous_event_sha256 IS NOT NULL)),
  CHECK((event_kind = 'dispatch_planned'
      AND effectful_step_reservation IS NOT NULL
      AND reserved_cost_usd_micros IS NOT NULL)
    OR (event_kind <> 'dispatch_planned'
      AND effectful_step_reservation IS NULL
      AND reserved_cost_usd_micros IS NULL))
);
PRAGMA user_version = 29;";
