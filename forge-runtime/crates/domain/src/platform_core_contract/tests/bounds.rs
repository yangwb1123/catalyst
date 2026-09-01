use std::cell::Cell;

use serde::{Serialize, Serializer, ser::SerializeSeq};
use serde_json::{Map, Value};

use super::super::{
    CommandEnvelope, EventEnvelope, MAX_ARRAY_ITEMS, MAX_EXTENSION_FIELDS, MAX_EXTENSIONS_BYTES,
    MAX_JSON_DEPTH, MAX_OBJECT_FIELDS, MAX_PAYLOAD_BYTES, MAX_STRING_BYTES,
    canonical_command_envelope_json, canonical_event_envelope_json,
    decode_canonical_command_envelope, decode_canonical_event_envelope, identity,
    validate_command_envelope, validate_event_envelope, wire,
};
use super::fixture;

#[test]
fn payload_byte_boundary_is_exact() {
    let mut value = fixture().command_envelope;
    value.payload = Some(Map::from_iter([
        ("a".into(), Value::String("x".repeat(MAX_STRING_BYTES))),
        ("b".into(), Value::String("x".repeat(MAX_STRING_BYTES - 15))),
    ]));
    let encoded = wire::canonical_value(
        &Value::Object(value.payload.clone().unwrap()),
        MAX_PAYLOAD_BYTES,
    )
    .expect("payload N");
    assert_eq!(encoded.len(), MAX_PAYLOAD_BYTES);
    assert!(validate_command_envelope(&value).is_ok());

    value
        .payload
        .as_mut()
        .unwrap()
        .insert("b".into(), Value::String("x".repeat(MAX_STRING_BYTES - 14)));
    assert!(validate_command_envelope(&value).is_err());
}

#[test]
fn collection_and_string_boundaries_are_enforced() {
    let mut value = fixture().command_envelope;
    value.payload = Some(Map::from_iter([(
        "items".into(),
        Value::Array(vec![Value::Null; MAX_ARRAY_ITEMS]),
    )]));
    assert!(validate_command_envelope(&value).is_ok());
    value.payload.as_mut().unwrap().insert(
        "items".into(),
        Value::Array(vec![Value::Null; MAX_ARRAY_ITEMS + 1]),
    );
    assert!(validate_command_envelope(&value).is_err());

    value = fixture().command_envelope;
    value.payload = Some(Map::from_iter([(
        "text".into(),
        Value::String("x".repeat(MAX_STRING_BYTES)),
    )]));
    assert!(canonical_command_envelope_json(&value).is_ok());
    value.payload.as_mut().unwrap().insert(
        "text".into(),
        Value::String("x".repeat(MAX_STRING_BYTES + 1)),
    );
    assert!(canonical_command_envelope_json(&value).is_err());
}

#[test]
fn extension_count_boundary_is_enforced() {
    let mut value = fixture().command_envelope;
    value.extensions.clear();
    for index in 0..MAX_EXTENSION_FIELDS {
        value.extensions.insert(
            format!("fixture.key_{index}"),
            Value::Number(i64::try_from(index).unwrap().into()),
        );
    }
    assert!(validate_command_envelope(&value).is_ok());
    value.extensions.insert("fixture.extra".into(), Value::Null);
    assert!(validate_command_envelope(&value).is_err());
}

#[test]
fn object_depth_and_extension_byte_boundaries_are_enforced() {
    let mut value = fixture().command_envelope;
    value.payload = Some(
        (0..MAX_OBJECT_FIELDS)
            .map(|index| (format!("field_{index}"), Value::Null))
            .collect(),
    );
    assert!(validate_command_envelope(&value).is_ok());
    value
        .payload
        .as_mut()
        .unwrap()
        .insert("field_extra".into(), Value::Null);
    assert!(validate_command_envelope(&value).is_err());

    value = fixture().command_envelope;
    value.extensions = Map::from_iter([(
        "fixture.large".into(),
        Value::String("x".repeat(MAX_EXTENSIONS_BYTES)),
    )]);
    assert!(validate_command_envelope(&value).is_err());

    let mut nested = Value::Null;
    for _ in 0..MAX_JSON_DEPTH {
        nested = Value::Array(vec![nested]);
    }
    value.extensions.clear();
    value.payload = Some(Map::from_iter([("nested".into(), nested)]));
    assert!(validate_command_envelope(&value).is_err());
}

