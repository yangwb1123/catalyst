use serde_json::{Map, Value};

use super::{
    MAX_ARRAY_ITEMS, MAX_JSON_DEPTH, MAX_OBJECT_FIELDS, MAX_RECORD_BYTES, MAX_STRING_BYTES,
    WorkIntent, WorkIntentContractError, invalid,
};

const TOP_FIELDS: [&str; 16] = [
    "api_version",
    "attestations",
    "binding",
    "canonicalization",
    "declared_at_unix_ms",
    "declared_owner",
    "freshness",
    "intent",
    "kind",
    "materiality",
    "origin",
    "references",
    "requester",
    "status",
    "work_intent_id",
    "work_intent_sha256",
];

const ATTESTATION_FIELDS: [&str; 14] = [
    "approval_attestation",
    "authentication_attestation",
    "authority_attestation",
    "completion_attestation",
    "effect_attestation",
    "execution_attestation",
    "freshness_attestation",
    "materiality_attestation",
    "ownership_attestation",
    "permission_attestation",
    "persistence_attestation",
    "reference_resolution_attestation",
    "scope_attestation",
    "truth_attestation",
];

const INTENT_FIELDS: [&str; 8] = [
    "deadline_unix_ms",
    "external_constraints",
    "goal",
    "non_goals",
    "open_questions",
    "scope",
    "success_signals",
    "work_type",
];

const REFERENCE_FIELDS: [&str; 4] = [
    "claim_record_refs",
    "evidence_record_refs",
    "local_artifact_declarations",
    "local_source_snapshot_declaration",
];

pub(super) fn decode_typed_and_shape(bytes: &[u8]) -> Result<WorkIntent, WorkIntentContractError> {
    if bytes.is_empty() || bytes.len() > MAX_RECORD_BYTES {
        return Err(invalid(format!(
            "WorkIntent byte length must be 1..={MAX_RECORD_BYTES}"
        )));
    }
    let intent: WorkIntent = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("WorkIntent is invalid typed JSON: {error}")))?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("WorkIntent is invalid JSON: {error}")))?;
    validate_json_value(&value, 1)?;
    validate_work_intent_shape(&value)?;
    Ok(intent)
}

pub(super) fn canonical_work_intent_unchecked(
    intent: &WorkIntent,
) -> Result<String, WorkIntentContractError> {
    let value = serde_json::to_value(intent)
        .map_err(|error| invalid(format!("WorkIntent cannot be represented as JSON: {error}")))?;
    validate_json_value(&value, 1)?;
    validate_work_intent_shape(&value)?;
    canonical_json_value(&value)
}

pub(super) fn canonical_json_value(value: &Value) -> Result<String, WorkIntentContractError> {
    validate_json_value(value, 1)?;
    let encoded = crate::governance_contract::codec::canonical_json(value)
        .map_err(|error| invalid(format!("canonical JSON failed: {}", error.message)))?;
    if encoded.len() > MAX_RECORD_BYTES {
        Err(invalid(format!(
            "canonical JSON exceeds {MAX_RECORD_BYTES} bytes"
        )))
    } else {
        Ok(encoded)
    }
}

pub(super) fn validate_json_value(
    value: &Value,
    depth: usize,
) -> Result<(), WorkIntentContractError> {
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

fn validate_array(items: &[Value], depth: usize) -> Result<(), WorkIntentContractError> {
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
    object: &Map<String, Value>,
    depth: usize,
) -> Result<(), WorkIntentContractError> {
    if object.len() > MAX_OBJECT_FIELDS {
        return Err(invalid(format!(
            "JSON object exceeds {MAX_OBJECT_FIELDS} fields"
        )));
    }
    for (key, value) in object {
        if !ascii_snake_case(key) {
            return Err(invalid(format!(
                "JSON object key {key:?} is not ASCII snake_case"
            )));
        }
        validate_wire_string(key)?;
        validate_json_value(value, depth + 1)?;
    }
    Ok(())
}

fn validate_wire_string(value: &str) -> Result<(), WorkIntentContractError> {
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
    matches!(value, '\u{0000}'..='\u{001f}' | '\u{007f}'..='\u{009f}')
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

fn validate_work_intent_shape(value: &Value) -> Result<(), WorkIntentContractError> {
    let root = exact_object(value, &TOP_FIELDS, "WorkIntent")?;
    exact_object(&root["attestations"], &ATTESTATION_FIELDS, "attestations")?;
    exact_object(
        &root["binding"],
        &["change_id", "project_id", "run_id"],
        "binding",
    )?;
    nullable_object(&root["declared_owner"], principal_shape)?;
    principal_shape(&root["requester"])?;
    exact_object(&root["intent"], &INTENT_FIELDS, "intent")?;
    exact_object(&root["materiality"], &["basis", "level"], "materiality")?;
    exact_object(&root["origin"], &["origin_kind", "origin_ref"], "origin")?;
    references_shape(&root["references"])
}

fn references_shape(value: &Value) -> Result<(), WorkIntentContractError> {
    let references = exact_object(value, &REFERENCE_FIELDS, "references")?;
    array_members(
        &references["claim_record_refs"],
        &["canonical_sha256", "record_id"],
        "claim_record_refs",
    )?;
    array_members(
        &references["evidence_record_refs"],
        &["canonical_sha256", "record_id"],
        "evidence_record_refs",
    )?;
    array_members(
        &references["local_artifact_declarations"],
        &["artifact_kind", "artifact_ref", "artifact_sha256"],
        "local_artifact_declarations",
    )?;
    nullable_object(
        &references["local_source_snapshot_declaration"],
        snapshot_shape,
    )
}

fn principal_shape(value: &Value) -> Result<(), WorkIntentContractError> {
    exact_object(
        value,
        &["authority_domain", "principal_id", "principal_type"],
        "principal",
    )?;
    Ok(())
}

fn snapshot_shape(value: &Value) -> Result<(), WorkIntentContractError> {
    exact_object(
        value,
        &["snapshot_id", "snapshot_sha256", "snapshot_type"],
        "source snapshot",
    )?;
    Ok(())
}

fn nullable_object(
    value: &Value,
    validate: fn(&Value) -> Result<(), WorkIntentContractError>,
) -> Result<(), WorkIntentContractError> {
    if value.is_null() {
        Ok(())
    } else {
        validate(value)
    }
}

fn array_members(
    value: &Value,
    fields: &[&str],
    label: &str,
) -> Result<(), WorkIntentContractError> {
    let items = value
        .as_array()
        .ok_or_else(|| invalid(format!("{label} must be an array")))?;
    for item in items {
        exact_object(item, fields, label)?;
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, WorkIntentContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        Err(invalid(format!(
            "{label} does not have its exact required fields"
        )))
    } else {
        Ok(object)
    }
}
