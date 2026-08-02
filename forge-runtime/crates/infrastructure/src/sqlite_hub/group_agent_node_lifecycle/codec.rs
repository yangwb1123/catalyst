use crate::runtime_domain::{HubEntity, HubStoreError};

use super::super::group_run_codec::{decode_hex_digest, encode_hex_digest};

pub(super) fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value)
        .ok_or_else(|| conflict(&format!("Node lifecycle {subject} digest is invalid")))
}

pub(super) fn stored_digest(bytes: &[u8], subject: &str) -> Result<String, HubStoreError> {
    let digest: [u8; 32] = bytes.try_into().map_err(|_| {
        corrupt(&format!(
            "stored Node lifecycle {subject} digest is not 32 bytes"
        ))
    })?;
    Ok(encode_hex_digest(&digest))
}

pub(super) fn stored_json(bytes: Vec<u8>, subject: &str) -> Result<String, HubStoreError> {
    String::from_utf8(bytes)
        .map_err(|_| corrupt(&format!("stored Node lifecycle {subject} is not UTF-8")))
}

pub(super) fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value)
        .map_err(|error| conflict(&format!("invalid Node lifecycle {subject}: {error}")))
}

pub(super) fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value)
        .map_err(|error| corrupt(&format!("invalid stored Node lifecycle {subject}: {error}")))
}

pub(super) fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeLifecycle,
        message: message.into(),
    }
}

pub(super) fn not_found(graph_run_id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentNodeLifecycle,
        id: graph_run_id.into(),
    }
}
