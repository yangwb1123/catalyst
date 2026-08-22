use super::{golden, *};
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::cell::Cell;

struct StatefulSerialize<'a> {
    calls: &'a Cell<usize>,
}

struct DuplicateKeys;

impl Serialize for DuplicateKeys {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_map(Some(2))?;
        object.serialize_entry("duplicate", &1)?;
        object.serialize_entry("duplicate", &2)?;
        object.end()
    }
}

impl Serialize for StatefulSerialize<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if call == 0 {
            serializer.serialize_str("stable")
        } else {
            serializer.serialize_f64(1.5)
        }
    }
}

#[test]
fn strict_wire_rejects_framing_duplicate_invalid_utf8_float_and_depth() {
    let raw = &super::GOLDEN[..super::GOLDEN.len() - 1];
    let text = std::str::from_utf8(raw).expect("UTF-8");
    let cases = [
        format!(" {text}"),
        format!("{text} "),
        text.replacen(
            "{\"api_version\":",
            "{\"api_version\":\"x\",\"api_version\":",
            1,
        ),
        text.replacen(
            "\"effect_replay_allowed\":false",
            "\"effect_replay_allowed\":0.0",
            1,
        ),
        text.replacen(
            "\"comparison_result\":\"EXACT_STRUCTURAL_REFERENCE_MATCH_ONLY\"",
            "\"comparison_result\":[[[[[[[[[[[[[[[[[\"x\"]]]]]]]]]]]]]]]]]",
            1,
        ),
    ];
    for changed in cases {
        assert!(decode_structural_replay_closure(changed.as_bytes()).is_err());
    }
    let mut invalid_utf8 = raw.to_vec();
    invalid_utf8[20] = 0xff;
    assert!(decode_structural_replay_closure(&invalid_utf8).is_err());
    assert!(decode_structural_replay_closure(super::GOLDEN).is_err());
}

#[test]
fn public_canonical_encoder_accepts_exact_twenty_eight_mib_and_rejects_n_plus_one() {
    assert_eq!(MAX_CLOSURE_BYTES, 28 * 1024 * 1024);
    let maximum = "x".repeat(16_384);
    let short = "x".repeat(10_993);
    let longer = "x".repeat(10_994);
    let full = vec![maximum.as_str(); 256];
    let mut final_inner = full.clone();
    final_inner[255] = short.as_str();
    let mut accepted = vec![full.clone(); 6];
    accepted.push(final_inner);
    super::super::wire::reset_allocation_calls();
    assert_eq!(
        canonical_json(&accepted).expect("exact N").len(),
        MAX_CLOSURE_BYTES
    );
    assert_eq!(super::super::wire::allocation_calls(), 1);
    accepted[6][255] = longer.as_str();
    super::super::wire::reset_allocation_calls();
    assert!(canonical_json(&accepted).is_err());
    assert_eq!(super::super::wire::allocation_calls(), 1);
}

