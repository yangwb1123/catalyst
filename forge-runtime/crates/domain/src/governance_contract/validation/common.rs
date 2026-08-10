use super::super::{
    CANONICALIZATION, GovernanceContractError, Integrity, Principal, RecordMetadata, invalid,
};

const MAX_IDENTIFIER_BYTES: usize = 160;

pub(super) fn validate_metadata(metadata: &RecordMetadata) -> Result<(), GovernanceContractError> {
    let identifiers = [
        metadata.aggregate_id.as_str(),
        metadata.project_id.as_str(),
        metadata.record_id.as_str(),
        metadata.scope.as_str(),
        metadata.source_revision.as_str(),
    ];
    if !identifiers.into_iter().all(is_identifier)
        || metadata.created_at_unix_ms < 0
        || metadata.sequence < 1
        || !is_digest(&metadata.context_sha256)
        || !is_digest(&metadata.policy_sha256)
        || !is_digest(&metadata.source_tree_sha256)
        || !sorted_unique_identifiers(&metadata.supersedes_record_ids)
    {
        return Err(invalid("record metadata is invalid"));
    }
    validate_principal(&metadata.created_by)
}

pub(super) fn validate_integrity(integrity: &Integrity) -> Result<(), GovernanceContractError> {
    if integrity.canonicalization == CANONICALIZATION && is_digest(&integrity.canonical_sha256) {
        Ok(())
    } else {
        Err(invalid("record integrity is invalid"))
    }
}

pub(super) fn validate_principal(principal: &Principal) -> Result<(), GovernanceContractError> {
    let identifiers = [
        principal.authority_domain.as_str(),
        principal.principal_id.as_str(),
        principal.role.as_str(),
        principal.run_id.as_str(),
    ];
    if identifiers.into_iter().all(is_identifier) {
        Ok(())
    } else {
        Err(invalid("declared principal metadata is invalid"))
    }
}

pub(super) fn validate_interval(
    from: i64,
    until: Option<i64>,
) -> Result<(), GovernanceContractError> {
    if from < 0 || until.is_some_and(|value| value <= from) {
        Err(invalid("validity interval is invalid"))
    } else {
        Ok(())
    }
}

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn is_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_IDENTIFIER_BYTES
        && bytes[0].is_ascii_lowercase_or_digit()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase_or_digit() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

pub(super) fn nonempty_text(value: &str, max_scalars: usize) -> bool {
    let scalar_count = value.chars().count();
    scalar_count > 0 && scalar_count <= max_scalars
}

pub(super) fn sorted_unique_identifiers(values: &[String]) -> bool {
    values.iter().all(|value| is_identifier(value))
        && values
            .windows(2)
            .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
}

pub(super) fn disjoint_sorted(left: &[String], right: &[String]) -> bool {
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => return false,
        }
    }
    true
}

trait AsciiIdentifierByte {
    fn is_ascii_lowercase_or_digit(&self) -> bool;
}

impl AsciiIdentifierByte for u8 {
    fn is_ascii_lowercase_or_digit(&self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}
