use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use super::validation::invalid;
use super::{
    MAX_SCHEDULED_GRAPH_CONTROLLER_EVENT_BYTES, MAX_SCHEDULED_GRAPH_CONTROLLER_HEADER_BYTES,
    SCHEDULED_GRAPH_CONTROLLER_EVENT_DIGEST_DOMAIN,
    SCHEDULED_GRAPH_CONTROLLER_HEADER_DIGEST_DOMAIN,
    SCHEDULED_GRAPH_CONTROLLER_PROFILE_DIGEST_DOMAIN, ScheduledGraphControllerEvent,
    ScheduledGraphControllerExecutionProfile, ScheduledGraphControllerHeader,
    ScheduledGraphControllerValidationError,
};

pub(super) fn canonical_json<T: Serialize>(
    value: &T,
) -> Result<String, ScheduledGraphControllerValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("controller JSON cannot be encoded"))
}

pub(super) fn profile_digest(
    value: &ScheduledGraphControllerExecutionProfile,
) -> Result<String, ScheduledGraphControllerValidationError> {
    let mut payload = value.clone();
    payload.profile_sha256.clear();
    digest(SCHEDULED_GRAPH_CONTROLLER_PROFILE_DIGEST_DOMAIN, &payload)
}

pub(super) fn header_digest(
    value: &ScheduledGraphControllerHeader,
) -> Result<String, ScheduledGraphControllerValidationError> {
    let mut payload = value.clone();
    payload.controller_id.clear();
    payload.controller_sha256.clear();
    digest(SCHEDULED_GRAPH_CONTROLLER_HEADER_DIGEST_DOMAIN, &payload)
}

pub(super) fn event_digest(
    value: &ScheduledGraphControllerEvent,
) -> Result<String, ScheduledGraphControllerValidationError> {
    let mut payload = value.clone();
    payload.event_sha256.clear();
    digest(SCHEDULED_GRAPH_CONTROLLER_EVENT_DIGEST_DOMAIN, &payload)
}

fn digest<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<String, ScheduledGraphControllerValidationError> {
    let encoded = canonical_json(value)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(encoded.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn decode_header_exact(
    bytes: &[u8],
) -> Result<ScheduledGraphControllerHeader, ScheduledGraphControllerValidationError> {
    let value: ScheduledGraphControllerHeader =
        decode_exact(bytes, MAX_SCHEDULED_GRAPH_CONTROLLER_HEADER_BYTES)?;
    value.validate()?;
    Ok(value)
}

pub(super) fn decode_event_exact(
    bytes: &[u8],
) -> Result<ScheduledGraphControllerEvent, ScheduledGraphControllerValidationError> {
    let value: ScheduledGraphControllerEvent =
        decode_exact(bytes, MAX_SCHEDULED_GRAPH_CONTROLLER_EVENT_BYTES)?;
    super::validation::validate_event_shape(&value)?;
    Ok(value)
}

fn decode_exact<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, ScheduledGraphControllerValidationError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid("controller JSON is outside its byte bound"));
    }
    let value: T =
        serde_json::from_slice(bytes).map_err(|_| invalid("controller JSON is malformed"))?;
    let exact = canonical_json(&value)?;
    if exact.as_bytes() != bytes {
        return Err(invalid("controller JSON is not exact canonical JSON"));
    }
    Ok(value)
}
