use rusqlite::{Connection, Row};

use crate::runtime_domain::HubStoreError;

use super::super::error;
use super::budget::MAX_SCAN_UNIQUE_RECORDS;
use super::integrity::IntegrityVerifier;

const IMMUTABLE_TAILS: &str =
    "SELECT r.record_kind,r.aggregate_id,r.record_id,r.sequence,r.canonical_sha256,
            r.appended_at_ms
     FROM governance_records r
     WHERE NOT EXISTS (
       SELECT 1 FROM governance_records newer
       WHERE newer.record_kind=r.record_kind AND newer.aggregate_id=r.aggregate_id
         AND newer.sequence>r.sequence)
     ORDER BY r.record_kind,r.aggregate_id LIMIT ?1";
const STRUCTURAL_HEADS: &str =
    "SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
     FROM governance_structural_heads ORDER BY record_kind,aggregate_id LIMIT ?1";
const SEMANTIC_HEADS: &str =
    "SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
     FROM governance_semantic_heads ORDER BY record_kind,aggregate_id LIMIT ?1";
const EXPECTED_CLAIMS: &str = "SELECT record_kind,aggregate_id,record_id,projection_sha256
     FROM governance_semantic_heads WHERE record_kind='KnowledgeClaim'
     ORDER BY record_kind,aggregate_id LIMIT ?1";
const CLAIM_CHILDREN: &str = "SELECT record_kind,aggregate_id,record_id,projection_sha256
     FROM governance_claim_semantic_views ORDER BY record_kind,aggregate_id LIMIT ?1";
const EXPECTED_JOBS: &str = "SELECT aggregate_id,record_id,claim_type,validation_due_unix_ms,
            validation_owner_id,required_evidence_types_json,
            validation_plan_sha256,projection_sha256
     FROM governance_claim_semantic_views WHERE validation_due_unix_ms IS NOT NULL
     ORDER BY aggregate_id LIMIT ?1";
const MATERIALIZED_JOBS: &str = "SELECT aggregate_id,record_id,claim_type,due_at_unix_ms,owner_id,
            required_evidence_types_json,validation_plan_sha256,projection_sha256
     FROM governance_claim_validation_jobs ORDER BY aggregate_id LIMIT ?1";
const IMMUTABLE_TAIL_RELATION: &str =
    "SELECT r.record_kind,r.aggregate_id,r.record_id,r.sequence,r.canonical_sha256,
            r.appended_at_ms
     FROM governance_records r
     WHERE NOT EXISTS (
       SELECT 1 FROM governance_records newer
       WHERE newer.record_kind=r.record_kind AND newer.aggregate_id=r.aggregate_id
         AND newer.sequence>r.sequence)";
const STRUCTURAL_HEAD_RELATION: &str =
    "SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
     FROM governance_structural_heads";
const SEMANTIC_HEAD_RELATION: &str =
    "SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
     FROM governance_semantic_heads";
const EXPECTED_CLAIM_RELATION: &str = "SELECT record_kind,aggregate_id,record_id,projection_sha256
     FROM governance_semantic_heads WHERE record_kind='KnowledgeClaim'";
const CLAIM_CHILD_RELATION: &str = "SELECT record_kind,aggregate_id,record_id,projection_sha256
     FROM governance_claim_semantic_views";
const EXPECTED_JOB_RELATION: &str =
    "SELECT aggregate_id,record_id,claim_type,validation_due_unix_ms,
            validation_owner_id,required_evidence_types_json,
            validation_plan_sha256,projection_sha256
     FROM governance_claim_semantic_views WHERE validation_due_unix_ms IS NOT NULL";
const MATERIALIZED_JOB_RELATION: &str =
    "SELECT aggregate_id,record_id,claim_type,due_at_unix_ms,owner_id,
            required_evidence_types_json,validation_plan_sha256,projection_sha256
     FROM governance_claim_validation_jobs";

#[derive(Eq, PartialEq)]
struct HeadIdentity {
    record_kind: String,
    aggregate_id: String,
    record_id: String,
    sequence: i64,
    canonical_sha256: Vec<u8>,
    updated_at_ms: i64,
}

#[derive(Eq, PartialEq)]
struct ClaimIdentity {
    record_kind: String,
    aggregate_id: String,
    record_id: String,
    projection_sha256: Vec<u8>,
}

#[derive(Eq, PartialEq)]
struct JobIdentity {
    aggregate_id: String,
    record_id: String,
    claim_type: String,
    due_at_unix_ms: i64,
    owner_id: String,
    required_evidence_types_json: String,
    validation_plan_sha256: Vec<u8>,
    projection_sha256: Vec<u8>,
}

