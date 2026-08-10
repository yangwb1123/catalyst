use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    ArtifactEvidenceContractError, ArtifactEvidenceRequest, ArtifactProvenanceRecord,
    MAX_REQUEST_BYTES, REQUEST_DIGEST_DOMAIN, SOURCE_DIGEST_DOMAIN, invalid,
};

const MAX_DEPTH: usize = 16;
const MAX_OBJECT_FIELDS: usize = 64;
const MAX_ARRAY_ITEMS: usize = 256;
const MAX_STRING_BYTES: usize = 16_384;

/// Decodes exact compact canonical request bytes and validates all source bindings.
///
/// # Errors
///
/// Returns an error for malformed, duplicate, unknown, noncanonical, oversized,
/// or semantically invalid input.
pub fn decode_canonical_request(
    bytes: &[u8],
) -> Result<ArtifactEvidenceRequest, ArtifactEvidenceContractError> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(invalid("request byte length is outside the v1 limit"));
    }
    let request: ArtifactEvidenceRequest = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("request is invalid JSON: {error}")))?;
    super::validation::validate_request(&request)?;
    let canonical = canonical_request_json(&request)?;
    if canonical.as_bytes() != bytes {
        return Err(invalid(
            "input is not exact compact canonical JSON for artifact evidence request",
        ));
    }
    Ok(request)
}

/// Encodes one request with the adapter-local `_format` compatibility rule.
///
/// # Errors
///
/// Returns an error for invalid source semantics or canonical JSON limits.
pub fn canonical_request_json(
    request: &ArtifactEvidenceRequest,
) -> Result<String, ArtifactEvidenceContractError> {
    super::validation::validate_request(request)?;
    canonical_json(request, "")
}

/// Encodes only the exact legacy artifact source object.
///
/// # Errors
///
/// Returns an error for invalid source semantics or canonical JSON limits.
pub fn canonical_artifact_json(
    artifact: &ArtifactProvenanceRecord,
) -> Result<String, ArtifactEvidenceContractError> {
    super::validation::validate_artifact(artifact)?;
    canonical_json(artifact, "artifact")
}

/// Computes the request identity over exact canonical request bytes.
///
/// # Errors
///
/// Returns an error when request validation or canonical encoding fails.
pub fn request_sha256(
    request: &ArtifactEvidenceRequest,
) -> Result<String, ArtifactEvidenceContractError> {
    Ok(digest_hex(
        REQUEST_DIGEST_DOMAIN,
        canonical_request_json(request)?.as_bytes(),
    ))
}

/// Computes the source identity over the canonical artifact object only.
///
/// # Errors
///
/// Returns an error when artifact validation or canonical encoding fails.
pub fn artifact_source_sha256(
    artifact: &ArtifactProvenanceRecord,
) -> Result<String, ArtifactEvidenceContractError> {
    Ok(digest_hex(
        SOURCE_DIGEST_DOMAIN,
        canonical_artifact_json(artifact)?.as_bytes(),
    ))
}

fn canonical_json(
    value: &impl Serialize,
    root_path: &str,
) -> Result<String, ArtifactEvidenceContractError> {
    let value = serde_json::to_value(value)
        .map_err(|error| invalid(format!("request cannot be encoded as JSON: {error}")))?;
    let mut output = String::new();
    write_value(&value, 1, root_path, &mut output)?;
    if output.len() > MAX_REQUEST_BYTES {
        return Err(invalid("canonical request exceeds the v1 byte limit"));
    }
    Ok(output)
}

fn write_value(
    value: &Value,
    depth: usize,
    path: &str,
    output: &mut String,
) -> Result<(), ArtifactEvidenceContractError> {
    if depth > MAX_DEPTH {
        return Err(invalid("canonical JSON exceeds the depth limit"));
    }
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => write_number(value, output)?,
        Value::String(value) => write_string(value, output)?,
        Value::Array(values) => write_array(values, depth, path, output)?,
        Value::Object(values) => write_object(values, depth, path, output)?,
    }
    Ok(())
}

fn write_number(
    value: &serde_json::Number,
    output: &mut String,
) -> Result<(), ArtifactEvidenceContractError> {
    let integer = value
        .as_i64()
        .ok_or_else(|| invalid("canonical JSON permits signed int64 integers only"))?;
    output.push_str(&integer.to_string());
    Ok(())
}

fn write_string(value: &str, output: &mut String) -> Result<(), ArtifactEvidenceContractError> {
    validate_string(value)?;
    let encoded = serde_json::to_string(value)
        .map_err(|error| invalid(format!("string cannot be encoded as JSON: {error}")))?;
    output.push_str(&encoded);
    Ok(())
}

fn write_array(
    values: &[Value],
    depth: usize,
    path: &str,
    output: &mut String,
) -> Result<(), ArtifactEvidenceContractError> {
    if values.len() > MAX_ARRAY_ITEMS {
        return Err(invalid("canonical JSON array exceeds the item limit"));
    }
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write_value(value, depth + 1, path, output)?;
    }
    output.push(']');
    Ok(())
}

fn write_object(
    values: &serde_json::Map<String, Value>,
    depth: usize,
    path: &str,
    output: &mut String,
) -> Result<(), ArtifactEvidenceContractError> {
    if values.len() > MAX_OBJECT_FIELDS {
        return Err(invalid("canonical JSON object exceeds the field limit"));
    }
    let mut entries: Vec<_> = values.iter().collect();
    entries.sort_unstable_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    output.push('{');
    for (index, (key, value)) in entries.into_iter().enumerate() {
        write_entry(index, key, value, depth, path, output)?;
    }
    output.push('}');
    Ok(())
}

fn write_entry(
    index: usize,
    key: &str,
    value: &Value,
    depth: usize,
    path: &str,
    output: &mut String,
) -> Result<(), ArtifactEvidenceContractError> {
    if !allowed_key(path, key) {
        return Err(invalid("canonical JSON object key is not allowed"));
    }
    if index > 0 {
        output.push(',');
    }
    write_string(key, output)?;
    output.push(':');
    let child_path = if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    };
    write_value(value, depth + 1, &child_path, output)
}

fn validate_string(value: &str) -> Result<(), ArtifactEvidenceContractError> {
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

fn allowed_key(path: &str, key: &str) -> bool {
    (path == "artifact" && key == "_format") || ascii_snake_case(key)
}

fn ascii_snake_case(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

pub(super) fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    crate::governance_contract::codec::lower_hex(&hasher.finalize())
}
