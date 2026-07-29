use crate::runtime_domain::{
    GROUP_EXECUTION_VERSION, GroupExecutionEvent, GroupExecutionEventKind,
    GroupExecutionJournalCursor, GroupExecutionReceipt, GroupExecutionRecord, GroupRunSnapshot,
    HubEntity, HubStoreError, MAX_GROUP_EXECUTION_CURSOR_JSON_BYTES,
    MAX_GROUP_EXECUTION_EVENT_JSON_BYTES, MAX_GROUP_EXECUTION_EVENTS,
};

use super::group_context_build::{canonical_json_bytes, digest_with_domain_bytes};

pub(super) const EVENT_DIGEST_DOMAIN: &[u8] = b"forge.group-execution-event.v1\0";

pub(super) struct EncodedEvent {
    pub json: String,
    pub digest: [u8; 32],
}

pub(super) fn encode_event(event: &GroupExecutionEvent) -> Result<EncodedEvent, HubStoreError> {
    let bytes = canonical_json_bytes(event)?;
    if bytes.is_empty() || bytes.len() > MAX_GROUP_EXECUTION_EVENT_JSON_BYTES {
        return Err(conflict(
            "Group Execution event exceeds its durable byte limit",
        ));
    }
    let digest = digest_with_domain_bytes(EVENT_DIGEST_DOMAIN, &bytes);
    let json = String::from_utf8(bytes).map_err(|error| {
        corrupt(&format!(
            "generated Group Execution event is not UTF-8: {error}"
        ))
    })?;
    Ok(EncodedEvent { json, digest })
}

pub(super) fn decode_event(
    row_sequence: i64,
    json: &str,
    stored_digest: &[u8],
) -> Result<GroupExecutionEvent, HubStoreError> {
    let sequence = u64::try_from(row_sequence)
        .map_err(|error| corrupt(&format!("invalid Group Execution event sequence: {error}")))?;
    if !(1..=MAX_GROUP_EXECUTION_EVENTS as u64).contains(&sequence)
        || json.is_empty()
        || json.len() > MAX_GROUP_EXECUTION_EVENT_JSON_BYTES
    {
        return Err(corrupt(
            "stored Group Execution event violates its durable bounds",
        ));
    }
    verify_event_digest(json.as_bytes(), stored_digest)?;
    let event: GroupExecutionEvent = serde_json::from_str(json).map_err(|error| {
        corrupt(&format!(
            "invalid stored Group Execution event JSON: {error}"
        ))
    })?;
    let canonical = canonical_json_bytes(&event)?;
    if canonical != json.as_bytes() || event.seq != sequence {
        return Err(corrupt(
            "stored Group Execution event is noncanonical or has a mismatched sequence",
        ));
    }
    Ok(event)
}

pub(super) fn encode_cursor(cursor: &GroupExecutionJournalCursor) -> Result<String, HubStoreError> {
    let encoded = serde_json::to_string(cursor)
        .map_err(|error| corrupt(&format!("Group Execution cursor cannot encode: {error}")))?;
    if encoded.is_empty() || encoded.len() > MAX_GROUP_EXECUTION_CURSOR_JSON_BYTES {
        return Err(conflict(
            "Group Execution cursor exceeds its durable byte limit",
        ));
    }
    Ok(encoded)
}

pub(super) fn decode_cursor(
    json: &str,
    record: &GroupExecutionRecord,
) -> Result<GroupExecutionJournalCursor, HubStoreError> {
    if json.is_empty() || json.len() > MAX_GROUP_EXECUTION_CURSOR_JSON_BYTES {
        return Err(corrupt(
            "stored Group Execution cursor violates its durable bounds",
        ));
    }
    let cursor: GroupExecutionJournalCursor = serde_json::from_str(json).map_err(|error| {
        corrupt(&format!(
            "invalid stored Group Execution cursor JSON: {error}"
        ))
    })?;
    cursor
        .validate_record(record)
        .map_err(|error| corrupt(&error.message))?;
    if encode_cursor(&cursor)? != json {
        return Err(corrupt("stored Group Execution cursor is not canonical"));
    }
    Ok(cursor)
}

