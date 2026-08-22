use std::collections::BTreeSet;

use super::{
    ArtifactReceipt, CANONICALIZATION, CapabilityInvocation, ExecutionReceipt, InteractionEvent,
    KernelOperationalContractError, MAX_ATTEMPT, MAX_CALL_COUNT, MAX_CONFIDENCE_MICROS,
    MAX_COST_MICROS, MAX_ELAPSED_MS, MAX_EVENTS, MAX_IO_ITEMS, MAX_NETWORK_BYTES, MAX_OUTPUT_BYTES,
    MAX_REASON_CODES, MAX_TOKEN_COUNT,
    constants::{
        ARTIFACT_RECEIPT_API, ARTIFACT_RECEIPT_KIND, ARTIFACT_RECEIPT_PREFIX, EVENT_API,
        EVENT_KIND, EVENT_PREFIX, EXECUTION_RECEIPT_API, EXECUTION_RECEIPT_KIND,
        EXECUTION_RECEIPT_PREFIX, INVOCATION_API, INVOCATION_KIND, INVOCATION_PREFIX,
    },
    invalid,
    primitives::{
        validate_artifact, validate_artifact_receipt_ref, validate_attestations, validate_bindings,
        validate_capability, validate_event_ref, validate_execution_ref, validate_grant_ref,
        validate_hash, validate_identifier, validate_identity_fields, validate_invocation_ref,
        validate_nonnegative, validate_principal, validate_sorted_unique, validate_string_set,
        validate_task_binding, validate_text,
    },
    wire,
};

fn validate_header(
    api: &str,
    kind: &str,
    expected_api: &str,
    expected_kind: &str,
) -> Result<(), KernelOperationalContractError> {
    if api == expected_api && kind == expected_kind {
        Ok(())
    } else {
        Err(invalid(
            "record api_version or kind differs from the frozen constant",
        ))
    }
}

pub(super) fn validate_artifact_receipt_body(
    value: &ArtifactReceipt,
    allow_blank: bool,
) -> Result<(), KernelOperationalContractError> {
    validate_header(
        &value.api_version,
        &value.kind,
        ARTIFACT_RECEIPT_API,
        ARTIFACT_RECEIPT_KIND,
    )?;
    if value.canonicalization != CANONICALIZATION {
        return Err(invalid("ArtifactReceipt canonicalization differs"));
    }
    validate_identity_fields(
        &value.artifact_receipt_id,
        &value.artifact_receipt_sha256,
        ARTIFACT_RECEIPT_PREFIX,
        "artifact_receipt",
        allow_blank,
    )?;
    validate_artifact(&value.artifact, "artifact")?;
    validate_attestations(&value.attestations)?;
    validate_bindings(&value.bindings)?;
    validate_nonnegative(value.content_bytes, i64::MAX, "content_bytes")?;
    validate_nonnegative(value.created_at_unix_ms, i64::MAX, "created_at_unix_ms")?;
    validate_principal(&value.producer, "producer")?;
    if !matches!(
        value.receipt_role.as_str(),
        "declared_input" | "declared_output"
    ) {
        return Err(invalid("receipt_role is unsupported"));
    }
    if (value.receipt_role == "declared_input") != value.producer_invocation_ref.is_none() {
        return Err(invalid(
            "declared_input requires null producer; output requires one",
        ));
    }
    if let Some(reference) = &value.producer_invocation_ref {
        validate_invocation_ref(reference)?;
    }
    validate_identifier(&value.slot, "slot")?;
    validate_task_binding(&value.task_binding)
}

pub(super) fn validate_invocation_body(
    value: &CapabilityInvocation,
    allow_blank: bool,
) -> Result<(), KernelOperationalContractError> {
    validate_header(
        &value.api_version,
        &value.kind,
        INVOCATION_API,
        INVOCATION_KIND,
    )?;
    if value.canonicalization != CANONICALIZATION {
        return Err(invalid("CapabilityInvocation canonicalization differs"));
    }
    validate_identity_fields(
        &value.invocation_id,
        &value.invocation_sha256,
        INVOCATION_PREFIX,
        "invocation",
        allow_blank,
    )?;
    if !(1..=MAX_ATTEMPT).contains(&value.attempt) {
        return Err(invalid("attempt is outside 1..=64"));
    }
    validate_invocation_members(value)?;
    validate_task_binding(&value.task_binding)
}

