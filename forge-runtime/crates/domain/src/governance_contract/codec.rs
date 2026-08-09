use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    GovernanceContractError, GovernanceRecord, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_OBJECT_FIELDS,
    MAX_RECORD_BYTES, MAX_RECORD_SET_BYTES, MAX_STRING_BYTES, invalid,
};

pub(super) fn canonical_payload_json(
    record: &GovernanceRecord,
) -> Result<String, GovernanceContractError> {
    let mut payload = record.clone();
    payload.integrity_mut().canonical_sha256.clear();
    let canonical = canonical_record_json(&payload)?;
    if canonical.len() + 64 > MAX_RECORD_BYTES {
        return Err(invalid("sealed record exceeds the canonical byte limit"));
    }
    Ok(canonical)
}

pub(super) fn canonical_record_json(
    record: &GovernanceRecord,
) -> Result<String, GovernanceContractError> {
    let canonical = canonical_json(record)?;
    if canonical.len() > MAX_RECORD_BYTES {
        return Err(invalid("record exceeds the canonical byte limit"));
    }
    Ok(canonical)
}

pub(super) fn expected_sha256(
    record: &GovernanceRecord,
) -> Result<String, GovernanceContractError> {
    let payload = canonical_payload_json(record)?;
    let mut digest = Sha256::new();
    digest.update(record.digest_domain());
    digest.update(payload.as_bytes());
    Ok(lower_hex(&digest.finalize()))
}

pub(super) fn decode_canonical_record(
    bytes: &[u8],
) -> Result<GovernanceRecord, GovernanceContractError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(invalid("record exceeds the canonical byte limit"));
    }
    let record: GovernanceRecord = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("record is invalid JSON: {error}")))?;
    record.validate()?;
    require_exact(bytes, &canonical_record_json(&record)?, "record")?;
    Ok(record)
}

pub(super) fn decode_canonical_record_set(
    bytes: &[u8],
) -> Result<Vec<GovernanceRecord>, GovernanceContractError> {
    if bytes.len() > MAX_RECORD_SET_BYTES {
        return Err(invalid("record set exceeds the canonical byte limit"));
    }
    let records: Vec<GovernanceRecord> = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("record set is invalid JSON: {error}")))?;
    let canonical = canonical_record_set_json(&records)?;
    require_exact(bytes, &canonical, "record set")?;
    super::validate_record_set(&records)?;
    Ok(records)
}

pub(super) fn canonical_record_set_json(
    records: &[GovernanceRecord],
) -> Result<String, GovernanceContractError> {
    let canonical = canonical_json(records)?;
    if canonical.len() > MAX_RECORD_SET_BYTES {
        return Err(invalid("record set exceeds the canonical byte limit"));
    }
    Ok(canonical)
}

fn canonical_json(value: &(impl Serialize + ?Sized)) -> Result<String, GovernanceContractError> {
    let value = serde_json::to_value(value)
        .map_err(|error| invalid(format!("value cannot be encoded as JSON: {error}")))?;
    let mut output = String::new();
    write_value(&value, 1, &mut output)?;
    Ok(output)
}

fn write_value(
    value: &Value,
    depth: usize,
    output: &mut String,
) -> Result<(), GovernanceContractError> {
    if depth > MAX_DEPTH {
        return Err(invalid("canonical JSON exceeds the depth limit"));
    }
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => write_number(value, output)?,
        Value::String(value) => write_string(value, output)?,
        Value::Array(values) => write_array(values, depth, output)?,
        Value::Object(values) => write_object(values, depth, output)?,
    }
    Ok(())
}

fn write_number(
    value: &serde_json::Number,
    output: &mut String,
) -> Result<(), GovernanceContractError> {
    let integer = value
        .as_i64()
        .ok_or_else(|| invalid("canonical JSON permits signed int64 integers only"))?;
    output.push_str(&integer.to_string());
    Ok(())
}

fn write_string(value: &str, output: &mut String) -> Result<(), GovernanceContractError> {
    validate_string(value)?;
    let encoded = serde_json::to_string(value)
        .map_err(|error| invalid(format!("string cannot be encoded as JSON: {error}")))?;
    output.push_str(&encoded);
    Ok(())
}

fn write_array(
    values: &[Value],
    depth: usize,
    output: &mut String,
) -> Result<(), GovernanceContractError> {
    if values.len() > MAX_ARRAY_ITEMS {
        return Err(invalid("canonical JSON array exceeds the item limit"));
    }
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write_value(value, depth + 1, output)?;
    }
    output.push(']');
    Ok(())
}

fn write_object(
    values: &serde_json::Map<String, Value>,
    depth: usize,
    output: &mut String,
) -> Result<(), GovernanceContractError> {
    if values.len() > MAX_OBJECT_FIELDS {
        return Err(invalid("canonical JSON object exceeds the field limit"));
    }
    let mut entries: Vec<_> = values.iter().collect();
    entries.sort_unstable_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    output.push('{');
    for (index, (key, value)) in entries.into_iter().enumerate() {
        if !ascii_snake_case(key) {
            return Err(invalid("canonical JSON object key is not ASCII snake_case"));
        }
        if index > 0 {
            output.push(',');
        }
        write_string(key, output)?;
        output.push(':');
        write_value(value, depth + 1, output)?;
    }
    output.push('}');
    Ok(())
}

fn validate_string(value: &str) -> Result<(), GovernanceContractError> {
    if value.len() > MAX_STRING_BYTES {
        return Err(invalid("canonical JSON string exceeds the byte limit"));
    }
    if value.chars().any(forbidden_scalar) {
        return Err(invalid(
            "canonical JSON string contains a forbidden Unicode scalar",
        ));
    }
    Ok(())
}

fn forbidden_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{0000}'..='\u{001f}'
            | '\u{007f}'
            | '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn ascii_snake_case(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

fn require_exact(input: &[u8], canonical: &str, kind: &str) -> Result<(), GovernanceContractError> {
    if input == canonical.as_bytes() {
        Ok(())
    } else {
        Err(invalid(format!(
            "input is not exact compact canonical JSON for {kind}"
        )))
    }
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
