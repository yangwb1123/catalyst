use sha2::{Digest, Sha256};

use super::*;

mod boundary;
mod graph;
mod matrix;
mod strict;

const GOLDEN: &[u8] = include_bytes!(
    "../../../../../../docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
);

fn golden() -> KernelDecisionReferenceClosure {
    assert_eq!(GOLDEN.last(), Some(&b'\n'));
    decode_closure(&GOLDEN[..GOLDEN.len() - 1]).expect("exact Kernel decision golden")
}

#[test]
fn golden_physical_semantic_and_canonical_pins() {
    let value = golden();
    assert_eq!(
        crate::governance_contract::codec::lower_hex(&Sha256::digest(GOLDEN)),
        "93f6225b745eacf966796cb671d723440890ae3ab02699dd40d6a078f539af1c"
    );
    assert_eq!(
        value.closure_sha256,
        "cdadf0e5fddbbda429939be4e68dc77dd0b52c0bb7e4fe955f1d485183908e58"
    );
    assert_eq!(
        canonical_json(&value).expect("canonical"),
        String::from_utf8(GOLDEN[..GOLDEN.len() - 1].to_vec()).expect("UTF-8")
    );
}

#[test]
fn every_atom_transaction_and_closure_reseals() {
    let value = golden();
    for mut atom in value.cognitive_atoms.clone() {
        let expected = atom.clone();
        atom.atom_id.clear();
        atom.atom_sha256.clear();
        assert_eq!(seal_cognitive_atom(&atom).expect("atom reseal"), expected);
    }
    let mut transaction = value.decision_transaction.clone();
    let expected_transaction = transaction.clone();
    transaction.decision_transaction_id.clear();
    transaction.decision_transaction_sha256.clear();
    assert_eq!(
        seal_decision_transaction(&transaction).expect("transaction reseal"),
        expected_transaction
    );
    let mut blank = value.clone();
    blank.closure_id.clear();
    blank.closure_sha256.clear();
    assert_eq!(seal_closure(&blank).expect("closure reseal"), value);
}

#[test]
fn zero_trigger_accepted_and_sixty_five_rejected() {
    let value = golden();
    let mut transaction = value.decision_transaction;
    transaction.decision_transaction_id.clear();
    transaction.decision_transaction_sha256.clear();
    transaction
        .guard_atom_refs
        .append(&mut transaction.trigger_atom_refs);
    transaction
        .guard_atom_refs
        .sort_by(|left, right| left.atom_id.as_bytes().cmp(right.atom_id.as_bytes()));
    assert!(transaction.trigger_atom_refs.is_empty());
    seal_decision_transaction(&transaction).expect("zero trigger valid");
    transaction.trigger_atom_refs = vec![transaction.goal_atom_ref.clone(); 65];
    assert!(seal_decision_transaction(&transaction).is_err());
}

#[test]
fn strict_wire_and_bad_types_rejected() {
    let raw = &GOLDEN[..GOLDEN.len() - 1];
    let text = std::str::from_utf8(raw).expect("UTF-8");
    let cases = [
        format!(" {text}"),
        format!("{text} "),
        text.replacen("\"atom_type\":\"constraint\"", "\"atom_type\":[]", 1),
        text.replacen("\"source_kind\":\"work_intent\"", "\"source_kind\":{}", 1),
        text.replacen(
            "\"authority_kind\":\"contract_artifact\"",
            "\"authority_kind\":[]",
            1,
        ),
        text.replacen("\"object_type\":\"string\"", "\"object_type\":{}", 1),
        text.replacen(
            "\"declared_hardness\":\"contract\"",
            "\"declared_hardness\":[]",
            1,
        ),
    ];
    for changed in cases {
        assert!(decode_closure(changed.as_bytes()).is_err());
    }
    assert!(decode_closure(GOLDEN).is_err());
}
