use crate::runtime_domain::{
    execution::attempt::AttemptRequest,
    platform_core_contract::{
        EventEnvelope, SourceComponent, canonical_event_envelope_json, event_envelope_sha256,
    },
};
use serde_json::{Map, Value};

use super::{AttemptAdmission, AttemptJournalError, MAX_EVENT_BYTES, codec};

pub(super) struct Candidate {
    pub admission: AttemptAdmission,
    pub request_json: String,
}

pub(super) fn prepare(
    request: &AttemptRequest,
    event: &EventEnvelope,
) -> Result<Candidate, AttemptJournalError> {
    let record = codec::encode(request)?;
    validate_binding(request, event, &record.sha256)?;
    let json = canonical_event_envelope_json(event).map_err(|_| AttemptJournalError::Invalid)?;
    if json.len() > MAX_EVENT_BYTES {
        return Err(AttemptJournalError::Invalid);
    }
    let event_sha256 = event_envelope_sha256(event).map_err(|_| AttemptJournalError::Invalid)?;
    Ok(Candidate {
        request_json: record.json,
        admission: AttemptAdmission {
            cursor: 0,
            request: request.clone(),
            request_sha256: record.sha256,
            event: event.clone(),
            canonical_event_json: json,
            event_sha256,
        },
    })
}

fn validate_binding(
    request: &AttemptRequest,
    event: &EventEnvelope,
    digest: &str,
) -> Result<(), AttemptJournalError> {
    let payload = Map::from_iter([
        ("request_sha256".into(), Value::String(digest.into())),
        ("state".into(), Value::String("requested".into())),
    ]);
    let valid = event.source_component == SourceComponent::Runtime
        && event.aggregate_ref == *request.attempt_ref()
        && event.scope_ref == *request.scope_ref()
        && event.source_snapshot_ref.as_ref() == Some(request.project_snapshot_ref())
        && event.aggregate_version == 1
        && event.sequence == 1
        && event.schema_name == "forge.runtime.attempt_requested"
        && event.schema_version == 1
        && event.causation_id.is_none()
        && event.extensions.is_empty()
        && event.payload_artifact_ref.is_none()
        && event.payload.as_ref() == Some(&payload);
    valid.then_some(()).ok_or(AttemptJournalError::Invalid)
}
