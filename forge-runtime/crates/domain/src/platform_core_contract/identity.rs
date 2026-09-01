use super::{EntityType, PlatformCoreContractError, RejectionCode, reject};

/// Validates one typed opaque Platform ID without inferring suffix semantics.
///
/// # Errors
/// Returns an error for an unknown type prefix or malformed Crockford suffix.
pub fn validate_platform_id(value: &str) -> Result<(), PlatformCoreContractError> {
    let bytes = value.as_bytes();
    if bytes.len() != 30 || bytes[3] != b'_' {
        return Err(reject(
            RejectionCode::IdentifierInvalid,
            "platform ID must have a three-byte prefix and 26-byte suffix",
        ));
    }
    if !PLATFORM_PREFIXES.contains(&&value[..3]) {
        return Err(reject(
            RejectionCode::IdentifierInvalid,
            format!("platform ID prefix {:?} is unsupported", &value[..3]),
        ));
    }
    validate_suffix(&bytes[4..])
}

pub(super) fn validate_typed_id(
    value: &str,
    prefix: &str,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    validate_platform_id(value)?;
    if value.as_bytes()[..3] != *prefix.as_bytes() {
        return Err(reject(
            RejectionCode::IdentifierInvalid,
            format!("{label} must use {prefix}_ namespace"),
        ));
    }
    Ok(())
}

pub(super) fn validate_entity_id(
    entity_type: &EntityType,
    value: &str,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    let Some(prefix) = entity_prefix(entity_type) else {
        return Err(reject(
            RejectionCode::ValueInvalid,
            format!("{label}.entity_type is unsupported"),
        ));
    };
    validate_typed_id(value, prefix, &format!("{label}.entity_id"))
}

pub(super) fn same_message_suffix(message_id: &str, specialized_id: &str) -> bool {
    message_id.len() == 30
        && specialized_id.len() == 30
        && message_id.as_bytes()[4..] == specialized_id.as_bytes()[4..]
}

fn validate_suffix(value: &[u8]) -> Result<(), PlatformCoreContractError> {
    if value.len() != 26 || !(b'0'..=b'7').contains(&value[0]) {
        return Err(reject(
            RejectionCode::IdentifierInvalid,
            "platform ID suffix is not a 128-bit Crockford value",
        ));
    }
    if value[1..].iter().any(|byte| !is_crockford(*byte)) {
        return Err(reject(
            RejectionCode::IdentifierInvalid,
            "platform ID suffix contains a non-Crockford byte",
        ));
    }
    Ok(())
}

fn is_crockford(value: u8) -> bool {
    value.is_ascii_digit()
        || value.is_ascii_lowercase() && !matches!(value, b'i' | b'l' | b'o' | b'u')
}

fn entity_prefix(value: &EntityType) -> Option<&'static str> {
    match value {
        EntityType::Action => Some("act"),
        EntityType::Actor => Some("acr"),
        EntityType::Artifact => Some("art"),
        EntityType::Attempt => Some("atm"),
        EntityType::Change => Some("chg"),
        EntityType::Objective => Some("obj"),
        EntityType::Project => Some("prj"),
        EntityType::ProjectSnapshot => Some("psn"),
        EntityType::Receipt => Some("rcp"),
        EntityType::Session => Some("ses"),
        EntityType::Space => Some("spc"),
        EntityType::Turn => Some("trn"),
        EntityType::WorkGraph => Some("wgr"),
        EntityType::WorkItem => Some("wki"),
        EntityType::Unknown(_) => None,
    }
}

const PLATFORM_PREFIXES: &[&str] = &[
    "act", "acr", "art", "atm", "chg", "cmd", "cor", "evt", "msg", "obj", "prj", "psn", "rcp",
    "ses", "spc", "trn", "ver", "wgr", "wki",
];
