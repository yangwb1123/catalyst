use serde::Serialize;

use super::{
    ArtifactReceiptRef, ArtifactRef, Attestations, CapabilityGrantRef, CapabilityIdentity,
    CapabilityInvocationRef, ExecutionReceiptRef, InteractionEventRef,
    KernelOperationalContractError, MAX_REFERENCE_BYTES, MAX_SHORT_BYTES, OperationalBindings,
    Principal, TaskBinding, invalid, wire,
};

pub(super) fn validate_text(
    value: &str,
    maximum: usize,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
    if value.is_empty() || value.len() > maximum {
        Err(invalid(format!(
            "{label} must contain 1..={maximum} UTF-8 bytes"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn validate_identifier(
    value: &str,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
    let bytes = value.as_bytes();
    let valid = (1..=MAX_SHORT_BYTES).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || index > 0 && matches!(*byte, b'.' | b'_' | b':' | b'/' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} does not match the frozen identifier grammar"
        )))
    }
}

pub(super) fn validate_hash(
    value: &str,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
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

pub(super) fn validate_nonnegative(
    value: i64,
    maximum: i64,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
    if (0..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be in 0..={maximum}")))
    }
}

pub(super) fn validate_principal(
    value: &Principal,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
    validate_text(
        &value.authority_domain,
        MAX_SHORT_BYTES,
        &format!("{label}.authority_domain"),
    )?;
    validate_text(
        &value.principal_id,
        MAX_SHORT_BYTES,
        &format!("{label}.principal_id"),
    )
}

pub(super) fn validate_task_binding(
    value: &TaskBinding,
) -> Result<(), KernelOperationalContractError> {
    for (label, text) in [
        ("change_id", &value.change_id),
        ("environment_id", &value.environment_id),
        ("node_id", &value.node_id),
        ("project_id", &value.project_id),
        ("role", &value.role),
        ("run_id", &value.run_id),
        ("task_id", &value.task_id),
    ] {
        validate_text(text, MAX_SHORT_BYTES, &format!("task_binding.{label}"))?;
    }
    for (label, text) in [
        ("attempt_id", &value.attempt_id),
        ("target_id", &value.target_id),
    ] {
        if let Some(text) = text {
            validate_text(text, MAX_SHORT_BYTES, &format!("task_binding.{label}"))?;
        }
    }
    Ok(())
}

pub(super) fn validate_bindings(
    value: &OperationalBindings,
) -> Result<(), KernelOperationalContractError> {
    for (label, hash) in [
        ("context_sha256", &value.context_sha256),
        ("environment_sha256", &value.environment_sha256),
        ("policy_sha256", &value.policy_sha256),
        ("source_tree_sha256", &value.source_tree_sha256),
    ] {
        validate_hash(hash, &format!("bindings.{label}"))?;
    }
    for (label, text) in [
        ("environment_profile_id", &value.environment_profile_id),
        ("source_profile_id", &value.source_profile_id),
        ("source_revision", &value.source_revision),
    ] {
        validate_text(text, MAX_SHORT_BYTES, &format!("bindings.{label}"))?;
    }
    Ok(())
}

pub(super) fn validate_capability(
    value: &CapabilityIdentity,
) -> Result<(), KernelOperationalContractError> {
    validate_hash(
        &value.capability_contract_sha256,
        "capability.capability_contract_sha256",
    )?;
    validate_text(
        &value.capability_id,
        MAX_SHORT_BYTES,
        "capability.capability_id",
    )?;
    validate_text(
        &value.capability_version,
        MAX_SHORT_BYTES,
        "capability.capability_version",
    )
}

pub(super) fn validate_grant_ref(
    value: &CapabilityGrantRef,
) -> Result<(), KernelOperationalContractError> {
    validate_text(
        &value.authority_domain,
        MAX_SHORT_BYTES,
        "capability_grant_ref.authority_domain",
    )?;
    validate_hash(&value.grant_sha256, "capability_grant_ref.grant_sha256")?;
    if value.grant_id == format!("capability-grant-{}", value.grant_sha256) {
        Ok(())
    } else {
        Err(invalid(
            "capability_grant_ref.grant_id must bind grant_sha256",
        ))
    }
}

pub(super) fn validate_attestations(
    value: &Attestations,
) -> Result<(), KernelOperationalContractError> {
    let values = [
        value.authorization_attestation,
        value.binding_authentication_attestation,
        value.completion_attestation,
        value.content_provenance_attestation,
        value.effect_attestation,
        value.event_append_attestation,
        value.execution_attestation,
        value.grant_authentication_attestation,
        value.outcome_attestation,
        value.permission_attestation,
        value.persistence_attestation,
        value.principal_authentication_attestation,
        value.transition_attestation,
        value.usage_measurement_attestation,
    ];
    if values.into_iter().any(|item| item) {
        Err(invalid(
            "every operational attestation must be exactly false",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_artifact(
    value: &ArtifactRef,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
    validate_text(
        &value.artifact_kind,
        MAX_SHORT_BYTES,
        &format!("{label}.artifact_kind"),
    )?;
    validate_text(
        &value.artifact_ref,
        MAX_REFERENCE_BYTES,
        &format!("{label}.artifact_ref"),
    )?;
    validate_hash(&value.artifact_sha256, &format!("{label}.artifact_sha256"))
}

fn validate_identity(
    identity: &str,
    digest: &str,
    prefix: &str,
    label: &str,
) -> Result<(), KernelOperationalContractError> {
    validate_hash(digest, &format!("{label}_sha256"))?;
    if identity == format!("{prefix}{digest}") {
        Ok(())
    } else {
        Err(invalid(format!("{label}_id must bind {label}_sha256")))
    }
}

pub(super) fn validate_identity_fields(
    identity: &str,
    digest: &str,
    prefix: &str,
    label: &str,
    allow_blank: bool,
) -> Result<(), KernelOperationalContractError> {
    if allow_blank && identity.is_empty() && digest.is_empty() {
        Ok(())
    } else {
        validate_identity(identity, digest, prefix, label)
    }
}

pub(super) fn validate_artifact_receipt_ref(
    value: &ArtifactReceiptRef,
) -> Result<(), KernelOperationalContractError> {
    validate_identity(
        &value.artifact_receipt_id,
        &value.artifact_receipt_sha256,
        "artifact-receipt-",
        "artifact_receipt",
    )
}

pub(super) fn validate_invocation_ref(
    value: &CapabilityInvocationRef,
) -> Result<(), KernelOperationalContractError> {
    validate_identity(
        &value.invocation_id,
        &value.invocation_sha256,
        "capability-invocation-",
        "invocation",
    )
}

pub(super) fn validate_event_ref(
    value: &InteractionEventRef,
) -> Result<(), KernelOperationalContractError> {
    validate_identity(
        &value.event_id,
        &value.event_sha256,
        "interaction-event-",
        "event",
    )
}

pub(super) fn validate_execution_ref(
    value: &ExecutionReceiptRef,
) -> Result<(), KernelOperationalContractError> {
    validate_identity(
        &value.execution_receipt_id,
        &value.execution_receipt_sha256,
        "execution-receipt-",
        "execution_receipt",
    )
}

pub(super) fn validate_sorted_unique<T, F>(
    values: &[T],
    label: &str,
    maximum: usize,
    nonempty: bool,
    validate: F,
) -> Result<(), KernelOperationalContractError>
where
    T: Serialize,
    F: Fn(&T) -> Result<(), KernelOperationalContractError>,
{
    if values.len() > maximum || nonempty && values.is_empty() {
        return Err(invalid(format!(
            "{label} cardinality is outside the frozen bound"
        )));
    }
    let mut keys = Vec::with_capacity(values.len());
    for value in values {
        validate(value)?;
        keys.push(wire::canonical_typed(value)?);
    }
    if keys.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly canonical-byte sorted and unique"
        )))
    }
}

pub(super) fn validate_string_set(
    values: &[String],
    label: &str,
    maximum: usize,
) -> Result<(), KernelOperationalContractError> {
    if values.len() > maximum {
        return Err(invalid(format!(
            "{label} cardinality is outside the frozen bound"
        )));
    }
    for value in values {
        validate_identifier(value, label)?;
    }
    if values
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly UTF-8 sorted and unique"
        )))
    }
}
