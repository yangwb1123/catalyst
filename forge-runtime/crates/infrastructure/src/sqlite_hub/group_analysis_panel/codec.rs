use crate::runtime_domain::{
    GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, GroupAnalysisPanelManifest, HubEntity,
    HubStoreError, MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES,
};

use super::super::{
    group_context_build::{canonical_json_bytes, digest_with_domain_bytes},
    group_run_codec::{decode_hex_digest, encode_hex_digest},
};

pub(super) struct EncodedManifest {
    pub bytes: Vec<u8>,
    pub digest: [u8; 32],
}

pub(super) fn encode(
    manifest: &GroupAnalysisPanelManifest,
) -> Result<EncodedManifest, HubStoreError> {
    manifest
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let bytes = canonical_json_bytes(manifest)?;
    validate_size(&bytes, false)?;
    let digest = digest_with_domain_bytes(GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, &bytes);
    Ok(EncodedManifest { bytes, digest })
}

pub(super) fn decode(
    bytes: &[u8],
    stored_digest: &[u8],
) -> Result<GroupAnalysisPanelManifest, HubStoreError> {
    validate_size(bytes, true)?;
    let digest = blob_digest(stored_digest)?;
    let actual = digest_with_domain_bytes(GROUP_ANALYSIS_PANEL_MANIFEST_DIGEST_DOMAIN, bytes);
    if actual != digest {
        return Err(corrupt("stored panel manifest digest disagrees"));
    }
    let manifest: GroupAnalysisPanelManifest = serde_json::from_slice(bytes)
        .map_err(|error| corrupt(&format!("invalid stored panel manifest: {error}")))?;
    manifest
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    if canonical_json_bytes(&manifest)? != bytes {
        return Err(corrupt("stored panel manifest is not canonical"));
    }
    Ok(manifest)
}

pub(super) fn decode_hex(value: &[u8], subject: &str) -> Result<String, HubStoreError> {
    let digest = blob_digest(value)
        .map_err(|_| corrupt(&format!("stored panel {subject} digest is invalid")))?;
    Ok(encode_hex_digest(&digest))
}

pub(super) fn encode_blob(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value).ok_or_else(|| conflict(&format!("panel {subject} digest is invalid")))
}

fn blob_digest(value: &[u8]) -> Result<[u8; 32], HubStoreError> {
    value
        .try_into()
        .map_err(|_| corrupt("stored panel digest is not 32 bytes"))
}

fn validate_size(bytes: &[u8], stored: bool) -> Result<(), HubStoreError> {
    if (1..=MAX_GROUP_ANALYSIS_PANEL_MANIFEST_BYTES).contains(&bytes.len()) {
        return Ok(());
    }
    if stored {
        Err(corrupt("stored panel manifest is outside its byte bound"))
    } else {
        Err(conflict("panel manifest exceeds its durable byte limit"))
    }
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
