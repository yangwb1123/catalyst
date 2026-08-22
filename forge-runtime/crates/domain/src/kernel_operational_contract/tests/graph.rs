use super::*;

#[test]
fn record_and_closure_cardinality_n_and_n_plus_one() {
    let closure = golden();
    let mut invocation = closure.capability_invocations[0].clone();
    invocation.invocation_id.clear();
    invocation.invocation_sha256.clear();
    invocation.declared_output_slots = (0..MAX_IO_ITEMS)
        .map(|index| format!("slot-{index:02}"))
        .collect();
    assert!(seal_capability_invocation(&invocation).is_ok());
    invocation.declared_output_slots.push("slot-32".to_owned());
    assert!(seal_capability_invocation(&invocation).is_err());
    event_artifact_bounds(&closure.interaction_events[0]);
    closure_count_bounds(&closure);
}

fn event_artifact_bounds(source: &InteractionEvent) {
    let mut event = source.clone();
    event.event_id.clear();
    event.event_sha256.clear();
    event.artifact_refs = (0..MAX_IO_ITEMS)
        .map(|index| ArtifactRef {
            artifact_kind: "fixture".to_owned(),
            artifact_ref: format!("fixture/{index:02}"),
            artifact_sha256: format!("{index:064x}"),
        })
        .collect();
    assert!(seal_interaction_event(&event).is_ok());
    event.artifact_refs.push(ArtifactRef {
        artifact_kind: "fixture".to_owned(),
        artifact_ref: "fixture/32".to_owned(),
        artifact_sha256: format!("{:064x}", 32),
    });
    assert!(seal_interaction_event(&event).is_err());
}

fn closure_count_bounds(source: &KernelOperationalReferenceClosure) {
    let mut value = source.clone();
    value.artifacts = vec![source.artifacts[0].clone(); MAX_ARTIFACTS];
    value.artifact_receipts = vec![source.artifact_receipts[0].clone(); MAX_ARTIFACT_RECEIPTS];
    value.capability_invocations = vec![source.capability_invocations[0].clone(); MAX_INVOCATIONS];
    value.interaction_events = vec![source.interaction_events[0].clone(); MAX_EVENTS];
    value.execution_receipts = vec![source.execution_receipts[0].clone(); MAX_EXECUTION_RECEIPTS];
    assert!(closure::validate_counts(&value).is_ok());
    let lengths = [
        MAX_ARTIFACTS + 1,
        MAX_ARTIFACT_RECEIPTS + 1,
        MAX_INVOCATIONS + 1,
        MAX_EVENTS + 1,
        MAX_EXECUTION_RECEIPTS + 1,
    ];
    for (index, length) in lengths.into_iter().enumerate() {
        let mut candidate = value.clone();
        exceed_one_count(&mut candidate, index, length, source);
        assert!(closure::validate_counts(&candidate).is_err());
    }
}

fn exceed_one_count(
    value: &mut KernelOperationalReferenceClosure,
    index: usize,
    length: usize,
    source: &KernelOperationalReferenceClosure,
) {
    match index {
        0 => value.artifacts = vec![source.artifacts[0].clone(); length],
        1 => value.artifact_receipts = vec![source.artifact_receipts[0].clone(); length],
        2 => value.capability_invocations = vec![source.capability_invocations[0].clone(); length],
        3 => value.interaction_events = vec![source.interaction_events[0].clone(); length],
        _ => value.execution_receipts = vec![source.execution_receipts[0].clone(); length],
    }
}

#[test]
fn attempt_elapsed_usage_and_wall_projection_bounds() {
    let closure = golden();
    let mut invocation = closure.capability_invocations[0].clone();
    invocation.invocation_id.clear();
    invocation.invocation_sha256.clear();
    invocation.attempt = MAX_ATTEMPT;
    let digest = "d".repeat(64);
    invocation.prior_execution_receipt_ref = Some(ExecutionReceiptRef {
        execution_receipt_id: format!("execution-receipt-{digest}"),
        execution_receipt_sha256: digest,
    });
    assert!(seal_capability_invocation(&invocation).is_ok());
    invocation.attempt += 1;
    assert!(seal_capability_invocation(&invocation).is_err());
    elapsed_usage_bounds(&closure.execution_receipts[0]);
}

fn elapsed_usage_bounds(source: &ExecutionReceipt) {
    let mut receipt = source.clone();
    receipt.execution_receipt_id.clear();
    receipt.execution_receipt_sha256.clear();
    receipt.ended_at_unix_ms = receipt.started_at_unix_ms + MAX_ELAPSED_MS;
    receipt.observed_usage.elapsed_ms = MAX_ELAPSED_MS;
    receipt.observed_usage.call_count = MAX_CALL_COUNT;
    receipt.observed_usage.cost_usd_micros = MAX_COST_MICROS;
    receipt.observed_usage.input_tokens = MAX_TOKEN_COUNT;
    receipt.observed_usage.network_bytes = MAX_NETWORK_BYTES;
    receipt.observed_usage.output_bytes = MAX_OUTPUT_BYTES;
    receipt.observed_usage.output_tokens = MAX_TOKEN_COUNT;
    assert!(seal_execution_receipt(&receipt).is_ok());
    receipt.ended_at_unix_ms += 1;
    receipt.observed_usage.elapsed_ms += 1;
    assert!(seal_execution_receipt(&receipt).is_err());
    receipt.ended_at_unix_ms -= 1;
    receipt.observed_usage.elapsed_ms -= 2;
    assert!(seal_execution_receipt(&receipt).is_err());
}

