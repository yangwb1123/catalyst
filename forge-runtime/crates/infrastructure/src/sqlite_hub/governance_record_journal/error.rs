use crate::runtime_domain::{HubEntity, HubStoreError};

pub(super) fn corrupt(message: impl Into<String>) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn conflict(message: impl Into<String>) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GovernanceRecord,
        message: message.into(),
    }
}

pub(super) fn not_found(id: impl Into<String>) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GovernanceRecord,
        id: id.into(),
    }
}

pub(super) fn read(error: rusqlite::Error) -> HubStoreError {
    super::super::read_error(error)
}

pub(super) fn write(error: rusqlite::Error) -> HubStoreError {
    super::super::write_error(HubEntity::GovernanceRecord, error)
}

pub(super) fn stored_usize(value: i64, subject: &str) -> Result<usize, HubStoreError> {
    usize::try_from(value).map_err(|error| corrupt(format!("invalid stored {subject}: {error}")))
}

pub(super) fn stored_u64(value: i64, subject: &str) -> Result<u64, HubStoreError> {
    u64::try_from(value).map_err(|error| corrupt(format!("invalid stored {subject}: {error}")))
}

pub(super) fn input_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| conflict(format!("invalid {subject}: {error}")))
}

pub(super) fn digest_blob(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    super::super::group_run_codec::decode_hex_digest(value)
        .ok_or_else(|| conflict(format!("{subject} is not a lowercase SHA-256 digest")))
}

pub(super) fn stored_digest(value: &[u8], subject: &str) -> Result<String, HubStoreError> {
    let digest: [u8; 32] = value
        .try_into()
        .map_err(|_| corrupt(format!("stored {subject} is not a SHA-256 digest")))?;
    Ok(super::super::group_run_codec::encode_hex_digest(&digest))
}
