use crate::runtime_domain::{
    HubEntity, HubStoreError, MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
    MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES, PrepareGroupAgentNodeDispatchRequest,
};

use super::super::super::group_run_codec::{decode_hex_digest, encode_hex_digest};

pub(super) struct EncodedPreparation {
    pub event_bytes: Vec<u8>,
    pub event_digest: [u8; 32],
}

pub(super) fn encode_candidate(
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<EncodedPreparation, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    require_bound(
        &request.provider_request_body,
        MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES,
        false,
        "provider request body",
    )?;
    let event_bytes = request.event_json.as_bytes().to_vec();
    require_bound(
        &event_bytes,
        MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
        false,
        "preparation event",
    )?;
    let event_digest = candidate_digest(
        &request
            .event
            .expected_sha256()
            .map_err(|error| conflict(&error.to_string()))?,
        "event",
    )?;
    Ok(EncodedPreparation {
        event_bytes,
        event_digest,
    })
}

pub(super) fn digest_hex(bytes: &[u8], subject: &str) -> Result<String, HubStoreError> {
    stored_digest(bytes, subject).map(|digest| encode_hex_digest(&digest))
}

pub(super) fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value).ok_or_else(|| {
        conflict(&format!(
            "Node Dispatch Request {subject} digest is invalid"
        ))
    })
}

fn stored_digest(bytes: &[u8], subject: &str) -> Result<[u8; 32], HubStoreError> {
    bytes.try_into().map_err(|_| {
        corrupt(&format!(
            "stored Node Dispatch Request {subject} digest is not 32 bytes"
        ))
    })
}

fn require_bound(
    bytes: &[u8],
    maximum: usize,
    stored: bool,
    subject: &str,
) -> Result<(), HubStoreError> {
    if (1..=maximum).contains(&bytes.len()) {
        return Ok(());
    }
    if stored {
        Err(corrupt(&format!(
            "stored {subject} is outside its byte bound"
        )))
    } else {
        Err(conflict(&format!(
            "{subject} exceeds its durable byte limit"
        )))
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeDispatchRequest,
        message: message.into(),
    }
}
