use serde::Serialize;

use crate::capability_grant_contract::{ApprovalRef, GrantTaskBinding, Principal, PrincipalType};

use super::{
    MAX_REFERENCE_TEXT_BYTES, MAX_SHORT_TEXT_BYTES, TransitionReceiptContractError, codec, invalid,
};

pub(super) fn text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(forbidden_scalar) {
        Err(invalid(format!("{label} violates the text contract")))
    } else {
        Ok(())
    }
}

pub(super) fn short_text(value: &str, label: &str) -> Result<(), TransitionReceiptContractError> {
    text(value, MAX_SHORT_TEXT_BYTES, label)
}

pub(super) fn reference_text(
    value: &str,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    text(value, MAX_REFERENCE_TEXT_BYTES, label)
}

pub(super) fn sha256(value: &str, label: &str) -> Result<(), TransitionReceiptContractError> {
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(invalid(format!("{label} is not lowercase SHA-256 hex")))
    }
}

pub(super) fn optional_sha256(
    value: Option<&str>,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    value.map_or(Ok(()), |digest| sha256(digest, label))
}

pub(super) fn stable_reason(
    value: &str,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    short_text(value, label)?;
    let bytes = value.as_bytes();
    let valid = bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!("{label} is not a stable reason code")))
    }
}

pub(super) fn principal(
    value: &Principal,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    short_text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
    )?;
    short_text(&value.principal_id, &format!("{label}.principal_id"))
}

pub(super) fn controller(
    value: &Principal,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    principal(value, label)?;
    if value.principal_type == PrincipalType::Agent {
        Err(invalid(format!(
            "{label} cannot declare an agent controller"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn task_binding(
    value: &GrantTaskBinding,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    for (field, text) in [
        ("change_id", value.change_id.as_str()),
        ("environment_id", value.environment_id.as_str()),
        ("node_id", value.node_id.as_str()),
        ("project_id", value.project_id.as_str()),
        ("role", value.role.as_str()),
        ("run_id", value.run_id.as_str()),
        ("task_id", value.task_id.as_str()),
    ] {
        short_text(text, &format!("{label}.{field}"))?;
    }
    optional_short(value.attempt_id.as_deref(), &format!("{label}.attempt_id"))?;
    optional_short(value.target_id.as_deref(), &format!("{label}.target_id"))
}

fn optional_short(value: Option<&str>, label: &str) -> Result<(), TransitionReceiptContractError> {
    value.map_or(Ok(()), |text| short_text(text, label))
}

pub(super) fn approval_ref(
    value: &ApprovalRef,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    short_text(&value.approval_id, &format!("{label}.approval_id"))?;
    sha256(&value.approval_sha256, &format!("{label}.approval_sha256"))?;
    short_text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
    )
}

pub(super) fn sorted_nodes<T: Serialize>(
    values: &[T],
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    let encoded = values
        .iter()
        .map(codec::canonical_unbounded)
        .collect::<Result<Vec<_>, _>>()?;
    if encoded.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly canonical-byte sorted and unique"
        )))
    }
}

pub(super) fn sorted_reasons(
    values: &[String],
    maximum: usize,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if values.len() > maximum {
        return Err(invalid(format!("{label} exceeds its item limit")));
    }
    for value in values {
        stable_reason(value, label)?;
    }
    if values.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly UTF-8 sorted and unique"
        )))
    }
}

fn forbidden_scalar(value: char) -> bool {
    matches!(value, '\u{0000}'..='\u{001f}' | '\u{007f}')
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}
