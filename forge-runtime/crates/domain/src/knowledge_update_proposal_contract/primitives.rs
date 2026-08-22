use serde::Serialize;

use crate::capability_grant_contract::{GrantTaskBinding, Principal};

use super::{
    KnowledgeUpdateProposalContractError, MAX_REFERENCE_TEXT_BYTES, MAX_SHORT_TEXT_BYTES,
    canonical, invalid,
};

pub(super) fn text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(forbidden_scalar) {
        Err(invalid(format!("{label} violates the text contract")))
    } else {
        Ok(())
    }
}

pub(super) fn short_text(
    value: &str,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    text(value, MAX_SHORT_TEXT_BYTES, label)
}

pub(super) fn reference_text(
    value: &str,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    text(value, MAX_REFERENCE_TEXT_BYTES, label)
}

pub(super) fn identifier(
    value: &str,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    short_text(value, label)?;
    let valid = value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!("{label} is not an ADR-0045 identifier")))
    }
}

pub(super) fn sha256(value: &str, label: &str) -> Result<(), KnowledgeUpdateProposalContractError> {
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    valid
        .then_some(())
        .ok_or_else(|| invalid(format!("{label} is not lowercase SHA-256 hex")))
}

pub(super) fn optional_sha256(
    value: Option<&str>,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    value.map_or(Ok(()), |digest| sha256(digest, label))
}

pub(super) fn principal(
    value: &Principal,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    short_text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
    )?;
    short_text(&value.principal_id, &format!("{label}.principal_id"))
}

pub(super) fn task_binding(
    value: &GrantTaskBinding,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    for (field, value) in [
        ("change_id", value.change_id.as_str()),
        ("environment_id", value.environment_id.as_str()),
        ("node_id", value.node_id.as_str()),
        ("project_id", value.project_id.as_str()),
        ("role", value.role.as_str()),
        ("run_id", value.run_id.as_str()),
        ("task_id", value.task_id.as_str()),
    ] {
        short_text(value, &format!("{label}.{field}"))?;
    }
    optional_short(value.attempt_id.as_deref(), &format!("{label}.attempt_id"))?;
    optional_short(value.target_id.as_deref(), &format!("{label}.target_id"))
}

fn optional_short(
    value: Option<&str>,
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    value.map_or(Ok(()), |text| short_text(text, label))
}

pub(super) fn sorted_nodes<T: Serialize>(
    values: &[T],
    label: &str,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let encoded = values
        .iter()
        .map(|value| canonical::encode(value, super::MAX_PROPOSAL_BYTES, label))
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
) -> Result<(), KnowledgeUpdateProposalContractError> {
    if values.is_empty() || values.len() > maximum {
        return Err(invalid(format!("{label} must contain 1..{maximum} values")));
    }
    for value in values {
        identifier(value, label)?;
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
