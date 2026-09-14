// SQLite v32 assigns dense change cursors within each verified owner principal.
pub(super) const MIGRATE_V31_TO_V32_SQL: &str = r"CREATE UNIQUE INDEX conversation_owners_principal_conversation
  ON conversation_owners(issuer, subject, tenant_id, conversation_id);
CREATE UNIQUE INDEX conversation_changes_conversation_cursor
  ON conversation_changes(conversation_id, cursor);
CREATE TABLE conversation_owner_change_heads (
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text' AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text' AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text' AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  last_cursor INTEGER NOT NULL CHECK(typeof(last_cursor) = 'integer' AND last_cursor >= 0),
  PRIMARY KEY(issuer, subject, tenant_id)
) STRICT, WITHOUT ROWID;
CREATE TABLE conversation_owner_change_rows (
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text' AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text' AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text' AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  owner_cursor INTEGER NOT NULL
    CHECK(typeof(owner_cursor) = 'integer' AND owner_cursor >= 1),
  conversation_id TEXT NOT NULL
    CHECK(typeof(conversation_id) = 'text'
      AND length(CAST(conversation_id AS BLOB)) BETWEEN 1 AND 128),
  hub_cursor INTEGER NOT NULL CHECK(typeof(hub_cursor) = 'integer' AND hub_cursor >= 1),
  PRIMARY KEY(issuer, subject, tenant_id, owner_cursor),
  UNIQUE(hub_cursor),
  FOREIGN KEY(issuer, subject, tenant_id)
    REFERENCES conversation_owner_change_heads(issuer, subject, tenant_id) ON DELETE RESTRICT,
  FOREIGN KEY(issuer, subject, tenant_id, conversation_id)
    REFERENCES conversation_owners(issuer, subject, tenant_id, conversation_id) ON DELETE RESTRICT,
  FOREIGN KEY(conversation_id, hub_cursor)
    REFERENCES conversation_changes(conversation_id, cursor) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
INSERT INTO conversation_owner_change_heads(issuer, subject, tenant_id, last_cursor)
  SELECT DISTINCT issuer, subject, tenant_id, 0 FROM conversation_owners;
INSERT INTO conversation_owner_change_rows(
  issuer, subject, tenant_id, owner_cursor, conversation_id, hub_cursor
)
SELECT o.issuer, o.subject, o.tenant_id,
       ROW_NUMBER() OVER (
         PARTITION BY o.issuer, o.subject, o.tenant_id ORDER BY c.cursor
       ), o.conversation_id, c.cursor
FROM conversation_owners AS o
JOIN conversation_changes AS c ON c.conversation_id = o.conversation_id;
UPDATE conversation_owner_change_heads
SET last_cursor = (
  SELECT COUNT(*) FROM conversation_owner_change_rows AS r
  WHERE r.issuer = conversation_owner_change_heads.issuer
    AND r.subject = conversation_owner_change_heads.subject
    AND r.tenant_id = conversation_owner_change_heads.tenant_id
);
PRAGMA user_version = 32;";
