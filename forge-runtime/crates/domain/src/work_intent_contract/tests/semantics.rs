use super::{super::*, support::*};

fn attestation_mut(value: &mut WorkIntentAttestations, index: usize) -> &mut bool {
    match index {
        0 => &mut value.approval_attestation,
        1 => &mut value.authentication_attestation,
        2 => &mut value.authority_attestation,
        3 => &mut value.completion_attestation,
        4 => &mut value.effect_attestation,
        5 => &mut value.execution_attestation,
        6 => &mut value.freshness_attestation,
        7 => &mut value.materiality_attestation,
        8 => &mut value.ownership_attestation,
        9 => &mut value.permission_attestation,
        10 => &mut value.persistence_attestation,
        11 => &mut value.reference_resolution_attestation,
        12 => &mut value.scope_attestation,
        13 => &mut value.truth_attestation,
        _ => panic!("attestation index"),
    }
}

#[test]
fn every_attestation_is_forced_false() {
    for index in 0..14 {
        let mut declaration = candidate();
        *attestation_mut(&mut declaration.attestations, index) = true;
        assert!(seal_work_intent(&declaration).is_err());
    }
}

#[test]
fn nullable_declarations_and_early_deadline_are_valid() {
    let mut declaration = candidate();
    declaration.declared_owner = None;
    declaration.binding.run_id = None;
    declaration.intent.deadline_unix_ms = Some(0);
    declaration.origin.origin_ref = None;
    declaration.references.local_source_snapshot_declaration = None;
    let sealed = seal_work_intent(&declaration).expect("nullable declaration");
    assert!(sealed.declared_owner.is_none());
    assert_eq!(sealed.intent.deadline_unix_ms, Some(0));
}

#[test]
fn narrative_order_is_authored_and_duplicates_are_rejected() {
    let mut declaration = candidate();
    declaration.intent.scope = vec!["z-last".into(), "a-first".into()];
    let sealed = seal_work_intent(&declaration).expect("authored order");
    assert_eq!(sealed.intent.scope, ["z-last", "a-first"]);
    declaration.intent.success_signals = vec!["same".into(), "same".into()];
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn narrative_cardinality_accepts_total_n_and_rejects_n_plus_one() {
    let mut declaration = candidate();
    declaration.intent.external_constraints = narrative("external", 64);
    declaration.intent.non_goals = narrative("non-goal", 64);
    declaration.intent.open_questions = narrative("question", 64);
    declaration.intent.scope = narrative("scope", 63);
    declaration.intent.success_signals = narrative("success", 1);
    seal_work_intent(&declaration).expect("256 narrative items");
    declaration.intent.scope.push("scope-063".into());
    assert!(seal_work_intent(&declaration).is_err());
    declaration
        .intent
        .external_constraints
        .push("external-064".into());
    assert!(seal_work_intent(&declaration).is_err());
}

fn narrative(prefix: &str, count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("{prefix}-{index:03}"))
        .collect()
}

#[test]
fn record_reference_limits_accept_n_and_reject_n_plus_one() {
    let mut declaration = candidate();
    declaration.references.claim_record_refs = (0..64)
        .map(|index| record_reference("claim", index))
        .collect();
    declaration.references.evidence_record_refs = (0..64)
        .map(|index| record_reference("evidence", index))
        .collect();
    seal_work_intent(&declaration).expect("64 plus 64 references");
    declaration
        .references
        .claim_record_refs
        .push(record_reference("claim", 64));
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn record_references_reject_order_overlap_and_non_identifier_ids() {
    let mut declaration = candidate();
    declaration.references.claim_record_refs = vec![
        record_reference("z-record", 0),
        record_reference("a-record", 0),
    ];
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration.references.evidence_record_refs = declaration.references.claim_record_refs.clone();
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration.references.claim_record_refs[0].record_id = "Uppercase".into();
    assert!(seal_work_intent(&declaration).is_err());
    declaration.references.claim_record_refs[0].record_id = "claim\nrecord".into();
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn artifact_limits_accept_n_and_reject_n_plus_one() {
    let mut declaration = candidate();
    declaration.references.local_artifact_declarations = (0..32).map(artifact).collect();
    seal_work_intent(&declaration).expect("32 artifacts");
    declaration
        .references
        .local_artifact_declarations
        .push(artifact(32));
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn artifacts_reject_canonical_order_and_kind_ref_pair_reuse() {
    let mut declaration = candidate();
    declaration.references.local_artifact_declarations = vec![artifact(1), artifact(0)];
    assert!(seal_work_intent(&declaration).is_err());
    let mut first = artifact(0);
    let mut second = first.clone();
    first.artifact_sha256 = "a".repeat(64);
    second.artifact_sha256 = "b".repeat(64);
    declaration.references.local_artifact_declarations = vec![first, second];
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn utf8_byte_limits_apply_to_short_reference_and_narrative_text() {
    let mut declaration = candidate();
    declaration.binding.change_id = "é".repeat(80);
    declaration.origin.origin_ref = Some("é".repeat(2_048));
    declaration.intent.goal = "é".repeat(8_192);
    seal_work_intent(&declaration).expect("exact UTF-8 byte limits");
    declaration.binding.change_id.push('é');
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration.origin.origin_ref = Some("é".repeat(2_049));
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration.intent.goal = "é".repeat(8_193);
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn materiality_snapshot_hash_and_identity_drift_fail_closed() {
    let mut declaration = candidate();
    declaration.materiality.basis = "verified".into();
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration
        .references
        .local_source_snapshot_declaration
        .as_mut()
        .expect("snapshot")
        .snapshot_id = "Uppercase".into();
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration
        .references
        .local_source_snapshot_declaration
        .as_mut()
        .expect("snapshot")
        .snapshot_id = "snapshot\nid".into();
    assert!(seal_work_intent(&declaration).is_err());
    let mut sealed = fixture();
    sealed.work_intent_sha256 = "0".repeat(64);
    assert!(validate_work_intent(&sealed).is_err());
    sealed = fixture();
    sealed.work_intent_id = format!("work-intent-{}", "0".repeat(64));
    assert!(validate_work_intent(&sealed).is_err());
}
