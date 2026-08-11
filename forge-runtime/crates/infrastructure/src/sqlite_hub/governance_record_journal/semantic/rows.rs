use rusqlite::{Connection, OptionalExtension, params};

use crate::runtime_domain::{GovernanceRecordKind, HubStoreError};

use super::super::error;

const PROJECTION_COLUMNS: &str =
    "h.record_kind,h.aggregate_id,h.semantic_view_version,h.record_id,h.sequence,
     h.canonical_sha256,h.project_id,h.scope,h.declared_state,h.valid_from_unix_ms,
     h.valid_until_unix_ms,h.projection_sha256,h.updated_at_ms,c.claim_type,c.subject,
     c.predicate,c.object_sha256,c.conflict_key_sha256,c.review_by_unix_ms,
     c.validation_due_unix_ms,c.validation_owner_id,c.validation_plan_sha256,
     c.required_evidence_types_json,c.projection_sha256";
const PROJECTION_JOIN: &str =
    " FROM governance_semantic_heads h LEFT JOIN governance_claim_semantic_views c
       ON c.record_kind=h.record_kind AND c.aggregate_id=h.aggregate_id";

#[derive(Clone, Debug)]
pub(super) struct RawProjection {
    pub record_kind: String,
    pub aggregate_id: String,
    pub semantic_view_version: i64,
    pub record_id: String,
    pub sequence: i64,
    pub canonical_sha256: Vec<u8>,
    pub project_id: String,
    pub scope: String,
    pub declared_state: String,
    pub valid_from_unix_ms: i64,
    pub valid_until_unix_ms: Option<i64>,
    pub projection_sha256: Vec<u8>,
    pub updated_at_ms: i64,
    pub claim_type: Option<String>,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_sha256: Option<Vec<u8>>,
    pub conflict_key_sha256: Option<Vec<u8>>,
    pub review_by_unix_ms: Option<i64>,
    pub validation_due_unix_ms: Option<i64>,
    pub validation_owner_id: Option<String>,
    pub validation_plan_sha256: Option<Vec<u8>>,
    pub required_evidence_types_json: Option<String>,
    pub claim_projection_sha256: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub(super) struct RawValidationJob {
    pub job_id: String,
    pub aggregate_id: String,
    pub record_id: String,
    pub claim_type: String,
    pub due_at_unix_ms: i64,
    pub owner_id: String,
    pub required_evidence_types_json: String,
    pub validation_plan_sha256: Vec<u8>,
    pub projection_sha256: Vec<u8>,
}

pub(super) fn find_projection(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<Option<RawProjection>, HubStoreError> {
    connection
        .query_row(
            &format!(
                "SELECT {PROJECTION_COLUMNS}{PROJECTION_JOIN}
                 WHERE h.record_kind=?1 AND h.aggregate_id=?2"
            ),
            params![kind.as_str(), aggregate_id],
            raw_projection,
        )
        .optional()
        .map_err(error::read)
}

pub(super) fn immutable_aggregate_ids(
    connection: &Connection,
    kind: GovernanceRecordKind,
    limit: i64,
) -> Result<Vec<String>, HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT aggregate_id FROM governance_records
             WHERE record_kind=?1 ORDER BY aggregate_id LIMIT ?2",
        )
        .map_err(error::read)?;
    statement
        .query_map(params![kind.as_str(), limit], |row| row.get(0))
        .map_err(error::read)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error::read)
}

pub(super) fn find_validation_job(
    connection: &Connection,
    aggregate_id: &str,
) -> Result<Option<RawValidationJob>, HubStoreError> {
    connection
        .query_row(
            "SELECT job_id,aggregate_id,record_id,claim_type,due_at_unix_ms,owner_id,
                    required_evidence_types_json,validation_plan_sha256,projection_sha256
             FROM governance_claim_validation_jobs WHERE aggregate_id=?1",
            [aggregate_id],
            |row| {
                Ok(RawValidationJob {
                    job_id: row.get(0)?,
                    aggregate_id: row.get(1)?,
                    record_id: row.get(2)?,
                    claim_type: row.get(3)?,
                    due_at_unix_ms: row.get(4)?,
                    owner_id: row.get(5)?,
                    required_evidence_types_json: row.get(6)?,
                    validation_plan_sha256: row.get(7)?,
                    projection_sha256: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(error::read)
}

pub(super) fn semantic_head_count(connection: &Connection) -> Result<i64, HubStoreError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM governance_semantic_heads",
            [],
            |row| row.get(0),
        )
        .map_err(error::read)
}

pub(super) fn structural_head_count(connection: &Connection) -> Result<i64, HubStoreError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM governance_structural_heads",
            [],
            |row| row.get(0),
        )
        .map_err(error::read)
}

