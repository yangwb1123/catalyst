use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GroupAgentGraphManifest, HubEntity, HubStoreError,
    MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES, PrepareGroupAgentGraph,
};

use super::super::{
    group_context_build::{canonical_json_bytes, digest_with_domain_bytes},
    group_run_codec::{decode_hex_digest, encode_hex_digest},
};

pub(super) struct EncodedManifest {
    pub bytes: Vec<u8>,
    pub digest: [u8; 32],
}

pub(super) fn encode_candidate(
    request: &PrepareGroupAgentGraph,
) -> Result<EncodedManifest, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let bytes = request.manifest_json.as_bytes().to_vec();
    validate_size(&bytes, false)?;
    let decoded: GroupAgentGraphManifest =
        serde_json::from_slice(&bytes).map_err(|error| conflict(&error.to_string()))?;
    if decoded != request.manifest || canonical_json_bytes(&decoded)? != bytes {
        return Err(conflict(
            "graph manifest JSON is not the exact canonical manifest",
        ));
    }
    require_canonical_edge_order(&decoded, false)?;
    let digest = digest_with_domain_bytes(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes);
    if encode_hex_digest(&digest) != request.manifest_sha256 {
        return Err(conflict("graph manifest digest disagrees with its bytes"));
    }
    Ok(EncodedManifest { bytes, digest })
}

pub(super) fn decode(
    bytes: &[u8],
    stored_digest: &[u8],
) -> Result<(GroupAgentGraphManifest, String), HubStoreError> {
    validate_size(bytes, true)?;
    let expected = blob_digest(stored_digest)?;
    let actual = digest_with_domain_bytes(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, bytes);
    if actual != expected {
        return Err(corrupt("stored graph manifest digest disagrees"));
    }
    let manifest: GroupAgentGraphManifest = serde_json::from_slice(bytes)
        .map_err(|error| corrupt(&format!("invalid stored graph manifest: {error}")))?;
    manifest
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    require_canonical_edge_order(&manifest, true)?;
    if canonical_json_bytes(&manifest)? != bytes {
        return Err(corrupt("stored graph manifest is not canonical"));
    }
    let json = String::from_utf8(bytes.to_vec())
        .map_err(|error| corrupt(&format!("stored graph manifest is not UTF-8: {error}")))?;
    Ok((manifest, json))
}

pub(super) fn decode_hex(value: &[u8], subject: &str) -> Result<String, HubStoreError> {
    let digest = blob_digest(value)
        .map_err(|_| corrupt(&format!("stored graph {subject} digest is invalid")))?;
    Ok(encode_hex_digest(&digest))
}

pub(super) fn encode_blob(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value).ok_or_else(|| conflict(&format!("graph {subject} digest is invalid")))
}

fn blob_digest(value: &[u8]) -> Result<[u8; 32], HubStoreError> {
    value
        .try_into()
        .map_err(|_| corrupt("stored graph digest is not 32 bytes"))
}

fn validate_size(bytes: &[u8], stored: bool) -> Result<(), HubStoreError> {
    if (1..=MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES).contains(&bytes.len()) {
        return Ok(());
    }
    if stored {
        Err(corrupt("stored graph manifest is outside its byte bound"))
    } else {
        Err(conflict("graph manifest exceeds its durable byte limit"))
    }
}

fn require_canonical_edge_order(
    manifest: &GroupAgentGraphManifest,
    stored: bool,
) -> Result<(), HubStoreError> {
    let ordered = manifest.edges.windows(2).all(|pair| pair[0] < pair[1]);
    if ordered {
        return Ok(());
    }
    if stored {
        Err(corrupt("stored graph edges are not in canonical order"))
    } else {
        Err(conflict("graph edges are not in canonical order"))
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraph,
        message: message.into(),
    }
}
