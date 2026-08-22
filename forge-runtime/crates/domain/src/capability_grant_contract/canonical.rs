use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    CapabilityGrantContractError, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_OBJECT_FIELDS, MAX_STRING_BYTES,
    invalid,
};

pub(super) fn encode(
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<String, CapabilityGrantContractError> {
    let value = serde_json::to_value(value)
        .map_err(|error| invalid(format!("{label} cannot be encoded as JSON: {error}")))?;
    let mut output = String::new();
    write_value(&value, 1, &mut output)?;
    if output.len() > maximum {
        return Err(invalid(format!("{label} exceeds the canonical byte limit")));
    }
    Ok(output)
}

pub(super) fn require_exact(
    input: &[u8],
    canonical: &str,
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    if input == canonical.as_bytes() {
        Ok(())
    } else {
        Err(invalid(format!(
            "input is not exact compact canonical JSON for {label}"
        )))
    }
}

pub(super) fn domain_sha256(domain: &[u8], payload: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(payload);
    lower_hex(&digest.finalize())
}

fn write_value(
    value: &Value,
    depth: usize,
    output: &mut String,
) -> Result<(), CapabilityGrantContractError> {
    if depth > MAX_DEPTH {
        return Err(invalid("canonical JSON exceeds the depth limit"));
    }
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => write_integer(value, output)?,
        Value::String(value) => write_string(value, output)?,
        Value::Array(values) => write_array(values, depth, output)?,
        Value::Object(values) => write_object(values, depth, output)?,
    }
    Ok(())
}

fn write_integer(
    value: &serde_json::Number,
    output: &mut String,
) -> Result<(), CapabilityGrantContractError> {
    if let Some(integer) = value.as_i64() {
        output.push_str(&integer.to_string());
        Ok(())
    } else {
        Err(invalid("canonical JSON permits signed int64 values only"))
    }
}

fn write_string(value: &str, output: &mut String) -> Result<(), CapabilityGrantContractError> {
    if value.len() > MAX_STRING_BYTES || value.chars().any(forbidden_scalar) {
        return Err(invalid("canonical JSON string violates text limits"));
    }
    let encoded = serde_json::to_string(value)
        .map_err(|error| invalid(format!("string cannot be encoded as JSON: {error}")))?;
    output.push_str(&encoded);
    Ok(())
}

fn write_array(
    values: &[Value],
    depth: usize,
    output: &mut String,
) -> Result<(), CapabilityGrantContractError> {
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
) -> Result<(), CapabilityGrantContractError> {
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

fn ascii_snake_case(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
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

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
