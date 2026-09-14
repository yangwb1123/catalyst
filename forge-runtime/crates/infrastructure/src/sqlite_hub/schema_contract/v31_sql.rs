// SQLite v31 migration: bind newly shared Conversations to one verified principal.
// Existing Conversations intentionally remain ownerless and are not claimable by migration.
pub(super) const MIGRATE_V30_TO_V31_SQL: &str = r"CREATE TABLE conversation_owners (
  conversation_id TEXT NOT NULL PRIMARY KEY
    REFERENCES conversations(id) ON DELETE RESTRICT
    CHECK(typeof(conversation_id) = 'text'
      AND length(CAST(conversation_id AS BLOB)) BETWEEN 1 AND 128),
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text'
      AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text'
      AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text'
      AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0)
) STRICT, WITHOUT ROWID;
CREATE INDEX conversation_owners_principal
  ON conversation_owners(issuer, subject, tenant_id, conversation_id);
PRAGMA user_version = 31;";
