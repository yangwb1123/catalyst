use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    GROUP_ANALYSIS_PANEL_VERSION, GroupAnalysisPanelInspection, GroupAnalysisPanelRecord,
    GroupAnalysisPanelStatus, HubEntity, HubStoreError, PrepareGroupAnalysisPanel,
    PrepareGroupAnalysisPanelDisposition, PrepareGroupAnalysisPanelResult,
};

use super::{
    super::{read_error, write_error},
    codec, read, rows,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupAnalysisPanel,
) -> Result<PrepareGroupAnalysisPanelResult, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = prepare_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn prepare_locked(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAnalysisPanel,
) -> Result<PrepareGroupAnalysisPanelResult, HubStoreError> {
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let inspection = read::validate_stored(transaction, stored)?;
        ensure_replay(&inspection, request)?;
        return Ok(result(
            PrepareGroupAnalysisPanelDisposition::Replayed,
            inspection,
        ));
    }
    if let Some(stored) = rows::find_by_id(transaction, &request.panel_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "panel ID already belongs to another idempotency key",
        ));
    }
    create(transaction, request)
}

fn create(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAnalysisPanel,
) -> Result<PrepareGroupAnalysisPanelResult, HubStoreError> {
    read::validate_manifest_candidate(transaction, &request.manifest)?;
    let encoded = codec::encode(&request.manifest)?;
    let record = record(request, &encoded);
    insert_panel(transaction, request, &record, &encoded)?;
    insert_analyses(transaction, &record.panel_id, &request.manifest)?;
    let inspection = reread_created(transaction, request, &record)?;
    Ok(result(
        PrepareGroupAnalysisPanelDisposition::Created,
        inspection,
    ))
}

fn reread_created(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAnalysisPanel,
    expected: &GroupAnalysisPanelRecord,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    let inspection = read::inspect_in_snapshot(transaction, &expected.panel_id)?;
    if inspection.panel == *expected && inspection.manifest == request.manifest {
        Ok(inspection)
    } else {
        Err(corrupt("created panel disagrees with its committed input"))
    }
}

fn record(
    request: &PrepareGroupAnalysisPanel,
    encoded: &codec::EncodedManifest,
) -> GroupAnalysisPanelRecord {
    GroupAnalysisPanelRecord {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        panel_id: request.panel_id.clone(),
        group_run_id: request.manifest.source.group_run_id.clone(),
        status: GroupAnalysisPanelStatus::Prepared,
        source_snapshot_sha256: request.manifest.source.snapshot_sha256.clone(),
        manifest_sha256: super::super::group_run_codec::encode_hex_digest(&encoded.digest),
        manifest_bytes: encoded.bytes.len(),
        analysis_count: request.manifest.contributions.len(),
        created_at_ms: request.created_at_ms,
    }
}

fn insert_panel(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAnalysisPanel,
    record: &GroupAnalysisPanelRecord,
    encoded: &codec::EncodedManifest,
) -> Result<(), HubStoreError> {
    let source = codec::encode_blob(&record.source_snapshot_sha256, "source")?;
    let manifest_bytes = to_i64(record.manifest_bytes, "manifest byte count")?;
    let analysis_count = to_i64(record.analysis_count, "analysis count")?;
    transaction
        .execute(
            "INSERT INTO group_analysis_panels(
               id,group_run_id,panel_version,status,source_snapshot_sha256,
               analysis_count,manifest_blob,manifest_bytes,manifest_sha256,
               idempotency_key,created_at_ms
             ) VALUES(?1,?2,?3,'prepared',?4,?5,?6,?7,?8,?9,?10)",
            params![
                record.panel_id,
                record.group_run_id,
                i64::from(record.v),
                source.as_slice(),
                analysis_count,
                encoded.bytes.as_slice(),
                manifest_bytes,
                encoded.digest.as_slice(),
                request.idempotency_key,
                to_i64(record.created_at_ms, "creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAnalysisPanel, error))?;
    Ok(())
}

fn insert_analyses(
    transaction: &Transaction<'_>,
    panel_id: &str,
    manifest: &crate::runtime_domain::GroupAnalysisPanelManifest,
) -> Result<(), HubStoreError> {
    for (position, contribution) in manifest.contributions.iter().enumerate() {
        let digest = codec::encode_blob(&contribution.result.result_sha256, "result")?;
        transaction
            .execute(
                "INSERT INTO group_analysis_panel_analyses(
                   panel_id,position,analysis_id,result_sha256
                 ) VALUES(?1,?2,?3,?4)",
                params![
                    panel_id,
                    to_i64(position, "analysis position")?,
                    contribution.analysis.analysis_id,
                    digest.as_slice(),
                ],
            )
            .map_err(|error| write_error(HubEntity::GroupAnalysisPanel, error))?;
    }
    Ok(())
}

fn ensure_replay(
    inspection: &GroupAnalysisPanelInspection,
    request: &PrepareGroupAnalysisPanel,
) -> Result<(), HubStoreError> {
    if inspection.manifest == request.manifest {
        Ok(())
    } else {
        Err(conflict(
            "idempotency key was reused with different panel input",
        ))
    }
}

fn result(
    disposition: PrepareGroupAnalysisPanelDisposition,
    inspection: GroupAnalysisPanelInspection,
) -> PrepareGroupAnalysisPanelResult {
    PrepareGroupAnalysisPanelResult {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        disposition,
        inspection,
    }
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| conflict(&format!("invalid {subject}: {error}")))
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
