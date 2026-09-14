use super::{ConversationOwner, HubEntity, HubStoreError, MAX_HUB_ENTITY_ID_BYTES};

pub(super) fn validate_owner(owner: &ConversationOwner) -> Result<(), HubStoreError> {
    for (value, maximum) in [
        (&owner.issuer, 2048),
        (&owner.subject, 255),
        (&owner.tenant_id, 256),
    ] {
        if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
            return Err(HubStoreError::Conflict {
                entity: HubEntity::ProjectExecutionConsent,
                message: "consent owner tuple is invalid".into(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_identifier(value: &str, label: &str) -> Result<(), HubStoreError> {
    if value.trim().is_empty()
        || value.len() > MAX_HUB_ENTITY_ID_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::ProjectExecutionConsent,
            message: format!("{label} identifier is invalid"),
        });
    }
    Ok(())
}

pub(super) fn validate_idempotency_key(value: &str) -> Result<(), HubStoreError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::ProjectExecutionConsent,
            message: "consent idempotency key is invalid".into(),
        });
    }
    Ok(())
}

pub(super) fn decode_timestamp(value: i64) -> Result<u64, HubStoreError> {
    u64::try_from(value).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid consent timestamp: {error}"),
    })
}

pub(super) fn to_i64(value: u64) -> Result<i64, HubStoreError> {
    i64::try_from(value).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::ProjectExecutionConsent,
        message: format!("consent timestamp exceeds SQLite's range: {error}"),
    })
}

pub(super) fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::ProjectExecutionConsent,
        message: message.into(),
    }
}

pub(super) fn not_found(grant_id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::ProjectExecutionConsent,
        id: grant_id.into(),
    }
}
