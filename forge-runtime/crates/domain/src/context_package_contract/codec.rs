use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    CONTEXT_DIGEST_DOMAIN, ContextPackage, ContextPackageBuildRequest, ContextPackageContractError,
    MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_OBJECT_FIELDS, MAX_PACKAGE_BYTES, MAX_REQUEST_BYTES,
    SNIPPET_DIGEST_DOMAIN, invalid,
};

/// Decodes and validates exact compact canonical build-request JSON.
///
/// # Errors
///
/// Returns an error for oversized, duplicate, unknown, non-canonical, or invalid input.
pub fn decode_canonical_request(
    bytes: &[u8],
) -> Result<ContextPackageBuildRequest, ContextPackageContractError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(invalid("build request exceeds the canonical byte limit"));
    }
    let request: ContextPackageBuildRequest = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("build request is invalid JSON: {error}")))?;
    super::validation::validate_request(&request)?;
    require_exact(bytes, &canonical_request_json(&request)?, "build request")?;
    Ok(request)
}

/// Encodes one validated build request as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when validation or canonical resource limits fail.
pub fn canonical_request_json(
    request: &ContextPackageBuildRequest,
) -> Result<String, ContextPackageContractError> {
    super::validation::validate_request(request)?;
    encode_bounded(request, MAX_REQUEST_BYTES, "build request")
}

/// Decodes exact compact canonical package JSON without trusting its derived fields.
///
/// Call [`super::validate_package`] with the original request and tokenizer before use.
///
/// # Errors
///
/// Returns an error for oversized, duplicate, unknown, non-canonical, or malformed input.
pub fn decode_canonical_package(
    bytes: &[u8],
) -> Result<ContextPackage, ContextPackageContractError> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err(invalid("context package exceeds the canonical byte limit"));
    }
    let package: ContextPackage = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("context package is invalid JSON: {error}")))?;
    super::package_validation::validate_package_shape(&package)?;
    require_exact(bytes, &canonical_package_json(&package)?, "context package")?;
    Ok(package)
}

/// Encodes one package as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when canonical resource limits fail.
pub fn canonical_package_json(
    package: &ContextPackage,
) -> Result<String, ContextPackageContractError> {
    encode_bounded(package, MAX_PACKAGE_BYTES, "context package")
}

/// Computes the domain-separated identity of a validated build request.
///
/// # Errors
///
/// Returns an error when the request is invalid or cannot be canonically encoded.
pub fn request_sha256(
    request: &ContextPackageBuildRequest,
) -> Result<String, ContextPackageContractError> {
    let canonical = canonical_request_json(request)?;
    Ok(domain_sha256(
        super::REQUEST_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

/// Computes the deterministic cache key over the exact canonical build request.
///
/// # Errors
///
/// Returns an error when the request is invalid or cannot be canonically encoded.
pub fn cache_key_sha256(
    request: &ContextPackageBuildRequest,
) -> Result<String, ContextPackageContractError> {
    let canonical = canonical_request_json(request)?;
    Ok(domain_sha256(
        super::CACHE_KEY_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

pub(super) fn snippet_sha256(
    snippet: &super::ContextSnippet,
) -> Result<String, ContextPackageContractError> {
    let mut payload = snippet.clone();
    payload.snippet_sha256.clear();
    let canonical = canonical_json(&payload)?;
    Ok(domain_sha256(SNIPPET_DIGEST_DOMAIN, canonical.as_bytes()))
}

pub(super) fn context_sha256(
    package: &ContextPackage,
) -> Result<String, ContextPackageContractError> {
    let mut payload = package.clone();
    payload.context_sha256.clear();
    let canonical = canonical_package_json(&payload)?;
    Ok(domain_sha256(CONTEXT_DIGEST_DOMAIN, canonical.as_bytes()))
}

pub(super) fn domain_sha256(domain: &[u8], payload: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(payload);
    lower_hex(&digest.finalize())
}

pub(super) fn raw_sha256(payload: &[u8]) -> String {
    lower_hex(&Sha256::digest(payload))
}

pub(super) fn canonical_json(
    value: &(impl Serialize + ?Sized),
) -> Result<String, ContextPackageContractError> {
    let value = serde_json::to_value(value)
        .map_err(|error| invalid(format!("value cannot be encoded as JSON: {error}")))?;
    let mut output = String::new();
    write_value(&value, 1, &mut output)?;
    Ok(output)
}

fn encode_bounded(
    value: &(impl Serialize + ?Sized),
    limit: usize,
    kind: &str,
) -> Result<String, ContextPackageContractError> {
    let canonical = canonical_json(value)?;
    if canonical.len() > limit {
        return Err(invalid(format!("{kind} exceeds the canonical byte limit")));
    }
    Ok(canonical)
}

fn write_value(
    value: &Value,
    depth: usize,
    output: &mut String,
) -> Result<(), ContextPackageContractError> {
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
) -> Result<(), ContextPackageContractError> {
    if let Some(integer) = value.as_i64() {
        output.push_str(&integer.to_string());
    } else if let Some(integer) = value.as_u64().and_then(|value| i64::try_from(value).ok()) {
        output.push_str(&integer.to_string());
    } else {
        return Err(invalid("canonical JSON permits integers only"));
    }
    Ok(())
}

fn write_string(value: &str, output: &mut String) -> Result<(), ContextPackageContractError> {
    if value.len() > super::MAX_STRING_BYTES {
        return Err(invalid("canonical JSON string exceeds the byte limit"));
    }
    if value
        .chars()
        .any(super::validation::forbidden_content_scalar)
    {
        return Err(invalid(
            "canonical JSON string contains a forbidden Unicode scalar",
        ));
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
) -> Result<(), ContextPackageContractError> {
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
) -> Result<(), ContextPackageContractError> {
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

fn require_exact(
    input: &[u8],
    canonical: &str,
    kind: &str,
) -> Result<(), ContextPackageContractError> {
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