fn future_artifact_candidate(future: bool) -> KernelOperationalReferenceClosure {
    let source = golden();
    let mut invocation = source.capability_invocations[0].clone();
    invocation.invocation_id.clear();
    invocation.invocation_sha256.clear();
    invocation.input_artifact_receipt_refs.clear();
    let invocation = seal_capability_invocation(&invocation).expect("invocation");
    let mut output = source.artifact_receipts[0].clone();
    output.artifact_receipt_id.clear();
    output.artifact_receipt_sha256.clear();
    output.producer_invocation_ref = Some(invocation_ref(&invocation));
    let output = seal_artifact_receipt(&output).expect("output");
    let mut event = source.interaction_events[0].clone();
    event.event_id.clear();
    event.event_sha256.clear();
    event.invocation_ref = invocation_ref(&invocation);
    event.artifact_refs = vec![output.artifact.clone()];
    event.occurred_at_unix_ms = output.created_at_unix_ms - i64::from(future);
    let event = seal_interaction_event(&event).expect("event");
    assemble_single_attempt(&source, invocation, output, event)
}

fn assemble_single_attempt(
    source: &KernelOperationalReferenceClosure,
    invocation: CapabilityInvocation,
    output: ArtifactReceipt,
    event: InteractionEvent,
) -> KernelOperationalReferenceClosure {
    let mut receipt = source.execution_receipts[0].clone();
    receipt.execution_receipt_id.clear();
    receipt.execution_receipt_sha256.clear();
    receipt.invocation_ref = invocation_ref(&invocation);
    receipt.event_refs = vec![event_ref(&event)];
    receipt.input_artifacts.clear();
    receipt.output_artifact_receipt_refs = vec![artifact_receipt_ref(&output)];
    let receipt = seal_execution_receipt(&receipt).expect("receipt");
    let mut candidate = source.clone();
    candidate.closure_id.clear();
    candidate.closure_sha256.clear();
    candidate.artifacts = vec![output.artifact.clone()];
    candidate.artifact_receipts = vec![output];
    candidate.capability_invocations = vec![invocation];
    candidate.interaction_events = vec![event];
    candidate.execution_receipts = vec![receipt];
    candidate
}

#[test]
fn event_artifact_requires_nonfuture_receipt() {
    assert!(seal_closure(&future_artifact_candidate(false)).is_ok());
    assert!(seal_closure(&future_artifact_candidate(true)).is_err());
}

#[test]
fn graph_rejects_retry_reference_time_projection_orphan_and_context_drift() {
    let source = golden();
    for candidate in graph_drifts(&source) {
        assert!(
            crate::kernel_operational_contract::graph::validate_reference_graph(&candidate)
                .is_err()
        );
    }
}

#[test]
fn event_times_are_nondecreasing_with_equality_allowed() {
    let source = golden();
    let mut equal = source.clone();
    equal.interaction_events[0].occurred_at_unix_ms =
        equal.interaction_events[1].occurred_at_unix_ms;
    assert!(crate::kernel_operational_contract::graph::validate_reference_graph(&equal).is_ok());
    let mut reversed = source;
    reversed.interaction_events[1].occurred_at_unix_ms =
        reversed.interaction_events[0].occurred_at_unix_ms - 1;
    assert!(
        crate::kernel_operational_contract::graph::validate_reference_graph(&reversed).is_err()
    );
}

fn graph_drifts(
    source: &KernelOperationalReferenceClosure,
) -> Vec<KernelOperationalReferenceClosure> {
    let mut values = Vec::new();
    let mut input_projection = source.clone();
    input_projection.execution_receipts[0]
        .input_artifacts
        .clear();
    values.push(input_projection);
    let mut output_projection = source.clone();
    output_projection.capability_invocations[0]
        .declared_output_slots
        .clear();
    values.push(output_projection);
    let mut successful_retry = source.clone();
    successful_retry.execution_receipts[0].outcome = "succeeded".to_owned();
    successful_retry.execution_receipts[0].reason_codes.clear();
    values.push(successful_retry);
    values.extend(more_graph_drifts(source));
    values
}

fn more_graph_drifts(
    source: &KernelOperationalReferenceClosure,
) -> Vec<KernelOperationalReferenceClosure> {
    let mut context = source.clone();
    context.interaction_events[0].bindings.policy_sha256 = "e".repeat(64);
    let mut unresolved = source.clone();
    let digest = "d".repeat(64);
    unresolved.capability_invocations[0].input_artifact_receipt_refs[0] = ArtifactReceiptRef {
        artifact_receipt_id: format!("artifact-receipt-{digest}"),
        artifact_receipt_sha256: digest.clone(),
    };
    let mut orphan = source.clone();
    let mut extra = orphan.artifact_receipts[2].clone();
    extra.artifact_receipt_id = format!("artifact-receipt-{digest}");
    extra.artifact_receipt_sha256 = digest;
    orphan.artifact_receipts.push(extra);
    let mut cause = source.clone();
    cause.interaction_events[1].causation_event_ref = None;
    let mut output_time = source.clone();
    output_time.artifact_receipts[0].created_at_unix_ms =
        output_time.execution_receipts[0].started_at_unix_ms - 1;
    vec![context, unresolved, orphan, cause, output_time]
}
