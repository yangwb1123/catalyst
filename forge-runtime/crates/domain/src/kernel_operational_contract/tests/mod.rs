use super::*;
use sha2::{Digest, Sha256};

mod graph;
mod strict;

const GOLDEN: &[u8] = include_bytes!(
    "../../../../../../docs/contracts/fixtures/kernel-operational-reference-closure-v1.json"
);

fn golden() -> KernelOperationalReferenceClosure {
    assert_eq!(GOLDEN.last(), Some(&b'\n'));
    decode_closure(&GOLDEN[..GOLDEN.len() - 1]).expect("exact operational golden")
}

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

fn empty_profile() -> KernelOperationalReferenceClosure {
    let source = golden();
    let mut invocation = source.capability_invocations[0].clone();
    invocation.invocation_id.clear();
    invocation.invocation_sha256.clear();
    invocation.input_artifact_receipt_refs.clear();
    invocation.declared_output_slots.clear();
    let invocation = seal_capability_invocation(&invocation).expect("empty invocation");
    let mut receipt = source.execution_receipts[0].clone();
    receipt.execution_receipt_id.clear();
    receipt.execution_receipt_sha256.clear();
    receipt.invocation_ref = invocation_ref(&invocation);
    receipt.event_refs.clear();
    receipt.input_artifacts.clear();
    receipt.output_artifact_receipt_refs.clear();
    receipt.outcome = "succeeded".to_owned();
    receipt.reason_codes.clear();
    let receipt = seal_execution_receipt(&receipt).expect("empty receipt");
    let mut closure = source;
    closure.closure_id.clear();
    closure.closure_sha256.clear();
    closure.artifacts.clear();
    closure.artifact_receipts.clear();
    closure.capability_invocations = vec![invocation];
    closure.interaction_events.clear();
    closure.execution_receipts = vec![receipt];
    seal_closure(&closure).expect("empty closure")
}

#[test]
fn golden_decodes() {
    let closure = golden();
    assert_eq!(
        closure.closure_sha256,
        "1db702583b8dae850413b75b80d620a6031ad452071908e33ea551a4f5feae0e"
    );
    let physical = crate::governance_contract::codec::lower_hex(&Sha256::digest(GOLDEN));
    assert_eq!(
        physical,
        "85f8d9887331fe95e52533c228e40b41750f04dfe10f3a7c77e5a4daff785f2f"
    );
    assert_eq!(
        canonical_json(&closure).expect("canonical"),
        String::from_utf8(GOLDEN[..GOLDEN.len() - 1].to_vec()).expect("UTF-8")
    );
}

#[test]
fn every_record_and_closure_reseals_exactly() {
    let closure = golden();
    for mut value in closure.artifact_receipts.clone() {
        let expected = value.clone();
        value.artifact_receipt_id.clear();
        value.artifact_receipt_sha256.clear();
        assert_eq!(seal_artifact_receipt(&value).expect("reseal"), expected);
    }
    for mut value in closure.capability_invocations.clone() {
        let expected = value.clone();
        value.invocation_id.clear();
        value.invocation_sha256.clear();
        assert_eq!(
            seal_capability_invocation(&value).expect("reseal"),
            expected
        );
    }
    reseal_events_and_receipts(&closure);
    let mut blank = closure.clone();
    blank.closure_id.clear();
    blank.closure_sha256.clear();
    assert_eq!(seal_closure(&blank).expect("closure reseal"), closure);
}

fn reseal_events_and_receipts(closure: &KernelOperationalReferenceClosure) {
    for mut value in closure.interaction_events.clone() {
        let expected = value.clone();
        value.event_id.clear();
        value.event_sha256.clear();
        assert_eq!(seal_interaction_event(&value).expect("reseal"), expected);
    }
    for mut value in closure.execution_receipts.clone() {
        let expected = value.clone();
        value.execution_receipt_id.clear();
        value.execution_receipt_sha256.clear();
        assert_eq!(seal_execution_receipt(&value).expect("reseal"), expected);
    }
}

#[test]
fn empty_io_output_and_event_profile_is_valid() {
    let closure = empty_profile();
    assert!(closure.artifacts.is_empty());
    assert!(closure.artifact_receipts.is_empty());
    assert!(closure.interaction_events.is_empty());
    let raw = canonical_json(&closure).expect("canonical empty profile");
    assert_eq!(
        decode_closure(raw.as_bytes()).expect("empty decode"),
        closure
    );
}
