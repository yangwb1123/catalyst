// SQLite v25 migration: add the local append-only governance-record journal
// and its rebuildable structural-head projection. The tables preserve exact
// canonical EvidenceRecord / KnowledgeClaim bytes; they do not attest truth,
// authority, freshness, validity, or lifecycle acceptance.
pub(super) const MIGRATE_V24_TO_V25_SQL: &str = r"CREATE TABLE governance_record_append_batches (
  batch_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(batch_id) = 'text'
      AND length(CAST(batch_id AS BLOB)) BETWEEN 1 AND 160),
  journal_version INTEGER NOT NULL
    CHECK(typeof(journal_version) = 'integer' AND journal_version = 1),
  idempotency_key TEXT NOT NULL UNIQUE
    CHECK(typeof(idempotency_key) = 'text'
      AND length(CAST(idempotency_key AS BLOB)) BETWEEN 1 AND 256),
  request_sha256 BLOB NOT NULL
    CHECK(typeof(request_sha256) = 'blob' AND length(request_sha256) = 32),
  record_set_sha256 BLOB NOT NULL
    CHECK(typeof(record_set_sha256) = 'blob' AND length(record_set_sha256) = 32),
  record_count INTEGER NOT NULL
    CHECK(typeof(record_count) = 'integer' AND record_count BETWEEN 1 AND 256),
  record_set_bytes INTEGER NOT NULL
    CHECK(typeof(record_set_bytes) = 'integer'
      AND record_set_bytes BETWEEN 1 AND 1048576),
  appended_at_ms INTEGER NOT NULL
    CHECK(typeof(appended_at_ms) = 'integer' AND appended_at_ms >= 0)
);
CREATE TABLE governance_records (
  record_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(record_id) = 'text'
      AND length(CAST(record_id AS BLOB)) BETWEEN 1 AND 160),
  batch_id TEXT NOT NULL
    REFERENCES governance_record_append_batches(batch_id) ON DELETE RESTRICT
    CHECK(typeof(batch_id) = 'text'
      AND length(CAST(batch_id AS BLOB)) BETWEEN 1 AND 160),
  batch_ordinal INTEGER NOT NULL
    CHECK(typeof(batch_ordinal) = 'integer'
      AND batch_ordinal BETWEEN 0 AND 255),
  record_kind TEXT NOT NULL
    CHECK(typeof(record_kind) = 'text'
      AND record_kind IN ('EvidenceRecord','KnowledgeClaim')),
  aggregate_id TEXT NOT NULL
    CHECK(typeof(aggregate_id) = 'text'
      AND length(CAST(aggregate_id AS BLOB)) BETWEEN 1 AND 160),
  sequence INTEGER NOT NULL
    CHECK(typeof(sequence) = 'integer' AND sequence >= 1),
  canonical_sha256 BLOB NOT NULL
    CHECK(typeof(canonical_sha256) = 'blob' AND length(canonical_sha256) = 32),
  canonical_record_blob BLOB NOT NULL
    CHECK(typeof(canonical_record_blob) = 'blob'
      AND length(canonical_record_blob) BETWEEN 1 AND 131072),
  canonical_record_bytes INTEGER NOT NULL
    CHECK(typeof(canonical_record_bytes) = 'integer'
      AND canonical_record_bytes BETWEEN 1 AND 131072
      AND canonical_record_bytes = length(canonical_record_blob)),
  created_at_unix_ms INTEGER NOT NULL
    CHECK(typeof(created_at_unix_ms) = 'integer' AND created_at_unix_ms >= 0),
  appended_at_ms INTEGER NOT NULL
    CHECK(typeof(appended_at_ms) = 'integer' AND appended_at_ms >= 0),
  UNIQUE(batch_id, batch_ordinal),
  UNIQUE(record_kind, aggregate_id, sequence)
);
CREATE INDEX governance_records_appended
  ON governance_records(appended_at_ms DESC, record_id DESC);
CREATE INDEX governance_records_aggregate_appended
  ON governance_records(aggregate_id, appended_at_ms DESC, record_id DESC);
CREATE INDEX governance_records_kind_appended
  ON governance_records(record_kind, appended_at_ms DESC, record_id DESC);
CREATE TABLE governance_structural_heads (
  record_kind TEXT NOT NULL
    CHECK(typeof(record_kind) = 'text'
      AND record_kind IN ('EvidenceRecord','KnowledgeClaim')),
  aggregate_id TEXT NOT NULL
    CHECK(typeof(aggregate_id) = 'text'
      AND length(CAST(aggregate_id AS BLOB)) BETWEEN 1 AND 160),
  record_id TEXT NOT NULL UNIQUE
    REFERENCES governance_records(record_id) ON DELETE RESTRICT
    CHECK(typeof(record_id) = 'text'
      AND length(CAST(record_id AS BLOB)) BETWEEN 1 AND 160),
  sequence INTEGER NOT NULL
    CHECK(typeof(sequence) = 'integer' AND sequence >= 1),
  canonical_sha256 BLOB NOT NULL
    CHECK(typeof(canonical_sha256) = 'blob' AND length(canonical_sha256) = 32),
  updated_at_ms INTEGER NOT NULL
    CHECK(typeof(updated_at_ms) = 'integer' AND updated_at_ms >= 0),
  PRIMARY KEY(record_kind, aggregate_id)
);
PRAGMA user_version = 25;";
