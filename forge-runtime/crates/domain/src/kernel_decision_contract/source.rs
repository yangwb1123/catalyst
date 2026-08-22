use serde_json::Value;

use super::{
    AtomSource, DeclaredAuthority, KernelDecisionContractError, invalid,
    primitives::{exact_object, hash, object_text, text},
};

pub(super) fn authority(value: &DeclaredAuthority) -> Result<(), KernelDecisionContractError> {
    match value.authority_kind.as_str() {
        "none" if value.authority_ref.is_null() => Ok(()),
        "none" => Err(invalid("none authority requires null authority_ref")),
        "approval_record" => approval_ref(&value.authority_ref),
        "architecture_decision" => adr_ref(&value.authority_ref),
        "contract_artifact" => artifact_ref(&value.authority_ref, "authority_ref"),
        _ => Err(invalid("authority_kind is unsupported")),
    }
}

fn approval_ref(value: &Value) -> Result<(), KernelDecisionContractError> {
    let object = exact_object(
        value,
        &["approval_id", "approval_sha256", "authority_domain"],
        "authority_ref",
    )?;
    let digest = object_text(object, "approval_sha256")?;
    hash(digest, "approval_sha256")?;
    if object_text(object, "approval_id")? != format!("approval-record-{digest}") {
        return Err(invalid("approval_id does not bind approval_sha256"));
    }
    text(
        object_text(object, "authority_domain")?,
        "authority_domain",
        160,
    )
}

fn adr_ref(value: &Value) -> Result<(), KernelDecisionContractError> {
    let object = exact_object(value, &["adr_id", "adr_self_sha256"], "authority_ref")?;
    let id = object_text(object, "adr_id")?;
    if !id
        .strip_prefix("ADR-")
        .is_some_and(|digits| digits.len() >= 4 && digits.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(invalid("adr_id is invalid"));
    }
    hash(object_text(object, "adr_self_sha256")?, "adr_self_sha256")
}

fn artifact_ref(value: &Value, label: &str) -> Result<(), KernelDecisionContractError> {
    let object = exact_object(
        value,
        &["artifact_kind", "artifact_ref", "artifact_sha256"],
        label,
    )?;
    text(object_text(object, "artifact_kind")?, "artifact_kind", 160)?;
    text(object_text(object, "artifact_ref")?, "artifact_ref", 4096)?;
    hash(object_text(object, "artifact_sha256")?, "artifact_sha256")
}

pub(super) fn source(
    value: &AtomSource,
    atom_type: &str,
) -> Result<(), KernelDecisionContractError> {
    if !source_admits(&value.source_kind, atom_type) {
        return Err(invalid("source_kind does not admit atom_type"));
    }
    let predecision = matches!(
        value.source_kind.as_str(),
        "artifact" | "cognitive_atom_v1" | "evidence_record" | "work_intent"
    );
    let expected = if predecision {
        "predecision"
    } else {
        "postdecision"
    };
    if value.source_phase != expected {
        return Err(invalid("source_phase does not match source_kind"));
    }
    source_ref(&value.source_kind, &value.source_ref)?;
    if let Some(selector) = &value.source_selector {
        json_pointer(selector)?;
    }
    if value.source_kind == "cognitive_atom_v1" && value.source_selector.is_some() {
        return Err(invalid("cognitive_atom_v1 selector must be null"));
    }
    Ok(())
}

fn source_admits(kind: &str, atom_type: &str) -> bool {
    match kind {
        "artifact" => atom_type_member(atom_type),
        "artifact_receipt" => matches!(atom_type, "evidence" | "object" | "observation"),
        "capability_invocation" => matches!(atom_type, "actor" | "operation"),
        "cognitive_atom_v1" => matches!(
            atom_type,
            "assumption"
                | "constraint"
                | "decision"
                | "fact"
                | "hypothesis"
                | "inference"
                | "unknown"
        ),
        "evidence_record" => matches!(atom_type, "evidence" | "observation"),
        "execution_receipt" => matches!(atom_type, "actor" | "evidence" | "observation"),
        "interaction_event" => matches!(
            atom_type,
            "actor" | "evidence" | "object" | "observation" | "operation"
        ),
        "work_intent" => matches!(
            atom_type,
            "acceptance" | "constraint" | "goal" | "preference" | "risk" | "unknown"
        ),
        _ => false,
    }
}