pub(super) fn clear(connection: &Connection) -> Result<(), HubStoreError> {
    connection
        .execute_batch(
            "DELETE FROM governance_claim_validation_jobs;
             DELETE FROM governance_claim_semantic_views;
             DELETE FROM governance_semantic_heads;",
        )
        .map_err(error::write)
}

pub(super) fn replace_projection(
    connection: &Connection,
    projection: &crate::runtime_domain::GovernanceSemanticProjection,
) -> Result<(), HubStoreError> {
    if projection.head.record_kind == GovernanceRecordKind::KnowledgeClaim {
        delete_claim_children(connection, &projection.head.aggregate_id)?;
    }
    upsert_head(connection, projection)?;
    if let Some(claim) = &projection.claim {
        insert_claim(connection, projection, claim)?;
        insert_validation_job(connection, projection)?;
    }
    Ok(())
}

fn delete_claim_children(connection: &Connection, aggregate_id: &str) -> Result<(), HubStoreError> {
    connection
        .execute(
            "DELETE FROM governance_claim_validation_jobs WHERE aggregate_id=?1",
            [aggregate_id],
        )
        .map_err(error::write)?;
    connection
        .execute(
            "DELETE FROM governance_claim_semantic_views WHERE aggregate_id=?1",
            [aggregate_id],
        )
        .map_err(error::write)?;
    Ok(())
}

fn upsert_head(
    connection: &Connection,
    projection: &crate::runtime_domain::GovernanceSemanticProjection,
) -> Result<(), HubStoreError> {
    let head = &projection.head;
    let canonical = error::digest_blob(&head.canonical_sha256, "semantic canonical digest")?;
    let digest = error::digest_blob(&projection.projection_sha256, "semantic projection digest")?;
    connection
        .execute(
            "INSERT INTO governance_semantic_heads(
               record_kind,aggregate_id,semantic_view_version,record_id,sequence,
               canonical_sha256,project_id,scope,declared_state,valid_from_unix_ms,
               valid_until_unix_ms,projection_sha256,updated_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(record_kind,aggregate_id) DO UPDATE SET
               semantic_view_version=excluded.semantic_view_version,
               record_id=excluded.record_id,sequence=excluded.sequence,
               canonical_sha256=excluded.canonical_sha256,project_id=excluded.project_id,
               scope=excluded.scope,declared_state=excluded.declared_state,
               valid_from_unix_ms=excluded.valid_from_unix_ms,
               valid_until_unix_ms=excluded.valid_until_unix_ms,
               projection_sha256=excluded.projection_sha256,updated_at_ms=excluded.updated_at_ms",
            params![
                head.record_kind.as_str(),
                head.aggregate_id,
                i64::from(projection.v),
                head.record_id,
                head.sequence,
                canonical.as_slice(),
                head.project_id,
                head.scope,
                head.declared_state,
                head.valid_from_unix_ms,
                head.valid_until_unix_ms,
                digest.as_slice(),
                error::input_i64(head.updated_at_ms, "semantic update time")?,
            ],
        )
        .map_err(error::write)?;
    Ok(())
}

