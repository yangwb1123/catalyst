// SQLite v30 migration: conversation baselines plus store-local change references.
pub(super) const MIGRATE_V29_TO_V30_SQL: &str = r"CREATE TABLE conversation_change_baselines (
  conversation_id TEXT NOT NULL PRIMARY KEY
    REFERENCES conversations(id) ON DELETE RESTRICT
    CHECK(typeof(conversation_id) = 'text'
      AND length(CAST(conversation_id AS BLOB)) BETWEEN 1 AND 128)
) STRICT, WITHOUT ROWID;
INSERT INTO conversation_change_baselines(conversation_id)
  SELECT id FROM conversations;
CREATE TABLE conversation_change_state (
  state_id INTEGER NOT NULL PRIMARY KEY CHECK(state_id = 1),
  last_cursor INTEGER NOT NULL
    CHECK(typeof(last_cursor) = 'integer' AND last_cursor >= 0)
) STRICT;
INSERT INTO conversation_change_state(state_id, last_cursor) VALUES(1, 0);
CREATE TABLE conversation_change_heads (
  conversation_id TEXT NOT NULL PRIMARY KEY
    REFERENCES conversations(id) ON DELETE RESTRICT
    CHECK(typeof(conversation_id) = 'text'
      AND length(CAST(conversation_id AS BLOB)) BETWEEN 1 AND 128),
  last_version INTEGER NOT NULL
    CHECK(typeof(last_version) = 'integer' AND last_version >= 0)
) STRICT, WITHOUT ROWID;
INSERT INTO conversation_change_heads(conversation_id, last_version)
  SELECT id, 0 FROM conversations;
CREATE TABLE conversation_changes (
  cursor INTEGER PRIMARY KEY
    CHECK(typeof(cursor) = 'integer' AND cursor > 0),
  conversation_id TEXT NOT NULL
    REFERENCES conversations(id) ON DELETE RESTRICT
    CHECK(typeof(conversation_id) = 'text'
      AND length(CAST(conversation_id AS BLOB)) BETWEEN 1 AND 128),
  entity_id TEXT NOT NULL
    CHECK(typeof(entity_id) = 'text'
      AND length(CAST(entity_id AS BLOB)) BETWEEN 1 AND 128),
  aggregate_version INTEGER NOT NULL
    CHECK(typeof(aggregate_version) = 'integer' AND aggregate_version > 0),
  event_schema_version INTEGER NOT NULL
    CHECK(typeof(event_schema_version) = 'integer' AND event_schema_version = 1),
  event_kind TEXT NOT NULL
    CHECK(typeof(event_kind) = 'text' AND event_kind IN (
      'conversation_created','prompt_appended')),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
);
CREATE UNIQUE INDEX conversation_changes_conversation_version
  ON conversation_changes(conversation_id, aggregate_version);
CREATE UNIQUE INDEX conversation_changes_event_entity
  ON conversation_changes(event_kind, entity_id);
PRAGMA user_version = 30;";
