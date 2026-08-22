use crate::kernel_operational_contract::ArtifactRef;

use super::graph_fixture::{lost_decision_closure, worst_escaped_decision_closure};
use super::{golden, *};

fn blank_manifest(mut value: StructuralReplayManifest) -> StructuralReplayManifest {
    value.manifest_id.clear();
    value.manifest_sha256.clear();
    value
}

macro_rules! reject_inventory_mutations {
    ($source:expr, $field:ident, $closure:expr) => {{
        let values = $source.$field.clone();
        let mut omitted = blank_manifest($source.clone());
        omitted.$field.pop();
        assert!(seal_structural_replay_manifest(&omitted, $closure).is_err());
        let mut duplicate = blank_manifest($source.clone());
        duplicate.$field.push(values[0].clone());
        assert!(seal_structural_replay_manifest(&duplicate, $closure).is_err());
        if values.len() > 1 {
            let mut reordered = blank_manifest($source.clone());
            reordered.$field.reverse();
            assert!(seal_structural_replay_manifest(&reordered, $closure).is_err());
        }
    }};
}

fn reflection_refs(count: usize) -> Vec<ArtifactRef> {
    let mut refs: Vec<_> = (0..count)
        .map(|index| ArtifactRef {
            artifact_kind: "reflection_report".to_owned(),
            artifact_ref: format!("fixture/reflection/{index:02}"),
            artifact_sha256: format!("{:064x}", index + 1),
        })
        .collect();
    refs.sort_by_key(|item| canonical_json(item).expect("canonical ArtifactRef"));
    refs
}

#[test]
fn manifest_is_the_complete_exact_ordered_projection() {
    let value = golden();
    let capsule = &value.decision_capsule;
    let manifest = &capsule.replay_manifest;
    assert_eq!(
        derive_structural_replay_manifest(&capsule.decision_closure).expect("manifest derivation"),
        *manifest
    );
    let operational = &capsule.decision_closure.operational_closure;
    assert_eq!(manifest.artifact_refs.len(), operational.artifacts.len());
    assert_eq!(
        manifest.artifact_receipt_refs.len(),
        operational.artifact_receipts.len()
    );
    assert_eq!(
        manifest.capability_invocation_refs.len(),
        operational.capability_invocations.len()
    );
    assert_eq!(
        manifest.interaction_event_refs.len(),
        operational.interaction_events.len()
    );
    assert_eq!(
        manifest.execution_receipt_refs.len(),
        operational.execution_receipts.len()
    );
    assert!(manifest.predecision_atom_refs.len() + manifest.postdecision_atom_refs.len() <= 256);
}

#[test]
fn every_manifest_inventory_rejects_omit_duplicate_and_reorder() {
    let value = golden();
    let source = &value.decision_capsule.replay_manifest;
    let closure = &value.decision_capsule.decision_closure;
    reject_inventory_mutations!(source, artifact_refs, closure);
    reject_inventory_mutations!(source, artifact_receipt_refs, closure);
    reject_inventory_mutations!(source, capability_invocation_refs, closure);
    reject_inventory_mutations!(source, interaction_event_refs, closure);
    reject_inventory_mutations!(source, execution_receipt_refs, closure);
    reject_inventory_mutations!(source, predecision_atom_refs, closure);
    reject_inventory_mutations!(source, postdecision_atom_refs, closure);
}

