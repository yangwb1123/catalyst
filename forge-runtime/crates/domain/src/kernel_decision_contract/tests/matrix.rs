use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use super::{golden, *};

fn promoted_attestation<T>(value: &T, field: &str) -> T
where
    T: Serialize + DeserializeOwned,
{
    let mut encoded = serde_json::to_value(value).expect("serialize attestation envelope");
    encoded["attestations"][field] = json!(true);
    serde_json::from_value(encoded).expect("deserialize attestation envelope")
}

#[test]
fn golden_covers_all_frozen_vocabularies() {
    let value = golden();
    let types = value
        .cognitive_atoms
        .iter()
        .map(|atom| atom.atom_type.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let sources = value
        .cognitive_atoms
        .iter()
        .map(|atom| atom.source.source_kind.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let hardness = value
        .cognitive_atoms
        .iter()
        .map(|atom| atom.declared_hardness.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let authorities = value
        .cognitive_atoms
        .iter()
        .map(|atom| atom.declared_authority.authority_kind.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(types.len(), 16);
    assert_eq!(sources.len(), 8);
    assert_eq!(hardness.len(), 6);
    assert_eq!(authorities.len(), 4);
    assert_eq!(
        serde_json::to_value(&value.cognitive_atoms[0].attestations)
            .expect("attestations")
            .as_object()
            .expect("object")
            .len(),
        22
    );
}

#[test]
fn restricted_source_type_matrix_rejects_cross_feed() {
    let value = golden();
    let cases = [
        ("artifact_receipt", "goal"),
        ("capability_invocation", "goal"),
        ("cognitive_atom_v1", "goal"),
        ("evidence_record", "goal"),
        ("execution_receipt", "goal"),
        ("interaction_event", "goal"),
        ("work_intent", "actor"),
    ];
    for (source_kind, atom_type) in cases {
        let mut atom = value
            .cognitive_atoms
            .iter()
            .find(|atom| atom.source.source_kind == source_kind)
            .expect("source kind")
            .clone();
        atom.atom_id.clear();
        atom.atom_sha256.clear();
        atom.atom_type = atom_type.to_owned();
        let error = seal_cognitive_atom(&atom).expect_err("source/type cross-feed");
        assert_eq!(error.message, "source_kind does not admit atom_type");
    }
}

fn assert_atom_rejected(mut atom: CognitiveAtom) {
    atom.atom_id.clear();
    atom.atom_sha256.clear();
    assert!(seal_cognitive_atom(&atom).is_err());
}

#[test]
fn none_and_inadmitted_hardness_authority_matrix() {
    let value = golden();
    let approval = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.declared_authority.authority_kind == "approval_record")
        .expect("approval authority")
        .declared_authority
        .clone();
    let mut legacy = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.source.source_kind == "cognitive_atom_v1")
        .expect("legacy")
        .clone();
    legacy.declared_hardness = "advisory".to_owned();
    let observation = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.atom_type == "observation")
        .expect("observation")
        .clone();
    let mut inadmitted = observation.clone();
    inadmitted.declared_hardness = "advisory".to_owned();
    let mut none_with_authority = observation;
    none_with_authority.declared_authority = approval;
    for atom in [legacy, inadmitted, none_with_authority] {
        assert_atom_rejected(atom);
    }
}

#[test]
fn required_and_contract_hardness_authority_matrix() {
    let value = golden();
    let none = DeclaredAuthority {
        authority_kind: "none".to_owned(),
        authority_ref: json!(null),
    };
    let mut constraint = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.declared_hardness == "contract")
        .expect("contract constraint")
        .clone();
    constraint.declared_authority = none.clone();
    let mut goal = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.atom_type == "goal")
        .expect("goal")
        .clone();
    goal.declared_authority = none;
    let mut decision = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.source.source_kind == "artifact" && atom.declared_hardness == "invariant")
        .expect("artifact invariant")
        .clone();
    decision.atom_type = "decision".to_owned();
    decision.declared_hardness = "required".to_owned();
    for atom in [constraint, goal, decision] {
        assert_atom_rejected(atom);
    }
}

