use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    ATOM_API, ATOM_DOMAIN, ATOM_KIND, ATOM_PREFIX, CANONICALIZATION, CognitiveAtom,
    KernelDecisionContractError, MAX_ATOM_BYTES, invalid,
    primitives::{attestations, bindings, identifier, identity, task, text},
    source::{atom_type_member, authority, source},
    wire,
};

fn proposition(value: &super::Proposition) -> Result<(), KernelDecisionContractError> {
    identifier(&value.predicate, "proposition.predicate")?;
    identifier(&value.subject, "proposition.subject")?;
    match (value.object_type.as_str(), &value.object_value) {
        ("artifact_ref", Value::String(text)) => identifier(text, "proposition.object_value"),
        ("string", Value::String(member)) => text(member, "proposition.object_value", 16_384),
        ("boolean", Value::Bool(_)) | ("null", Value::Null) => Ok(()),
        ("integer", Value::Number(number)) if number.as_i64().is_some() => Ok(()),
        ("artifact_ref" | "string" | "boolean" | "integer" | "null", _) => Err(invalid(
            "proposition.object_value does not match object_type",
        )),
        _ => Err(invalid("proposition.object_type is unsupported")),
    }
}

fn scope(value: &CognitiveAtom) -> Result<(), KernelDecisionContractError> {
    text(&value.scope.project, "scope.project", 160)?;
    if value.scope.project != value.task_binding.project_id {
        return Err(invalid("scope.project must equal task_binding.project_id"));
    }
    if let Some(module) = &value.scope.module {
        text(module, "scope.module", 160)?;
    }
    if let Some(object) = &value.scope.object {
        text(object, "scope.object", 160)?;
        if object != &value.proposition.subject {
            return Err(invalid("scope.object must equal proposition.subject"));
        }
    }
    Ok(())
}

fn confidence(value: &CognitiveAtom) -> Result<(), KernelDecisionContractError> {
    let required = matches!(
        value.atom_type.as_str(),
        "assumption" | "hypothesis" | "inference"
    );
    let valid = value
        .confidence_micros
        .is_some_and(|number| (0..=1_000_000).contains(&number));
    if required == valid && (required || value.confidence_micros.is_none()) {
        Ok(())
    } else {
        Err(invalid(
            "confidence_micros presence does not match atom_type",
        ))
    }
}

fn admitted_hardness(atom_type: &str, hardness: &str) -> bool {
    match atom_type {
        "acceptance" | "goal" => matches!(hardness, "advisory" | "preferred" | "required"),
        "actor" | "evidence" | "fact" | "object" | "observation" | "operation" => {
            hardness == "none"
        }
        "assumption" | "hypothesis" | "inference" | "risk" | "unknown" => {
            matches!(hardness, "advisory" | "none")
        }
        "constraint" => matches!(
            hardness,
            "advisory" | "contract" | "invariant" | "preferred" | "required"
        ),
        "decision" => matches!(hardness, "advisory" | "required"),
        "preference" => matches!(hardness, "advisory" | "preferred"),
        _ => false,
    }
}

fn hardness(value: &CognitiveAtom) -> Result<(), KernelDecisionContractError> {
    let authority_kind = value.declared_authority.authority_kind.as_str();
    if value.source.source_kind == "cognitive_atom_v1" {
        if value.declared_hardness == "none" && authority_kind == "none" {
            return Ok(());
        }
        return Err(invalid(
            "legacy source requires none hardness and authority",
        ));
    }
    if !admitted_hardness(&value.atom_type, &value.declared_hardness) {
        return Err(invalid("declared_hardness is not admitted by atom_type"));
    }
    match value.declared_hardness.as_str() {
        "none" if authority_kind != "none" => Err(invalid("none hardness requires none authority")),
        "contract" | "invariant" if authority_kind != "contract_artifact" => Err(invalid(
            "contract/invariant hardness requires contract artifact",
        )),
        "required" if authority_kind == "none" => {
            Err(invalid("required hardness requires declared authority"))
        }
        "required"
            if value.atom_type == "decision"
                && !matches!(authority_kind, "approval_record" | "architecture_decision") =>
        {
            Err(invalid("required decision requires ADR or Approval"))
        }
        _ => Ok(()),
    }
}

fn epistemic(value: &CognitiveAtom) -> Result<(), KernelDecisionContractError> {
    if value.source.source_kind != "cognitive_atom_v1" {
        if value.epistemic_state == "declared" {
            return Ok(());
        }
        return Err(invalid("nonlegacy epistemic_state must be declared"));
    }
    let valid = match value.atom_type.as_str() {
        "assumption" | "hypothesis" => matches!(value.epistemic_state.as_str(), "open" | "testing"),
        "constraint" | "inference" => value.epistemic_state == "candidate",
        "decision" => value.epistemic_state == "proposed",
        "fact" => matches!(value.epistemic_state.as_str(), "candidate" | "contested"),
        "unknown" => matches!(value.epistemic_state.as_str(), "investigating" | "open"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("legacy epistemic_state is outside ADR-0047 states"))
    }
}

