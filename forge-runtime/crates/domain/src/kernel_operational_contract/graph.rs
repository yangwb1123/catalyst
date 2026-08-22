use std::collections::{BTreeSet, HashMap};

use super::{
    ArtifactReceipt, ArtifactReceiptRef, ArtifactRef, CapabilityInvocation,
    CapabilityInvocationRef, ExecutionReceipt, ExecutionReceiptRef, InteractionEvent,
    InteractionEventRef, KernelOperationalContractError, KernelOperationalReferenceClosure,
    invalid, wire,
};

fn invocation_ref(value: &CapabilityInvocation) -> CapabilityInvocationRef {
    CapabilityInvocationRef {
        invocation_id: value.invocation_id.clone(),
        invocation_sha256: value.invocation_sha256.clone(),
    }
}

fn artifact_receipt_ref(value: &ArtifactReceipt) -> ArtifactReceiptRef {
    ArtifactReceiptRef {
        artifact_receipt_id: value.artifact_receipt_id.clone(),
        artifact_receipt_sha256: value.artifact_receipt_sha256.clone(),
    }
}

fn event_ref(value: &InteractionEvent) -> InteractionEventRef {
    InteractionEventRef {
        event_id: value.event_id.clone(),
        event_sha256: value.event_sha256.clone(),
    }
}

fn execution_ref(value: &ExecutionReceipt) -> ExecutionReceiptRef {
    ExecutionReceiptRef {
        execution_receipt_id: value.execution_receipt_id.clone(),
        execution_receipt_sha256: value.execution_receipt_sha256.clone(),
    }
}

fn validate_common_context(
    closure: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    let base = &closure.capability_invocations[0];
    let artifacts_match = closure
        .artifact_receipts
        .iter()
        .all(|value| value.bindings == base.bindings && value.task_binding == base.task_binding);
    let invocations_match = closure
        .capability_invocations
        .iter()
        .all(|value| value.bindings == base.bindings && value.task_binding == base.task_binding);
    let events_match = closure
        .interaction_events
        .iter()
        .all(|value| value.bindings == base.bindings && value.task_binding == base.task_binding);
    let receipts_match = closure
        .execution_receipts
        .iter()
        .all(|value| value.bindings == base.bindings && value.task_binding == base.task_binding);
    if artifacts_match && invocations_match && events_match && receipts_match {
        Ok(())
    } else {
        Err(invalid(
            "every record must carry identical bindings and TaskBinding",
        ))
    }
}

fn retry_static_matches(first: &CapabilityInvocation, next: &CapabilityInvocation) -> bool {
    first.bindings == next.bindings
        && first.capability == next.capability
        && first.capability_grant_ref == next.capability_grant_ref
        && first.correlation_id == next.correlation_id
        && first.declared_output_slots == next.declared_output_slots
        && first.idempotency_key == next.idempotency_key
        && first.input_artifact_receipt_refs == next.input_artifact_receipt_refs
        && first.requested_action_sha256 == next.requested_action_sha256
        && first.subject == next.subject
        && first.task_binding == next.task_binding
}

fn validate_retry_chain(
    closure: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    if closure.capability_invocations.len() != closure.execution_receipts.len() {
        return Err(invalid(
            "each invocation requires exactly one ExecutionReceipt",
        ));
    }
    let base = &closure.capability_invocations[0];
    for (index, (invocation, receipt)) in closure
        .capability_invocations
        .iter()
        .zip(&closure.execution_receipts)
        .enumerate()
    {
        let attempt = i64::try_from(index + 1).map_err(|_| invalid("attempt overflow"))?;
        if invocation.attempt != attempt || receipt.attempt != attempt {
            return Err(invalid(
                "invocations and receipts must be contiguous attempts 1..N",
            ));
        }
        if !retry_static_matches(base, invocation) {
            return Err(invalid(
                "retry invocations must preserve the complete static request",
            ));
        }
        let prior = index
            .checked_sub(1)
            .map(|prior| execution_ref(&closure.execution_receipts[prior]));
        if invocation.prior_execution_receipt_ref != prior
            || receipt.prior_execution_receipt_ref != prior
        {
            return Err(invalid("prior receipt must be the preceding attempt"));
        }
        validate_retry_time(closure, index, invocation)?;
    }
    Ok(())
}