pub(super) fn validate(
    connection: &Connection,
    verifier: &mut IntegrityVerifier<'_>,
) -> Result<(), HubStoreError> {
    let immutable = head_identities(connection, verifier, IMMUTABLE_TAILS)?;
    let structural = head_identities(connection, verifier, STRUCTURAL_HEADS)?;
    compare(verifier, &immutable, &structural)?;
    let semantic = head_identities(connection, verifier, SEMANTIC_HEADS)?;
    compare(verifier, &structural, &semantic)?;
    let expected_claims = claim_identities(connection, verifier, EXPECTED_CLAIMS)?;
    let claim_children = claim_identities(connection, verifier, CLAIM_CHILDREN)?;
    compare(verifier, &expected_claims, &claim_children)?;
    let expected_jobs = job_identities(connection, verifier, EXPECTED_JOBS)?;
    let jobs = job_identities(connection, verifier, MATERIALIZED_JOBS)?;
    compare(verifier, &expected_jobs, &jobs)
}

pub(super) fn validate_unbounded(connection: &Connection) -> Result<(), HubStoreError> {
    let immutable = differs(
        connection,
        IMMUTABLE_TAIL_RELATION,
        STRUCTURAL_HEAD_RELATION,
    )?;
    let semantic = differs(connection, STRUCTURAL_HEAD_RELATION, SEMANTIC_HEAD_RELATION)?;
    let claims = differs(connection, EXPECTED_CLAIM_RELATION, CLAIM_CHILD_RELATION)?;
    let jobs = differs(connection, EXPECTED_JOB_RELATION, MATERIALIZED_JOB_RELATION)?;
    if immutable || semantic || claims || jobs {
        Err(parity_error())
    } else {
        Ok(())
    }
}

fn head_identities(
    connection: &Connection,
    verifier: &mut IntegrityVerifier<'_>,
    sql: &str,
) -> Result<Vec<HeadIdentity>, HubStoreError> {
    query_identities(connection, verifier, sql, |row| {
        Ok(HeadIdentity {
            record_kind: row.get(0)?,
            aggregate_id: row.get(1)?,
            record_id: row.get(2)?,
            sequence: row.get(3)?,
            canonical_sha256: row.get(4)?,
            updated_at_ms: row.get(5)?,
        })
    })
}

fn claim_identities(
    connection: &Connection,
    verifier: &mut IntegrityVerifier<'_>,
    sql: &str,
) -> Result<Vec<ClaimIdentity>, HubStoreError> {
    query_identities(connection, verifier, sql, |row| {
        Ok(ClaimIdentity {
            record_kind: row.get(0)?,
            aggregate_id: row.get(1)?,
            record_id: row.get(2)?,
            projection_sha256: row.get(3)?,
        })
    })
}

fn job_identities(
    connection: &Connection,
    verifier: &mut IntegrityVerifier<'_>,
    sql: &str,
) -> Result<Vec<JobIdentity>, HubStoreError> {
    query_identities(connection, verifier, sql, |row| {
        Ok(JobIdentity {
            aggregate_id: row.get(0)?,
            record_id: row.get(1)?,
            claim_type: row.get(2)?,
            due_at_unix_ms: row.get(3)?,
            owner_id: row.get(4)?,
            required_evidence_types_json: row.get(5)?,
            validation_plan_sha256: row.get(6)?,
            projection_sha256: row.get(7)?,
        })
    })
}

fn query_identities<T>(
    connection: &Connection,
    verifier: &mut IntegrityVerifier<'_>,
    sql: &str,
    decode: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, HubStoreError> {
    let query_limit = i64::try_from(MAX_SCAN_UNIQUE_RECORDS + 1)
        .map_err(|problem| error::corrupt(format!("semantic parity bound: {problem}")))?;
    let mut statement = connection.prepare(sql).map_err(error::read)?;
    let values = statement
        .query_map([query_limit], decode)
        .map_err(error::read)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error::read)?;
    verifier.batches_spend(values.len())?;
    if values.len() > MAX_SCAN_UNIQUE_RECORDS {
        return Err(HubStoreError::Unavailable {
            message: "governance semantic parity exceeds the v1 scan bound".into(),
        });
    }
    Ok(values)
}

fn compare<T: Eq>(
    verifier: &mut IntegrityVerifier<'_>,
    expected: &[T],
    actual: &[T],
) -> Result<(), HubStoreError> {
    verifier.batches_spend(expected.len().max(actual.len()))?;
    if expected == actual {
        Ok(())
    } else {
        Err(parity_error())
    }
}

fn differs(connection: &Connection, expected: &str, actual: &str) -> Result<bool, HubStoreError> {
    let left = format!("SELECT EXISTS(SELECT 1 FROM ({expected} EXCEPT {actual}) LIMIT 1)");
    let right = format!("SELECT EXISTS(SELECT 1 FROM ({actual} EXCEPT {expected}) LIMIT 1)");
    let missing: bool = connection
        .query_row(&left, [], |row| row.get(0))
        .map_err(error::read)?;
    let extra: bool = connection
        .query_row(&right, [], |row| row.get(0))
        .map_err(error::read)?;
    Ok(missing || extra)
}

fn parity_error() -> HubStoreError {
    error::corrupt(
        "governance semantic materialization has a missing, extra, or divergent identity",
    )
}
