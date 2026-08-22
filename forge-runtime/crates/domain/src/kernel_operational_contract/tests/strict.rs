use serde_json::Value;

use super::*;

fn strict_value(bytes: &[u8]) -> Result<Value, KernelOperationalContractError> {
    wire::decode_typed(bytes, bytes.len())
}

fn mutate(raw: &[u8], path: &[&str], value: Option<Value>) -> Vec<u8> {
    let mut root: Value = serde_json::from_slice(raw).expect("mutation source");
    let mut current = root.as_object_mut().expect("mutation root");
    for field in &path[..path.len() - 1] {
        current = current
            .get_mut(*field)
            .and_then(Value::as_object_mut)
            .expect("mutation path");
    }
    let field = path[path.len() - 1];
    if let Some(value) = value {
        current.insert(field.to_owned(), value);
    } else {
        current.remove(field);
    }
    wire::canonical_typed(&root)
        .expect("canonical mutation")
        .into_bytes()
}

fn invocation_result(bytes: &[u8]) -> Result<(), KernelOperationalContractError> {
    decode_capability_invocation(bytes).map(|_| ())
}

fn artifact_receipt_result(bytes: &[u8]) -> Result<(), KernelOperationalContractError> {
    decode_artifact_receipt(bytes).map(|_| ())
}

fn event_result(bytes: &[u8]) -> Result<(), KernelOperationalContractError> {
    decode_interaction_event(bytes).map(|_| ())
}

fn execution_result(bytes: &[u8]) -> Result<(), KernelOperationalContractError> {
    decode_execution_receipt(bytes).map(|_| ())
}

fn artifact_ref_result(bytes: &[u8]) -> Result<(), KernelOperationalContractError> {
    decode_artifact_ref(bytes).map(|_| ())
}

fn closure_result(bytes: &[u8]) -> Result<(), KernelOperationalContractError> {
    decode_closure(bytes).map(|_| ())
}

#[test]
fn rejects_duplicate_unknown_missing_and_noncanonical_framing() {
    let raw = &GOLDEN[..GOLDEN.len() - 1];
    for changed in [GOLDEN.to_vec(), [b" ", raw].concat(), [raw, b" "].concat()] {
        assert!(decode_closure(&changed).is_err());
    }
    let text = std::str::from_utf8(raw).expect("golden UTF-8");
    let duplicate = text.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"x\",\"api_version\":",
        1,
    );
    assert!(decode_closure(duplicate.as_bytes()).is_err());
    let unknown = mutate(raw, &["authority"], Some(Value::Bool(false)));
    assert!(decode_closure(&unknown).is_err());
    let event = canonical_json(&golden().interaction_events[0]).expect("event");
    let missing_optional = mutate(event.as_bytes(), &["target"], None);
    assert!(decode_interaction_event(&missing_optional).is_err());
}

#[test]
fn rejects_float_nonfinite_invalid_utf8_and_int64_overflow() {
    for raw in [
        b"1.0".as_slice(),
        b"1e0",
        b"NaN",
        b"Infinity",
        b"9223372036854775808",
        b"-9223372036854775809",
        b"01",
        b"-0",
    ] {
        assert!(
            strict_value(raw).is_err(),
            "accepted {}",
            String::from_utf8_lossy(raw)
        );
    }
    assert!(strict_value(&[b'"', 0xff, b'"']).is_err());
    assert!(strict_value(b"9223372036854775807").is_ok());
    assert!(strict_value(b"-9223372036854775808").is_ok());
}

#[test]
fn rejects_control_c1_bidi_surrogate_and_line_separators() {
    let forbidden = [
        '\0', '\u{001f}', '\u{007f}', '\u{0080}', '\u{009f}', '\u{061c}', '\u{200e}', '\u{200f}',
        '\u{2028}', '\u{2029}', '\u{202e}', '\u{2066}', '\u{2069}',
    ];
    for scalar in forbidden {
        let raw = serde_json::to_vec(&scalar.to_string()).expect("encoded scalar");
        assert!(
            strict_value(&raw).is_err(),
            "accepted U+{:04X}",
            u32::from(scalar)
        );
    }
    for raw in [br#""\ud800""#.as_slice(), br#""\udfff""#] {
        assert!(strict_value(raw).is_err());
    }
}

#[test]
fn locks_depth_fields_arrays_and_utf8_string_byte_boundaries() {
    let depth16 = format!("{}0{}", "[".repeat(15), "]".repeat(15));
    let depth17 = format!("[{depth16}]");
    assert!(strict_value(depth16.as_bytes()).is_ok());
    assert!(strict_value(depth17.as_bytes()).is_err());
    assert!(strict_value(object_fields(64).as_bytes()).is_ok());
    assert!(strict_value(object_fields(65).as_bytes()).is_err());
    assert!(strict_value(integer_array(256).as_bytes()).is_ok());
    assert!(strict_value(integer_array(257).as_bytes()).is_err());
    let at_limit = format!("\"{}\"", "é".repeat(MAX_STRING_BYTES / 2));
    let above = format!("\"{}\"", "é".repeat(MAX_STRING_BYTES / 2 + 1));
    assert!(strict_value(at_limit.as_bytes()).is_ok());
    assert!(strict_value(above.as_bytes()).is_err());
}