fn validate_retry_time(
    closure: &KernelOperationalReferenceClosure,
    index: usize,
    invocation: &CapabilityInvocation,
) -> Result<(), KernelOperationalContractError> {
    if index == 0 {
        return Ok(());
    }
    let prior = &closure.execution_receipts[index - 1];
    if prior.outcome == "succeeded" {
        return Err(invalid("a succeeded execution cannot be retried"));
    }
    if invocation.requested_at_unix_ms < prior.ended_at_unix_ms {
        Err(invalid("retry request precedes the prior attempt end"))
    } else {
        Ok(())
    }
}

fn receipt_map(
    values: &[ArtifactReceipt],
) -> Result<HashMap<&str, &ArtifactReceipt>, KernelOperationalContractError> {
    let mut result = HashMap::with_capacity(values.len());
    for value in values {
        if result
            .insert(value.artifact_receipt_id.as_str(), value)
            .is_some()
        {
            return Err(invalid("duplicate ArtifactReceipt identity"));
        }
    }
    Ok(result)
}

fn sorted_artifacts(
    values: impl IntoIterator<Item = ArtifactRef>,
) -> Result<Vec<ArtifactRef>, KernelOperationalContractError> {
    let mut keyed = Vec::new();
    for value in values {
        keyed.push((wire::canonical_typed(&value)?, value));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, value)| value).collect())
}

fn resolve_inputs(
    invocation: &CapabilityInvocation,
    receipt: &ExecutionReceipt,
    available: &HashMap<&str, &ArtifactReceipt>,
    used: &mut BTreeSet<String>,
) -> Result<(), KernelOperationalContractError> {
    let mut artifacts = Vec::with_capacity(invocation.input_artifact_receipt_refs.len());
    let mut seen = BTreeSet::new();
    for reference in &invocation.input_artifact_receipt_refs {
        let member = available
            .get(reference.artifact_receipt_id.as_str())
            .ok_or_else(|| invalid("input ArtifactReceipt reference is unresolved"))?;
        if artifact_receipt_ref(member) != *reference {
            return Err(invalid(
                "input ArtifactReceipt reference digest is unresolved",
            ));
        }
        if member.receipt_role != "declared_input"
            || member.created_at_unix_ms > invocation.requested_at_unix_ms
        {
            return Err(invalid("invocation input role or time is invalid"));
        }
        if !seen.insert(member.artifact.clone()) {
            return Err(invalid(
                "invocation inputs must project distinct ArtifactRefs",
            ));
        }
        used.insert(member.artifact_receipt_id.clone());
        artifacts.push(member.artifact.clone());
    }
    if receipt.input_artifacts == sorted_artifacts(artifacts)? {
        Ok(())
    } else {
        Err(invalid(
            "ExecutionReceipt input_artifacts must exactly project inputs",
        ))
    }
}

fn validate_outputs(
    invocation: &CapabilityInvocation,
    receipt: &ExecutionReceipt,
    values: &[ArtifactReceipt],
    used: &mut BTreeSet<String>,
) -> Result<(), KernelOperationalContractError> {
    let reference = invocation_ref(invocation);
    let outputs: Vec<_> = values
        .iter()
        .filter(|value| value.producer_invocation_ref.as_ref() == Some(&reference))
        .collect();
    let mut slots = Vec::with_capacity(outputs.len());
    let mut refs = Vec::with_capacity(outputs.len());
    for output in outputs {
        if output.created_at_unix_ms < receipt.started_at_unix_ms
            || output.created_at_unix_ms > receipt.ended_at_unix_ms
        {
            return Err(invalid(
                "output ArtifactReceipt time is outside its execution",
            ));
        }
        used.insert(output.artifact_receipt_id.clone());
        slots.push(output.slot.clone());
        refs.push(artifact_receipt_ref(output));
    }
    slots.sort();
    refs.sort_by(|left, right| left.artifact_receipt_id.cmp(&right.artifact_receipt_id));
    if slots.windows(2).any(|pair| pair[0] == pair[1]) || slots != invocation.declared_output_slots
    {
        return Err(invalid(
            "declared_output_slots must exactly cover output receipts",
        ));
    }
    if refs == receipt.output_artifact_receipt_refs {
        Ok(())
    } else {
        Err(invalid(
            "ExecutionReceipt must reference every exact output receipt",
        ))
    }
}

