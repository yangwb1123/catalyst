use super::{
    ArtifactRef, CANONICALIZATION, EntityType, MAX_ARTIFACT_BYTES, MAX_UNIX_MILLISECONDS,
    PlatformCoreContractError, RejectionCode, RetentionClass, Sensitivity, identity, invalid,
    references, reject, wire,
};

/// Validates supplied `ArtifactRef` declarations without resolving content.
///
/// # Errors
///
/// Returns an error for malformed identity, provenance, classification, or bounds.
pub fn validate_artifact_ref(value: &ArtifactRef) -> Result<(), PlatformCoreContractError> {
    validate_artifact_values(value)?;
    validate_artifact_references(value)?;
    validate_artifact_relations(value)
}

pub(super) fn validate_artifact_values(
    value: &ArtifactRef,
) -> Result<(), PlatformCoreContractError> {
    if value.canonicalization != CANONICALIZATION {
        return Err(invalid("ArtifactRef canonicalization is unsupported"));
    }
    if !wire::lower_token(&value.artifact_kind) || value.artifact_kind.len() > 64 {
        return Err(invalid("artifact_kind must be a bounded lowercase token"));
    }
    wire::validate_hash(&value.content_digest, "content_digest")?;
    identity::validate_typed_id(&value.logical_id, "art", "logical_id")?;
    validate_artifact_details(value)
}

fn validate_artifact_details(value: &ArtifactRef) -> Result<(), PlatformCoreContractError> {
    validate_media_type(&value.media_type)?;
    identity::validate_typed_id(&value.producer_attempt_id, "atm", "producer_attempt_id")?;
    references::validate_entity_ref(&value.source_snapshot_ref, "source_snapshot_ref")?;
    references::validate_record_ref(&value.provenance_ref, "provenance_ref")?;
    validate_artifact_bounds(value)
}

pub(super) fn validate_artifact_references(
    value: &ArtifactRef,
) -> Result<(), PlatformCoreContractError> {
    if value.source_snapshot_ref.entity_type != EntityType::ProjectSnapshot {
        return Err(reject(
            RejectionCode::ReferenceMismatch,
            "source_snapshot_ref must reference project_snapshot",
        ));
    }
    Ok(())
}

pub(super) fn validate_artifact_relations(
    value: &ArtifactRef,
) -> Result<(), PlatformCoreContractError> {
    if value.content_id != format!("sha256:{}", value.content_digest) {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "content_id must equal sha256: plus content_digest",
        ));
    }
    Ok(())
}

fn validate_artifact_bounds(value: &ArtifactRef) -> Result<(), PlatformCoreContractError> {
    if !(1..=MAX_UNIX_MILLISECONDS).contains(&value.created_at_unix_ms) {
        return Err(invalid(format!(
            "created_at_unix_ms must be in 1..{MAX_UNIX_MILLISECONDS}"
        )));
    }
    if !(0..=MAX_ARTIFACT_BYTES).contains(&value.size_bytes) {
        return Err(invalid(format!(
            "size_bytes must be in 0..{MAX_ARTIFACT_BYTES}"
        )));
    }
    if matches!(&value.retention_class, RetentionClass::Unknown(_)) {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "retention_class is unsupported",
        ));
    }
    if matches!(&value.sensitivity, Sensitivity::Unknown(_)) {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "sensitivity is unsupported",
        ));
    }
    Ok(())
}

fn validate_media_type(value: &str) -> Result<(), PlatformCoreContractError> {
    wire::validate_text(value, "media_type", 128, true)?;
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() != 2 || parts.iter().any(|part| part.is_empty()) {
        return Err(invalid(
            "media_type must contain one type/subtype separator",
        ));
    }
    if parts.iter().any(|part| {
        !part.as_bytes()[0].is_ascii_lowercase() && !part.as_bytes()[0].is_ascii_digit()
    }) {
        return Err(invalid(
            "media_type type and subtype must start with lowercase alphanumeric",
        ));
    }
    if parts
        .iter()
        .flat_map(|part| part.bytes())
        .any(|byte| !media_type_byte(byte))
    {
        return Err(invalid("media_type contains an unsupported byte"));
    }
    Ok(())
}

fn media_type_byte(value: u8) -> bool {
    value.is_ascii_lowercase()
        || value.is_ascii_digit()
        || matches!(
            value,
            b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
        )
}