fn validate_invocation_members(
    value: &CapabilityInvocation,
) -> Result<(), KernelOperationalContractError> {
    validate_attestations(&value.attestations)?;
    validate_bindings(&value.bindings)?;
    validate_capability(&value.capability)?;
    validate_grant_ref(&value.capability_grant_ref)?;
    validate_identifier(&value.correlation_id, "correlation_id")?;
    validate_string_set(
        &value.declared_output_slots,
        "declared_output_slots",
        MAX_IO_ITEMS,
    )?;
    validate_identifier(&value.idempotency_key, "idempotency_key")?;
    validate_sorted_unique(
        &value.input_artifact_receipt_refs,
        "input_artifact_receipt_refs",
        MAX_IO_ITEMS,
        false,
        validate_artifact_receipt_ref,
    )?;
    if (value.attempt == 1) != value.prior_execution_receipt_ref.is_none() {
        return Err(invalid(
            "attempt one requires null prior receipt; retry requires one",
        ));
    }
    if let Some(reference) = &value.prior_execution_receipt_ref {
        validate_execution_ref(reference)?;
    }
    validate_hash(&value.requested_action_sha256, "requested_action_sha256")?;
    validate_nonnegative(value.requested_at_unix_ms, i64::MAX, "requested_at_unix_ms")?;
    validate_principal(&value.subject, "subject")
}

pub(super) fn validate_event_body(
    value: &InteractionEvent,
    allow_blank: bool,
) -> Result<(), KernelOperationalContractError> {
    validate_header(&value.api_version, &value.kind, EVENT_API, EVENT_KIND)?;
    if value.canonicalization != CANONICALIZATION {
        return Err(invalid("InteractionEvent canonicalization differs"));
    }
    validate_identity_fields(
        &value.event_id,
        &value.event_sha256,
        EVENT_PREFIX,
        "event",
        allow_blank,
    )?;
    validate_principal(&value.actor, "actor")?;
    validate_sorted_unique(
        &value.artifact_refs,
        "artifact_refs",
        MAX_IO_ITEMS,
        false,
        |artifact| validate_artifact(artifact, "artifact_refs"),
    )?;
    validate_event_members(value)?;
    validate_task_binding(&value.task_binding)
}

fn validate_event_members(value: &InteractionEvent) -> Result<(), KernelOperationalContractError> {
    validate_attestations(&value.attestations)?;
    validate_bindings(&value.bindings)?;
    if !(1..=i64::try_from(MAX_EVENTS).unwrap_or(i64::MAX)).contains(&value.logical_sequence) {
        return Err(invalid("logical_sequence is outside 1..=256"));
    }
    if (value.logical_sequence == 1) != value.causation_event_ref.is_none() {
        return Err(invalid(
            "sequence one requires null cause; later events require one",
        ));
    }
    if let Some(reference) = &value.causation_event_ref {
        validate_event_ref(reference)?;
    }
    if let Some(confidence) = value.confidence_micros {
        validate_nonnegative(confidence, MAX_CONFIDENCE_MICROS, "confidence_micros")?;
    }
    validate_identifier(&value.correlation_id, "correlation_id")?;
    validate_invocation_ref(&value.invocation_ref)?;
    validate_text(&value.object_ref, super::MAX_REFERENCE_BYTES, "object_ref")?;
    validate_nonnegative(value.occurred_at_unix_ms, i64::MAX, "occurred_at_unix_ms")?;
    if let Some(target) = &value.target {
        validate_principal(target, "target")?;
    }
    if !matches!(
        value.verb.as_str(),
        "approve"
            | "execute"
            | "observe"
            | "propose"
            | "reject"
            | "request"
            | "rollback"
            | "verify"
    ) {
        return Err(invalid("verb is unsupported"));
    }
    Ok(())
}