#[test]
fn public_canonical_captures_a_stateful_serializer_exactly_once() {
    let calls = Cell::new(0);
    let value = StatefulSerialize { calls: &calls };
    super::super::wire::reset_allocation_calls();
    assert_eq!(
        canonical_json(&value).expect("single capture"),
        "\"stable\""
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(super::super::wire::allocation_calls(), 1);
}

#[test]
fn public_canonical_rejects_root_and_nested_duplicate_serialized_keys() {
    for result in [
        canonical_json(&DuplicateKeys),
        canonical_json(&vec![DuplicateKeys]),
    ] {
        let error = result.expect_err("duplicate key must fail");
        assert!(error.message.contains("duplicate JSON object key"));
    }
}

#[test]
fn byte_ceiling_preempts_unbounded_profile_traversal() {
    let error = super::super::wire::premeasure_only(&vec![0; 257], 1)
        .expect_err("one-byte ceiling must reject before array-profile traversal");
    assert!(error.message.contains("bounded JSON measurement failed"));
}

#[test]
fn configured_document_limits_and_n_plus_one_decode_are_exact() {
    assert_eq!(
        (
            MAX_MANIFEST_BYTES,
            MAX_CAPSULE_BYTES,
            MAX_BRANCH_BYTES,
            MAX_CLOSURE_BYTES
        ),
        (
            4 * 1024 * 1024,
            26 * 1024 * 1024,
            64 * 1024,
            28 * 1024 * 1024
        )
    );
    let value = golden();
    assert!(
        decode_structural_replay_manifest(
            &vec![b' '; MAX_MANIFEST_BYTES + 1],
            &value.decision_capsule.decision_closure
        )
        .is_err()
    );
    assert!(decode_decision_capsule(&vec![b' '; MAX_CAPSULE_BYTES + 1]).is_err());
    assert!(
        decode_evaluation_branch(&vec![b' '; MAX_BRANCH_BYTES + 1], &value.decision_capsule)
            .is_err()
    );
    assert!(decode_structural_replay_closure(&vec![b' '; MAX_CLOSURE_BYTES + 1]).is_err());
}

fn assert_local_boundary<T: serde::Serialize>(value: &T) {
    let size = canonical_json(value).expect("canonical").len();
    super::super::wire::canonical_with_max(value, size).expect("N accepted");
    assert!(super::super::wire::canonical_with_max(value, size - 1).is_err());
}

#[test]
fn blank_and_sealed_document_measurements_are_independently_load_bearing() {
    let value = golden();
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id.clear();
    manifest.manifest_sha256.clear();
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_id.clear();
    capsule.capsule_sha256.clear();
    let mut branch = value.evaluation_branch.clone();
    branch.branch_id.clear();
    branch.branch_sha256.clear();
    let mut outer = value.clone();
    outer.closure_id.clear();
    outer.closure_sha256.clear();
    assert_local_boundary(&manifest);
    assert_local_boundary(&value.decision_capsule.replay_manifest);
    assert_local_boundary(&capsule);
    assert_local_boundary(&value.decision_capsule);
    assert_local_boundary(&branch);
    assert_local_boundary(&value.evaluation_branch);
    assert_local_boundary(&outer);
    assert_local_boundary(&value);
}

#[test]
fn stale_nested_seals_and_promoted_controls_fail_closed() {
    let value = golden();
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_id.clear();
    capsule.capsule_sha256.clear();
    capsule.decision_closure.closure_sha256 = "0".repeat(64);
    assert!(seal_decision_capsule(&capsule).is_err());
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id.clear();
    manifest.manifest_sha256.clear();
    manifest.effect_replay_allowed = true;
    assert!(
        seal_structural_replay_manifest(&manifest, &value.decision_capsule.decision_closure)
            .is_err()
    );
    let mut branch = value.evaluation_branch;
    branch.branch_id.clear();
    branch.branch_sha256.clear();
    branch.history_rewrite_allowed = true;
    assert!(seal_evaluation_branch(&branch, &value.decision_capsule).is_err());
}

fn assert_rejected_before_clone<T>(
    operation: impl FnOnce() -> Result<T, DecisionCapsuleContractError>,
) {
    super::super::wire::reset_clone_calls();
    super::super::wire::reset_allocation_calls();
    assert!(operation().is_err());
    assert_eq!(super::super::wire::clone_calls(), 0);
    assert_eq!(super::super::wire::allocation_calls(), 0);
}

#[test]
fn manifest_and_branch_relations_reject_before_container_clone() {
    let value = golden();
    let digest = "0".repeat(64);
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id.clear();
    manifest.manifest_sha256.clear();
    manifest.decision_closure_ref.closure_id =
        format!("kernel-decision-reference-closure-{digest}");
    manifest.decision_closure_ref.closure_sha256 = digest.clone();
    super::super::wire::reset_clone_calls();
    assert!(
        structural_replay_manifest_digest(&manifest, &value.decision_capsule.decision_closure)
            .is_err()
    );
    assert_eq!(super::super::wire::clone_calls(), 0);

    let mut branch = value.evaluation_branch;
    branch.branch_id.clear();
    branch.branch_sha256.clear();
    branch.capsule_ref.capsule_id = format!("decision-capsule-{digest}");
    branch.capsule_ref.capsule_sha256 = digest;
    super::super::wire::reset_clone_calls();
    assert!(evaluation_branch_digest(&branch, &value.decision_capsule).is_err());
    assert_eq!(super::super::wire::clone_calls(), 0);
}

#[test]
fn capsule_and_outer_relations_reject_before_their_container_clone() {
    let value = golden();
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_id.clear();
    capsule.capsule_sha256.clear();
    let digest = "0".repeat(64);
    capsule.replay_manifest.decision_closure_ref.closure_id =
        format!("kernel-decision-reference-closure-{digest}");
    capsule.replay_manifest.decision_closure_ref.closure_sha256 = digest.clone();
    super::super::wire::reset_clone_calls();
    assert!(decision_capsule_digest(&capsule).is_err());
    assert_eq!(super::super::wire::clone_calls(), 0);

    let mut outer = value;
    outer.closure_id.clear();
    outer.closure_sha256.clear();
    outer.evaluation_branch.capsule_ref.capsule_id = format!("decision-capsule-{digest}");
    outer.evaluation_branch.capsule_ref.capsule_sha256 = digest;
    super::super::wire::reset_clone_calls();
    assert!(structural_replay_closure_digest(&outer).is_err());
    assert_eq!(super::super::wire::clone_calls(), 0);
}

#[test]
fn oversized_manifest_and_branch_reject_before_any_internal_clone() {
    let value = golden();
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id.clear();
    manifest.manifest_sha256.clear();
    manifest.kind = "x".repeat(MAX_MANIFEST_BYTES);
    let dependency = &value.decision_capsule.decision_closure;
    assert_rejected_before_clone(|| structural_replay_manifest_digest(&manifest, dependency));
    assert_rejected_before_clone(|| seal_structural_replay_manifest(&manifest, dependency));

    let mut branch = value.evaluation_branch.clone();
    branch.branch_id.clear();
    branch.branch_sha256.clear();
    branch.kind = "x".repeat(MAX_BRANCH_BYTES);
    assert_rejected_before_clone(|| evaluation_branch_digest(&branch, &value.decision_capsule));
    assert_rejected_before_clone(|| seal_evaluation_branch(&branch, &value.decision_capsule));
}

#[test]
fn oversized_capsule_and_outer_reject_before_any_internal_clone() {
    let value = golden();
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_id.clear();
    capsule.capsule_sha256.clear();
    capsule.result = "x".repeat(MAX_CAPSULE_BYTES);
    assert_rejected_before_clone(|| decision_capsule_digest(&capsule));
    assert_rejected_before_clone(|| seal_decision_capsule(&capsule));
    drop(capsule);

    let mut outer = value;
    outer.closure_id.clear();
    outer.closure_sha256.clear();
    outer.result = "x".repeat(MAX_CLOSURE_BYTES);
    assert_rejected_before_clone(|| structural_replay_closure_digest(&outer));
    assert_rejected_before_clone(|| seal_structural_replay_closure(&outer));
}

fn assert_profile_rejects<T: serde::Serialize + ?Sized>(value: &T) {
    super::super::wire::reset_allocation_calls();
    assert!(canonical_json(value).is_err());
    assert_eq!(super::super::wire::allocation_calls(), 1);
}

#[test]
fn public_canonical_profile_rejects_c1_bad_keys_and_integer_overflow_after_bounded_capture() {
    for float in [0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_profile_rejects(&vec![float]);
    }
    for scalar in ['\u{0080}', '\u{0085}', '\u{009f}'] {
        assert_profile_rejects(&scalar.to_string());
    }
    let mut bad_key = serde_json::Map::new();
    bad_key.insert("Bad-Key".to_owned(), serde_json::Value::Null);
    assert_profile_rejects(&bad_key);
    assert_profile_rejects(&(i64::MAX as u64 + 1));
    super::super::wire::reset_allocation_calls();
    assert_eq!(
        canonical_json(&i64::MAX).expect("i64 N"),
        i64::MAX.to_string()
    );
    assert_eq!(super::super::wire::allocation_calls(), 1);
}

fn nested_array(depth: usize) -> serde_json::Value {
    (0..depth).fold(serde_json::Value::Null, |value, _| {
        serde_json::Value::Array(vec![value])
    })
}

#[test]
fn public_profile_depth_array_and_string_n_plus_one_use_bounded_capture() {
    canonical_json(&nested_array(15)).expect("depth N");
    assert_profile_rejects(&nested_array(16));
    canonical_json(&vec![0; 256]).expect("array N");
    assert_profile_rejects(&vec![0; 257]);
    canonical_json(&"x".repeat(16_384)).expect("string N");
    assert_profile_rejects(&"x".repeat(16_385));
}

fn object_with_fields(count: usize) -> serde_json::Map<String, serde_json::Value> {
    (0..count)
        .map(|index| (format!("field_{index:02}"), serde_json::Value::Null))
        .collect()
}

#[test]
fn public_profile_object_n_plus_one_uses_bounded_capture() {
    canonical_json(&object_with_fields(64)).expect("object N");
    assert_profile_rejects(&object_with_fields(65));
}

#[test]
fn every_digest_rejects_own_string_n_plus_one_before_clone_or_allocation() {
    let value = golden();
    let over = "x".repeat(crate::governance_contract::MAX_STRING_BYTES + 1);
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.manifest_id = over.clone();
    assert_rejected_before_clone(|| {
        structural_replay_manifest_digest(&manifest, &value.decision_capsule.decision_closure)
    });
    let mut capsule = value.decision_capsule.clone();
    capsule.capsule_id = over.clone();
    assert_rejected_before_clone(|| decision_capsule_digest(&capsule));
    let mut branch = value.evaluation_branch.clone();
    branch.branch_id = over.clone();
    assert_rejected_before_clone(|| evaluation_branch_digest(&branch, &value.decision_capsule));
    let mut outer = value;
    outer.closure_id = over;
    assert_rejected_before_clone(|| structural_replay_closure_digest(&outer));
}

#[test]
fn semantic_array_ceilings_precede_clone_and_dependency_work() {
    let value = golden();
    let mut manifest = value.decision_capsule.replay_manifest.clone();
    manifest.artifact_refs = vec![manifest.artifact_refs[0].clone(); 257];
    assert_rejected_before_clone(|| {
        structural_replay_manifest_digest(&manifest, &value.decision_capsule.decision_closure)
    });
    let mut outer = value.clone();
    outer.reflection_report_artifact_refs =
        vec![outer.reflection_report_artifact_refs[0].clone(); 33];
    assert_rejected_before_clone(|| structural_replay_closure_digest(&outer));
    assert_rejected_before_clone(|| {
        derive_structural_replay_closure(
            &value.decision_capsule,
            &outer.reflection_report_artifact_refs,
        )
    });
}

fn exact_profile_document(target: usize) -> Vec<Vec<String>> {
    let mut document: Vec<Vec<String>> = Vec::new();
    let mut length = 2;
    while length < target {
        let new_group = document.last().is_none_or(|group| group.len() == 256);
        let overhead = if new_group {
            if document.is_empty() { 4 } else { 5 }
        } else {
            3
        };
        let remaining = target - length;
        if remaining < overhead {
            let prior = document
                .last_mut()
                .and_then(|group| group.last_mut())
                .unwrap();
            prior.truncate(prior.len() - (overhead - remaining));
            length -= overhead - remaining;
        }
        let content = (target - length - overhead).min(16_384);
        if new_group {
            document.push(Vec::new());
        }
        document.last_mut().unwrap().push("x".repeat(content));
        length += overhead + content;
    }
    assert_eq!(length, target);
    document
}

#[test]
fn every_document_n_plus_one_rejects_before_clone_or_canonical_allocation() {
    for maximum in [
        MAX_MANIFEST_BYTES,
        MAX_CAPSULE_BYTES,
        MAX_BRANCH_BYTES,
        MAX_CLOSURE_BYTES,
    ] {
        let document = exact_profile_document(maximum + 1);
        assert_rejected_before_clone(|| super::super::wire::bounded_clone(&document, maximum));
        assert_profile_rejects_with_max(&document, maximum);
    }
}

fn assert_profile_rejects_with_max<T: serde::Serialize + ?Sized>(value: &T, maximum: usize) {
    super::super::wire::reset_allocation_calls();
    assert!(super::super::wire::canonical_with_max(value, maximum).is_err());
    assert_eq!(super::super::wire::allocation_calls(), 1);
}
