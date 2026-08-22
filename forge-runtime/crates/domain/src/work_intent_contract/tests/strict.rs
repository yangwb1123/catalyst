use serde_json::{Value, json};

use super::{super::*, support::*};

fn canonical_value(value: &Value) -> Vec<u8> {
    super::super::wire::canonical_json_value(value)
        .expect("canonical test value")
        .into_bytes()
}

fn without_path(path: &[&str]) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(fixture_body()).expect("fixture value");
    let (field, parents) = path.split_last().expect("nonempty path");
    let mut current = &mut value;
    for parent in parents {
        current = current.get_mut(*parent).expect("fixture parent");
    }
    current
        .as_object_mut()
        .expect("fixture object")
        .remove(*field)
        .expect("fixture field");
    canonical_value(&value)
}

#[test]
fn decoder_rejects_root_and_nested_duplicate_keys() {
    let body = std::str::from_utf8(fixture_body()).expect("UTF-8 fixture");
    let root = body.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"x\",\"api_version\":",
        1,
    );
    let nested = body.replacen(
        "\"binding\":{\"change_id\":",
        "\"binding\":{\"run_id\":null,\"change_id\":",
        1,
    );
    assert!(decode_canonical_work_intent(root.as_bytes()).is_err());
    assert!(decode_canonical_work_intent(nested.as_bytes()).is_err());
}

#[test]
fn required_nullable_members_cannot_disappear() {
    for path in [
        &["declared_owner"][..],
        &["binding", "run_id"],
        &["intent", "deadline_unix_ms"],
        &["origin", "origin_ref"],
        &["references", "local_source_snapshot_declaration"],
    ] {
        assert!(decode_canonical_work_intent(&without_path(path)).is_err());
    }
}

#[test]
fn decoder_rejects_unknown_noncanonical_and_lf_input() {
    let body = fixture_body();
    let mut leading_space = vec![b' '];
    leading_space.extend_from_slice(body);
    let mut trailing_space = body.to_vec();
    trailing_space.push(b' ');
    let unknown = std::str::from_utf8(body).expect("fixture text").replacen(
        "{\"api_version\":",
        "{\"alien\":null,\"api_version\":",
        1,
    );
    for bytes in [
        leading_space,
        trailing_space,
        FIXTURE_BYTES.to_vec(),
        unknown.into_bytes(),
    ] {
        assert!(decode_canonical_work_intent(&bytes).is_err());
    }
}

#[test]
fn decoder_rejects_float_bool_tool_invalid_utf8_and_forbidden_text() {
    let body = std::str::from_utf8(fixture_body()).expect("fixture text");
    let float = body.replacen("1700000000000", "1.0", 1);
    let exponent = body.replacen("1700000000000", "1e3", 1);
    let negative_zero = body.replacen("1700000000000", "-0", 1);
    let boolean = body.replacen("1700000000000", "true", 1);
    let tool = body.replacen(
        "\"principal_type\":\"agent\"",
        "\"principal_type\":\"tool\"",
        1,
    );
    let c1 = body.replacen("Publish an authority-neutral", "bad\\u0085", 1);
    let bidi = body.replacen("Publish an authority-neutral", "bad\\u202e", 1);
    for bytes in [
        float.into_bytes(),
        exponent.into_bytes(),
        negative_zero.into_bytes(),
        boolean.into_bytes(),
        tool.into_bytes(),
        c1.into_bytes(),
        bidi.into_bytes(),
        vec![b'\"', 0xff, b'\"'],
    ] {
        assert!(decode_canonical_work_intent(&bytes).is_err());
    }
}

#[test]
fn generic_wire_limits_match_depth_field_array_and_integer_bounds() {
    let valid_depth: Value = serde_json::from_str("[[[[[[[0]]]]]]]").expect("depth eight");
    let invalid_depth: Value = serde_json::from_str("[[[[[[[[0]]]]]]]]").expect("depth nine");
    assert!(super::super::wire::validate_json_value(&valid_depth, 1).is_ok());
    assert!(super::super::wire::validate_json_value(&invalid_depth, 1).is_err());
    assert!(super::super::wire::validate_json_value(&json!(i64::MIN), 1).is_ok());
    let unsigned = serde_json::from_str::<Value>("9223372036854775808").expect("u64 JSON");
    assert!(super::super::wire::validate_json_value(&unsigned, 1).is_err());
}

#[test]
fn generic_wire_limits_accept_n_and_reject_n_plus_one() {
    let array = Value::Array((0..MAX_ARRAY_ITEMS).map(|value| json!(value)).collect());
    assert!(super::super::wire::validate_json_value(&array, 1).is_ok());
    let oversized = Value::Array((0..=MAX_ARRAY_ITEMS).map(|value| json!(value)).collect());
    assert!(super::super::wire::validate_json_value(&oversized, 1).is_err());
    let object = (0..MAX_OBJECT_FIELDS)
        .map(|index| (format!("k{index}"), json!(index)))
        .collect();
    assert!(super::super::wire::validate_json_value(&Value::Object(object), 1).is_ok());
    let oversized_object = (0..=MAX_OBJECT_FIELDS)
        .map(|index| (format!("k{index}"), json!(index)))
        .collect();
    assert!(super::super::wire::validate_json_value(&Value::Object(oversized_object), 1).is_err());
}
