// SQLite v28 migration: atomically persisted, content-addressed direct-parent
// lineage for bounded Project Run root-input branches.
pub(super) const MIGRATE_V27_TO_V28_SQL: &str = r"CREATE TABLE run_lineages (
  child_run_id TEXT NOT NULL PRIMARY KEY
    REFERENCES runs(id) ON DELETE RESTRICT
    CHECK(typeof(child_run_id) = 'text'
      AND length(CAST(child_run_id AS BLOB)) BETWEEN 1 AND 128),
  lineage_version INTEGER NOT NULL
    CHECK(typeof(lineage_version) = 'integer' AND lineage_version = 1),
  relation_kind TEXT NOT NULL
    CHECK(typeof(relation_kind) = 'text' AND relation_kind = 'branch'),
  branch_mode TEXT NOT NULL
    CHECK(typeof(branch_mode) = 'text' AND branch_mode = 'root_input'),
  parent_run_id TEXT NOT NULL
    REFERENCES runs(id) ON DELETE RESTRICT
    CHECK(typeof(parent_run_id) = 'text'
      AND length(CAST(parent_run_id AS BLOB)) BETWEEN 1 AND 128),
  source_event_seq INTEGER NOT NULL
    CHECK(typeof(source_event_seq) = 'integer' AND source_event_seq = 1),
  source_event_sha256 BLOB NOT NULL
    CHECK(typeof(source_event_sha256) = 'blob' AND length(source_event_sha256) = 32),
  lineage_sha256 BLOB NOT NULL UNIQUE
    CHECK(typeof(lineage_sha256) = 'blob' AND length(lineage_sha256) = 32),
  created_at_ms INTEGER NOT NULL
    CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  CHECK(child_run_id <> parent_run_id)
);
CREATE INDEX run_lineages_parent
  ON run_lineages(parent_run_id, created_at_ms DESC, child_run_id DESC);
PRAGMA user_version = 28;";