fn object_fields(count: usize) -> String {
    let fields: Vec<_> = (0..count)
        .map(|index| format!("\"a{index:02}\":0"))
        .collect();
    format!("{{{}}}", fields.join(","))
}

fn integer_array(count: usize) -> String {
    let values: Vec<_> = (0..count).map(|index| index.to_string()).collect();
    format!("[{}]", values.join(","))
}

#[test]
fn every_standalone_decoder_locks_its_exact_byte_ceiling() {
    let closure = golden();
    let valid = [
        canonical_json(&closure.artifacts[0]).expect("artifact"),
        canonical_json(&closure.artifact_receipts[0]).expect("artifact receipt"),
        canonical_json(&closure.capability_invocations[0]).expect("invocation"),
        canonical_json(&closure.interaction_events[0]).expect("event"),
        canonical_json(&closure.execution_receipts[0]).expect("receipt"),
        canonical_json(&closure).expect("closure"),
    ];
    let decoders: [(usize, Decoder); 6] = [
        (MAX_ARTIFACT_REF_BYTES, artifact_ref_result),
        (MAX_ARTIFACT_RECEIPT_BYTES, artifact_receipt_result),
        (MAX_INVOCATION_BYTES, invocation_result),
        (MAX_EVENT_BYTES, event_result),
        (MAX_EXECUTION_RECEIPT_BYTES, execution_result),
        (MAX_CLOSURE_BYTES, closure_result),
    ];
    for (index, (maximum, decode)) in decoders.into_iter().enumerate() {
        assert!(decode(valid[index].as_bytes()).is_ok());
        assert!(decode(&vec![0; maximum + 1]).is_err());
    }
}

#[test]
fn reused_nested_shapes_reject_missing_extra_bad_hash_bool_and_negative() {
    let closure = golden();
    let invocation = canonical_json(&closure.capability_invocations[0]).expect("invocation");
    let artifact = canonical_json(&closure.artifact_receipts[0]).expect("artifact receipt");
    let receipt = canonical_json(&closure.execution_receipts[0]).expect("receipt");
    let mut tests = basic_mutations(invocation.as_bytes(), artifact.as_bytes());
    tests.extend(grant_mutations(invocation.as_bytes()));
    tests.extend(usage_mutations(receipt.as_bytes()));
    for (raw, decode) in tests {
        assert!(decode(&raw).is_err());
    }
}

type Decoder = fn(&[u8]) -> Result<(), KernelOperationalContractError>;

fn basic_mutations(invocation: &[u8], artifact: &[u8]) -> Vec<(Vec<u8>, Decoder)> {
    vec![
        (
            mutate(invocation, &["subject", "authority_domain"], None),
            invocation_result,
        ),
        (
            mutate(
                invocation,
                &["task_binding", "extra"],
                Some(Value::Bool(false)),
            ),
            invocation_result,
        ),
        (
            mutate(
                invocation,
                &["capability", "capability_contract_sha256"],
                Some(Value::String("bad".into())),
            ),
            invocation_result,
        ),
        (
            mutate(
                artifact,
                &["artifact", "artifact_kind"],
                Some(Value::from(1)),
            ),
            artifact_receipt_result,
        ),
    ]
}

fn grant_mutations(invocation: &[u8]) -> Vec<(Vec<u8>, Decoder)> {
    vec![
        (
            mutate(
                invocation,
                &["capability_grant_ref", "authority_domain"],
                None,
            ),
            invocation_result,
        ),
        (
            mutate(
                invocation,
                &["capability_grant_ref", "extra"],
                Some(Value::from(0)),
            ),
            invocation_result,
        ),
        (
            mutate(
                invocation,
                &["capability_grant_ref", "grant_sha256"],
                Some(Value::String("bad".into())),
            ),
            invocation_result,
        ),
        (
            mutate(
                invocation,
                &["capability_grant_ref", "grant_id"],
                Some(Value::from(1)),
            ),
            invocation_result,
        ),
    ]
}

fn usage_mutations(receipt: &[u8]) -> Vec<(Vec<u8>, Decoder)> {
    vec![
        (
            mutate(receipt, &["observed_usage", "call_count"], None),
            execution_result,
        ),
        (
            mutate(receipt, &["observed_usage", "extra"], Some(Value::from(0))),
            execution_result,
        ),
        (
            mutate(
                receipt,
                &["observed_usage", "call_count"],
                Some(Value::Bool(true)),
            ),
            execution_result,
        ),
        (
            mutate(
                receipt,
                &["observed_usage", "call_count"],
                Some(Value::from(-1)),
            ),
            execution_result,
        ),
    ]
}
