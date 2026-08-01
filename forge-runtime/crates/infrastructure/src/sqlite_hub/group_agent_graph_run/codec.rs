use super::{
    super::group_run_codec::{decode_hex_digest, encode_hex_digest},
    BeginGroupAgentGraphRun, GroupAgentGraphCorePlan, GroupAgentGraphRunEvent, HubEntity,
    HubStoreError, MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES, MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
};

pub(super) struct EncodedCandidate {
    pub plan_bytes: Vec<u8>,
    pub plan_digest: [u8; 32],
    pub event_bytes: Vec<u8>,
    pub event_digest: [u8; 32],
}

pub(super) fn encode_candidate(
    request: &BeginGroupAgentGraphRun,
) -> Result<EncodedCandidate, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let plan_bytes = request.plan_json.as_bytes().to_vec();
    let event_bytes = request.event_json.as_bytes().to_vec();
    require_bound(
        &plan_bytes,
        MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES,
        false,
        "plan",
    )?;
    require_bound(
        &event_bytes,
        MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
        false,
        "event",
    )?;
    let plan_digest = candidate_digest(&request.plan.plan_sha256, "plan")?;
    let event_digest = candidate_digest(
        &request
            .event
            .expected_sha256()
            .map_err(|error| conflict(&error.to_string()))?,
        "event",
    )?;
    Ok(EncodedCandidate {
        plan_bytes,
        plan_digest,
        event_bytes,
        event_digest,
    })
}

pub(super) fn decode_plan(
    bytes: &[u8],
    stored_digest: &[u8],
) -> Result<(GroupAgentGraphCorePlan, String), HubStoreError> {
    require_bound(bytes, MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES, true, "plan")?;
    let digest = stored_digest_value(stored_digest, "plan")?;
    let plan: GroupAgentGraphCorePlan = serde_json::from_slice(bytes)
        .map_err(|error| corrupt(&format!("invalid stored Core Plan JSON: {error}")))?;
    plan.validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    if candidate_digest(&plan.plan_sha256, "plan").map_err(|error| corrupt(&error.to_string()))?
        != digest
    {
        return Err(corrupt("stored Core Plan digest disagrees"));
    }
    let canonical = plan
        .canonical_json()
        .map_err(|error| corrupt(&error.to_string()))?;
    if canonical.as_bytes() != bytes {
        return Err(corrupt("stored Core Plan JSON is not canonical"));
    }
    Ok((plan, canonical))
}

pub(super) fn decode_event(
    bytes: &[u8],
    stored_digest: &[u8],
) -> Result<(GroupAgentGraphRunEvent, String), HubStoreError> {
    require_bound(bytes, MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES, true, "event")?;
    let digest = stored_digest_value(stored_digest, "event")?;
    let event: GroupAgentGraphRunEvent = serde_json::from_slice(bytes)
        .map_err(|error| corrupt(&format!("invalid stored Graph Run event JSON: {error}")))?;
    event
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    let expected = event
        .expected_sha256()
        .map_err(|error| corrupt(&error.to_string()))?;
    if candidate_digest(&expected, "event").map_err(|error| corrupt(&error.to_string()))? != digest
    {
        return Err(corrupt("stored Graph Run event digest disagrees"));
    }
    let canonical = event
        .canonical_json()
        .map_err(|error| corrupt(&error.to_string()))?;
    if canonical.as_bytes() != bytes {
        return Err(corrupt("stored Graph Run event JSON is not canonical"));
    }
    Ok((event, canonical))
}

pub(super) fn digest_hex(bytes: &[u8], subject: &str) -> Result<String, HubStoreError> {
    stored_digest_value(bytes, subject).map(|digest| encode_hex_digest(&digest))
}

pub(super) fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value)
        .ok_or_else(|| conflict(&format!("Graph Run {subject} digest is invalid")))
}

fn stored_digest_value(bytes: &[u8], subject: &str) -> Result<[u8; 32], HubStoreError> {
    bytes.try_into().map_err(|_| {
        corrupt(&format!(
            "stored Graph Run {subject} digest is not 32 bytes"
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
            "stored Graph Run {subject} is outside its byte bound"
        )))
    } else {
        Err(conflict(&format!(
            "Graph Run {subject} exceeds its durable byte limit"
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
        entity: HubEntity::GroupAgentGraphRun,
        message: message.into(),
    }
}