pub(super) fn atom_type_member(value: &str) -> bool {
    matches!(
        value,
        "acceptance"
            | "actor"
            | "assumption"
            | "constraint"
            | "decision"
            | "evidence"
            | "fact"
            | "goal"
            | "hypothesis"
            | "inference"
            | "object"
            | "observation"
            | "operation"
            | "preference"
            | "risk"
            | "unknown"
    )
}

fn source_ref(kind: &str, value: &Value) -> Result<(), KernelDecisionContractError> {
    match kind {
        "artifact" => artifact_ref(value, "source_ref"),
        "cognitive_atom_v1" => legacy_ref(value),
        "evidence_record" => evidence_ref(value),
        "work_intent" => bound_ref(
            value,
            "work_intent_id",
            "work_intent_sha256",
            "work-intent-",
        ),
        "artifact_receipt" => bound_ref(
            value,
            "artifact_receipt_id",
            "artifact_receipt_sha256",
            "artifact-receipt-",
        ),
        "capability_invocation" => bound_ref(
            value,
            "invocation_id",
            "invocation_sha256",
            "capability-invocation-",
        ),
        "interaction_event" => bound_ref(value, "event_id", "event_sha256", "interaction-event-"),
        _ => bound_ref(
            value,
            "execution_receipt_id",
            "execution_receipt_sha256",
            "execution-receipt-",
        ),
    }
}

fn bound_ref(
    value: &Value,
    id_field: &str,
    hash_field: &str,
    prefix: &str,
) -> Result<(), KernelDecisionContractError> {
    let object = exact_object(value, &[id_field, hash_field], "source_ref")?;
    let digest = object_text(object, hash_field)?;
    hash(digest, hash_field)?;
    if object_text(object, id_field)? != format!("{prefix}{digest}") {
        return Err(invalid(format!("{id_field} does not bind {hash_field}")));
    }
    Ok(())
}

fn legacy_ref(value: &Value) -> Result<(), KernelDecisionContractError> {
    let object = exact_object(value, &["atom_id", "canonical_sha256"], "source_ref")?;
    let id = object_text(object, "atom_id")?;
    if !id.strip_prefix("atom-").is_some_and(valid_hash_text) {
        return Err(invalid("legacy atom_id is invalid"));
    }
    hash(object_text(object, "canonical_sha256")?, "canonical_sha256")
}

fn valid_hash_text(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn evidence_ref(value: &Value) -> Result<(), KernelDecisionContractError> {
    let object = exact_object(value, &["canonical_sha256", "record_id"], "source_ref")?;
    hash(object_text(object, "canonical_sha256")?, "canonical_sha256")?;
    text(object_text(object, "record_id")?, "record_id", 160)
}

fn json_pointer(value: &str) -> Result<(), KernelDecisionContractError> {
    text(value, "source_selector", 4096)?;
    if !value.starts_with('/') {
        return Err(invalid("source_selector must be a nonempty JSON pointer"));
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if index + 1 == bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                return Err(invalid("source_selector has noncanonical escape"));
            }
            index += 1;
        }
        index += 1;
    }
    Ok(())
}

pub(super) fn raw_reference(value: &Value) -> Result<(&str, &str), KernelDecisionContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("operational source_ref must be an object"))?;
    let mut id = None;
    let mut digest = None;
    for (field, member) in object {
        if field.ends_with("_id") {
            id = member.as_str();
        }
        if field.ends_with("_sha256") {
            digest = member.as_str();
        }
    }
    match (id, digest) {
        (Some(id), Some(digest)) => Ok((id, digest)),
        _ => Err(invalid("operational source_ref fields are missing")),
    }
}
