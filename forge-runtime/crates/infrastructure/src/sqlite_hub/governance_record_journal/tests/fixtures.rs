use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Deserialize;
use tempfile::TempDir;

use crate::runtime_domain::AppendGovernanceRecordBatch;
use crate::runtime_domain::governance_contract::GovernanceRecord;

use super::super::super::SqliteHubStore;

const GOLDEN: &str =
    include_str!("../../../../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");

#[derive(Deserialize)]
struct GoldenFixture {
    records: Vec<GoldenEntry>,
}

#[derive(Deserialize)]
struct GoldenEntry {
    record: GovernanceRecord,
}

pub(super) struct StoreFixture {
    _directory: TempDir,
    pub path: PathBuf,
    pub store: SqliteHubStore,
}

impl StoreFixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().expect("journal tempdir");
        let path = directory.path().join("hub.sqlite3");
        let store = SqliteHubStore::open(&path).expect("open journal fixture");
        Self {
            _directory: directory,
            path,
            store,
        }
    }

    pub fn connection(&self) -> Connection {
        let connection = Connection::open(&self.path).expect("open fixture connection");
        connection
            .execute_batch("PRAGMA foreign_keys=ON")
            .expect("enable fixture foreign keys");
        connection
    }
}

pub(super) fn golden_records() -> Vec<GovernanceRecord> {
    serde_json::from_str::<GoldenFixture>(GOLDEN)
        .expect("governance golden fixture")
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

pub(super) fn request(
    records: &[GovernanceRecord],
    key: &str,
    appended_at_ms: u64,
) -> AppendGovernanceRecordBatch {
    AppendGovernanceRecordBatch::from_canonical_record_set(
        exact_set(records),
        key.into(),
        appended_at_ms,
    )
    .expect("valid journal request")
}

pub(super) fn exact_set(records: &[GovernanceRecord]) -> String {
    let mut records: Vec<_> = records.iter().collect();
    records.sort_by(|left, right| {
        left.metadata()
            .record_id
            .as_bytes()
            .cmp(right.metadata().record_id.as_bytes())
    });
    let records: Vec<_> = records
        .into_iter()
        .map(|record| record.canonical_record_json().expect("canonical record"))
        .collect();
    format!("[{}]", records.join(","))
}

pub(super) fn reseal(record: &mut GovernanceRecord) {
    match record {
        GovernanceRecord::Evidence(evidence) => evidence.integrity.canonical_sha256.clear(),
        GovernanceRecord::Claim(claim) => claim.integrity.canonical_sha256.clear(),
    }
    let digest = record.expected_sha256().expect("record digest");
    match record {
        GovernanceRecord::Evidence(evidence) => evidence.integrity.canonical_sha256 = digest,
        GovernanceRecord::Claim(claim) => claim.integrity.canonical_sha256 = digest,
    }
}

pub(super) fn change_record_id(record: &mut GovernanceRecord, record_id: &str) {
    match record {
        GovernanceRecord::Evidence(evidence) => evidence.metadata.record_id = record_id.into(),
        GovernanceRecord::Claim(claim) => claim.metadata.record_id = record_id.into(),
    }
    reseal(record);
}

pub(super) fn evidence_successor(
    prior: &GovernanceRecord,
    record_id: &str,
    sequence: i64,
) -> GovernanceRecord {
    let mut next = prior.clone();
    let GovernanceRecord::Evidence(evidence) = &mut next else {
        panic!("expected evidence fixture");
    };
    evidence.metadata.record_id = record_id.into();
    evidence.metadata.sequence = sequence;
    evidence.metadata.supersedes_record_ids = vec![prior.metadata().record_id.clone()];
    reseal(&mut next);
    next
}

pub(super) fn claim_variant(
    template: &GovernanceRecord,
    record_id: &str,
    aggregate_id: &str,
    derived_from: Vec<String>,
) -> GovernanceRecord {
    let mut record = template.clone();
    let GovernanceRecord::Claim(claim) = &mut record else {
        panic!("expected claim fixture");
    };
    claim.metadata.record_id = record_id.into();
    claim.metadata.aggregate_id = aggregate_id.into();
    claim.spec.derived_from_claim_record_ids = derived_from;
    reseal(&mut record);
    record
}

pub(super) fn claim_successor(
    prior: &GovernanceRecord,
    record_id: &str,
    derived_from: Vec<String>,
) -> GovernanceRecord {
    let mut next = prior.clone();
    let GovernanceRecord::Claim(claim) = &mut next else {
        panic!("expected claim fixture");
    };
    claim.metadata.record_id = record_id.into();
    claim.metadata.sequence += 1;
    claim.metadata.supersedes_record_ids = vec![prior.metadata().record_id.clone()];
    claim.spec.derived_from_claim_record_ids = derived_from;
    reseal(&mut next);
    next
}

pub(super) fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("journal row count")
}

pub(super) fn checkpoint(path: &Path) {
    let connection = Connection::open(path).expect("open checkpoint connection");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .expect("checkpoint journal fixture");
}

pub(super) fn rewrite_record_batch(
    fixture: &StoreFixture,
    record: &GovernanceRecord,
    prior: &AppendGovernanceRecordBatch,
    replacement: &AppendGovernanceRecordBatch,
) {
    let canonical = record.canonical_record_json().expect("rewritten canonical");
    let connection = fixture.connection();
    connection
        .execute_batch("PRAGMA foreign_keys=OFF")
        .expect("disable foreign keys for coherent stored rewrite");
    connection
        .execute(
            "UPDATE governance_records SET canonical_record_blob=?1,canonical_record_bytes=?2,
             canonical_sha256=?3 WHERE record_id=?4",
            params![
                canonical.as_bytes(),
                i64::try_from(canonical.len()).expect("canonical length fits SQLite"),
                digest_bytes(&record.integrity().canonical_sha256),
                record.metadata().record_id
            ],
        )
        .expect("rewrite stored record");
    rewrite_batch_identity(&connection, prior, replacement);
    connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA foreign_key_check;")
        .expect("rewritten batch remains referentially complete");
}

fn rewrite_batch_identity(
    connection: &Connection,
    prior: &AppendGovernanceRecordBatch,
    replacement: &AppendGovernanceRecordBatch,
) {
    connection
        .execute(
            "UPDATE governance_records SET batch_id=?1 WHERE batch_id=?2",
            params![replacement.batch_id, prior.batch_id],
        )
        .expect("move records to rewritten batch");
    connection
        .execute(
            "UPDATE governance_record_append_batches SET batch_id=?1,request_sha256=?2,
             record_set_sha256=?3,record_set_bytes=?4 WHERE batch_id=?5",
            params![
                replacement.batch_id,
                digest_bytes(&replacement.request_sha256),
                digest_bytes(&replacement.record_set_sha256),
                i64::try_from(replacement.canonical_record_set_json.len())
                    .expect("record set length fits SQLite"),
                prior.batch_id
            ],
        )
        .expect("rewrite exact batch metadata");
}

fn digest_bytes(value: &str) -> Vec<u8> {
    super::super::super::group_run_codec::decode_hex_digest(value)
        .expect("fixture digest bytes")
        .to_vec()
}
