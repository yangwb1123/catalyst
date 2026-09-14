// SQLite v33 stores immutable owner-to-Project execution consent and its
// append-only grant/revocation events. No Run or execution state is created.
pub(super) const MIGRATE_V32_TO_V33_SQL: &str = r"CREATE TABLE project_execution_consent_grants (
  grant_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(grant_id) = 'text'
      AND length(CAST(grant_id AS BLOB)) BETWEEN 1 AND 128),
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text'
      AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text'
      AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text'
      AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT
    CHECK(typeof(project_id) = 'text'
      AND length(CAST(project_id AS BLOB)) BETWEEN 1 AND 128),
  profile_id TEXT NOT NULL
    CHECK(typeof(profile_id) = 'text'
      AND length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  profile_sha256 BLOB NOT NULL
    CHECK(typeof(profile_sha256) = 'blob' AND length(profile_sha256) = 32),
  granted_at_ms INTEGER NOT NULL
    CHECK(typeof(granted_at_ms) = 'integer' AND granted_at_ms >= 0),
  expires_at_ms INTEGER NOT NULL
    CHECK(typeof(expires_at_ms) = 'integer'
      AND expires_at_ms > granted_at_ms
      AND expires_at_ms - granted_at_ms <= 2592000000),
  UNIQUE(grant_id, issuer, subject, tenant_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX project_execution_consent_active_lookup
  ON project_execution_consent_grants(
    issuer, subject, tenant_id, project_id, expires_at_ms
  );
CREATE TABLE project_execution_consent_events (
  event_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(event_id) = 'text'
      AND length(CAST(event_id AS BLOB)) BETWEEN 1 AND 128),
  grant_id TEXT NOT NULL
    CHECK(typeof(grant_id) = 'text'
      AND length(CAST(grant_id AS BLOB)) BETWEEN 1 AND 128),
  event_sequence INTEGER NOT NULL
    CHECK(typeof(event_sequence) = 'integer' AND event_sequence IN (1, 2)),
  event_kind TEXT NOT NULL
    CHECK(typeof(event_kind) = 'text' AND event_kind IN ('granted', 'revoked')),
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text'
      AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text'
      AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text'
      AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  idempotency_key TEXT NOT NULL
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 128),
  occurred_at_ms INTEGER NOT NULL
    CHECK(typeof(occurred_at_ms) = 'integer' AND occurred_at_ms >= 0),
  CHECK((event_sequence = 1 AND event_kind = 'granted')
     OR (event_sequence = 2 AND event_kind = 'revoked')),
  UNIQUE(grant_id, event_sequence),
  UNIQUE(issuer, subject, tenant_id, idempotency_key),
  FOREIGN KEY(grant_id, issuer, subject, tenant_id)
    REFERENCES project_execution_consent_grants(grant_id, issuer, subject, tenant_id)
    ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
PRAGMA user_version = 33;";
