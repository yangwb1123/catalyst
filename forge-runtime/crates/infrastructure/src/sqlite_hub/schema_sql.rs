pub(super) const CREATE_V1_SCHEMA_SQL: &str = "CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  canonical_path TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0)
);
CREATE TABLE groups (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0)
);
CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  scope_kind TEXT NOT NULL CHECK(scope_kind IN ('global','project','group')),
  scope_id TEXT,
  title TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms >= created_at_ms),
  CHECK(
    (scope_kind = 'global' AND scope_id IS NULL) OR
    (scope_kind != 'global' AND scope_id IS NOT NULL)
  )
);
CREATE TABLE group_projects (
  group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  role TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  added_at_ms INTEGER NOT NULL CHECK(added_at_ms >= 0),
  PRIMARY KEY(group_id, project_id)
);
CREATE TABLE prompts (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0)
);
CREATE INDEX conversations_scope
  ON conversations(scope_kind, scope_id, updated_at_ms DESC);
CREATE INDEX prompts_conversation
  ON prompts(conversation_id, created_at_ms DESC, id DESC);
PRAGMA user_version = 1;";

pub(super) const MIGRATE_V1_TO_V2_SQL: &str = "CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT,
  prompt_id TEXT NOT NULL REFERENCES prompts(id) ON DELETE RESTRICT,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  execution_json TEXT NOT NULL,
  cursor_json TEXT NOT NULL,
  journal_bytes INTEGER NOT NULL CHECK(journal_bytes >= 0),
  idempotency_key TEXT NOT NULL UNIQUE,
  protocol_version INTEGER NOT NULL CHECK(protocol_version > 0),
  created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0)
);
CREATE TABLE run_events (
  run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
  seq INTEGER NOT NULL CHECK(seq > 0),
  event_json TEXT NOT NULL,
  PRIMARY KEY(run_id, seq)
);
CREATE TABLE run_assistant_prompts (
  run_id TEXT PRIMARY KEY REFERENCES runs(id) ON DELETE RESTRICT,
  prompt_id TEXT NOT NULL UNIQUE REFERENCES prompts(id) ON DELETE CASCADE
);
CREATE INDEX runs_conversation
  ON runs(conversation_id, created_at_ms DESC, id DESC);
PRAGMA user_version = 2;";

pub(super) const MIGRATE_V2_TO_V3_SQL: &str = "CREATE TABLE group_runs (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE RESTRICT
    CHECK(typeof(group_id) = 'text'
      AND length(CAST(group_id AS BLOB)) BETWEEN 1 AND 128),
  run_version INTEGER NOT NULL CHECK(run_version = 1),
  status TEXT NOT NULL CHECK(status = 'prepared'),
  context_version INTEGER NOT NULL CHECK(context_version = 1),
  context_slice_sha256 BLOB NOT NULL
    CHECK(typeof(context_slice_sha256) = 'blob' AND length(context_slice_sha256) = 32),
  context_blob BLOB NOT NULL
    CHECK(typeof(context_blob) = 'blob' AND length(context_blob) BETWEEN 1 AND 8388608),
  snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(snapshot_sha256) = 'blob' AND length(snapshot_sha256) = 32),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0)
);
CREATE INDEX group_runs_group
  ON group_runs(group_id, created_at_ms DESC, id DESC);
PRAGMA user_version = 3;";

pub(super) const MIGRATE_V3_TO_V4_SQL: &str = "CREATE TABLE group_executions (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  group_run_id TEXT NOT NULL REFERENCES group_runs(id) ON DELETE RESTRICT
    CHECK(typeof(group_run_id) = 'text'
      AND length(CAST(group_run_id AS BLOB)) BETWEEN 1 AND 128),
  execution_version INTEGER NOT NULL
    CHECK(typeof(execution_version) = 'integer' AND execution_version = 1),
  mode TEXT NOT NULL
    CHECK(typeof(mode) = 'text' AND mode = 'offline_snapshot_validation'),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text' AND status IN ('incomplete', 'completed')),
  source_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(source_snapshot_sha256) = 'blob'
      AND length(source_snapshot_sha256) = 32),
  cursor_json TEXT NOT NULL
    CHECK(typeof(cursor_json) = 'text'
      AND length(CAST(cursor_json AS BLOB)) BETWEEN 1 AND 65536),
  journal_bytes INTEGER NOT NULL
    CHECK(typeof(journal_bytes) = 'integer'
      AND journal_bytes BETWEEN 0 AND 196608),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  protocol_version INTEGER NOT NULL
    CHECK(typeof(protocol_version) = 'integer' AND protocol_version = 1),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE TABLE group_execution_events (
  execution_id TEXT NOT NULL REFERENCES group_executions(id) ON DELETE RESTRICT
    CHECK(typeof(execution_id) = 'text'
      AND length(CAST(execution_id AS BLOB)) BETWEEN 1 AND 128),
  seq INTEGER NOT NULL
    CHECK(typeof(seq) = 'integer' AND seq BETWEEN 1 AND 3),
  event_json TEXT NOT NULL
    CHECK(typeof(event_json) = 'text'
      AND length(CAST(event_json AS BLOB)) BETWEEN 1 AND 65536),
  event_sha256 BLOB NOT NULL
    CHECK(typeof(event_sha256) = 'blob' AND length(event_sha256) = 32),
  PRIMARY KEY(execution_id, seq)
);
CREATE INDEX group_executions_group_run
  ON group_executions(group_run_id, created_at_ms DESC, id DESC);
