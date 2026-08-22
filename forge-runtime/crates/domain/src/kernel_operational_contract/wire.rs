use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{
    KernelOperationalContractError, MAX_ARRAY_ITEMS, MAX_CLOSURE_BYTES, MAX_JSON_DEPTH,
    MAX_OBJECT_FIELDS, MAX_STRING_BYTES, invalid,
};

pub(super) fn decode_typed<T>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, KernelOperationalContractError>
where
    T: DeserializeOwned + Serialize,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!("JSON byte length must be 1..={maximum}")));
    }
    let typed: T = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid typed JSON: {error}")))?;
    let parsed: Value =
        serde_json::from_slice(bytes).map_err(|error| invalid(format!("invalid JSON: {error}")))?;
    validate_json_value(&parsed, 1)?;
    let canonical = canonical_typed_with_max(&typed, maximum)?;
    if canonical.as_bytes() == bytes {
        Ok(typed)
    } else {
        Err(invalid("input is not exact compact canonical JSON"))
    }
}

pub(super) fn canonical_typed_with_max<T>(
    value: &T,
    maximum: usize,
) -> Result<String, KernelOperationalContractError>
where
    T: Serialize + ?Sized,
{
    let node = serde_json::to_value(value)
        .map_err(|error| invalid(format!("value cannot be represented as JSON: {error}")))?;
    validate_json_value(&node, 1)?;
    let canonical = crate::governance_contract::codec::canonical_json(&node)
        .map_err(|error| invalid(format!("canonical JSON failed: {}", error.message)))?;
    if canonical.len() > maximum {
        Err(invalid(format!("canonical JSON exceeds {maximum} bytes")))
    } else {
        Ok(canonical)
    }
}

pub(super) fn canonical_typed<T>(value: &T) -> Result<String, KernelOperationalContractError>
where
    T: Serialize + ?Sized,
{
    canonical_typed_with_max(value, MAX_CLOSURE_BYTES)
}

fn validate_json_value(value: &Value, depth: usize) -> Result<(), KernelOperationalContractError> {
    if depth > MAX_JSON_DEPTH {
        return Err(invalid(format!("JSON depth exceeds {MAX_JSON_DEPTH}")));
    }
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) if number.as_i64().is_some() => Ok(()),
        Value::Number(_) => Err(invalid("JSON numbers must be signed int64 integers")),
        Value::String(text) => validate_wire_string(text),
        Value::Array(items) => validate_array(items, depth),
        Value::Object(object) => validate_object(object, depth),
    }
}

fn validate_array(items: &[Value], depth: usize) -> Result<(), KernelOperationalContractError> {
    if items.len() > MAX_ARRAY_ITEMS {
        return Err(invalid(format!(
            "JSON array exceeds {MAX_ARRAY_ITEMS} items"
        )));
    }
    for item in items {
        validate_json_value(item, depth + 1)?;
    }
    Ok(())
}

fn validate_object(
    object: &serde_json::Map<String, Value>,
    depth: usize,
) -> Result<(), KernelOperationalContractError> {
    if object.len() > MAX_OBJECT_FIELDS {
        return Err(invalid(format!(
            "JSON object exceeds {MAX_OBJECT_FIELDS} fields"
        )));
    }
    for (key, child) in object {
        if !ascii_snake_case(key) {
            return Err(invalid(format!(
                "object key {key:?} is not ASCII snake_case"
            )));
        }
        validate_wire_string(key)?;
        validate_json_value(child, depth + 1)?;
    }
    Ok(())
}

fn validate_wire_string(value: &str) -> Result<(), KernelOperationalContractError> {
    if value.len() > MAX_STRING_BYTES {
        return Err(invalid(format!(
            "JSON string exceeds {MAX_STRING_BYTES} UTF-8 bytes"
        )));
    }
    if value.chars().any(forbidden_scalar) {
        Err(invalid("JSON string contains a forbidden Unicode scalar"))
    } else {
        Ok(())
    }
}

fn forbidden_scalar(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn ascii_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
