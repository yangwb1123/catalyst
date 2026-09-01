use super::super::{
    canonical_artifact_ref_json, canonical_command_envelope_json, canonical_event_envelope_json,
    decode_canonical_artifact_ref, decode_canonical_command_envelope,
    decode_canonical_event_envelope,
};
use super::{fixture, replace_once};

#[test]
fn command_decoder_rejects_noncanonical_framing_and_shapes() {
    let canonical =
        canonical_command_envelope_json(&fixture().command_envelope).expect("command canonical");
    let actor = r#"{"actor_id":"acr_0000000000000000000000000a","actor_type":"service"}"#;
    let mut invalid_utf8 = canonical.as_bytes().to_vec();
    invalid_utf8[20] = 0xff;
    let cases = [
        format!(" {canonical}").into_bytes(),
        format!("{canonical}null").into_bytes(),
        replace_once(
            &canonical,
            "{\"actor_ref\":",
            "{\"extra\":true,\"actor_ref\":",
        ),
        replace_once(
            &canonical,
            "{\"actor_ref\":",
            &format!("{{\"actor_ref\":{actor},\"actor_ref\":"),
        ),
        replace_once(
            &canonical,
            "\"payload\":{\"agent_adapter\":\"codex\",",
            "\"payload\":{\"agent_adapter\":\"codex\",\"agent_adapter\":\"codex\",",
        ),
        invalid_utf8,
    ];
    assert_invalid_commands(cases);
}

#[test]
fn command_decoder_rejects_invalid_integer_scalars() {
    let canonical =
        canonical_command_envelope_json(&fixture().command_envelope).expect("command canonical");
    let cases = [
        replace_once(
            &canonical,
            r#""expected_version":3"#,
            r#""expected_version":3.0"#,
        ),
        replace_once(
            &canonical,
            r#""expected_version":3"#,
            r#""expected_version":9223372036854775808"#,
        ),
        replace_once(
            &canonical,
            r#""envelope_version":1"#,
            r#""envelope_version":true"#,
        ),
        replace_once(
            &canonical,
            r#""schema_version":1"#,
            r#""schema_version":true"#,
        ),
        replace_once(
            &canonical,
            r#""expected_version":3"#,
            r#""expected_version":true"#,
        ),
        replace_once(
            &canonical,
            r#""issued_at_unix_ms":1787961600000"#,
            r#""issued_at_unix_ms":true"#,
        ),
        replace_once(
            &canonical,
            r#""deadline_unix_ms":1787961660000"#,
            r#""deadline_unix_ms":true"#,
        ),
    ];
    assert_invalid_commands(cases);
}

#[test]
fn command_decoder_rejects_version_and_text_drift() {
    let canonical =
        canonical_command_envelope_json(&fixture().command_envelope).expect("command canonical");
    let cases = [
        replace_once(
            &canonical,
            r#""envelope_version":1"#,
            r#""envelope_version":2"#,
        ),
        replace_once(&canonical, r#""causation_id":null,"#, ""),
        replace_once(
            &canonical,
            r#""agent_adapter":"codex""#,
            r#""agent_adapter":"codex\u202e""#,
        ),
    ];
    assert_invalid_commands(cases);
}

fn assert_invalid_commands(cases: impl IntoIterator<Item = Vec<u8>>) {
    for bytes in cases {
        assert!(decode_canonical_command_envelope(&bytes).is_err());
    }
}

#[test]
fn event_decoder_rejects_nested_unknown_missing_and_future_envelope() {
    let canonical =
        canonical_event_envelope_json(&fixture().event_envelope).expect("event canonical");
    let cases = [
        replace_once(
            &canonical,
            r#""actor_type":"service""#,
            r#""actor_type":"service","extra":true"#,
        ),
        replace_once(
            &canonical,
            r#""source_snapshot_ref":{"entity_id"#,
            r#""source_snapshot_ref":{"extra":true,"entity_id"#,
        ),
        replace_once(&canonical, r#""extensions":{},"#, ""),
        replace_once(
            &canonical,
            r#""envelope_version":1"#,
            r#""envelope_version":2"#,
        ),
        replace_once(
            &canonical,
            r#""aggregate_version":4"#,
            r#""aggregate_version":true"#,
        ),
        replace_once(
            &canonical,
            r#""occurred_at_unix_ms":1787961605000"#,
            r#""occurred_at_unix_ms":true"#,
        ),
        replace_once(&canonical, r#""sequence":9"#, r#""sequence":true"#),
        replace_once(
            &canonical,
            r#""source_component":"runtime""#,
            r#""source_component":"unknown""#,
        ),
    ];
    for bytes in cases {
        assert!(decode_canonical_event_envelope(&bytes).is_err());
    }
}

#[test]
fn artifact_decoder_rejects_identity_enum_and_framing_drift() {
    let canonical =
        canonical_artifact_ref_json(&fixture().artifact_ref).expect("artifact canonical");
    let cases = [
        replace_once(&canonical, r#""size_bytes":4096"#, r#""size_bytes":4e3"#),
        replace_once(&canonical, r#""size_bytes":4096"#, r#""size_bytes":true"#),
        replace_once(
            &canonical,
            r#""created_at_unix_ms":1787961604000"#,
            r#""created_at_unix_ms":true"#,
        ),
        replace_once(
            &canonical,
            r#""content_digest":"#,
            r#""extra":null,"content_digest":"#,
        ),
        replace_once(&canonical, r#""sensitivity":"internal","#, ""),
        replace_once(
            &canonical,
            r#""retention_class":"project""#,
            r#""retention_class":"forever""#,
        ),
        format!("{canonical}\n").into_bytes(),
    ];
    for bytes in cases {
        assert!(decode_canonical_artifact_ref(&bytes).is_err());
    }
}
