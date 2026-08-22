use crate::{
    kernel_decision_contract::AtomRef,
    kernel_operational_contract::{
        ArtifactReceiptRef, ArtifactRef, CapabilityInvocationRef, ExecutionReceiptRef,
        InteractionEventRef,
    },
};
use serde_json::Value;

use super::{golden, *};

fn canonical_value(value: &Value) -> Vec<u8> {
    canonical_json(value)
        .expect("canonical mutation")
        .into_bytes()
}

#[test]
fn exact_object_field_counts_and_unknown_missing_types_rejected() {
    let value = serde_json::to_value(golden()).expect("JSON value");
    let paths: [(&[&str], usize); 4] = [
        (&[], 10),
        (&["decision_capsule"], 10),
        (&["decision_capsule", "replay_manifest"], 19),
        (&["evaluation_branch"], 13),
    ];
    for (path, count) in paths {
        let mut node = &value;
        for key in path {
            node = &node[*key];
        }
        assert_eq!(node.as_object().expect("object").len(), count);
        let mut changed = value.clone();
        let mut target = &mut changed;
        for key in path {
            target = &mut target[*key];
        }
        target
            .as_object_mut()
            .expect("object")
            .insert("unknown".to_owned(), Value::Null);
        assert!(decode_structural_replay_closure(&canonical_value(&changed)).is_err());
    }
    let mut missing = value.clone();
    missing.as_object_mut().expect("outer").remove("kind");
    assert!(decode_structural_replay_closure(&canonical_value(&missing)).is_err());
    let mut wrong = value;
    wrong["evaluation_branch"]["comparison_result"] = Value::Bool(false);
    assert!(decode_structural_replay_closure(&canonical_value(&wrong)).is_err());
}

#[test]
fn every_attestation_is_false_and_rejects_true_or_integer_zero() {
    let value = serde_json::to_value(golden()).expect("JSON value");
    let paths: [&[&str]; 4] = [
        &["attestations"],
        &["decision_capsule", "attestations"],
        &["decision_capsule", "replay_manifest", "attestations"],
        &["evaluation_branch", "attestations"],
    ];
    for path in paths {
        let mut attestations = &value;
        for key in path {
            attestations = &attestations[*key];
        }
        assert_eq!(attestations.as_object().expect("attestations").len(), 32);
        for field in attestations.as_object().expect("attestations").keys() {
            for replacement in [Value::Bool(true), Value::Number(0.into())] {
                let mut changed = value.clone();
                let mut target = &mut changed;
                for key in path {
                    target = &mut target[*key];
                }
                target[field] = replacement;
                assert!(decode_structural_replay_closure(&canonical_value(&changed)).is_err());
            }
        }
    }
}

#[test]
fn own_identity_only_may_be_blank_for_sealing() {
    let value = golden();
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id.clear();
    assert!(
        seal_structural_replay_manifest(&manifest, &value.decision_capsule.decision_closure)
            .is_err()
    );
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_sha256.clear();
    assert!(seal_decision_capsule(&capsule).is_err());
    let mut branch = value.evaluation_branch.clone();
    branch.branch_id.clear();
    assert!(seal_evaluation_branch(&branch, &value.decision_capsule).is_err());
    let mut outer = value;
    outer.closure_sha256.clear();
    assert!(seal_structural_replay_closure(&outer).is_err());
}

#[test]
fn exact_results_controls_and_domains_are_load_bearing() {
    let value = golden();
    assert_eq!(value.result, SUCCESS_MARKER);
    assert_eq!(value.decision_capsule.result, CAPSULE_RESULT);
    let manifest = &value.decision_capsule.replay_manifest;
    assert!(!manifest.effect_replay_allowed && !manifest.history_rewrite_allowed);
    assert!(!value.evaluation_branch.effect_replay_allowed);
    assert!(!value.evaluation_branch.history_rewrite_allowed);
    assert_eq!(
        structural_replay_manifest_digest(manifest, &value.decision_capsule.decision_closure)
            .expect("manifest digest"),
        manifest.manifest_sha256
    );
    assert_eq!(
        decision_capsule_digest(&value.decision_capsule).expect("capsule digest"),
        value.decision_capsule.capsule_sha256
    );
    assert_eq!(
        evaluation_branch_digest(&value.evaluation_branch, &value.decision_capsule)
            .expect("branch digest"),
        value.evaluation_branch.branch_sha256
    );
    assert_eq!(
        structural_replay_closure_digest(&value).expect("closure digest"),
        value.closure_sha256
    );
}