fn insert_claim(
    connection: &Connection,
    projection: &crate::runtime_domain::GovernanceSemanticProjection,
    claim: &crate::runtime_domain::GovernanceClaimSemanticFields,
) -> Result<(), HubStoreError> {
    let object = error::digest_blob(&claim.object_sha256, "claim object digest")?;
    let conflict = error::digest_blob(&claim.conflict_key_sha256, "claim conflict digest")?;
    let plan = claim
        .validation_plan_sha256
        .as_deref()
        .map(|value| error::digest_blob(value, "validation plan digest"))
        .transpose()?;
    let projection_digest =
        error::digest_blob(&projection.projection_sha256, "claim projection digest")?;
    let evidence = serde_json::to_string(&claim.required_evidence_types)
        .map_err(|problem| error::conflict(format!("cannot encode evidence types: {problem}")))?;
    connection
        .execute(
            "INSERT INTO governance_claim_semantic_views(
               record_kind,aggregate_id,record_id,claim_type,subject,predicate,
               object_sha256,conflict_key_sha256,review_by_unix_ms,validation_due_unix_ms,
               validation_owner_id,validation_plan_sha256,required_evidence_types_json,
               projection_sha256
             ) VALUES('KnowledgeClaim',?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                projection.head.aggregate_id,
                projection.head.record_id,
                claim_type_name(claim.claim_type),
                claim.subject,
                claim.predicate,
                object.as_slice(),
                conflict.as_slice(),
                claim.review_by_unix_ms,
                claim.validation_due_unix_ms,
                claim.validation_owner_id,
                plan.as_ref().map(<[u8; 32]>::as_slice),
                evidence,
                projection_digest.as_slice(),
            ],
        )
        .map_err(error::write)?;
    Ok(())
}

fn insert_validation_job(
    connection: &Connection,
    projection: &crate::runtime_domain::GovernanceSemanticProjection,
) -> Result<(), HubStoreError> {
    let Some(job) = crate::runtime_domain::governance_validation_job(projection, 0)
        .map_err(|problem| error::conflict(problem.message))?
    else {
        return Ok(());
    };
    let plan = error::digest_blob(&job.validation_plan_sha256, "validation plan digest")?;
    let projection_digest = error::digest_blob(
        &projection.projection_sha256,
        "validation projection digest",
    )?;
    let evidence = serde_json::to_string(&job.required_evidence_types)
        .map_err(|problem| error::conflict(format!("cannot encode evidence types: {problem}")))?;
    connection
        .execute(
            "INSERT INTO governance_claim_validation_jobs(
               job_id,aggregate_id,record_id,claim_type,due_at_unix_ms,owner_id,
               required_evidence_types_json,validation_plan_sha256,projection_sha256
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                job.job_id,
                job.aggregate_id,
                job.record_id,
                claim_type_name(job.claim_type),
                job.due_at_unix_ms,
                job.owner_id,
                evidence,
                plan.as_slice(),
                projection_digest.as_slice(),
            ],
        )
        .map_err(error::write)?;
    Ok(())
}

fn raw_projection(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawProjection> {
    Ok(RawProjection {
        record_kind: row.get(0)?,
        aggregate_id: row.get(1)?,
        semantic_view_version: row.get(2)?,
        record_id: row.get(3)?,
        sequence: row.get(4)?,
        canonical_sha256: row.get(5)?,
        project_id: row.get(6)?,
        scope: row.get(7)?,
        declared_state: row.get(8)?,
        valid_from_unix_ms: row.get(9)?,
        valid_until_unix_ms: row.get(10)?,
        projection_sha256: row.get(11)?,
        updated_at_ms: row.get(12)?,
        claim_type: row.get(13)?,
        subject: row.get(14)?,
        predicate: row.get(15)?,
        object_sha256: row.get(16)?,
        conflict_key_sha256: row.get(17)?,
        review_by_unix_ms: row.get(18)?,
        validation_due_unix_ms: row.get(19)?,
        validation_owner_id: row.get(20)?,
        validation_plan_sha256: row.get(21)?,
        required_evidence_types_json: row.get(22)?,
        claim_projection_sha256: row.get(23)?,
    })
}

pub(super) const fn claim_type_name(
    value: crate::runtime_domain::governance_contract::ClaimType,
) -> &'static str {
    use crate::runtime_domain::governance_contract::ClaimType;
    match value {
        ClaimType::Assumption => "assumption",
        ClaimType::Constraint => "constraint",
        ClaimType::Decision => "decision",
        ClaimType::Fact => "fact",
        ClaimType::Hypothesis => "hypothesis",
        ClaimType::Inference => "inference",
        ClaimType::Lesson => "lesson",
        ClaimType::Proposal => "proposal",
        ClaimType::Unknown => "unknown",
    }
}