fn events_for<'a>(
    invocation: &CapabilityInvocation,
    receipt: &ExecutionReceipt,
    values: &'a [InteractionEvent],
) -> Result<Vec<&'a InteractionEvent>, KernelOperationalContractError> {
    let reference = invocation_ref(invocation);
    let members: Vec<_> = values
        .iter()
        .filter(|value| value.invocation_ref == reference)
        .collect();
    let mut refs: Vec<InteractionEventRef> = Vec::with_capacity(members.len());
    for (index, event) in members.iter().enumerate() {
        if event.logical_sequence != i64::try_from(index + 1).unwrap_or(i64::MAX) {
            return Err(invalid(
                "events must be contiguous sequences per invocation",
            ));
        }
        let cause = index.checked_sub(1).map(|prior| refs[prior].clone());
        if event.causation_event_ref != cause {
            return Err(invalid("event causation must be immediately preceding"));
        }
        if event.occurred_at_unix_ms < receipt.started_at_unix_ms
            || event.occurred_at_unix_ms > receipt.ended_at_unix_ms
        {
            return Err(invalid("event time is outside its execution"));
        }
        if index > 0 && event.occurred_at_unix_ms < members[index - 1].occurred_at_unix_ms {
            return Err(invalid(
                "event times must be nondecreasing by logical sequence",
            ));
        }
        refs.push(event_ref(event));
    }
    if refs == receipt.event_refs {
        Ok(members)
    } else {
        Err(invalid(
            "ExecutionReceipt event_refs must exactly cover its events",
        ))
    }
}

fn validate_attempt<'a>(
    invocation: &CapabilityInvocation,
    receipt: &ExecutionReceipt,
    artifact_map: &HashMap<&str, &ArtifactReceipt>,
    artifacts: &[ArtifactReceipt],
    events: &'a [InteractionEvent],
    used: &mut BTreeSet<String>,
) -> Result<Vec<&'a InteractionEvent>, KernelOperationalContractError> {
    if receipt.invocation_ref != invocation_ref(invocation) {
        return Err(invalid(
            "ExecutionReceipt must reference its matching invocation",
        ));
    }
    if receipt.correlation_id != invocation.correlation_id {
        return Err(invalid("receipt and invocation correlation_id differ"));
    }
    if receipt.started_at_unix_ms < invocation.requested_at_unix_ms {
        return Err(invalid("execution starts before its invocation request"));
    }
    resolve_inputs(invocation, receipt, artifact_map, used)?;
    validate_outputs(invocation, receipt, artifacts, used)?;
    let members = events_for(invocation, receipt, events)?;
    if members
        .iter()
        .any(|event| event.correlation_id != invocation.correlation_id)
    {
        return Err(invalid("event and invocation correlation_id differ"));
    }
    Ok(members)
}

fn validate_artifact_inventory(
    closure: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    let mut created: HashMap<ArtifactRef, Vec<i64>> = HashMap::new();
    let mut projected = BTreeSet::new();
    for receipt in &closure.artifact_receipts {
        created
            .entry(receipt.artifact.clone())
            .or_default()
            .push(receipt.created_at_unix_ms);
        projected.insert(receipt.artifact.clone());
    }
    for event in &closure.interaction_events {
        for artifact in &event.artifact_refs {
            let nonfuture = created.get(artifact).is_some_and(|times| {
                times
                    .iter()
                    .any(|created| *created <= event.occurred_at_unix_ms)
            });
            if !nonfuture {
                return Err(invalid(
                    "Event ArtifactRef needs a non-future included receipt",
                ));
            }
            projected.insert(artifact.clone());
        }
    }
    for receipt in &closure.execution_receipts {
        projected.extend(receipt.input_artifacts.iter().cloned());
    }
    if closure.artifacts == sorted_artifacts(projected)? {
        Ok(())
    } else {
        Err(invalid(
            "closure artifacts must exactly equal all referenced ArtifactRefs",
        ))
    }
}

pub(super) fn validate_reference_graph(
    closure: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    validate_common_context(closure)?;
    validate_retry_chain(closure)?;
    let artifact_map = receipt_map(&closure.artifact_receipts)?;
    let mut used = BTreeSet::new();
    let mut flattened = Vec::with_capacity(closure.interaction_events.len());
    for (invocation, receipt) in closure
        .capability_invocations
        .iter()
        .zip(&closure.execution_receipts)
    {
        flattened.extend(validate_attempt(
            invocation,
            receipt,
            &artifact_map,
            &closure.artifact_receipts,
            &closure.interaction_events,
            &mut used,
        )?);
    }
    if flattened != closure.interaction_events.iter().collect::<Vec<_>>() {
        return Err(invalid(
            "interaction_events must be ordered by attempt then sequence",
        ));
    }
    if used.len() != artifact_map.len() {
        return Err(invalid("ArtifactReceipt inventory contains an orphan"));
    }
    validate_artifact_inventory(closure)
}
