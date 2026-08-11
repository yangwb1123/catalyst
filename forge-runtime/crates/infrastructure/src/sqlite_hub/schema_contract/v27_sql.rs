// SQLite v27 migration: add a rebuildable semantic projection over the exact
// governance journal. These rows expose declared lifecycle and explicit-time
// freshness/conflict candidates only; they confer no truth or authority.
pub(super) const MIGRATE_V26_TO_V27_SQL: &str = r"CREATE TABLE governance_semantic_heads (
  record_kind TEXT NOT NULL
    CHECK(typeof(record_kind) = 'text'
      AND record_kind IN ('EvidenceRecord','KnowledgeClaim')),
  aggregate_id TEXT NOT NULL
    CHECK(typeof(aggregate_id) = 'text'
      AND length(CAST(aggregate_id AS BLOB)) BETWEEN 1 AND 160),
  semantic_view_version INTEGER NOT NULL
    CHECK(typeof(semantic_view_version) = 'integer' AND semantic_view_version = 1),
  record_id TEXT NOT NULL UNIQUE
    REFERENCES governance_records(record_id) ON DELETE RESTRICT
    CHECK(typeof(record_id) = 'text'
      AND length(CAST(record_id AS BLOB)) BETWEEN 1 AND 160),
  sequence INTEGER NOT NULL
    CHECK(typeof(sequence) = 'integer' AND sequence >= 1),
  canonical_sha256 BLOB NOT NULL
    CHECK(typeof(canonical_sha256) = 'blob' AND length(canonical_sha256) = 32),
  project_id TEXT NOT NULL
    CHECK(typeof(project_id) = 'text'
      AND length(CAST(project_id AS BLOB)) BETWEEN 1 AND 160),
  scope TEXT NOT NULL
    CHECK(typeof(scope) = 'text'
      AND length(CAST(scope AS BLOB)) BETWEEN 1 AND 160),
  declared_state TEXT NOT NULL
    CHECK(typeof(declared_state) = 'text'
      AND declared_state IN (
        'accepted','accepted_risk','active','adopted','candidate','confirmed',
        'contested','deprecated','draft','expired','investigating','invalid',
        'invalidated','observed','open','promoted','proposed','rejected','repeated',
        'resolved','retracted','retired','stale','submitted','superseded','supported',
        'testing','unavailable','valid','validated','waived')),
  valid_from_unix_ms INTEGER NOT NULL
    CHECK(typeof(valid_from_unix_ms) = 'integer' AND valid_from_unix_ms >= 0),
  valid_until_unix_ms INTEGER
    CHECK(valid_until_unix_ms IS NULL
      OR typeof(valid_until_unix_ms) = 'integer'
         AND valid_until_unix_ms > valid_from_unix_ms),
  projection_sha256 BLOB NOT NULL
    CHECK(typeof(projection_sha256) = 'blob' AND length(projection_sha256) = 32),
  updated_at_ms INTEGER NOT NULL
    CHECK(typeof(updated_at_ms) = 'integer' AND updated_at_ms >= 0),
  PRIMARY KEY(record_kind, aggregate_id),
  FOREIGN KEY(record_kind, aggregate_id, sequence)
    REFERENCES governance_records(record_kind, aggregate_id, sequence)
    ON DELETE RESTRICT
);
CREATE INDEX governance_semantic_heads_state_validity
  ON governance_semantic_heads(record_kind, declared_state, valid_until_unix_ms, aggregate_id);
