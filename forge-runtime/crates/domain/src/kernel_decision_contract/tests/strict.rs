use super::{golden, *};

#[test]
fn duplicate_unknown_utf8_control_surrogate_depth_and_size_rejected() {
    let raw = &super::GOLDEN[..super::GOLDEN.len() - 1];
    let text = std::str::from_utf8(raw).expect("UTF-8");
    let cases = [
        text.replacen("{\"api_version\":", "{\"unknown\":0,\"api_version\":", 1),
        text.replacen(
            "{\"api_version\":",
            "{\"api_version\":\"x\",\"api_version\":",
            1,
        ),
        text.replacen("\"value-03\"", "\"\\ud800\"", 1),
        text.replacen("\"value-03\"", "\"\\u202e\"", 1),
        text.replacen(
            "\"atom_type\":\"constraint\"",
            "\"atom_type\":[[[[[[[[[[[[[[[[[\"x\"]]]]]]]]]]]]]]]]]",
            1,
        ),
    ];
    for changed in cases {
        assert!(decode_closure(changed.as_bytes()).is_err());
    }
    let mut invalid_utf8 = raw.to_vec();
    invalid_utf8[20] = 0xff;
    assert!(decode_closure(&invalid_utf8).is_err());
    let oversized = vec![b' '; MAX_CLOSURE_BYTES + 1];
    assert!(decode_closure(&oversized).is_err());
}

#[test]
fn forbidden_scalar_in_memory_seals_rejected() {
    let value = golden();
    let mut atom = value.cognitive_atoms[0].clone();
    atom.atom_id.clear();
    atom.atom_sha256.clear();
    atom.proposition.object_value = serde_json::json!("x\u{202e}");
    assert!(seal_cognitive_atom(&atom).is_err());
    let mut transaction = value.decision_transaction.clone();
    transaction.decision_transaction_id.clear();
    transaction.decision_transaction_sha256.clear();
    transaction.idempotency_key = "x\u{202e}".to_owned();
    assert!(seal_decision_transaction(&transaction).is_err());
    let mut closure = value;
    closure.closure_id.clear();
    closure.closure_sha256.clear();
    closure.result = "x\u{202e}".to_owned();
    assert!(seal_closure(&closure).is_err());
}

#[test]
fn public_canonical_json_rejects_c1_controls() {
    for scalar in ['\u{0080}', '\u{009f}'] {
        let value = serde_json::json!({"value": scalar.to_string()});
        assert!(
            canonical_json(&value).is_err(),
            "accepted U+{:04X}",
            scalar as u32
        );
    }
}
