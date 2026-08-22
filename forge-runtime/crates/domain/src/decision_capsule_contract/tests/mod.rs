use sha2::{Digest, Sha256};

use super::*;

mod contract;
mod graph;
mod graph_fixture {
    use std::collections::HashMap;

    use crate::{
        kernel_decision_contract as decision,
        kernel_operational_contract::{self as operational, ArtifactRef},
    };

    fn invocation_ref(
        value: &operational::CapabilityInvocation,
    ) -> operational::CapabilityInvocationRef {
        operational::CapabilityInvocationRef {
            invocation_id: value.invocation_id.clone(),
            invocation_sha256: value.invocation_sha256.clone(),
        }
    }

    fn execution_ref(value: &operational::ExecutionReceipt) -> operational::ExecutionReceiptRef {
        operational::ExecutionReceiptRef {
            execution_receipt_id: value.execution_receipt_id.clone(),
            execution_receipt_sha256: value.execution_receipt_sha256.clone(),
        }
    }

    fn artifact_receipt_ref(
        value: &operational::ArtifactReceipt,
    ) -> operational::ArtifactReceiptRef {
        operational::ArtifactReceiptRef {
            artifact_receipt_id: value.artifact_receipt_id.clone(),
            artifact_receipt_sha256: value.artifact_receipt_sha256.clone(),
        }
    }

    fn event_ref(value: &operational::InteractionEvent) -> operational::InteractionEventRef {
        operational::InteractionEventRef {
            event_id: value.event_id.clone(),
            event_sha256: value.event_sha256.clone(),
        }
    }

    fn source_reference_id(value: &serde_json::Value) -> Option<&str> {
        value
            .as_object()?
            .iter()
            .find_map(|(field, member)| field.ends_with("_id").then(|| member.as_str()).flatten())
    }