CREATE TABLE governance_claim_semantic_views (
  record_kind TEXT NOT NULL
    CHECK(typeof(record_kind) = 'text' AND record_kind = 'KnowledgeClaim'),
  aggregate_id TEXT NOT NULL
    CHECK(typeof(aggregate_id) = 'text'
      AND length(CAST(aggregate_id AS BLOB)) BETWEEN 1 AND 160),
  record_id TEXT NOT NULL UNIQUE
    REFERENCES governance_semantic_heads(record_id) ON DELETE RESTRICT
    CHECK(typeof(record_id) = 'text'
      AND length(CAST(record_id AS BLOB)) BETWEEN 1 AND 160),
  claim_type TEXT NOT NULL
    CHECK(typeof(claim_type) = 'text'
      AND claim_type IN ('assumption','constraint','decision','fact','hypothesis',
        'inference','lesson','proposal','unknown')),
  subject TEXT NOT NULL
    CHECK(typeof(subject) = 'text'
      AND length(CAST(subject AS BLOB)) BETWEEN 1 AND 160),
  predicate TEXT NOT NULL
    CHECK(typeof(predicate) = 'text'
      AND length(CAST(predicate AS BLOB)) BETWEEN 1 AND 160),
  object_sha256 BLOB NOT NULL
    CHECK(typeof(object_sha256) = 'blob' AND length(object_sha256) = 32),
  conflict_key_sha256 BLOB NOT NULL
    CHECK(typeof(conflict_key_sha256) = 'blob' AND length(conflict_key_sha256) = 32),
  review_by_unix_ms INTEGER
    CHECK(review_by_unix_ms IS NULL
      OR typeof(review_by_unix_ms) = 'integer' AND review_by_unix_ms >= 0),
  validation_due_unix_ms INTEGER
    CHECK(validation_due_unix_ms IS NULL
      OR typeof(validation_due_unix_ms) = 'integer' AND validation_due_unix_ms >= 0),
  validation_owner_id TEXT
    CHECK(validation_owner_id IS NULL
      OR typeof(validation_owner_id) = 'text'
         AND length(CAST(validation_owner_id AS BLOB)) BETWEEN 1 AND 160),
  validation_plan_sha256 BLOB
    CHECK(validation_plan_sha256 IS NULL
      OR typeof(validation_plan_sha256) = 'blob' AND length(validation_plan_sha256) = 32),
  required_evidence_types_json TEXT NOT NULL
    CHECK(typeof(required_evidence_types_json) = 'text'
      AND length(CAST(required_evidence_types_json AS BLOB)) BETWEEN 2 AND 1024),
  projection_sha256 BLOB NOT NULL
    CHECK(typeof(projection_sha256) = 'blob' AND length(projection_sha256) = 32),
  PRIMARY KEY(record_kind, aggregate_id),
  FOREIGN KEY(record_kind, aggregate_id)
    REFERENCES governance_semantic_heads(record_kind, aggregate_id)
    ON DELETE RESTRICT,
  CHECK((validation_due_unix_ms IS NULL
      AND validation_owner_id IS NULL
      AND validation_plan_sha256 IS NULL
      AND required_evidence_types_json = '[]')
    OR (validation_due_unix_ms IS NOT NULL
      AND validation_owner_id IS NOT NULL
      AND validation_plan_sha256 IS NOT NULL
      AND required_evidence_types_json <> '[]'))
);
CREATE INDEX governance_claim_semantic_conflicts
  ON governance_claim_semantic_views(conflict_key_sha256, object_sha256, aggregate_id);
CREATE TABLE governance_claim_validation_jobs (
  job_id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(job_id) = 'text'
      AND length(CAST(job_id AS BLOB)) BETWEEN 1 AND 160),
  aggregate_id TEXT NOT NULL UNIQUE
    CHECK(typeof(aggregate_id) = 'text'
      AND length(CAST(aggregate_id AS BLOB)) BETWEEN 1 AND 160),
  record_id TEXT NOT NULL UNIQUE
    REFERENCES governance_claim_semantic_views(record_id) ON DELETE RESTRICT
    CHECK(typeof(record_id) = 'text'
      AND length(CAST(record_id AS BLOB)) BETWEEN 1 AND 160),
  claim_type TEXT NOT NULL
    CHECK(typeof(claim_type) = 'text'
      AND claim_type IN ('assumption','hypothesis')),
  due_at_unix_ms INTEGER NOT NULL
    CHECK(typeof(due_at_unix_ms) = 'integer' AND due_at_unix_ms >= 0),
  owner_id TEXT NOT NULL
    CHECK(typeof(owner_id) = 'text'
      AND length(CAST(owner_id AS BLOB)) BETWEEN 1 AND 160),
  required_evidence_types_json TEXT NOT NULL
    CHECK(typeof(required_evidence_types_json) = 'text'
      AND length(CAST(required_evidence_types_json AS BLOB)) BETWEEN 3 AND 1024),
  validation_plan_sha256 BLOB NOT NULL
    CHECK(typeof(validation_plan_sha256) = 'blob' AND length(validation_plan_sha256) = 32),
  projection_sha256 BLOB NOT NULL
    CHECK(typeof(projection_sha256) = 'blob' AND length(projection_sha256) = 32)
);
CREATE INDEX governance_claim_validation_jobs_due
  ON governance_claim_validation_jobs(due_at_unix_ms, job_id);
PRAGMA user_version = 27;";
