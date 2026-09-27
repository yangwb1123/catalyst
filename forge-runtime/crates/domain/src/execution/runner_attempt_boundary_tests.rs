use super::*;
use serde_json::{Value, json};

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-runner-attempt-boundary-v1.json");

#[test]
fn canonical_attempt_boundary_fixture_is_strict_and_preview_only() {
    let observation = decode(FIXTURE.as_bytes()).expect("canonical Attempt boundary fixture");
    observation.validate().expect("valid Attempt boundary");
    assert!(observation.attempt_boundary_ready);
    assert!(observation.preview_only);
    assert_eq!(observation.rejection_reasons, Vec::<String>::new());
    assert_eq!(
        observation.authority,
        RunnerAttemptBoundaryAuthority::default()
    );
    let encoded = serde_json::to_value(observation).expect("encode observation");
    let expected: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    assert_eq!(encoded, expected);
}

#[test]
fn attempt_boundary_rejects_unknown_duplicate_and_trailing_json() {
    let unknown = FIXTURE.replacen(
        "  \"evaluation_mode\": \"attempt_lifecycle_dispatch_boundary_preview\",\n",
        "  \"evaluation_mode\": \"attempt_lifecycle_dispatch_boundary_preview\",\n  \"unexpected\": true,\n",
        1,
    );
    assert!(decode(unknown.as_bytes()).is_err());

    let duplicate = FIXTURE.replacen(
        "  \"schema_version\": \"forge.runner-attempt-boundary/v1\",\n",
        "  \"schema_version\": \"forge.runner-attempt-boundary/v1\",\n  \"schema_version\": \"forge.runner-attempt-boundary/v1\",\n",
        1,
    );
    assert!(decode(duplicate.as_bytes()).is_err());
    assert!(decode(format!("{FIXTURE} {{}}\n").as_bytes()).is_err());
}

#[test]
fn attempt_boundary_rejects_lifecycle_and_authority_drift() {
    for (field, value) in [
        ("next_attempt_state", json!("running")),
        ("transition", json!("observe_completed")),
        ("attempt_transition_dispatchable", json!(false)),
        ("attempt_boundary_ready", json!(false)),
        ("preview_only", json!(false)),
        (
            "authority",
            json!({
                "attempt_persisted": true,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }),
        ),
    ] {
        let mut value_json: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        value_json[field] = value;
        let observation: RunnerAttemptBoundaryObservation =
            serde_json::from_value(value_json).expect("typed Attempt boundary");
        assert!(observation.validate().is_err(), "drift accepted: {field}");
    }
}

#[test]
fn attempt_boundary_exposes_known_non_dispatchable_edge_without_authority() {
    let mut value_json: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    value_json["current_attempt_state"] = json!("running");
    value_json["next_attempt_state"] = json!("completed");
    value_json["transition"] = json!("observe_completed");
    value_json["attempt_transition_dispatchable"] = json!(false);
    value_json["attempt_boundary_ready"] = json!(false);
    value_json["rejection_reasons"] = json!(["attempt_transition_not_dispatchable"]);
    let observation: RunnerAttemptBoundaryObservation =
        serde_json::from_value(value_json).expect("typed terminal Attempt boundary");
    observation
        .validate()
        .expect("known terminal edge remains observable");
    assert!(!observation.attempt_boundary_ready);
    assert_eq!(
        observation.authority,
        RunnerAttemptBoundaryAuthority::default()
    );
}