fn validate_body(
    value: &CognitiveAtom,
    allow_blank: bool,
) -> Result<(), KernelDecisionContractError> {
    if value.api_version != ATOM_API
        || value.canonicalization != CANONICALIZATION
        || value.effective_hardness != "none"
        || value.instruction_allowed
        || value.kind != ATOM_KIND
    {
        return Err(invalid("CognitiveAtom constants differ"));
    }
    identity(
        &value.atom_id,
        &value.atom_sha256,
        ATOM_PREFIX,
        "atom",
        allow_blank,
    )?;
    if !atom_type_member(&value.atom_type) {
        return Err(invalid("atom_type is unsupported"));
    }
    attestations(&value.attestations)?;
    bindings(&value.bindings)?;
    task(&value.task_binding)?;
    proposition(&value.proposition)?;
    scope(value)?;
    authority(&value.declared_authority)?;
    source(&value.source, &value.atom_type)?;
    validate_state(value)
}

fn validate_state(value: &CognitiveAtom) -> Result<(), KernelDecisionContractError> {
    if value.validity.valid_from_unix_ms < 0
        || value
            .validity
            .valid_until_unix_ms
            .is_some_and(|end| end < 0 || end <= value.validity.valid_from_unix_ms)
    {
        return Err(invalid("CognitiveAtom validity is invalid"));
    }
    confidence(value)?;
    hardness(value)?;
    epistemic(value)
}

fn digest(value: &CognitiveAtom) -> Result<String, KernelDecisionContractError> {
    let mut blank = value.clone();
    blank.atom_id.clear();
    blank.atom_sha256.clear();
    validate_body(&blank, true)?;
    let canonical = wire::canonical_with_max(&blank, MAX_ATOM_BYTES)?;
    let mut hasher = Sha256::new();
    hasher.update(ATOM_DOMAIN);
    hasher.update(canonical.as_bytes());
    Ok(crate::governance_contract::codec::lower_hex(
        &hasher.finalize(),
    ))
}

/// Validates one exact, sealed `CognitiveAtom` v2.
///
/// # Errors
///
/// Returns an error for any wire-independent shape, semantic, bound, or digest violation.
pub fn validate_cognitive_atom(value: &CognitiveAtom) -> Result<(), KernelDecisionContractError> {
    validate_body(value, false)?;
    if value.atom_sha256 != digest(value)? {
        return Err(invalid("atom_sha256 does not match canonical preimage"));
    }
    wire::canonical_with_max(value, MAX_ATOM_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity `CognitiveAtom` v2 copy.
///
/// # Errors
///
/// Returns an error for nonblank identity or any invalid member, bound, or canonical preimage.
pub fn seal_cognitive_atom(
    value: &CognitiveAtom,
) -> Result<CognitiveAtom, KernelDecisionContractError> {
    if !value.atom_id.is_empty() || !value.atom_sha256.is_empty() {
        return Err(invalid("sealing CognitiveAtom requires blank identity"));
    }
    let mut sealed = value.clone();
    let digest = digest(&sealed)?;
    sealed.atom_id = format!("{ATOM_PREFIX}{digest}");
    sealed.atom_sha256 = digest;
    validate_cognitive_atom(&sealed)?;
    Ok(sealed)
}

/// Decodes exact compact canonical `CognitiveAtom` v2 bytes.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, semantically invalid, or unsealed bytes.
pub fn decode_cognitive_atom(bytes: &[u8]) -> Result<CognitiveAtom, KernelDecisionContractError> {
    let value = wire::decode_typed(bytes, MAX_ATOM_BYTES)?;
    validate_cognitive_atom(&value)?;
    Ok(value)
}

pub(super) fn validate_atoms(values: &[CognitiveAtom]) -> Result<(), KernelDecisionContractError> {
    if values.is_empty() || values.len() > 256 {
        return Err(invalid("cognitive_atoms cardinality must be 1..=256"));
    }
    for value in values {
        validate_cognitive_atom(value)?;
    }
    if !values
        .windows(2)
        .all(|pair| pair[0].atom_id.as_bytes() < pair[1].atom_id.as_bytes())
    {
        return Err(invalid(
            "cognitive_atoms must be strictly atom-id sorted and unique",
        ));
    }
    Ok(())
}
