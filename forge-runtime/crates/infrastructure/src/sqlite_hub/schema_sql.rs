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
