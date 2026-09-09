use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{AttemptJournalError, MAX_REQUEST_BYTES, codec, crash_fixture};

#[test]
fn private_codec_round_trips_explicit_nulls_and_domain_separated_digest() {
    let request = crash_fixture::request();
    let record = codec::encode(&request).unwrap();
    assert_eq!(codec::decode(&record.json).unwrap(), request);
    let value: Value = serde_json::from_str(&record.json).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 17);
    for field in [
        "context_artifact_ref",
        "workspace_capability_ref",
        "grant_ref",
    ] {
        assert_eq!(value[field], Value::Null);
    }
    assert_eq!(value["format"], "forge.runtime.attempt-request.storage.v1");
    assert_eq!(value["version"], 1);
    let mut digest = Sha256::new();
    digest.update(b"forge.runtime.attempt-request.storage.v1\0");
    digest.update(record.json.as_bytes());
    assert_eq!(record.sha256, format!("{:x}", digest.finalize()));
    assert_ne!(
        record.sha256,
        format!("{:x}", Sha256::digest(record.json.as_bytes()))
    );
}

#[test]
fn every_storage_field_and_nested_field_is_required_exactly() {
    let value = value();
    for key in value.as_object().unwrap().keys() {
        let mut changed = value.clone();
        changed.as_object_mut().unwrap().remove(key);
        rejected_value(&changed);
    }
    for field in [
        "scope_ref",
        "attempt_ref",
        "work_item_ref",
        "project_ref",
        "project_snapshot_ref",
        "control_versions",
        "executor",
        "budget",
    ] {
        for key in value[field].as_object().unwrap().keys() {
            let mut changed = value.clone();
            changed[field].as_object_mut().unwrap().remove(key);
            rejected_value(&changed);
        }
        let mut changed = value.clone();
        changed[field]["extra"] = json!(1);
        rejected_value(&changed);
    }
    let mut changed = value;
    changed["extra"] = json!(null);
    rejected_value(&changed);
}

#[test]
fn alternate_encodings_duplicates_and_storage_versions_are_rejected() {
    let canonical = codec::encode(&crash_fixture::request()).unwrap().json;
    for text in [
        format!("{canonical}\n"),
        format!(" {canonical}"),
        canonical.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        canonical.replacen("\"version\":1", "\"version\":1e0", 1),
        canonical.replacen("\"timeout_ms\":1000", "\"timeout_ms\":1000.0", 1),
        canonical.replacen("\"space_id\":", "\"space_id\":null,\"space_id\":", 1),
        canonical.replacen(
            "\"grant_ref\":null",
            "\"grant_ref\":null,\"grant_ref\":null",
            1,
        ),
        canonical.replacen("\"version\"", "\"ver\\u0073ion\"", 1),
        serde_json::to_string_pretty(&value()).unwrap(),
    ] {
        assert_ne!(text, canonical);
        assert_eq!(codec::decode(&text), Err(AttemptJournalError::Corrupt));
    }
    for (key, replacement) in [
        ("version", json!(0)),
        ("version", json!(2)),
        ("format", json!("forge.runtime.attempt-request.storage.v2")),
        ("timeout_ms", json!(0)),
    ] {
        let mut changed = value();
        changed[key] = replacement;
        rejected_value(&changed);
    }
}

#[test]
fn restored_records_reenter_the_frozen_request_validator() {
    for (field, key, replacement) in [
        ("budget", "max_duration_ms", json!(0)),
        ("budget", "max_model_calls", json!(-1)),
        ("control_versions", "work_item_version", json!(0)),
        ("attempt_ref", "entity_type", json!("session")),
        (
            "project_ref",
            "entity_id",
            json!("prj_00000000000000000000000002"),
        ),
        ("executor", "adapter_version", json!("")),
        (
            "scope_ref",
            "session_id",
            json!("ses_00000000000000000000000001"),
        ),
    ] {
        let mut changed = value();
        changed[field][key] = replacement;
        rejected_value(&changed);
    }
    let mut changed = value();
    changed["requested_effects"] = json!(["repo.write", "process.exec"]);
    rejected_value(&changed);
    assert_eq!(codec::decode(""), Err(AttemptJournalError::Corrupt));
    assert_eq!(
        codec::decode(&" ".repeat(MAX_REQUEST_BYTES + 1)),
        Err(AttemptJournalError::Corrupt)
    );
}

fn value() -> Value {
    serde_json::from_str(&codec::encode(&crash_fixture::request()).unwrap().json).unwrap()
}

fn rejected_value(value: &Value) {
    let text = serde_json::to_string(value).unwrap();
    assert_eq!(
        codec::decode(&text),
        Err(AttemptJournalError::Corrupt),
        "{text}"
    );
}