pub(super) fn validate_record_metadata(record: &GroupExecutionRecord) -> Result<(), HubStoreError> {
    GroupExecutionJournalCursor::new(record)
        .map(|_| ())
        .map_err(|error| corrupt(&error.message))
}

pub(super) fn validate_source_binding(
    record: &GroupExecutionRecord,
    snapshot: &GroupRunSnapshot,
) -> Result<(), HubStoreError> {
    let matches = record.group_run_id == snapshot.run.run_id
        && record.source_snapshot_sha256 == snapshot.run.snapshot_sha256;
    if matches {
        Ok(())
    } else {
        Err(corrupt(
            "Group Execution metadata does not match its frozen source snapshot",
        ))
    }
}

pub(super) fn validate_receipt_binding(
    record: &GroupExecutionRecord,
    snapshot: &GroupRunSnapshot,
    receipt: &GroupExecutionReceipt,
) -> Result<(), HubStoreError> {
    let matches = receipt.v == GROUP_EXECUTION_VERSION
        && receipt.execution_id == record.execution_id
        && receipt.group_run_id == snapshot.run.run_id
        && receipt.group_id == snapshot.run.group_id
        && receipt.context_version == snapshot.run.context_version
        && receipt.context_slice_sha256 == snapshot.run.context_slice_sha256
        && receipt.snapshot_sha256 == snapshot.run.snapshot_sha256
        && receipt.snapshot_bytes == snapshot.run.snapshot_bytes
        && receipt.stats == snapshot.context.payload.stats;
    if matches {
        Ok(())
    } else {
        Err(corrupt(
            "Group Execution receipt does not exactly describe its frozen snapshot",
        ))
    }
}

pub(super) fn validate_event_source(
    record: &GroupExecutionRecord,
    snapshot: &GroupRunSnapshot,
    event: &GroupExecutionEvent,
) -> Result<(), HubStoreError> {
    match &event.kind {
        GroupExecutionEventKind::ExecutionStarted {
            group_run_id,
            snapshot_sha256,
        } if group_run_id == &snapshot.run.run_id
            && snapshot_sha256 == &snapshot.run.snapshot_sha256 =>
        {
            Ok(())
        }
        GroupExecutionEventKind::SnapshotVerified { receipt } => {
            validate_receipt_binding(record, snapshot, receipt)
        }
        GroupExecutionEventKind::ExecutionFinished { .. } => Ok(()),
        GroupExecutionEventKind::ExecutionStarted { .. } => Err(conflict(
            "execution_started does not match the frozen source snapshot",
        )),
    }
}

fn verify_event_digest(bytes: &[u8], stored: &[u8]) -> Result<(), HubStoreError> {
    let stored: &[u8; 32] = stored
        .try_into()
        .map_err(|_| corrupt("stored Group Execution event digest is not 32 bytes"))?;
    let expected = digest_with_domain_bytes(EVENT_DIGEST_DOMAIN, bytes);
    if stored == &expected {
        Ok(())
    } else {
        Err(corrupt(
            "stored Group Execution event digest does not match its JSON",
        ))
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupExecution,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{EVENT_DIGEST_DOMAIN, digest_with_domain_bytes};
    use crate::sqlite_hub::group_run_codec::encode_hex_digest;

    #[test]
    fn event_digest_domain_has_a_stable_golden_vector() {
        let digest = digest_with_domain_bytes(EVENT_DIGEST_DOMAIN, b"{}");

        assert_eq!(
            encode_hex_digest(&digest),
            "849c9c064a667b035e2cb12e66580af5a839534940fffc53c3827f0c1a68f36e"
        );
    }
}
