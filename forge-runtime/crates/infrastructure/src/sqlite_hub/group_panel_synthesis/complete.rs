use rusqlite::{Connection, Transaction};

use super::{
    CompleteGroupPanelSynthesis, CompleteGroupPanelSynthesisDisposition,
    CompleteGroupPanelSynthesisResult, GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
    GROUP_PANEL_SYNTHESIS_RESULT_VERSION, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisDispatchClaim, GroupPanelSynthesisEvent, GroupPanelSynthesisEventKind,
    GroupPanelSynthesisInspection, GroupPanelSynthesisRecord, GroupPanelSynthesisRecovery,
    GroupPanelSynthesisResultArtifact, GroupPanelSynthesisResultReceipt, GroupPanelSynthesisStatus,
    HubStoreError, codec, read, read_error, rows, sql, write,
};

pub(super) fn complete(
    connection: &mut Connection,
    request: &CompleteGroupPanelSynthesis,
) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
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
    request: &CompleteGroupPanelSynthesis,
) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
    let id = &request.artifact.result.synthesis_id;
    let stored = rows::find_by_id(transaction, id)?.ok_or_else(|| write::not_found(id))?;
    let validated = read::validate_stored(transaction, stored)?;
    let (expected, encoded) =
        codec::encode_result(&request.artifact.result, request.artifact.created_at_ms)?;
    if expected != request.artifact {
        return Err(write::conflict(
            "synthesis result bytes, digest, or byte count disagrees",
        ));
    }
    match validated.inspection.recovery {
        GroupPanelSynthesisRecovery::Terminal { .. } => complete_replay(validated, request),
        GroupPanelSynthesisRecovery::DispatchUnknown { .. } => {
            complete_dispatched(transaction, validated, request, &encoded)
        }
        _ => Err(write::conflict(
            "synthesis cannot complete before dispatch is claimed",
        )),
    }
}

fn complete_replay(
    validated: read::ValidatedSynthesis,
    request: &CompleteGroupPanelSynthesis,
) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
    if validated.inspection.result.as_ref() != Some(&request.artifact) {
        return Err(write::conflict(
            "completed synthesis was replayed with a different result",
        ));
    }
    Ok(result(
        CompleteGroupPanelSynthesisDisposition::Replayed,
        validated.inspection,
    ))
}

fn complete_dispatched(
    transaction: &Transaction<'_>,
    mut validated: read::ValidatedSynthesis,
    request: &CompleteGroupPanelSynthesis,
    encoded: &codec::EncodedJson,
) -> Result<CompleteGroupPanelSynthesisResult, HubStoreError> {
    let record = validated.inspection.synthesis.clone();
    let claim = validated
        .inspection
        .dispatch
        .clone()
        .ok_or_else(|| write::corrupt("dispatch-unknown synthesis has no dispatch claim"))?;
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
    let persisted = write::reread_validated(transaction, &record.synthesis_id)?.inspection;
    if persisted != inspection {
        return Err(write::corrupt(
            "persisted synthesis completion disagrees with its committed result",
        ));
    }
    Ok(result(
        CompleteGroupPanelSynthesisDisposition::Created,
        persisted,
    ))
}

fn completion_event(
    record: &GroupPanelSynthesisRecord,
    claim: &GroupPanelSynthesisDispatchClaim,
    artifact: &GroupPanelSynthesisResultArtifact,
    sequence: u64,
) -> GroupPanelSynthesisEvent {
    GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: record.synthesis_id.clone(),
        seq: sequence,
        kind: GroupPanelSynthesisEventKind::SynthesisCompleted {
            receipt: GroupPanelSynthesisResultReceipt {
                v: GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
                synthesis_id: record.synthesis_id.clone(),
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
    current: &GroupPanelSynthesisInspection,
    event: GroupPanelSynthesisEvent,
    artifact: &GroupPanelSynthesisResultArtifact,
) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
    let mut record = current.synthesis.clone();
    record.status = GroupPanelSynthesisStatus::Completed;
    let mut events = current.events.clone();
    events.push(event);
    GroupPanelSynthesisInspection::validate(record, events, Some(artifact.clone()))
        .map_err(|error| write::conflict(&error.message))
}

fn persist(
    transaction: &Transaction<'_>,
    validated: &mut read::ValidatedSynthesis,
    event: &GroupPanelSynthesisEvent,
    artifact: &GroupPanelSynthesisResultArtifact,
    encoded: &codec::EncodedJson,
) -> Result<(), HubStoreError> {
    sql::insert_result(
        transaction,
        &sql::ResultRow {
            synthesis_id: &artifact.result.synthesis_id,
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
    disposition: CompleteGroupPanelSynthesisDisposition,
    inspection: GroupPanelSynthesisInspection,
) -> CompleteGroupPanelSynthesisResult {
    CompleteGroupPanelSynthesisResult {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        disposition,
        inspection,
    }
}
