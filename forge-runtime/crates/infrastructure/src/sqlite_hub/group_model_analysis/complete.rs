use rusqlite::{Connection, Transaction};

use super::{
    CompleteGroupModelAnalysis, CompleteGroupModelAnalysisDisposition,
    CompleteGroupModelAnalysisResult, GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
    GROUP_MODEL_ANALYSIS_RESULT_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent, GroupModelAnalysisEventKind,
    GroupModelAnalysisInspection, GroupModelAnalysisRecord, GroupModelAnalysisRecovery,
    GroupModelAnalysisResultArtifact, GroupModelAnalysisResultReceipt, GroupModelAnalysisStatus,
    HubStoreError, codec, read, read_error, rows, sql, write,
};

pub(super) fn complete(
    connection: &mut Connection,
    request: &CompleteGroupModelAnalysis,
) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
    request
        .validate()
        .map_err(|error| write::conflict(&error.message))?;
    let transaction = write::immediate(connection)?;
    let result = complete_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn complete_locked(
    transaction: &Transaction<'_>,
    request: &CompleteGroupModelAnalysis,
) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
    let id = &request.artifact.result.analysis_id;
    let stored = rows::find_by_id(transaction, id)?.ok_or_else(|| write::not_found(id))?;
    let validated = read::validate_stored(transaction, stored)?;
    let (expected, encoded) =
        codec::encode_result(&request.artifact.result, request.artifact.created_at_ms)?;
    if expected != request.artifact {
        return Err(write::conflict(
            "analysis result bytes, digest, or byte count disagrees",
        ));
    }
    match validated.inspection.recovery {
        GroupModelAnalysisRecovery::Terminal { .. } => complete_replay(validated, request),
        GroupModelAnalysisRecovery::DispatchUnknown { .. } => {
            complete_dispatched(transaction, validated, request, &encoded)
        }
        _ => Err(write::conflict(
            "analysis cannot complete before dispatch is claimed",
        )),
    }
}

fn complete_replay(
    validated: read::ValidatedAnalysis,
    request: &CompleteGroupModelAnalysis,
) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
    if validated.inspection.result.as_ref() != Some(&request.artifact) {
        return Err(write::conflict(
            "completed analysis was replayed with a different result",
        ));
    }
    Ok(result(
        CompleteGroupModelAnalysisDisposition::Replayed,
        validated.inspection,
    ))
}

fn complete_dispatched(
    transaction: &Transaction<'_>,
    mut validated: read::ValidatedAnalysis,
    request: &CompleteGroupModelAnalysis,
    encoded: &codec::EncodedJson,
) -> Result<CompleteGroupModelAnalysisResult, HubStoreError> {
    let record = validated.inspection.analysis.clone();
    let claim = validated
        .inspection
        .dispatch
        .clone()
        .ok_or_else(|| write::corrupt("dispatch-unknown analysis has no dispatch claim"))?;
    let event = completion_event(
        &record,
        &claim,
        &request.artifact,
        validated.cursor.next_sequence(),
    );
    let inspection = next_inspection(&validated.inspection, event.clone(), &request.artifact)?;
    persist(
        transaction,
        &mut validated,
        &event,
        &request.artifact,
        encoded,
    )?;
    Ok(result(
        CompleteGroupModelAnalysisDisposition::Created,
        inspection,
    ))
}

fn completion_event(
    record: &GroupModelAnalysisRecord,
    claim: &GroupModelAnalysisDispatchClaim,
    artifact: &GroupModelAnalysisResultArtifact,
    sequence: u64,
) -> GroupModelAnalysisEvent {
    GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: record.analysis_id.clone(),
        seq: sequence,
        kind: GroupModelAnalysisEventKind::AnalysisCompleted {
            receipt: GroupModelAnalysisResultReceipt {
                v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
                analysis_id: record.analysis_id.clone(),
                dispatch_id: claim.dispatch_id.clone(),
                request_sha256: record.request_sha256.clone(),
                outcome: artifact.result.outcome,
                result_sha256: artifact.result_sha256.clone(),
                result_bytes: artifact.result_bytes,
                usage: artifact.result.usage,
                created_at_ms: artifact.created_at_ms,
            },
        },
    }
}

fn next_inspection(
    current: &GroupModelAnalysisInspection,
    event: GroupModelAnalysisEvent,
    artifact: &GroupModelAnalysisResultArtifact,
) -> Result<GroupModelAnalysisInspection, HubStoreError> {
    let mut record = current.analysis.clone();
    record.status = GroupModelAnalysisStatus::Completed;
    let mut events = current.events.clone();
    events.push(event);
    GroupModelAnalysisInspection::validate(record, events, Some(artifact.clone()))
        .map_err(|error| write::conflict(&error.message))
}

fn persist(
    transaction: &Transaction<'_>,
    validated: &mut read::ValidatedAnalysis,
    event: &GroupModelAnalysisEvent,
    artifact: &GroupModelAnalysisResultArtifact,
    encoded: &codec::EncodedJson,
) -> Result<(), HubStoreError> {
    sql::insert_result(
        transaction,
        &sql::ResultRow {
            analysis_id: &artifact.result.analysis_id,
            result_version: i64::from(artifact.result.v),
            blob: encoded.json.as_bytes(),
            result_bytes: write::to_i64(artifact.result_bytes, "result byte count")?,
            sha256: &encoded.digest,
            created_at_ms: write::to_i64(artifact.created_at_ms, "result creation time")?,
        },
    )?;
    write::append_transition(
        transaction,
        validated,
        event,
        "dispatch_unknown",
        "completed",
    )
}

fn result(
    disposition: CompleteGroupModelAnalysisDisposition,
    inspection: GroupModelAnalysisInspection,
) -> CompleteGroupModelAnalysisResult {
    CompleteGroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        disposition,
        inspection,
    }
}
