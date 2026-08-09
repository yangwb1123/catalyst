// v25 — endpoint pinning relaxation for group_model_analyses /
// group_panel_syntheses: the endpoint CHECK now accepts the official
// endpoint OR an explicit caller-pinned self-hosted /v1/responses gateway
// (the provider-side endpoint policy still enforces https-anywhere /
// http-loopback-only). Data-bearing rebuild of BOTH parent tables AND their
// children (events/results): dropping children first eliminates any dangling
// FK toward a dropped parent at COMMIT (defer_foreign_keys defers the final
// re-check; a child pointing at a dropped name would fail it).
pub(super) const MIGRATE_V24_TO_V25_SQL: &str = r"
CREATE TABLE group_model_analyses_v25 (
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
      AND length(CAST(endpoint AS BLOB)) BETWEEN 1 AND 512
      AND (endpoint = 'https://api.openai.com/v1/responses'
        OR (endpoint LIKE 'http://%' OR endpoint LIKE 'https://%')
           AND endpoint LIKE '%/v1/responses')),
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
INSERT INTO group_model_analyses_v25 SELECT * FROM group_model_analyses;
CREATE TABLE group_model_analysis_events_v25 (
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
INSERT INTO group_model_analysis_events_v25 SELECT * FROM group_model_analysis_events;
CREATE TABLE group_model_analysis_results_v25 (
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
INSERT INTO group_model_analysis_results_v25 SELECT * FROM group_model_analysis_results;
CREATE TABLE group_panel_syntheses_v25 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  panel_id TEXT NOT NULL REFERENCES group_analysis_panels(id) ON DELETE RESTRICT
    CHECK(typeof(panel_id) = 'text'
      AND length(CAST(panel_id AS BLOB)) BETWEEN 1 AND 128),
  group_run_id TEXT NOT NULL REFERENCES group_runs(id) ON DELETE RESTRICT
    CHECK(typeof(group_run_id) = 'text'
      AND length(CAST(group_run_id AS BLOB)) BETWEEN 1 AND 128),
  synthesis_version INTEGER NOT NULL
    CHECK(typeof(synthesis_version) = 'integer' AND synthesis_version = 1),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text'
      AND status IN ('awaiting_consent', 'dispatch_unknown', 'completed')),
  source_snapshot_sha256 BLOB NOT NULL
    CHECK(typeof(source_snapshot_sha256) = 'blob'
      AND length(source_snapshot_sha256) = 32),
  panel_manifest_sha256 BLOB NOT NULL
    CHECK(typeof(panel_manifest_sha256) = 'blob'
      AND length(panel_manifest_sha256) = 32),
  provider TEXT NOT NULL
    CHECK(typeof(provider) = 'text' AND provider = 'openai_responses'),
  endpoint TEXT NOT NULL
    CHECK(typeof(endpoint) = 'text'
      AND length(CAST(endpoint AS BLOB)) BETWEEN 1 AND 512
      AND (endpoint = 'https://api.openai.com/v1/responses'
        OR (endpoint LIKE 'http://%' OR endpoint LIKE 'https://%')
           AND endpoint LIKE '%/v1/responses')),
  model TEXT NOT NULL
    CHECK(typeof(model) = 'text'
      AND length(CAST(model AS BLOB)) BETWEEN 1 AND 128),
  system_prompt_version INTEGER NOT NULL
    CHECK(typeof(system_prompt_version) = 'integer'
      AND system_prompt_version = 1),
  system_prompt_sha256 BLOB NOT NULL
    CHECK(typeof(system_prompt_sha256) = 'blob'
      AND length(system_prompt_sha256) = 32),
  output_target TEXT NOT NULL
    CHECK(typeof(output_target) = 'text' AND output_target = 'local_artifact'),
  writeback_target TEXT NOT NULL
    CHECK(typeof(writeback_target) = 'text' AND writeback_target = 'none'),
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
INSERT INTO group_panel_syntheses_v25 SELECT * FROM group_panel_syntheses;
CREATE TABLE group_panel_synthesis_events_v25 (
  synthesis_id TEXT NOT NULL REFERENCES group_panel_syntheses(id) ON DELETE RESTRICT
    CHECK(typeof(synthesis_id) = 'text'
      AND length(CAST(synthesis_id AS BLOB)) BETWEEN 1 AND 128),
  seq INTEGER NOT NULL
    CHECK(typeof(seq) = 'integer' AND seq BETWEEN 1 AND 3),
  event_json TEXT NOT NULL
    CHECK(typeof(event_json) = 'text'
      AND length(CAST(event_json AS BLOB)) BETWEEN 1 AND 65536),
  event_sha256 BLOB NOT NULL
    CHECK(typeof(event_sha256) = 'blob' AND length(event_sha256) = 32),
  PRIMARY KEY(synthesis_id, seq)

);
INSERT INTO group_panel_synthesis_events_v25 SELECT * FROM group_panel_synthesis_events;
CREATE TABLE group_panel_synthesis_results_v25 (
  synthesis_id TEXT NOT NULL PRIMARY KEY
    REFERENCES group_panel_syntheses(id) ON DELETE RESTRICT
    CHECK(typeof(synthesis_id) = 'text'
      AND length(CAST(synthesis_id AS BLOB)) BETWEEN 1 AND 128),
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
INSERT INTO group_panel_synthesis_results_v25 SELECT * FROM group_panel_synthesis_results;
DROP TABLE group_panel_synthesis_results;
DROP TABLE group_panel_synthesis_events;
DROP TABLE group_model_analysis_results;
DROP TABLE group_model_analysis_events;
DROP TABLE group_panel_syntheses;
DROP TABLE group_model_analyses;
ALTER TABLE group_model_analyses_v25 RENAME TO group_model_analyses;
ALTER TABLE group_model_analysis_events_v25 RENAME TO group_model_analysis_events;
ALTER TABLE group_model_analysis_results_v25 RENAME TO group_model_analysis_results;
ALTER TABLE group_panel_syntheses_v25 RENAME TO group_panel_syntheses;
ALTER TABLE group_panel_synthesis_events_v25 RENAME TO group_panel_synthesis_events;
ALTER TABLE group_panel_synthesis_results_v25 RENAME TO group_panel_synthesis_results;
CREATE INDEX group_model_analyses_group_run
  ON group_model_analyses(group_run_id, created_at_ms DESC, id DESC);
CREATE INDEX group_model_analyses_created
  ON group_model_analyses(created_at_ms DESC, id DESC);
CREATE INDEX group_panel_syntheses_panel
  ON group_panel_syntheses(panel_id, created_at_ms DESC, id DESC);
CREATE INDEX group_panel_syntheses_created
  ON group_panel_syntheses(created_at_ms DESC, id DESC);
-- The rebuild DROPs parents whose children still carry FK references;
-- defer_foreign_keys (set by the migration framework) defers the re-check
-- to COMMIT, but SQLite then reports the dropped-parent state even after
-- the RENAME restored the name. Turning defer OFF here forces the final
-- re-check NOW, against the consistent post-rebuild schema.
PRAGMA defer_foreign_keys = OFF;
PRAGMA user_version = 25;";