#[test]
fn digest_helpers_ignore_only_arbitrary_own_identity_fields() {
    let value = golden();
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id = "arbitrary-manifest-id".to_owned();
    manifest.manifest_sha256 = "stale-manifest-hash".to_owned();
    assert_eq!(
        structural_replay_manifest_digest(&manifest, &value.decision_capsule.decision_closure)
            .expect("manifest preimage"),
        value.decision_capsule.replay_manifest.manifest_sha256
    );
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_id = "arbitrary-capsule-id".to_owned();
    capsule.capsule_sha256 = "stale-capsule-hash".to_owned();
    assert_eq!(
        decision_capsule_digest(&capsule).expect("capsule preimage"),
        value.decision_capsule.capsule_sha256
    );
    let mut branch = value.evaluation_branch.clone();
    branch.branch_id = "arbitrary-branch-id".to_owned();
    branch.branch_sha256 = "stale-branch-hash".to_owned();
    assert_eq!(
        evaluation_branch_digest(&branch, &value.decision_capsule).expect("branch preimage"),
        value.evaluation_branch.branch_sha256
    );
    let mut outer = value.clone();
    outer.closure_id = "arbitrary-closure-id".to_owned();
    outer.closure_sha256 = "stale-closure-hash".to_owned();
    assert_eq!(
        structural_replay_closure_digest(&outer).expect("outer preimage"),
        value.closure_sha256
    );
}

fn assert_local_rejection<T>(operation: impl FnOnce() -> Result<T, DecisionCapsuleContractError>) {
    super::super::wire::reset_clone_calls();
    super::super::wire::reset_allocation_calls();
    assert!(operation().is_err());
    assert_eq!(super::super::wire::clone_calls(), 0);
    assert_eq!(super::super::wire::allocation_calls(), 0);
}

fn assert_local_rejection_contains<T>(
    expected: &str,
    operation: impl FnOnce() -> Result<T, DecisionCapsuleContractError>,
) {
    super::super::wire::reset_clone_calls();
    super::super::wire::reset_allocation_calls();
    let Err(error) = operation() else {
        panic!("local invalidity accepted");
    };
    assert!(error.to_string().contains(expected), "error={error}");
    assert_eq!(super::super::wire::clone_calls(), 0);
    assert_eq!(super::super::wire::allocation_calls(), 0);
}

#[test]
fn local_invalidity_preempts_every_large_dependency() {
    let value = golden();
    let dependency = &value.decision_capsule.decision_closure;
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.decision_closure_ref.closure_id = "drift".to_owned();
    assert_local_rejection(|| structural_replay_manifest_digest(&manifest, dependency));
    assert_local_rejection(|| validate_structural_replay_manifest(&manifest, dependency));
    manifest.manifest_id.clear();
    manifest.manifest_sha256.clear();
    assert_local_rejection(|| seal_structural_replay_manifest(&manifest, dependency));

    let mut branch = value.evaluation_branch.clone();
    branch.comparison_result = "drift".to_owned();
    assert_local_rejection(|| evaluation_branch_digest(&branch, &value.decision_capsule));
    assert_local_rejection(|| validate_evaluation_branch(&branch, &value.decision_capsule));
    branch.branch_id.clear();
    branch.branch_sha256.clear();
    assert_local_rejection(|| seal_evaluation_branch(&branch, &value.decision_capsule));

    let mut outer = value;
    outer.reflection_report_artifact_refs[0].artifact_kind = "drift".to_owned();
    assert_local_rejection(|| structural_replay_closure_digest(&outer));
    assert_local_rejection(|| validate_structural_replay_closure(&outer));
    outer.closure_id.clear();
    outer.closure_sha256.clear();
    assert_local_rejection(|| seal_structural_replay_closure(&outer));
}