pub(super) fn validate_execution_receipt_body(
    value: &ExecutionReceipt,
    allow_blank: bool,
) -> Result<(), KernelOperationalContractError> {
    validate_header(
        &value.api_version,
        &value.kind,
        EXECUTION_RECEIPT_API,
        EXECUTION_RECEIPT_KIND,
    )?;
    if value.canonicalization != CANONICALIZATION {
        return Err(invalid("ExecutionReceipt canonicalization differs"));
    }
    validate_identity_fields(
        &value.execution_receipt_id,
        &value.execution_receipt_sha256,
        EXECUTION_RECEIPT_PREFIX,
        "execution_receipt",
        allow_blank,
    )?;
    if !(1..=MAX_ATTEMPT).contains(&value.attempt) {
        return Err(invalid("attempt is outside 1..=64"));
    }
    validate_execution_members(value)?;
    validate_task_binding(&value.task_binding)
}

fn validate_execution_members(
    value: &ExecutionReceipt,
) -> Result<(), KernelOperationalContractError> {
    validate_attestations(&value.attestations)?;
    validate_bindings(&value.bindings)?;
    validate_identifier(&value.correlation_id, "correlation_id")?;
    if value.started_at_unix_ms < 0
        || value.ended_at_unix_ms < value.started_at_unix_ms
        || value.ended_at_unix_ms - value.started_at_unix_ms > MAX_ELAPSED_MS
    {
        return Err(invalid("execution wall interval is invalid or exceeds max"));
    }
    validate_event_refs(&value.event_refs)?;
    validate_sorted_unique(
        &value.input_artifacts,
        "input_artifacts",
        MAX_IO_ITEMS,
        false,
        |artifact| validate_artifact(artifact, "input_artifacts"),
    )?;
    validate_invocation_ref(&value.invocation_ref)?;
    validate_observed_usage(value)?;
    if !matches!(
        value.outcome.as_str(),
        "cancelled" | "failed" | "inconclusive" | "lost" | "succeeded"
    ) {
        return Err(invalid("outcome is unsupported"));
    }
    validate_sorted_unique(
        &value.output_artifact_receipt_refs,
        "output_artifact_receipt_refs",
        MAX_IO_ITEMS,
        false,
        validate_artifact_receipt_ref,
    )?;
    validate_execution_final(value)
}

fn validate_event_refs(
    values: &[super::InteractionEventRef],
) -> Result<(), KernelOperationalContractError> {
    if values.len() > MAX_EVENTS {
        return Err(invalid("event_refs cardinality exceeds 256"));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_event_ref(value)?;
        if !seen.insert(wire::canonical_typed(value)?) {
            return Err(invalid("event_refs must be unique"));
        }
    }
    Ok(())
}

fn validate_observed_usage(value: &ExecutionReceipt) -> Result<(), KernelOperationalContractError> {
    let usage = &value.observed_usage;
    for (label, number, maximum) in [
        ("call_count", usage.call_count, MAX_CALL_COUNT),
        ("cost_usd_micros", usage.cost_usd_micros, MAX_COST_MICROS),
        ("elapsed_ms", usage.elapsed_ms, MAX_ELAPSED_MS),
        ("input_tokens", usage.input_tokens, MAX_TOKEN_COUNT),
        ("network_bytes", usage.network_bytes, MAX_NETWORK_BYTES),
        ("output_bytes", usage.output_bytes, MAX_OUTPUT_BYTES),
        ("output_tokens", usage.output_tokens, MAX_TOKEN_COUNT),
    ] {
        validate_nonnegative(number, maximum, &format!("observed_usage.{label}"))?;
    }
    if usage.elapsed_ms == value.ended_at_unix_ms - value.started_at_unix_ms {
        Ok(())
    } else {
        Err(invalid(
            "observed_usage.elapsed_ms must equal the wall interval",
        ))
    }
}

fn validate_execution_final(
    value: &ExecutionReceipt,
) -> Result<(), KernelOperationalContractError> {
    if (value.attempt == 1) != value.prior_execution_receipt_ref.is_none() {
        return Err(invalid(
            "attempt one requires null prior receipt; retry requires one",
        ));
    }
    if let Some(reference) = &value.prior_execution_receipt_ref {
        validate_execution_ref(reference)?;
    }
    validate_string_set(&value.reason_codes, "reason_codes", MAX_REASON_CODES)?;
    if (value.outcome == "succeeded") != value.reason_codes.is_empty() {
        return Err(invalid(
            "succeeded requires no reasons; every other outcome requires one",
        ));
    }
    validate_principal(&value.executor, "executor")
}
