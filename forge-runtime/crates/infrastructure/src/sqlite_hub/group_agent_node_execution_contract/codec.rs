use crate::runtime_domain::{
    AdmitGroupAgentNodeExecutionContract, GroupAgentNodeExecutionContract, HubEntity,
    HubStoreError, MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES,
};

use super::super::group_run_codec::{decode_hex_digest, encode_hex_digest};

pub(super) struct EncodedAdmission {
    pub contract_bytes: Vec<u8>,
    pub contract_digest: [u8; 32],
    pub event_bytes: Vec<u8>,
    pub event_digest: [u8; 32],
}

pub(super) fn encode_candidate(
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<EncodedAdmission, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let contract_bytes = request.contract_json.as_bytes().to_vec();
    require_bound(&contract_bytes, false)?;
    let contract_digest = candidate_digest(&request.contract.contract_sha256, "contract")?;
    let event_bytes = request.event_json.as_bytes().to_vec();
    let event_digest = candidate_digest(
        &request
            .event
            .expected_sha256()
            .map_err(|error| conflict(&error.to_string()))?,
        "event",
    )?;
    Ok(EncodedAdmission {
        contract_bytes,
        contract_digest,
        event_bytes,
        event_digest,
    })
}

pub(super) fn decode_contract(
    bytes: &[u8],
    stored_digest: &[u8],
) -> Result<(GroupAgentNodeExecutionContract, String), HubStoreError> {
    require_bound(bytes, true)?;
    let digest = stored_digest_value(stored_digest, "contract")?;
    let contract: GroupAgentNodeExecutionContract =
        serde_json::from_slice(bytes).map_err(|error| {
            corrupt(&format!(
                "invalid stored Node Execution Contract JSON: {error}"
            ))
        })?;
    contract
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    let expected = candidate_digest(&contract.contract_sha256, "contract")
        .map_err(|error| corrupt(&error.to_string()))?;
    if expected != digest {
        return Err(corrupt("stored Node Execution Contract digest disagrees"));
    }
    let canonical = contract
        .canonical_json()
        .map_err(|error| corrupt(&error.to_string()))?;
    if canonical.as_bytes() != bytes {
        return Err(corrupt(
            "stored Node Execution Contract JSON is not canonical",
        ));
    }
    Ok((contract, canonical))
}

pub(super) fn digest_hex(bytes: &[u8], subject: &str) -> Result<String, HubStoreError> {
    stored_digest_value(bytes, subject).map(|digest| encode_hex_digest(&digest))
}

pub(super) fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value).ok_or_else(|| {
        conflict(&format!(
            "Node Execution Contract {subject} digest is invalid"
        ))
    })
}

fn stored_digest_value(bytes: &[u8], subject: &str) -> Result<[u8; 32], HubStoreError> {
    bytes.try_into().map_err(|_| {
        corrupt(&format!(
            "stored Node Execution Contract {subject} digest is not 32 bytes"
        ))
    })
}

fn require_bound(bytes: &[u8], stored: bool) -> Result<(), HubStoreError> {
    if (1..=MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES).contains(&bytes.len()) {
        return Ok(());
    }
    if stored {
        Err(corrupt(
            "stored Node Execution Contract is outside its byte bound",
        ))
    } else {
        Err(conflict(
            "Node Execution Contract exceeds its durable byte limit",
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
        entity: HubEntity::GroupAgentNodeExecutionContract,
        message: message.into(),
    }
}
