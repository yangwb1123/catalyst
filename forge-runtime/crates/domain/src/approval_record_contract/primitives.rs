use crate::capability_grant_contract::{AuthorityClass, Principal, PrincipalType};

use super::{ApprovalRecordContractError, MAX_PROOF_TEXT_BYTES, MAX_SHORT_TEXT_BYTES, invalid};

pub(super) fn text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(forbidden_scalar) {
        Err(invalid(format!("{label} violates the text contract")))
    } else {
        Ok(())
    }
}

pub(super) fn short_text(value: &str, label: &str) -> Result<(), ApprovalRecordContractError> {
    text(value, MAX_SHORT_TEXT_BYTES, label)
}

pub(super) fn sha256(value: &str, label: &str) -> Result<(), ApprovalRecordContractError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(format!("{label} is not lowercase SHA-256 hex")))
    }
}

pub(super) fn stable_identifier(
    value: &str,
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    short_text(value, label)?;
    let bytes = value.as_bytes();
    if bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        Ok(())
    } else {
        Err(invalid(format!("{label} is not a stable identifier")))
    }
}

pub(super) fn nonnegative(value: i64, label: &str) -> Result<(), ApprovalRecordContractError> {
    if value >= 0 {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be nonnegative")))
    }
}

pub(super) fn principal(value: &Principal, label: &str) -> Result<(), ApprovalRecordContractError> {
    short_text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
    )?;
    short_text(&value.principal_id, &format!("{label}.principal_id"))
}

pub(super) fn approver(value: &Principal, label: &str) -> Result<(), ApprovalRecordContractError> {
    principal(value, label)?;
    if matches!(
        value.principal_type,
        PrincipalType::Human | PrincipalType::Operator
    ) {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be a human or operator")))
    }
}

pub(super) fn authority_source(
    class: AuthorityClass,
    principal_type: PrincipalType,
) -> Result<(), ApprovalRecordContractError> {
    let allowed = match class {
        AuthorityClass::ExternalOperator => {
            matches!(
                principal_type,
                PrincipalType::Human | PrincipalType::Operator
            )
        }
        AuthorityClass::ForgeosKernel => principal_type == PrincipalType::Service,
    };
    if allowed {
        Ok(())
    } else {
        Err(invalid(
            "authority_source principal_type contradicts authority_class",
        ))
    }
}

pub(super) fn base64url(value: &str, label: &str) -> Result<(), ApprovalRecordContractError> {
    let valid = value.len() >= 16
        && value.len() <= MAX_PROOF_TEXT_BYTES
        && value.len() % 4 != 1
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        && canonical_base64url_tail(value);
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} is not canonical unpadded base64url"
        )))
    }
}

pub(super) fn strictly_sorted<T: serde::Serialize>(
    values: &[T],
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    let encoded = values
        .iter()
        .map(super::codec::canonical_unbounded)
        .collect::<Result<Vec<_>, _>>()?;
    if encoded
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly canonical-byte sorted and unique"
        )))
    }
}

pub(super) fn strictly_sorted_strings(
    values: &[String],
    label: &str,
) -> Result<(), ApprovalRecordContractError> {
    if values
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly UTF-8 sorted and unique"
        )))
    }
}

fn canonical_base64url_tail(value: &str) -> bool {
    let Some(last) = value.bytes().last().and_then(base64url_value) else {
        return false;
    };
    match value.len() % 4 {
        2 => last.trailing_zeros() >= 4,
        3 => last.trailing_zeros() >= 2,
        _ => true,
    }
}

fn base64url_value(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
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
