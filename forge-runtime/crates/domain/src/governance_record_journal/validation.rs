use sha2::{Digest, Sha256};

use crate::governance_contract::{
    GovernanceRecord, MAX_ARRAY_ITEMS, MAX_RECORD_BYTES, decode_canonical_record,
    decode_canonical_record_batch,
};

use super::{
    AppendGovernanceRecordBatch, GOVERNANCE_RECORD_APPEND_REQUEST_DIGEST_DOMAIN,
    GOVERNANCE_RECORD_BATCH_ID_PREFIX, GOVERNANCE_RECORD_JOURNAL_VERSION,
    GOVERNANCE_RECORD_SET_DIGEST_DOMAIN, GovernanceRecordAppendReceipt, GovernanceRecordInspection,
    GovernanceRecordJournalError, GovernanceRecordKind, GovernanceRecordListFilter,
    GovernanceRecordMetadata, GovernanceStructuralHead,
    MAX_GOVERNANCE_RECORD_IDEMPOTENCY_KEY_BYTES, MAX_GOVERNANCE_RECORD_IDENTIFIER_BYTES,
    MAX_GOVERNANCE_RECORD_LIST_LIMIT, invalid,
};

pub(crate) fn build_append_request(
    canonical_record_set_json: String,
    idempotency_key: String,
    appended_at_ms: u64,
) -> Result<AppendGovernanceRecordBatch, GovernanceRecordJournalError> {
    validate_key_and_time(&idempotency_key, appended_at_ms)?;
    decode_batch(canonical_record_set_json.as_bytes())?;
    let record_set_sha256 = governance_record_set_sha256(canonical_record_set_json.as_bytes());
    let request_sha256 = governance_record_append_request_sha256(
        &idempotency_key,
        canonical_record_set_json.as_bytes(),
    );
    Ok(AppendGovernanceRecordBatch {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: format!("{GOVERNANCE_RECORD_BATCH_ID_PREFIX}{request_sha256}"),
        request_sha256,
        record_set_sha256,
        canonical_record_set_json,
        idempotency_key,
        appended_at_ms,
    })
}

pub(crate) fn validate_request(
    request: &AppendGovernanceRecordBatch,
) -> Result<(), GovernanceRecordJournalError> {
    if request.v != GOVERNANCE_RECORD_JOURNAL_VERSION {
        return Err(invalid("append request version is unsupported"));
    }
    validate_key_and_time(&request.idempotency_key, request.appended_at_ms)?;
    decode_batch(request.canonical_record_set_json.as_bytes())?;
    let set_digest = governance_record_set_sha256(request.canonical_record_set_json.as_bytes());
    let request_digest = governance_record_append_request_sha256(
        &request.idempotency_key,
        request.canonical_record_set_json.as_bytes(),
    );
    let valid = request.record_set_sha256 == set_digest
        && request.request_sha256 == request_digest
        && request.batch_id == format!("{GOVERNANCE_RECORD_BATCH_ID_PREFIX}{request_digest}");
    valid
        .then_some(())
        .ok_or_else(|| invalid("append request deterministic identity diverged"))
}

pub(crate) fn request_records(
    request: &AppendGovernanceRecordBatch,
) -> Result<Vec<GovernanceRecord>, GovernanceRecordJournalError> {
    validate_request(request)?;
    decode_batch(request.canonical_record_set_json.as_bytes())
}

fn decode_batch(bytes: &[u8]) -> Result<Vec<GovernanceRecord>, GovernanceRecordJournalError> {
    decode_canonical_record_batch(bytes).map_err(|error| invalid(error.message))
}

#[must_use]
pub fn governance_record_set_sha256(bytes: &[u8]) -> String {
    digest_hex(GOVERNANCE_RECORD_SET_DIGEST_DOMAIN, &[bytes])
}

#[must_use]
pub fn governance_record_append_request_sha256(idempotency_key: &str, record_set: &[u8]) -> String {
    let key_length = u64::try_from(idempotency_key.len()).unwrap_or(u64::MAX);
    let set_length = u64::try_from(record_set.len()).unwrap_or(u64::MAX);
    digest_hex(
        GOVERNANCE_RECORD_APPEND_REQUEST_DIGEST_DOMAIN,
        &[
            &key_length.to_be_bytes(),
            idempotency_key.as_bytes(),
            &set_length.to_be_bytes(),
            record_set,
        ],
    )
}

