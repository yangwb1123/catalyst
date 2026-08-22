use std::collections::BTreeSet;

use super::{
    API_VERSION, CANONICALIZATION, LocalArtifactDeclaration, MAX_LOCAL_ARTIFACTS,
    MAX_NARRATIVE_ITEMS, MAX_NARRATIVE_TOTAL, MAX_RECORD_REFS_PER_KIND, MAX_RECORD_REFS_TOTAL,
    MAX_REFERENCE_TEXT_BYTES, MAX_SHORT_TEXT_BYTES, MAX_STRING_BYTES, Principal, RecordReference,
    SourceSnapshot, WorkIntent, WorkIntentAttestations, WorkIntentContractError, invalid, wire,
};

/// Validates one sealed authority-neutral `WorkIntent` declaration.
///
/// # Errors
///
/// Returns an error for invalid fields, bounds, ordering, attestations, or self-identity.
pub fn validate_work_intent(intent: &WorkIntent) -> Result<(), WorkIntentContractError> {
    validate_body(intent)?;
    validate_hash(&intent.work_intent_sha256, "work_intent_sha256")?;
    if intent.work_intent_id != format!("work-intent-{}", intent.work_intent_sha256) {
        return Err(invalid(
            "work_intent_id must equal work-intent- plus work_intent_sha256",
        ));
    }
    wire::canonical_work_intent_unchecked(intent)?;
    let expected = super::codec::work_intent_sha256_unchecked(intent)?;
    if intent.work_intent_sha256 == expected {
        Ok(())
    } else {
        Err(invalid(
            "work_intent_sha256 does not match the canonical preimage",
        ))
    }
}

pub(super) fn validate_body(intent: &WorkIntent) -> Result<(), WorkIntentContractError> {
    if intent.api_version != API_VERSION
        || intent.canonicalization != CANONICALIZATION
        || intent.declared_at_unix_ms < 0
    {
        return Err(invalid(
            "WorkIntent envelope constants or timestamp are invalid",
        ));
    }
    validate_attestations(&intent.attestations)?;
    validate_binding(intent)?;
    if let Some(owner) = &intent.declared_owner {
        validate_principal(owner, "declared_owner")?;
    }
    validate_principal(&intent.requester, "requester")?;
    validate_intent(intent)?;
    validate_materiality_and_origin(intent)?;
    validate_references(intent)
}

fn validate_attestations(
    attestations: &WorkIntentAttestations,
) -> Result<(), WorkIntentContractError> {
    let values = [
        attestations.approval_attestation,
        attestations.authentication_attestation,
        attestations.authority_attestation,
        attestations.completion_attestation,
        attestations.effect_attestation,
        attestations.execution_attestation,
        attestations.freshness_attestation,
        attestations.materiality_attestation,
        attestations.ownership_attestation,
        attestations.permission_attestation,
        attestations.persistence_attestation,
        attestations.reference_resolution_attestation,
        attestations.scope_attestation,
        attestations.truth_attestation,
    ];
    if values.into_iter().any(|value| value) {
        Err(invalid("every WorkIntent attestation must be false"))
    } else {
        Ok(())
    }
}

fn validate_binding(intent: &WorkIntent) -> Result<(), WorkIntentContractError> {
    validate_text(
        &intent.binding.change_id,
        MAX_SHORT_TEXT_BYTES,
        "binding.change_id",
    )?;
    validate_text(
        &intent.binding.project_id,
        MAX_SHORT_TEXT_BYTES,
        "binding.project_id",
    )?;
    if let Some(run_id) = &intent.binding.run_id {
        validate_text(run_id, MAX_SHORT_TEXT_BYTES, "binding.run_id")?;
    }
    Ok(())
}

fn validate_principal(principal: &Principal, label: &str) -> Result<(), WorkIntentContractError> {
    validate_text(
        &principal.authority_domain,
        MAX_SHORT_TEXT_BYTES,
        &format!("{label}.authority_domain"),
    )?;
    validate_text(
        &principal.principal_id,
        MAX_SHORT_TEXT_BYTES,
        &format!("{label}.principal_id"),
    )
}

fn validate_intent(intent: &WorkIntent) -> Result<(), WorkIntentContractError> {
    if intent
        .intent
        .deadline_unix_ms
        .is_some_and(|value| value < 0)
    {
        return Err(invalid(
            "intent.deadline_unix_ms must be nonnegative or null",
        ));
    }
    validate_text(&intent.intent.goal, MAX_STRING_BYTES, "intent.goal")?;
    let declarations = [
        (
            &intent.intent.external_constraints,
            false,
            "external_constraints",
        ),
        (&intent.intent.non_goals, false, "non_goals"),
        (&intent.intent.open_questions, false, "open_questions"),
        (&intent.intent.scope, true, "scope"),
        (&intent.intent.success_signals, true, "success_signals"),
    ];
    let mut total = 0;
    for (values, nonempty, label) in declarations {
        total += validate_narrative(values, nonempty, label)?;
    }
    if total > MAX_NARRATIVE_TOTAL {
        Err(invalid(format!(
            "intent narrative arrays exceed {MAX_NARRATIVE_TOTAL} total items"
        )))
    } else {
        Ok(())
    }
}