    fn reference_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).expect("reference JSON")
    }

    #[derive(Default)]
    struct RetryMappings {
        artifact_receipts: HashMap<String, operational::ArtifactReceiptRef>,
        events: HashMap<String, operational::InteractionEventRef>,
        source_refs: HashMap<String, serde_json::Value>,
    }

    fn lost_first_receipt(
        mut value: operational::ExecutionReceipt,
    ) -> operational::ExecutionReceipt {
        value.execution_receipt_id.clear();
        value.execution_receipt_sha256.clear();
        value.outcome = "lost".to_owned();
        value.reason_codes = vec!["fixture_lost".to_owned()];
        operational::seal_execution_receipt(&value).expect("lost receipt")
    }

    fn retry_invocation(
        mut value: operational::CapabilityInvocation,
        first: &operational::ExecutionReceipt,
    ) -> operational::CapabilityInvocation {
        value.invocation_id.clear();
        value.invocation_sha256.clear();
        value.prior_execution_receipt_ref = Some(execution_ref(first));
        operational::seal_capability_invocation(&value).expect("retry")
    }

    fn reseal_retry_receipts(
        closure: &mut operational::KernelOperationalReferenceClosure,
        old_ref: &operational::CapabilityInvocationRef,
        new_ref: &operational::CapabilityInvocationRef,
        mappings: &mut RetryMappings,
    ) {
        for receipt in &mut closure.artifact_receipts {
            if receipt.producer_invocation_ref.as_ref() != Some(old_ref) {
                continue;
            }
            let old_id = receipt.artifact_receipt_id.clone();
            receipt.artifact_receipt_id.clear();
            receipt.artifact_receipt_sha256.clear();
            receipt.producer_invocation_ref = Some(new_ref.clone());
            *receipt = operational::seal_artifact_receipt(receipt).expect("retry artifact receipt");
            let replacement = artifact_receipt_ref(receipt);
            mappings
                .source_refs
                .insert(old_id.clone(), reference_value(&replacement));
            mappings.artifact_receipts.insert(old_id, replacement);
        }
    }

    fn reseal_retry_events(
        closure: &mut operational::KernelOperationalReferenceClosure,
        old_ref: &operational::CapabilityInvocationRef,
        new_ref: &operational::CapabilityInvocationRef,
        mappings: &mut RetryMappings,
    ) {
        let mut prior = None;
        for event in &mut closure.interaction_events {
            if &event.invocation_ref != old_ref {
                continue;
            }
            let old_id = event.event_id.clone();
            event.event_id.clear();
            event.event_sha256.clear();
            event.invocation_ref = new_ref.clone();
            event.causation_event_ref = prior;
            *event = operational::seal_interaction_event(event).expect("retry event");
            let replacement = event_ref(event);
            prior = Some(replacement.clone());
            mappings
                .source_refs
                .insert(old_id.clone(), reference_value(&replacement));
            mappings.events.insert(old_id, replacement);
        }
    }

    fn successful_retry_receipt(
        mut value: operational::ExecutionReceipt,
        first: &operational::ExecutionReceipt,
        invocation_ref: operational::CapabilityInvocationRef,
        mappings: &mut RetryMappings,
    ) -> operational::ExecutionReceipt {
        let old_id = value.execution_receipt_id.clone();
        value.execution_receipt_id.clear();
        value.execution_receipt_sha256.clear();
        value.invocation_ref = invocation_ref;
        value.prior_execution_receipt_ref = Some(execution_ref(first));
        for reference in &mut value.event_refs {
            if let Some(replacement) = mappings.events.get(&reference.event_id) {
                *reference = replacement.clone();
            }
        }
        for reference in &mut value.output_artifact_receipt_refs {
            if let Some(replacement) = mappings
                .artifact_receipts
                .get(&reference.artifact_receipt_id)
            {
                *reference = replacement.clone();
            }
        }
        let sealed = operational::seal_execution_receipt(&value).expect("successful retry receipt");
        mappings
            .source_refs
            .insert(old_id, reference_value(&execution_ref(&sealed)));
        sealed
    }

    fn lost_operational_closure(
        source: &operational::KernelOperationalReferenceClosure,
    ) -> (
        operational::KernelOperationalReferenceClosure,
        RetryMappings,
    ) {
        let mut closure = source.clone();
        let old_first = closure.execution_receipts[0].clone();
        let old_second = closure.execution_receipts[1].clone();
        let old_invocation = closure.capability_invocations[1].clone();
        let old_first_id = old_first.execution_receipt_id.clone();
        let first = lost_first_receipt(old_first);
        let invocation = retry_invocation(old_invocation.clone(), &first);
        let old_ref = invocation_ref(&old_invocation);
        let new_ref = invocation_ref(&invocation);
        let mut mappings = RetryMappings::default();
        mappings
            .source_refs
            .insert(old_first_id, reference_value(&execution_ref(&first)));
        mappings
            .source_refs
            .insert(old_invocation.invocation_id, reference_value(&new_ref));
        reseal_retry_receipts(&mut closure, &old_ref, &new_ref, &mut mappings);
        reseal_retry_events(&mut closure, &old_ref, &new_ref, &mut mappings);
        let second = successful_retry_receipt(old_second, &first, new_ref, &mut mappings);
        closure.capability_invocations[1] = invocation;
        closure.execution_receipts = vec![first, second];
        closure
            .artifact_receipts
            .sort_by(|left, right| left.artifact_receipt_id.cmp(&right.artifact_receipt_id));
        closure.closure_id.clear();
        closure.closure_sha256.clear();
        let sealed = operational::seal_closure(&closure).expect("lost/retry operational closure");
        (sealed, mappings)
    }

    fn reseal_retry_atoms(
        closure: &mut decision::KernelDecisionReferenceClosure,
        mappings: &RetryMappings,
    ) {
        for atom in &mut closure.cognitive_atoms {
            if atom.source.source_phase != "postdecision" {
                continue;
            }
            let Some(old_id) = source_reference_id(&atom.source.source_ref) else {
                continue;
            };
            let Some(replacement) = mappings.source_refs.get(old_id) else {
                continue;
            };
            atom.atom_id.clear();
            atom.atom_sha256.clear();
            atom.source.source_ref = replacement.clone();
            *atom = decision::seal_cognitive_atom(atom).expect("retry source atom");
        }
        closure
            .cognitive_atoms
            .sort_by(|left, right| left.atom_id.cmp(&right.atom_id));
    }

    pub(super) fn lost_decision_closure(
        source: &decision::KernelDecisionReferenceClosure,
    ) -> decision::KernelDecisionReferenceClosure {
        let mut closure = source.clone();
        let (operational, mappings) = lost_operational_closure(&closure.operational_closure);
        closure.operational_closure = operational;
        reseal_retry_atoms(&mut closure, &mappings);
        closure.closure_id.clear();
        closure.closure_sha256.clear();
        decision::seal_closure(&closure).expect("lost/retry decision closure")
    }

    fn sort_canonical<T: serde::Serialize>(values: &mut [T]) {
        values.sort_by_cached_key(|value| {
            operational::canonical_json(value).expect("canonical sort key")
        });
    }

    fn worst_escaped_artifact(index: usize) -> ArtifactRef {
        ArtifactRef {
            artifact_kind: if index == 0 {
                "reflection_report".to_owned()
            } else {
                "\"".repeat(160)
            },
            artifact_ref: format!("{index:03}{}", "\\".repeat(4093)),
            artifact_sha256: format!("{:064x}", index + 1),
        }
    }

    fn maximum_receipt_sets(
        source: &decision::KernelDecisionReferenceClosure,
    ) -> (
        Vec<operational::ArtifactReceipt>,
        Vec<operational::ArtifactReceipt>,
    ) {
        let receipts = &source.operational_closure.artifact_receipts;
        let input_base = receipts
            .iter()
            .find(|receipt| receipt.receipt_role == "declared_input")
            .expect("input receipt");
        let output_base = receipts
            .iter()
            .find(|receipt| receipt.receipt_role == "declared_output")
            .expect("output receipt");
        let inputs = (0..32)
            .map(|index| {
                let mut receipt = input_base.clone();
                receipt.artifact_receipt_id.clear();
                receipt.artifact_receipt_sha256.clear();
                receipt.artifact = worst_escaped_artifact(index);
                receipt.slot = format!("input-{index:02}");
                operational::seal_artifact_receipt(&receipt).expect("input receipt")
            })
            .collect();
        let pending_outputs = (32..64)
            .map(|index| {
                let mut receipt = output_base.clone();
                receipt.artifact_receipt_id.clear();
                receipt.artifact_receipt_sha256.clear();
                receipt.artifact = worst_escaped_artifact(index);
                receipt.slot = format!("output-{index:02}");
                receipt
            })
            .collect();
        (inputs, pending_outputs)
    }

    fn maximum_transaction(
        source: &decision::KernelDecisionReferenceClosure,
        inputs: &[operational::ArtifactReceipt],
    ) -> decision::DecisionTransaction {
        let mut transaction = source.decision_transaction.clone();
        transaction.decision_transaction_id.clear();
        transaction.decision_transaction_sha256.clear();
        transaction.read_artifact_receipt_refs = inputs.iter().map(artifact_receipt_ref).collect();
        sort_canonical(&mut transaction.read_artifact_receipt_refs);
        transaction.write_slots = (32..64).map(|index| format!("output-{index:02}")).collect();
        decision::seal_decision_transaction(&transaction).expect("maximum transaction")
    }

    fn maximum_invocation(
        source: &decision::KernelDecisionReferenceClosure,
        transaction: &decision::DecisionTransaction,
    ) -> operational::CapabilityInvocation {
        let mut invocation = source.operational_closure.capability_invocations[0].clone();
        invocation.invocation_id.clear();
        invocation.invocation_sha256.clear();
        invocation.correlation_id = transaction.decision_transaction_id.clone();
        invocation.declared_output_slots = transaction.write_slots.clone();
        invocation.input_artifact_receipt_refs = transaction.read_artifact_receipt_refs.clone();
        operational::seal_capability_invocation(&invocation).expect("maximum invocation")
    }

    fn maximum_outputs(
        pending: Vec<operational::ArtifactReceipt>,
        invocation: &operational::CapabilityInvocationRef,
    ) -> Vec<operational::ArtifactReceipt> {
        pending
            .into_iter()
            .map(|mut receipt| {
                receipt.producer_invocation_ref = Some(invocation.clone());
                operational::seal_artifact_receipt(&receipt).expect("output receipt")
            })
            .collect()
    }

    fn maximum_execution_receipt(
        source: &decision::KernelDecisionReferenceClosure,
        transaction: &decision::DecisionTransaction,
        invocation: operational::CapabilityInvocationRef,
        inputs: &[operational::ArtifactReceipt],
        outputs: &[operational::ArtifactReceipt],
    ) -> operational::ExecutionReceipt {
        let mut receipt = source.operational_closure.execution_receipts[0].clone();
        receipt.execution_receipt_id.clear();
        receipt.execution_receipt_sha256.clear();
        receipt.correlation_id = transaction.decision_transaction_id.clone();
        receipt.event_refs.clear();
        receipt.input_artifacts = inputs.iter().map(|item| item.artifact.clone()).collect();
        sort_canonical(&mut receipt.input_artifacts);
        receipt.invocation_ref = invocation;
        receipt.outcome = "succeeded".to_owned();
        receipt.output_artifact_receipt_refs = outputs.iter().map(artifact_receipt_ref).collect();
        sort_canonical(&mut receipt.output_artifact_receipt_refs);
        receipt.reason_codes.clear();
        operational::seal_execution_receipt(&receipt).expect("maximum receipt")
    }

    fn maximum_operational_closure(
        source: &decision::KernelDecisionReferenceClosure,
        invocation: operational::CapabilityInvocation,
        receipt: operational::ExecutionReceipt,
        inputs: &[operational::ArtifactReceipt],
        outputs: &[operational::ArtifactReceipt],
    ) -> operational::KernelOperationalReferenceClosure {
        let mut closure = source.operational_closure.clone();
        closure.closure_id.clear();
        closure.closure_sha256.clear();
        closure.artifact_receipts = inputs.iter().chain(outputs).cloned().collect();
        closure
            .artifact_receipts
            .sort_by(|left, right| left.artifact_receipt_id.cmp(&right.artifact_receipt_id));
        closure.artifacts = inputs
            .iter()
            .chain(outputs)
            .map(|item| item.artifact.clone())
            .collect();
        sort_canonical(&mut closure.artifacts);
        closure.capability_invocations = vec![invocation];
        closure.execution_receipts = vec![receipt];
        closure.interaction_events.clear();
        operational::seal_closure(&closure).expect("maximum operational closure")
    }

    pub(super) fn worst_escaped_decision_closure(
        source: &decision::KernelDecisionReferenceClosure,
    ) -> decision::KernelDecisionReferenceClosure {
        let (inputs, pending_outputs) = maximum_receipt_sets(source);
        let transaction = maximum_transaction(source, &inputs);
        let invocation = maximum_invocation(source, &transaction);
        let invocation_reference = invocation_ref(&invocation);
        let outputs = maximum_outputs(pending_outputs, &invocation_reference);
        let receipt = maximum_execution_receipt(
            source,
            &transaction,
            invocation_reference,
            &inputs,
            &outputs,
        );
        let operational =
            maximum_operational_closure(source, invocation, receipt, &inputs, &outputs);
        let mut closure = source.clone();
        closure.closure_id.clear();
        closure.closure_sha256.clear();
        closure
            .cognitive_atoms
            .retain(|atom| atom.source.source_phase == "predecision");
        closure.decision_transaction = transaction;
        closure.operational_closure = operational;
        decision::seal_closure(&closure).expect("maximum decision closure")
    }
}
mod strict;