fn assert_nested_capsule_and_branch_preflight(
    value: &StructuralReplayClosure,
    capsule: &DecisionCapsule,
) {
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        decision_capsule_digest(capsule)
    });
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        validate_decision_capsule(capsule)
    });
    let mut blank_capsule = capsule.clone();
    blank_capsule.capsule_id.clear();
    blank_capsule.capsule_sha256.clear();
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        seal_decision_capsule(&blank_capsule)
    });

    let mut blank_branch = value.evaluation_branch.clone();
    blank_branch.branch_id.clear();
    blank_branch.branch_sha256.clear();
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        evaluation_branch_digest(&value.evaluation_branch, capsule)
    });
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        validate_evaluation_branch(&value.evaluation_branch, capsule)
    });
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        seal_evaluation_branch(&blank_branch, capsule)
    });
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        derive_evaluation_branch(capsule)
    });
}

fn assert_nested_outer_preflight(mut outer: StructuralReplayClosure, capsule: &DecisionCapsule) {
    outer.reflection_report_artifact_refs.clear();
    outer.evaluation_branch.branch_mode.push_str("_drift");
    outer
        .decision_capsule
        .replay_manifest
        .replay_mode
        .push_str("_drift");
    outer.decision_capsule.decision_closure.closure_id = "x".repeat(MAX_CAPSULE_BYTES + 1);
    assert_local_rejection_contains("EvaluationBranch constants differ", || {
        structural_replay_closure_digest(&outer)
    });
    assert_local_rejection_contains("EvaluationBranch constants differ", || {
        validate_structural_replay_closure(&outer)
    });
    outer.closure_id.clear();
    outer.closure_sha256.clear();
    assert_local_rejection_contains("EvaluationBranch constants differ", || {
        seal_structural_replay_closure(&outer)
    });
    assert_local_rejection_contains("StructuralReplayManifest constants differ", || {
        derive_structural_replay_closure(capsule, &[])
    });
}

#[test]
fn nested_local_invalidity_preempts_every_composite_public_route() {
    let value = golden();
    let mut capsule = value.decision_capsule.clone();
    capsule.replay_manifest.replay_mode.push_str("_drift");
    capsule.decision_closure.closure_id = "x".repeat(MAX_CAPSULE_BYTES + 1);
    assert_nested_capsule_and_branch_preflight(&value, &capsule);
    assert_nested_outer_preflight(value, &capsule);
}

fn exact_hash(index: usize) -> String {
    format!("{:064x}", index + 1)
}

fn worst_artifacts(count: usize) -> Vec<ArtifactRef> {
    let mut values: Vec<_> = (0..count)
        .map(|index| ArtifactRef {
            artifact_kind: "\"".repeat(160),
            artifact_ref: "\\".repeat(4_096),
            artifact_sha256: exact_hash(index),
        })
        .collect();
    values.sort_by_cached_key(|value| canonical_json(value).expect("artifact key"));
    values
}

fn manifest_envelope(base: &StructuralReplayManifest) -> StructuralReplayManifest {
    let mut value = base.clone();
    value.manifest_id.clear();
    value.manifest_sha256.clear();
    value.artifact_refs = worst_artifacts(64);
    value.artifact_receipt_refs = (0..64)
        .map(|index| ArtifactReceiptRef {
            artifact_receipt_id: format!("artifact-receipt-{}", exact_hash(index)),
            artifact_receipt_sha256: exact_hash(index),
        })
        .collect();
    value.capability_invocation_refs = (0..64)
        .map(|index| CapabilityInvocationRef {
            invocation_id: format!("capability-invocation-{}", exact_hash(index)),
            invocation_sha256: exact_hash(index),
        })
        .collect();
    value.interaction_event_refs = (0..256)
        .map(|index| InteractionEventRef {
            event_id: format!("interaction-event-{}", exact_hash(index)),
            event_sha256: exact_hash(index),
        })
        .collect();
    value.execution_receipt_refs = (0..64)
        .map(|index| ExecutionReceiptRef {
            execution_receipt_id: format!("execution-receipt-{}", exact_hash(index)),
            execution_receipt_sha256: exact_hash(index),
        })
        .collect();
    let atoms: Vec<_> = (0..256)
        .map(|index| AtomRef {
            atom_id: format!("cognitive-atom-{}", exact_hash(index)),
            atom_sha256: exact_hash(index),
        })
        .collect();
    value.predecision_atom_refs = atoms;
    value.postdecision_atom_refs.clear();
    value
}