fn validate_narrative(
    values: &[String],
    nonempty: bool,
    label: &str,
) -> Result<usize, WorkIntentContractError> {
    if values.len() > MAX_NARRATIVE_ITEMS || (nonempty && values.is_empty()) {
        return Err(invalid(format!("intent.{label} cardinality is invalid")));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(value, MAX_STRING_BYTES, &format!("intent.{label}"))?;
        if !unique.insert(value.as_str()) {
            return Err(invalid(format!(
                "intent.{label} must contain unique authored entries"
            )));
        }
    }
    Ok(values.len())
}

fn validate_materiality_and_origin(intent: &WorkIntent) -> Result<(), WorkIntentContractError> {
    if intent.materiality.basis != "caller_declaration_only" {
        return Err(invalid(
            "materiality.basis must equal caller_declaration_only",
        ));
    }
    if let Some(reference) = &intent.origin.origin_ref {
        validate_text(reference, MAX_REFERENCE_TEXT_BYTES, "origin.origin_ref")?;
    }
    Ok(())
}

fn validate_references(intent: &WorkIntent) -> Result<(), WorkIntentContractError> {
    let references = &intent.references;
    validate_record_refs(&references.claim_record_refs, "claim_record_refs")?;
    validate_record_refs(&references.evidence_record_refs, "evidence_record_refs")?;
    if references.claim_record_refs.len() + references.evidence_record_refs.len()
        > MAX_RECORD_REFS_TOTAL
    {
        return Err(invalid(format!(
            "combined record references exceed {MAX_RECORD_REFS_TOTAL}"
        )));
    }
    let claims: BTreeSet<_> = references
        .claim_record_refs
        .iter()
        .map(|reference| reference.record_id.as_str())
        .collect();
    if references
        .evidence_record_refs
        .iter()
        .any(|reference| claims.contains(reference.record_id.as_str()))
    {
        return Err(invalid("claim and evidence record IDs must be disjoint"));
    }
    validate_artifacts(&references.local_artifact_declarations)?;
    if let Some(snapshot) = &references.local_source_snapshot_declaration {
        validate_snapshot(snapshot)?;
    }
    Ok(())
}

fn validate_record_refs(
    references: &[RecordReference],
    label: &str,
) -> Result<(), WorkIntentContractError> {
    if references.len() > MAX_RECORD_REFS_PER_KIND {
        return Err(invalid(format!(
            "{label} exceeds {MAX_RECORD_REFS_PER_KIND} entries"
        )));
    }
    for reference in references {
        validate_identifier(&reference.record_id, &format!("{label}.record_id"))?;
        validate_hash(
            &reference.canonical_sha256,
            &format!("{label}.canonical_sha256"),
        )?;
    }
    if references
        .windows(2)
        .all(|pair| pair[0].record_id.as_bytes() < pair[1].record_id.as_bytes())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly record_id sorted and unique"
        )))
    }
}

fn validate_artifacts(
    artifacts: &[LocalArtifactDeclaration],
) -> Result<(), WorkIntentContractError> {
    if artifacts.len() > MAX_LOCAL_ARTIFACTS {
        return Err(invalid(format!(
            "local artifacts exceed {MAX_LOCAL_ARTIFACTS} entries"
        )));
    }
    let mut pairs = BTreeSet::new();
    let mut encoded = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        validate_artifact(artifact)?;
        if !pairs.insert((
            artifact.artifact_kind.as_str(),
            artifact.artifact_ref.as_str(),
        )) {
            return Err(invalid("artifact kind/ref pairs must be unique"));
        }
        let value = serde_json::to_value(artifact)
            .map_err(|error| invalid(format!("artifact cannot be encoded: {error}")))?;
        encoded.push(wire::canonical_json_value(&value)?);
    }
    if encoded.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(
            "artifacts must be strictly sorted and unique by canonical bytes",
        ))
    }
}

fn validate_artifact(artifact: &LocalArtifactDeclaration) -> Result<(), WorkIntentContractError> {
    validate_text(
        &artifact.artifact_kind,
        MAX_SHORT_TEXT_BYTES,
        "artifact_kind",
    )?;
    validate_text(
        &artifact.artifact_ref,
        MAX_REFERENCE_TEXT_BYTES,
        "artifact_ref",
    )?;
    validate_hash(&artifact.artifact_sha256, "artifact_sha256")
}

fn validate_snapshot(snapshot: &SourceSnapshot) -> Result<(), WorkIntentContractError> {
    validate_identifier(&snapshot.snapshot_id, "snapshot_id")?;
    validate_hash(&snapshot.snapshot_sha256, "snapshot_sha256")
}

fn validate_text(value: &str, maximum: usize, label: &str) -> Result<(), WorkIntentContractError> {
    if value.is_empty() || value.len() > maximum {
        Err(invalid(format!(
            "{label} must contain 1..={maximum} UTF-8 bytes"
        )))
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, label: &str) -> Result<(), WorkIntentContractError> {
    let bytes = value.as_bytes();
    let valid = (1..=MAX_SHORT_TEXT_BYTES).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'/' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must match the ADR-0045 identifier grammar"
        )))
    }
}

fn validate_hash(value: &str, label: &str) -> Result<(), WorkIntentContractError> {
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be lowercase SHA-256 hex")))
    }
}