#[test]
fn failed_lost_retry_and_success_attempts_are_preserved() {
    let value = golden();
    let operational = &value.decision_capsule.decision_closure.operational_closure;
    let outcomes: Vec<_> = operational
        .execution_receipts
        .iter()
        .map(|receipt| receipt.outcome.as_str())
        .collect();
    assert_eq!(outcomes, ["failed", "succeeded"]);
    assert_eq!(
        value
            .decision_capsule
            .replay_manifest
            .capability_invocation_refs
            .len(),
        2
    );
    assert_eq!(
        value
            .decision_capsule
            .replay_manifest
            .execution_receipt_refs
            .len(),
        2
    );
    assert!(
        operational.capability_invocations[1]
            .prior_execution_receipt_ref
            .is_some()
    );
    let lost = lost_decision_closure(&value.decision_capsule.decision_closure);
    assert_eq!(
        lost.operational_closure.execution_receipts[0].outcome,
        "lost"
    );
    assert_eq!(
        lost.operational_closure.execution_receipts[1].outcome,
        "succeeded"
    );
    assert!(
        lost.operational_closure.capability_invocations[1]
            .prior_execution_receipt_ref
            .is_some()
    );
    let manifest = derive_structural_replay_manifest(&lost).expect("lost/retry manifest");
    assert_eq!(manifest.capability_invocation_refs.len(), 2);
    assert_eq!(manifest.execution_receipt_refs.len(), 2);
    let capsule = derive_decision_capsule(&lost).expect("lost/retry capsule");
    derive_structural_replay_closure(&capsule, &[]).expect("lost/retry outer closure");
}

#[test]
fn full_projection_accepts_sixty_four_worst_escaped_and_opaque_artifacts() {
    let value = golden();
    let decision = worst_escaped_decision_closure(&value.decision_capsule.decision_closure);
    assert_eq!(decision.operational_closure.artifacts.len(), 64);
    assert_eq!(
        decision
            .operational_closure
            .artifacts
            .iter()
            .filter(|artifact| artifact.artifact_kind == "reflection_report")
            .count(),
        1
    );
    let manifest = derive_structural_replay_manifest(&decision).expect("maximum manifest");
    assert_eq!(manifest.artifact_refs.len(), 64);
    let capsule = derive_decision_capsule(&decision).expect("maximum capsule");
    let outer = derive_structural_replay_closure(&capsule, &[]).expect("maximum outer closure");
    validate_structural_replay_closure(&outer).expect("maximum closure validation");
    assert!(canonical_json(&manifest).expect("manifest JSON").len() <= MAX_MANIFEST_BYTES);
}

#[test]
fn branch_is_unique_and_substitution_or_candidate_fields_fail() {
    let value = golden();
    assert_eq!(
        derive_evaluation_branch(&value.decision_capsule).expect("branch"),
        value.evaluation_branch
    );
    let mut branch = value.evaluation_branch.clone();
    branch.branch_id.clear();
    branch.branch_sha256.clear();
    branch.manifest_ref.manifest_sha256 = "0".repeat(64);
    branch.manifest_ref.manifest_id = format!(
        "structural-replay-manifest-{}",
        branch.manifest_ref.manifest_sha256
    );
    assert!(seal_evaluation_branch(&branch, &value.decision_capsule).is_err());
    let mut node = serde_json::to_value(&value).expect("JSON value");
    node["evaluation_branch"]["candidate_capsule"] = serde_json::Value::Null;
    let raw = canonical_json(&node).expect("canonical candidate");
    assert!(decode_structural_replay_closure(raw.as_bytes()).is_err());
}

#[test]
fn dedicated_reflection_report_refs_are_outer_only_and_bounded() {
    let value = golden();
    let capsule_value = serde_json::to_value(&value.decision_capsule).expect("capsule value");
    assert!(
        capsule_value
            .get("reflection_report_artifact_refs")
            .is_none()
    );
    derive_structural_replay_closure(&value.decision_capsule, &[]).expect("zero refs");
    derive_structural_replay_closure(&value.decision_capsule, &reflection_refs(32))
        .expect("32 refs");
    assert!(
        derive_structural_replay_closure(&value.decision_capsule, &reflection_refs(33)).is_err()
    );
    let mut wrong = reflection_refs(1);
    wrong[0].artifact_kind = "other".to_owned();
    assert!(derive_structural_replay_closure(&value.decision_capsule, &wrong).is_err());
    let duplicate = vec![reflection_refs(1)[0].clone(); 2];
    assert!(derive_structural_replay_closure(&value.decision_capsule, &duplicate).is_err());
    let mut reordered = reflection_refs(2);
    reordered.reverse();
    assert!(derive_structural_replay_closure(&value.decision_capsule, &reordered).is_err());
}