fn worst_reflection_refs() -> Vec<ArtifactRef> {
    let mut values: Vec<_> = (0..32)
        .map(|index| ArtifactRef {
            artifact_kind: "reflection_report".to_owned(),
            artifact_ref: "\\".repeat(4_096),
            artifact_sha256: exact_hash(index),
        })
        .collect();
    values.sort_by_cached_key(|value| canonical_json(value).expect("reflection key"));
    super::super::primitives::reflection_refs(&values).expect("reflection semantic N");
    values
}

fn assert_conservative_manifest_byte_bounds(value: &StructuralReplayClosure) {
    let base = &value.decision_capsule.replay_manifest;
    let mut shape = base.clone();
    shape.manifest_id.clear();
    shape.manifest_sha256.clear();
    shape.artifact_refs = worst_artifacts(256);
    super::super::manifest::local_shape(&shape, true).expect("manifest shape semantic N");
    assert_eq!(canonical_json(&shape).expect("shape").len(), 2_218_274);

    let envelope = manifest_envelope(base);
    super::super::manifest::local_shape(&envelope, true).expect("manifest envelope semantic N");
    assert_eq!(
        canonical_json(&envelope).expect("blank envelope").len(),
        684_285
    );
    let mut sealed_envelope = envelope;
    sealed_envelope.manifest_id.clone_from(&base.manifest_id);
    sealed_envelope
        .manifest_sha256
        .clone_from(&base.manifest_sha256);
    super::super::manifest::local_shape(&sealed_envelope, false)
        .expect("sealed manifest envelope semantic N");
    assert_eq!(
        canonical_json(&sealed_envelope)
            .expect("sealed envelope")
            .len(),
        684_440
    );
}

fn assert_conservative_capsule_and_outer_byte_bounds(value: &StructuralReplayClosure) {
    let base = &value.decision_capsule.replay_manifest;
    let capsule_size = canonical_json(&value.decision_capsule)
        .expect("capsule")
        .len();
    let decision_size = canonical_json(&value.decision_capsule.decision_closure)
        .expect("decision")
        .len();
    let manifest_size = canonical_json(base).expect("manifest").len();
    let capsule_overhead = capsule_size - decision_size - manifest_size;
    assert_eq!(capsule_overhead, 1_867);
    assert_eq!(20 * 1024 * 1024 + 684_440 + capsule_overhead, 21_657_827);

    let branch_size = canonical_json(&value.evaluation_branch)
        .expect("branch")
        .len();
    assert_eq!(branch_size, 2_305);
    let refs_size = canonical_json(&worst_reflection_refs())
        .expect("maximum reflection refs")
        .len();
    assert_eq!(refs_size, 266_657);
    let outer_size = canonical_json(&value).expect("outer").len();
    let golden_refs_size = canonical_json(&value.reflection_report_artifact_refs)
        .expect("golden refs")
        .len();
    let outer_overhead = outer_size - capsule_size - branch_size - golden_refs_size;
    assert_eq!(outer_overhead, 2_083);
    assert_eq!(
        21_657_827 + branch_size + refs_size + outer_overhead,
        21_928_872
    );
}

#[test]
fn conservative_manifest_capsule_and_outer_byte_bounds_are_exact() {
    let value = golden();
    assert_conservative_manifest_byte_bounds(&value);
    assert_conservative_capsule_and_outer_byte_bounds(&value);
}
