use crate::runtime_domain::governance_contract::{
    GovernanceRecord, MAX_ARRAY_ITEMS, decode_canonical_record,
};
use crate::runtime_domain::{
    GOVERNANCE_RECORD_JOURNAL_VERSION, GovernanceRecordInspection, GovernanceRecordKind,
    GovernanceRecordMetadata, GovernanceStructuralHead, HubStoreError,
};

use super::{error, rows};

pub(super) struct DecodedRecord {
    pub inspection: GovernanceRecordInspection,
    pub record: GovernanceRecord,
}

pub(super) fn inspection(
    raw: rows::RawRecord,
) -> Result<GovernanceRecordInspection, HubStoreError> {
    let metadata = metadata(&raw)?;
    let content = raw
        .canonical_record_blob
        .map(|bytes| {
            String::from_utf8(bytes).map_err(|error| {
                error::corrupt(format!("stored governance record is not UTF-8: {error}"))
            })
        })
        .transpose()?;
    let inspection = GovernanceRecordInspection {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        metadata,
        canonical_record_json: content,
    };
    inspection
        .validate()
        .map_err(|problem| error::corrupt(problem.message))?;
    Ok(inspection)
}

pub(super) fn decoded(raw: rows::RawRecord) -> Result<DecodedRecord, HubStoreError> {
    let inspection = inspection(raw)?;
    let canonical = inspection
        .canonical_record_json
        .as_deref()
        .ok_or_else(|| error::corrupt("stored governance record content was not loaded"))?;
    let record = decode_canonical_record(canonical.as_bytes())
        .map_err(|problem| error::corrupt(problem.message))?;
    Ok(DecodedRecord { inspection, record })
}

pub(super) fn validated_inspection(
    raw: rows::RawRecord,
    include_record: bool,
) -> Result<GovernanceRecordInspection, HubStoreError> {
    let mut inspection = decoded(raw)?.inspection;
    if !include_record {
        inspection.canonical_record_json = None;
    }
    Ok(inspection)
}

pub(super) fn head(raw: rows::RawHead) -> Result<GovernanceStructuralHead, HubStoreError> {
    let head = GovernanceStructuralHead {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        record_kind: kind(&raw.record_kind)?,
        aggregate_id: raw.aggregate_id,
        record_id: raw.record_id,
        sequence: raw.sequence,
        canonical_sha256: error::stored_digest(&raw.canonical_sha256, "head digest")?,
        updated_at_ms: error::stored_u64(raw.updated_at_ms, "head update time")?,
    };
    head.validate()
        .map_err(|problem| error::corrupt(problem.message))?;
    Ok(head)
}

pub(super) fn kind(value: &str) -> Result<GovernanceRecordKind, HubStoreError> {
    match value {
        "EvidenceRecord" => Ok(GovernanceRecordKind::EvidenceRecord),
        "KnowledgeClaim" => Ok(GovernanceRecordKind::KnowledgeClaim),
        _ => Err(error::corrupt(
            "stored governance record kind is unsupported",
        )),
    }
}

fn metadata(raw: &rows::RawRecord) -> Result<GovernanceRecordMetadata, HubStoreError> {
    let ordinal = error::stored_usize(raw.batch_ordinal, "batch ordinal")?;
    let batch_count = error::stored_usize(raw.batch_record_count, "batch record count")?;
    if raw.batch_version != i64::from(GOVERNANCE_RECORD_JOURNAL_VERSION)
        || raw.appended_at_ms != raw.batch_appended_at_ms
        || !(1..=MAX_ARRAY_ITEMS).contains(&batch_count)
        || ordinal >= batch_count
    {
        return Err(error::corrupt(
            "stored governance record disagrees with its append batch",
        ));
    }
    Ok(GovernanceRecordMetadata {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: raw.batch_id.clone(),
        batch_ordinal: ordinal,
        record_id: raw.record_id.clone(),
        record_kind: kind(&raw.record_kind)?,
        aggregate_id: raw.aggregate_id.clone(),
        sequence: raw.sequence,
        canonical_sha256: error::stored_digest(&raw.canonical_sha256, "record digest")?,
        canonical_record_bytes: error::stored_usize(
            raw.canonical_record_bytes,
            "canonical record byte count",
        )?,
        created_at_unix_ms: raw.created_at_unix_ms,
        appended_at_ms: error::stored_u64(raw.appended_at_ms, "append time")?,
    })
}