#[test]
fn every_ineffective_field_promotion_rejected() {
    let value = golden();
    let mut hardness = value.cognitive_atoms[0].clone();
    hardness.atom_id.clear();
    hardness.atom_sha256.clear();
    hardness.effective_hardness = "advisory".to_owned();
    assert!(seal_cognitive_atom(&hardness).is_err());
    let mut instruction = value.cognitive_atoms[0].clone();
    instruction.atom_id.clear();
    instruction.atom_sha256.clear();
    instruction.instruction_allowed = true;
    assert!(seal_cognitive_atom(&instruction).is_err());

    let encoded =
        serde_json::to_value(&value.cognitive_atoms[0].attestations).expect("attestations");
    let fields = encoded.as_object().expect("attestation object").keys();
    for field in fields {
        let mut atom = value.cognitive_atoms[0].clone();
        atom.atom_id.clear();
        atom.atom_sha256.clear();
        let atom = promoted_attestation(&atom, field);
        let mut transaction = value.decision_transaction.clone();
        transaction.decision_transaction_id.clear();
        transaction.decision_transaction_sha256.clear();
        let transaction = promoted_attestation(&transaction, field);
        let mut closure = value.clone();
        closure.closure_id.clear();
        closure.closure_sha256.clear();
        let closure = promoted_attestation(&closure, field);
        assert!(seal_cognitive_atom(&atom).is_err());
        assert!(seal_decision_transaction(&transaction).is_err());
        assert!(seal_closure(&closure).is_err());
    }
}

#[test]
fn legacy_and_evidence_cross_contract_positive_vectors() {
    let value = golden();
    let mut legacy = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.source.source_kind == "cognitive_atom_v1")
        .expect("legacy")
        .clone();
    legacy.atom_id.clear();
    legacy.atom_sha256.clear();
    legacy.source.source_ref = json!({
        "atom_id": "atom-99045a525632c18aec6b1c783ba1925e4603b4378b389e5ce86621ab25b145ae",
        "canonical_sha256": "3905ee9fd8293924644dd5d9a1da522ffe944dc58db51a26ee6c584e1335ce20"
    });
    seal_cognitive_atom(&legacy).expect("real non-equal ADR-0047 ref");
    let mut evidence = value
        .cognitive_atoms
        .iter()
        .find(|atom| atom.source.source_kind == "evidence_record")
        .expect("evidence")
        .clone();
    evidence.atom_id.clear();
    evidence.atom_sha256.clear();
    evidence.source.source_ref = json!({
        "canonical_sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "record_id": "fixture evidence record / v1"
    });
    seal_cognitive_atom(&evidence).expect("ADR-0060 text record ID");
}

#[test]
fn proposition_scope_role_order_and_uniqueness_rejected() {
    let value = golden();
    let mut atom = value.cognitive_atoms[0].clone();
    atom.atom_id.clear();
    atom.atom_sha256.clear();
    atom.proposition.object_type = "artifact_ref".to_owned();
    for invalid in ["artifact with spaces", "artifact$illegal"] {
        atom.proposition.object_value = json!(invalid);
        assert!(seal_cognitive_atom(&atom).is_err());
    }
    let mut transaction = value.decision_transaction;
    transaction.decision_transaction_id.clear();
    transaction.decision_transaction_sha256.clear();
    transaction.trigger_atom_refs = transaction.guard_atom_refs[..2].to_vec();
    transaction.trigger_atom_refs.swap(0, 1);
    assert!(seal_decision_transaction(&transaction).is_err());
    transaction.trigger_atom_refs[0] = transaction.trigger_atom_refs[1].clone();
    assert!(seal_decision_transaction(&transaction).is_err());
}
