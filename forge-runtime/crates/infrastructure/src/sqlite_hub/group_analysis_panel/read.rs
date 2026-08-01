use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::runtime_domain::{
    GROUP_ANALYSIS_PANEL_VERSION, GroupAnalysisPanelInspection, GroupAnalysisPanelManifest,
    GroupAnalysisPanelRecord, GroupAnalysisPanelStatus, GroupModelAnalysisOutcome,
    GroupModelAnalysisRecovery, HubEntity, HubStoreError, MAX_GROUP_ANALYSIS_PANEL_ANALYSES,
    MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT,
};

use super::{
    super::{group_model_analysis, group_run_codec, group_run_read, read_error},
    codec, rows,
};

pub(super) fn inspect(
    connection: &mut Connection,
    panel_id: &str,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, panel_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(in crate::sqlite_hub) fn inspect_in_snapshot(
    connection: &Connection,
    panel_id: &str,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    let stored = rows::find_by_id(connection, panel_id)?
        .ok_or_else(|| not_found(HubEntity::GroupAnalysisPanel, panel_id))?;
    validate_stored(connection, stored)
}

pub(super) fn list(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAnalysisPanelRecord>, HubStoreError> {
    validate_list_request(connection, group_run_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, group_run_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredPanel,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    if stored.manifest_blob.len() != record.manifest_bytes {
        return Err(corrupt("stored panel manifest byte count disagrees"));
    }
    let digest = codec::encode_blob(&record.manifest_sha256, "manifest")
        .map_err(|error| corrupt(&error.to_string()))?;
    let manifest = codec::decode(&stored.manifest_blob, &digest)?;
    validate_manifest_inputs(connection, &manifest)?;
    validate_analysis_rows(connection, &record.panel_id, &manifest)?;
    let inspection = GroupAnalysisPanelInspection {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        panel: record,
        manifest,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_manifest_inputs(
    connection: &Connection,
    manifest: &GroupAnalysisPanelManifest,
) -> Result<(), HubStoreError> {
    validate_manifest_inputs_as(connection, manifest, InputClassification::Stored)
}

pub(super) fn validate_manifest_candidate(
    connection: &Connection,
    manifest: &GroupAnalysisPanelManifest,
) -> Result<(), HubStoreError> {
    validate_manifest_inputs_as(connection, manifest, InputClassification::Candidate)
}

fn validate_manifest_inputs_as(
    connection: &Connection,
    manifest: &GroupAnalysisPanelManifest,
    classification: InputClassification,
) -> Result<(), HubStoreError> {
    let stored_run = group_run_read::find_by_id(connection, &manifest.source.group_run_id)?
        .ok_or_else(|| classification.mismatch("panel references a missing frozen Group Run"))?;
    let snapshot = group_run_read::decode_stored(stored_run)?;
    let source = group_model_analysis::read::source_from_snapshot(&snapshot);
    if source != manifest.source {
        return Err(classification.mismatch("panel source does not match its frozen Group Run"));
    }
    for contribution in &manifest.contributions {
        validate_contribution(connection, contribution, &manifest.source, classification)?;
    }
    Ok(())
}

fn validate_contribution(
    connection: &Connection,
    contribution: &crate::runtime_domain::GroupAnalysisPanelContribution,
    source: &crate::runtime_domain::GroupModelAnalysisSource,
    classification: InputClassification,
) -> Result<(), HubStoreError> {
    let analysis = group_model_analysis::read::inspect_in_snapshot(
        connection,
        &contribution.analysis.analysis_id,
    )
    .map_err(|error| classification.reference_error(error))?;
    let exact = analysis.analysis == contribution.analysis
        && analysis.result.as_ref() == Some(&contribution.result)
        && analysis.prepared.as_ref().map(|receipt| &receipt.source) == Some(source)
        && analysis.recovery
            == GroupModelAnalysisRecovery::Terminal {
                outcome: GroupModelAnalysisOutcome::Completed,
            };
    exact.then_some(()).ok_or_else(|| {
        classification.mismatch("panel contribution disagrees with its completed analysis")
    })
}

#[derive(Clone, Copy)]
enum InputClassification {
    Candidate,
    Stored,
}

impl InputClassification {
    fn mismatch(self, message: &str) -> HubStoreError {
        match self {
            Self::Candidate => conflict(message),
            Self::Stored => corrupt(message),
        }
    }

    fn reference_error(self, error: HubStoreError) -> HubStoreError {
        match error {
            HubStoreError::NotFound { .. } => {
                self.mismatch("panel references a missing completed analysis")
            }
            other => other,
        }
    }
}

fn validate_analysis_rows(
    connection: &Connection,
    panel_id: &str,
    manifest: &GroupAnalysisPanelManifest,
) -> Result<(), HubStoreError> {
    let rows = rows::load_analyses(connection, panel_id)?;
    if rows.len() != manifest.contributions.len() || rows.len() > MAX_GROUP_ANALYSIS_PANEL_ANALYSES
    {
        return Err(corrupt("stored panel analysis row count disagrees"));
    }
    for (position, (row, contribution)) in rows.iter().zip(&manifest.contributions).enumerate() {
        let expected_position =
            i64::try_from(position).map_err(|error| corrupt(&error.to_string()))?;
        let result_digest = codec::decode_hex(&row.result_sha256, "result")?;
        if row.position != expected_position
            || row.analysis_id != contribution.analysis.analysis_id
            || result_digest != contribution.result.result_sha256
        {
            return Err(corrupt(
                "stored panel analysis row disagrees with its manifest",
            ));
        }
    }
    Ok(())
}

fn metadata_record(raw: rows::RawPanelMetadata) -> Result<GroupAnalysisPanelRecord, HubStoreError> {
    let record = GroupAnalysisPanelRecord {
        v: convert(raw.panel_version, "panel version")?,
        panel_id: raw.id,
        group_run_id: raw.group_run_id,
        status: parse_status(&raw.status)?,
        source_snapshot_sha256: codec::decode_hex(&raw.source_snapshot_sha256, "source")?,
        manifest_sha256: codec::decode_hex(&raw.manifest_sha256, "manifest")?,
        manifest_bytes: convert(raw.manifest_bytes, "manifest byte count")?,
        analysis_count: convert(raw.analysis_count, "analysis count")?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_ANALYSIS_PANEL_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt("stored panel idempotency key is invalid"))
    }
}

fn validate_list_request(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT).contains(&limit) {
        return Err(conflict("panel list limit is outside its bounds"));
    }
    let Some(id) = group_run_id else {
        return Ok(());
    };
    connection
        .query_row("SELECT 1 FROM group_runs WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::GroupRun, id))
}

fn parse_status(value: &str) -> Result<GroupAnalysisPanelStatus, HubStoreError> {
    match value {
        "prepared" => Ok(GroupAnalysisPanelStatus::Prepared),
        _ => Err(corrupt("stored Group Analysis Panel status is unsupported")),
    }
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| corrupt(&format!("invalid panel {subject}: {error}")))
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAnalysisPanel,
        message: message.into(),
    }
}
