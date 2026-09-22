use std::path::PathBuf;

use serde_json::Value;

use super::{decode_remote_response, execute, write_output};
use crate::args::{Command, DeviceCommand, parse_tokens};

const FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-device-credential-candidate-v1.json");

#[test]
fn credential_candidate_fixture_is_strict_and_metadata_only() {
    let bytes = FIXTURE.as_bytes();
    crate::device_json_unique::reject_duplicate_keys(bytes).expect("unique fixture keys");
    let value: Value = serde_json::from_slice(bytes).expect("fixture JSON");
    let decoded = decode_remote_response(&value).expect("valid candidate response");
    assert_eq!(
        decoded.schema_version,
        "forge.device-credential-lifecycle/v1"
    );
    assert_eq!(decoded.evaluation_mode, "pure_device_credential_lifecycle");
    assert_eq!(decoded.device_id, "device-1");
    assert_eq!(decoded.revision, 7);
    assert_eq!(decoded.next.credential_id, "credential-1");
    assert!(decoded.previous.is_none());
    assert!(decoded.preview_only);
    assert!(decoded.candidate_published);
    assert!(!decoded.authority.owner_binding_matched);
    assert!(!decoded.authority.owner_authenticated);
    assert!(!decoded.authority.credential_material_made);
    assert!(!decoded.authority.persisted);
    assert!(!decoded.authority.inventory_authoritative);
    assert!(!decoded.authority.execution_authorized);
}

#[test]
fn credential_candidate_preview_reads_the_canonical_file_without_network() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-credential-candidate-v1.json");
    let output = execute(&DeviceCommand::CredentialCandidatePreview {
        input: path.display().to_string(),
    })
    .expect("credential candidate preview");
    let mut text = Vec::new();
    write_output(&output, false, &mut text).expect("human output");
    let text = String::from_utf8(text).expect("UTF-8 output");
    assert!(text.contains("offline device credential candidate"));
    assert!(text.contains("revision=7"));
    assert!(text.contains("credential_material_made=false"));
}

#[test]
fn credential_candidate_decoder_rejects_unknown_secret_owner_and_authority_drift() {
    let mutations: [(&str, fn(&mut Value)); 5] = [
        ("unknown", |value: &mut Value| {
            value["unexpected"] = Value::Bool(true);
        }),
        ("secret", |value: &mut Value| {
            value["credential_material"] = Value::String("secret".into());
        }),
        ("owner", |value: &mut Value| {
            value["next"]["owner"]["subject"] = Value::String("other-user".into());
        }),
        ("authority", |value: &mut Value| {
            value["authority"]["credential_material_made"] = Value::Bool(true);
        }),
        ("preview", |value: &mut Value| {
            value["preview_only"] = Value::Bool(false);
        }),
    ];
    for (name, mutate) in mutations {
        let mut value: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        mutate(&mut value);
        assert!(
            decode_remote_response(&value).is_err(),
            "mutation {name} must fail"
        );
    }

    let duplicate = FIXTURE.replacen(
        r#"  "schema_version": "forge.device-credential-lifecycle/v1","#,
        r#"  "schema_version": "forge.device-credential-lifecycle/v1",
  "schema_version": "forge.device-credential-lifecycle/v1","#,
        1,
    );
    assert!(crate::device_json_unique::reject_duplicate_keys(duplicate.as_bytes()).is_err());
}

#[test]
fn credential_candidate_decoder_matches_core_identifier_alphabet() {
    for character in ['+', '/'] {
        let mut value: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        value["device_id"] = Value::String(format!("device{character}1"));
        value["next"]["device_id"] = Value::String(format!("device{character}1"));
        assert!(
            decode_remote_response(&value).is_err(),
            "identifier character {character:?} must be rejected"
        );

        let mut key_value: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        key_value["next"]["key_id"] = Value::String(format!("key{character}1"));
        assert!(
            decode_remote_response(&key_value).is_err(),
            "key identifier character {character:?} must be rejected"
        );
    }
}

#[test]
fn credential_candidate_preview_argument_requires_one_input() {
    let parsed = parse_tokens(
        [
            "--json",
            "device",
            "credential-candidate-preview",
            "--input",
            "candidate.json",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .expect("candidate preview arguments");
    assert_eq!(
        parsed.command,
        Command::Device(DeviceCommand::CredentialCandidatePreview {
            input: "candidate.json".into()
        })
    );
    for tokens in [
        vec!["device", "credential-candidate-preview"],
        vec![
            "device",
            "credential-candidate-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid credential candidate arguments must fail"
        );
    }
}

#[test]
fn credential_candidate_fixture_serializes_without_secret_material() {
    let value: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let decoded = decode_remote_response(&value).expect("valid candidate response");
    let encoded = serde_json::to_string(&decoded).expect("candidate JSON");
    assert!(!encoded.contains("\"credential_material\":"));
    assert!(!encoded.contains("\"bearer\":"));
    assert!(!encoded.contains("\"private_key\":"));
}
