// SQLite v34 stores a consent-checked owner-bound Prompt submission as an
// inert pending intent with a closed, payload-free initial event.
pub(super) const MIGRATE_V33_TO_V34_SQL: &str = r"CREATE TABLE pending_run_intents (
  intent_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(intent_id) = 'text'
      AND length(CAST(intent_id AS BLOB)) BETWEEN 1 AND 128),
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text'
      AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text'
      AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text'
      AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE RESTRICT
    CHECK(typeof(conversation_id) = 'text'
      AND length(CAST(conversation_id AS BLOB)) BETWEEN 1 AND 128),
  prompt_id TEXT NOT NULL UNIQUE REFERENCES prompts(id) ON DELETE RESTRICT
    CHECK(typeof(prompt_id) = 'text'
      AND length(CAST(prompt_id AS BLOB)) BETWEEN 1 AND 128),
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT
    CHECK(typeof(project_id) = 'text'
      AND length(CAST(project_id AS BLOB)) BETWEEN 1 AND 128),
  consent_grant_id TEXT NOT NULL
    CHECK(typeof(consent_grant_id) = 'text'
      AND length(CAST(consent_grant_id AS BLOB)) BETWEEN 1 AND 128),
  profile_id TEXT NOT NULL
    CHECK(typeof(profile_id) = 'text'
      AND length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
  profile_sha256 BLOB NOT NULL
    CHECK(typeof(profile_sha256) = 'blob' AND length(profile_sha256) = 32),
  idempotency_key TEXT NOT NULL
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 128),
  aggregate_version INTEGER NOT NULL
    CHECK(typeof(aggregate_version) = 'integer' AND aggregate_version > 0),
  submitted_at_ms INTEGER NOT NULL
    CHECK(typeof(submitted_at_ms) = 'integer' AND submitted_at_ms >= 0),
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK(typeof(status) = 'text' AND status = 'pending'),
  UNIQUE(intent_id, issuer, subject, tenant_id),
  UNIQUE(issuer, subject, tenant_id, idempotency_key),
  FOREIGN KEY(consent_grant_id, issuer, subject, tenant_id)
    REFERENCES project_execution_consent_grants(grant_id, issuer, subject, tenant_id)
    ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
CREATE INDEX pending_run_intents_owner_conversation_page
  ON pending_run_intents(
    issuer, subject, tenant_id, conversation_id, submitted_at_ms DESC, intent_id DESC
  );
CREATE TABLE pending_run_intent_events (
  event_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(event_id) = 'text'
      AND length(CAST(event_id AS BLOB)) BETWEEN 1 AND 128),
  intent_id TEXT NOT NULL
    CHECK(typeof(intent_id) = 'text'
      AND length(CAST(intent_id AS BLOB)) BETWEEN 1 AND 128),
  event_sequence INTEGER NOT NULL
    CHECK(typeof(event_sequence) = 'integer' AND event_sequence = 1),
  event_kind TEXT NOT NULL
    CHECK(typeof(event_kind) = 'text' AND event_kind = 'submitted'),
  issuer TEXT NOT NULL
    CHECK(typeof(issuer) = 'text'
      AND length(CAST(issuer AS BLOB)) BETWEEN 1 AND 2048),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text'
      AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 255),
  tenant_id TEXT NOT NULL
    CHECK(typeof(tenant_id) = 'text'
      AND length(CAST(tenant_id AS BLOB)) BETWEEN 1 AND 256),
  occurred_at_ms INTEGER NOT NULL
    CHECK(typeof(occurred_at_ms) = 'integer' AND occurred_at_ms >= 0),
  UNIQUE(intent_id, event_sequence),
  FOREIGN KEY(intent_id, issuer, subject, tenant_id)
    REFERENCES pending_run_intents(intent_id, issuer, subject, tenant_id)
    ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
PRAGMA user_version = 34;";