fn digest_hex(domain: &[u8], parts: &[&[u8]]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for part in parts {
        digest.update(part);
    }
    lower_hex(&digest.finalize())
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn validate_key_and_time(
    key: &str,
    appended_at_ms: u64,
) -> Result<(), GovernanceRecordJournalError> {
    validate_governance_record_idempotency_key(key)?;
    if i64::try_from(appended_at_ms).is_err() {
        return Err(invalid("append timestamp is invalid"));
    }
    Ok(())
}

/// Validates the bounded caller-owned journal replay key without reading a record set.
///
/// # Errors
///
/// Returns an error for blank, oversized, control-bearing, or bidi-bearing keys.
pub fn validate_governance_record_idempotency_key(
    key: &str,
) -> Result<(), GovernanceRecordJournalError> {
    if valid_text(key, MAX_GOVERNANCE_RECORD_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(invalid("idempotency key is invalid"))
    }
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && !value.trim().is_empty()
        && !value
            .chars()
            .any(|value| value.is_control() || is_bidi(value))
}

#[must_use]
pub fn is_governance_record_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_GOVERNANCE_RECORD_IDENTIFIER_BYTES
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

fn is_bidi(value: char) -> bool {
    matches!(
        value,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn valid_batch_id(value: &str) -> bool {
    value
        .strip_prefix(GOVERNANCE_RECORD_BATCH_ID_PREFIX)
        .is_some_and(lower_sha256)
}

pub(crate) fn validate_receipt(
    receipt: &GovernanceRecordAppendReceipt,
) -> Result<(), GovernanceRecordJournalError> {
    let ids_are_sorted = receipt
        .record_ids
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes());
    let valid = receipt.v == GOVERNANCE_RECORD_JOURNAL_VERSION
        && receipt.batch_id
            == format!(
                "{GOVERNANCE_RECORD_BATCH_ID_PREFIX}{}",
                receipt.request_sha256
            )
        && lower_sha256(&receipt.request_sha256)
        && lower_sha256(&receipt.record_set_sha256)
        && receipt.record_count == receipt.record_ids.len()
        && (1..=MAX_ARRAY_ITEMS).contains(&receipt.record_count)
        && receipt
            .record_ids
            .iter()
            .all(|id| is_governance_record_identifier(id))
        && ids_are_sorted
        && i64::try_from(receipt.appended_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("append receipt is invalid"))
}

pub(crate) fn validate_metadata(
    metadata: &GovernanceRecordMetadata,
) -> Result<(), GovernanceRecordJournalError> {
    let valid = metadata.v == GOVERNANCE_RECORD_JOURNAL_VERSION
        && valid_batch_id(&metadata.batch_id)
        && metadata.batch_ordinal < MAX_ARRAY_ITEMS
        && is_governance_record_identifier(&metadata.record_id)
        && is_governance_record_identifier(&metadata.aggregate_id)
        && metadata.sequence > 0
        && lower_sha256(&metadata.canonical_sha256)
        && (1..=MAX_RECORD_BYTES).contains(&metadata.canonical_record_bytes)
        && metadata.created_at_unix_ms >= 0
        && i64::try_from(metadata.appended_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("journal record metadata is invalid"))
}

pub(crate) fn validate_inspection(
    inspection: &GovernanceRecordInspection,
) -> Result<(), GovernanceRecordJournalError> {
    if inspection.v != GOVERNANCE_RECORD_JOURNAL_VERSION {
        return Err(invalid("journal inspection version is unsupported"));
    }
    inspection.metadata.validate()?;
    let Some(canonical) = &inspection.canonical_record_json else {
        return Ok(());
    };
    let record =
        decode_canonical_record(canonical.as_bytes()).map_err(|error| invalid(error.message))?;
    let metadata = record.metadata();
    let expected_kind = GovernanceRecordKind::from(&record);
    let valid = canonical.len() == inspection.metadata.canonical_record_bytes
        && expected_kind == inspection.metadata.record_kind
        && metadata.record_id == inspection.metadata.record_id
        && metadata.aggregate_id == inspection.metadata.aggregate_id
        && metadata.sequence == inspection.metadata.sequence
        && metadata.created_at_unix_ms == inspection.metadata.created_at_unix_ms
        && record.integrity().canonical_sha256 == inspection.metadata.canonical_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("revealed record diverges from journal metadata"))
}

pub(crate) fn validate_head(
    head: &GovernanceStructuralHead,
) -> Result<(), GovernanceRecordJournalError> {
    let valid = head.v == GOVERNANCE_RECORD_JOURNAL_VERSION
        && is_governance_record_identifier(&head.aggregate_id)
        && is_governance_record_identifier(&head.record_id)
        && head.sequence > 0
        && lower_sha256(&head.canonical_sha256)
        && i64::try_from(head.updated_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("structural head is invalid"))
}

pub(crate) fn validate_filter(
    filter: &GovernanceRecordListFilter,
) -> Result<(), GovernanceRecordJournalError> {
    let aggregate_valid = filter
        .aggregate_id
        .as_deref()
        .is_none_or(is_governance_record_identifier);
    if !(1..=MAX_GOVERNANCE_RECORD_LIST_LIMIT).contains(&filter.limit) || !aggregate_valid {
        return Err(invalid("journal list filter is invalid"));
    }
    Ok(())
}

fn lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