CREATE INDEX group_executions_created
  ON group_executions(created_at_ms DESC, id DESC);
PRAGMA user_version = 4;";

pub(super) const MIGRATE_V4_TO_V5_SQL: &str = "CREATE TABLE group_model_analyses (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  group_run_id TEXT NOT NULL REFERENCES group_runs(id) ON DELETE RESTRICT
    CHECK(typeof(group_run_id) = 'text'
      AND length(CAST(group_run_id AS BLOB)) BETWEEN 1 AND 128),
  analysis_version INTEGER NOT NULL
    CHECK(typeof(analysis_version) = 'integer' AND analysis_version = 1),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text'
      AND status IN ('awaiting_consent', 'dispatch_unknown', 'completed')),
  source_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(source_snapshot_sha256) = 'blob'
      AND length(source_snapshot_sha256) = 32),
  provider TEXT NOT NULL
    CHECK(typeof(provider) = 'text' AND provider = 'openai_responses'),
  endpoint TEXT NOT NULL
    CHECK(typeof(endpoint) = 'text'
      AND endpoint = 'https://api.openai.com/v1/responses'),
  model TEXT NOT NULL
    CHECK(typeof(model) = 'text'
      AND length(CAST(model AS BLOB)) BETWEEN 1 AND 128),
  system_prompt_version INTEGER NOT NULL
    CHECK(typeof(system_prompt_version) = 'integer'
      AND system_prompt_version = 1),
  system_prompt_sha256 BLOB NOT NULL
    CHECK(typeof(system_prompt_sha256) = 'blob'
      AND length(system_prompt_sha256) = 32),
  max_output_tokens INTEGER NOT NULL
    CHECK(typeof(max_output_tokens) = 'integer'
      AND max_output_tokens BETWEEN 1 AND 32768),
  max_model_output_bytes INTEGER NOT NULL
    CHECK(typeof(max_model_output_bytes) = 'integer'
      AND max_model_output_bytes BETWEEN 1 AND 65536),
  max_model_events INTEGER NOT NULL
    CHECK(typeof(max_model_events) = 'integer'
      AND max_model_events BETWEEN 1 AND 4096),
  config_json TEXT NOT NULL
    CHECK(typeof(config_json) = 'text'
      AND length(CAST(config_json AS BLOB)) BETWEEN 1 AND 131072),
  config_sha256 BLOB NOT NULL
    CHECK(typeof(config_sha256) = 'blob' AND length(config_sha256) = 32),
  request_body BLOB NOT NULL
    CHECK(typeof(request_body) = 'blob'
      AND length(request_body) BETWEEN 1 AND 16777216),
  request_bytes INTEGER NOT NULL
    CHECK(typeof(request_bytes) = 'integer'
      AND request_bytes BETWEEN 1 AND 16777216
      AND request_bytes = length(request_body)),
  request_sha256 BLOB NOT NULL
    CHECK(typeof(request_sha256) = 'blob' AND length(request_sha256) = 32),
  cursor_json TEXT NOT NULL
    CHECK(typeof(cursor_json) = 'text'
      AND length(CAST(cursor_json AS BLOB)) BETWEEN 1 AND 65536),
  journal_bytes INTEGER NOT NULL
    CHECK(typeof(journal_bytes) = 'integer'
      AND journal_bytes BETWEEN 1 AND 196608),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  protocol_version INTEGER NOT NULL
    CHECK(typeof(protocol_version) = 'integer' AND protocol_version = 1),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE TABLE group_model_analysis_events (
  analysis_id TEXT NOT NULL REFERENCES group_model_analyses(id) ON DELETE RESTRICT
    CHECK(typeof(analysis_id) = 'text'
      AND length(CAST(analysis_id AS BLOB)) BETWEEN 1 AND 128),
  seq INTEGER NOT NULL
    CHECK(typeof(seq) = 'integer' AND seq BETWEEN 1 AND 3),
  event_json TEXT NOT NULL
    CHECK(typeof(event_json) = 'text'
      AND length(CAST(event_json AS BLOB)) BETWEEN 1 AND 65536),
  event_sha256 BLOB NOT NULL
    CHECK(typeof(event_sha256) = 'blob' AND length(event_sha256) = 32),
  PRIMARY KEY(analysis_id, seq)
);
CREATE TABLE group_model_analysis_results (
  analysis_id TEXT NOT NULL PRIMARY KEY
    REFERENCES group_model_analyses(id) ON DELETE RESTRICT
    CHECK(typeof(analysis_id) = 'text'
      AND length(CAST(analysis_id AS BLOB)) BETWEEN 1 AND 128),
  result_version INTEGER NOT NULL
    CHECK(typeof(result_version) = 'integer' AND result_version = 1),
  result_blob BLOB NOT NULL
    CHECK(typeof(result_blob) = 'blob'
      AND length(result_blob) BETWEEN 1 AND 524288),
  result_bytes INTEGER NOT NULL
    CHECK(typeof(result_bytes) = 'integer'
      AND result_bytes BETWEEN 1 AND 524288
      AND result_bytes = length(result_blob)),
  result_sha256 BLOB NOT NULL
    CHECK(typeof(result_sha256) = 'blob' AND length(result_sha256) = 32),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE INDEX group_model_analyses_group_run
  ON group_model_analyses(group_run_id, created_at_ms DESC, id DESC);
CREATE INDEX group_model_analyses_created
  ON group_model_analyses(created_at_ms DESC, id DESC);
PRAGMA user_version = 5;";