const GOLDEN: &[u8] = include_bytes!(
    "../../../../../../docs/contracts/fixtures/decision-capsule-structural-replay-v1.json"
);
const SCHEMA: &[u8] = include_bytes!(
    "../../../../../../docs/contracts/decision-capsule-structural-replay-core-v1.schema.json"
);

fn golden() -> StructuralReplayClosure {
    assert_eq!(GOLDEN.last(), Some(&b'\n'));
    decode_structural_replay_closure(&GOLDEN[..GOLDEN.len() - 1]).expect("exact ADR-0092 golden")
}

#[test]
fn physical_canonical_and_four_object_pins_match() {
    let value = golden();
    assert_eq!(
        crate::governance_contract::codec::lower_hex(&Sha256::digest(SCHEMA)),
        "6145c150c8be7ee3934e9d93aec6ab89ddbe4cb6ba77a69b88d2e586616eae1f"
    );
    assert_eq!(
        crate::governance_contract::codec::lower_hex(&Sha256::digest(GOLDEN)),
        "d54494f49851cc4146905bbd64c0815fe7d79704476c0aeb1113f270d5cbb2d0"
    );
    assert_eq!(
        value.decision_capsule.replay_manifest.manifest_sha256,
        "40d1fa34a2fc9b31856d3f16edd1cc346f47d0b447040539b667279f0f67365c"
    );
    assert_eq!(
        value.decision_capsule.capsule_sha256,
        "f02c172fb5d65a36841361a9969dd8ad79eae08c548d1c6d0bbea5a564276b59"
    );
    assert_eq!(
        value.evaluation_branch.branch_sha256,
        "4442cf99caa21eda32a1c4062cfe66b333dff5188f4b818a9c69bf5cb829949a"
    );
    assert_eq!(
        value.closure_sha256,
        "38f14574e9a9531371d55800f1f77bbdb79648a121c0f774a2a9c0083cf13497"
    );
    assert_eq!(
        canonical_json(&value).expect("canonical"),
        std::str::from_utf8(&GOLDEN[..GOLDEN.len() - 1]).expect("UTF-8")
    );
}

#[test]
fn deterministic_full_derivation_matches_golden() {
    let expected = golden();
    let capsule = derive_decision_capsule(&expected.decision_capsule.decision_closure)
        .expect("capsule derivation");
    assert_eq!(capsule, expected.decision_capsule);
    let outer =
        derive_structural_replay_closure(&capsule, &expected.reflection_report_artifact_refs)
            .expect("outer derivation");
    assert_eq!(outer, expected);
}