#[test]
fn platform_id_boundaries_are_enforced() {
    for value in [
        "spc_00000000000000000000000000",
        "act_7zzzzzzzzzzzzzzzzzzzzzzzzz",
    ] {
        assert!(identity::validate_platform_id(value).is_ok());
    }
    for value in [
        "spc_80000000000000000000000000",
        "spc_0000000000000000000000000i",
        "run_00000000000000000000000001",
        "spc_0000000000000000000000001",
    ] {
        assert!(identity::validate_platform_id(value).is_err());
    }
}

#[test]
fn canonical_writer_stops_at_high_fanout() {
    let shared = Value::Array(vec![Value::Null; MAX_ARRAY_ITEMS]);
    let fanout = Value::Array(vec![shared; MAX_ARRAY_ITEMS]);
    let value = Value::Object(Map::from_iter([("fanout".into(), fanout)]));
    assert!(wire::canonical_value(&value, MAX_PAYLOAD_BYTES).is_err());
}

struct ExcessiveSequence<'a>(&'a Cell<usize>);

impl Serialize for ExcessiveSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        loop {
            self.0.set(self.0.get() + 1);
            sequence.serialize_element(&false)?;
        }
    }
}

#[test]
fn typed_writer_stops_a_programmatic_sequence_at_the_byte_ceiling() {
    let visits = Cell::new(0);
    assert!(wire::canonical_typed(&ExcessiveSequence(&visits), MAX_PAYLOAD_BYTES).is_err());
    assert!(visits.get() < MAX_PAYLOAD_BYTES);
}

#[test]
fn whole_envelope_depth_is_exact_for_payloads_and_extensions() {
    for arrays in [MAX_JSON_DEPTH - 3, MAX_JSON_DEPTH - 2] {
        let accepted = arrays == MAX_JSON_DEPTH - 3;
        let nested = nested_arrays(arrays);

        let mut command = fixture().command_envelope;
        command.payload = Some(Map::from_iter([("nested".into(), nested.clone())]));
        command.payload_artifact_ref = None;
        assert_command_depth(&command, accepted);
        command = fixture().command_envelope;
        command.extensions = Map::from_iter([("fixture.nested".into(), nested.clone())]);
        assert_command_depth(&command, accepted);

        let mut event = fixture().event_envelope;
        event.payload = Some(Map::from_iter([("nested".into(), nested.clone())]));
        event.payload_artifact_ref = None;
        assert_event_depth(&event, accepted);
        event = fixture().event_envelope;
        event.extensions = Map::from_iter([("fixture.nested".into(), nested)]);
        assert_event_depth(&event, accepted);
    }
}

fn nested_arrays(count: usize) -> Value {
    (0..count).fold(Value::Null, |value, _| Value::Array(vec![value]))
}

fn assert_command_depth(value: &CommandEnvelope, accepted: bool) {
    assert_eq!(validate_command_envelope(value).is_ok(), accepted);
    let encoded = canonical_command_envelope_json(value);
    assert_eq!(encoded.is_ok(), accepted);
    if let Ok(encoded) = encoded {
        assert!(decode_canonical_command_envelope(encoded.as_bytes()).is_ok());
    } else {
        let raw = serde_json::to_vec(&serde_json::to_value(value).unwrap()).unwrap();
        let error = decode_canonical_command_envelope(&raw).unwrap_err();
        assert!(error.message.contains("depth"));
    }
}

fn assert_event_depth(value: &EventEnvelope, accepted: bool) {
    assert_eq!(validate_event_envelope(value).is_ok(), accepted);
    let encoded = canonical_event_envelope_json(value);
    assert_eq!(encoded.is_ok(), accepted);
    if let Ok(encoded) = encoded {
        assert!(decode_canonical_event_envelope(encoded.as_bytes()).is_ok());
    } else {
        let raw = serde_json::to_vec(&serde_json::to_value(value).unwrap()).unwrap();
        let error = decode_canonical_event_envelope(&raw).unwrap_err();
        assert!(error.message.contains("depth"));
    }
}
