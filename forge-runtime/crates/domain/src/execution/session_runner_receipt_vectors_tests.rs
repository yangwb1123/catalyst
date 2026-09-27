use super::session_runner_receipt::{
    SESSION_RUNNER_RECEIPT_OBSERVATION_EVALUATION_MODE,
    SESSION_RUNNER_RECEIPT_OBSERVATION_SCHEMA_VERSION, SessionRunnerReceiptAuthority,
    SessionRunnerReceiptObservation,
};
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-vectors-v1.json"
);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionRunnerReceiptVectorsFixture {
    schema_version: String,
    evaluation_mode: String,
    authority: SessionRunnerReceiptAuthority,
    vectors: Vec<SessionRunnerReceiptVector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionRunnerReceiptVector {
    name: String,
    observation: SessionRunnerReceiptObservation,
    expected: SessionRunnerReceiptVectorExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionRunnerReceiptVectorExpected {
    command_id: String,
    command_sha256: String,
    attempt_id: String,
    target_id: String,
    disposition_kind: String,
    observed_at_ms: u64,
    uncertain: bool,
    reconciliation_required: bool,
    manual_review_required: bool,
    automatic_retry: bool,
    follow_up: String,
}

#[test]
fn session_runner_receipt_vectors_match_all_terminal_outcomes() {
    let fixture: SessionRunnerReceiptVectorsFixture =
        serde_json::from_str(FIXTURE).expect("strict session receipt vectors");
    assert_eq!(
        fixture.schema_version,
        "forge.session-runner-receipt-vectors/v1"
    );
    assert_eq!(
        fixture.evaluation_mode,
        "pure_session_runner_receipt_vectors_only"
    );
    assert_eq!(fixture.authority, SessionRunnerReceiptAuthority::default());
    assert_eq!(fixture.vectors.len(), 3);

    for vector in fixture.vectors {
        assert!(!vector.name.is_empty());
        vector
            .observation
            .validate()
            .expect("valid session receipt observation");
        let receipt = &vector.observation.receipt_observation;
        assert_eq!(receipt.command_id, vector.expected.command_id);
        assert_eq!(receipt.command_sha256, vector.expected.command_sha256);
        assert_eq!(receipt.attempt_id, vector.expected.attempt_id);
        assert_eq!(receipt.target_id, vector.expected.target_id);
        assert_eq!(receipt.disposition_kind, vector.expected.disposition_kind);
        assert_eq!(receipt.observed_at_ms, vector.expected.observed_at_ms);
        assert_eq!(receipt.uncertain, vector.expected.uncertain);
        assert_eq!(
            receipt.reconciliation_required,
            vector.expected.reconciliation_required
        );
        assert_eq!(
            receipt.manual_review_required,
            vector.expected.manual_review_required
        );
        assert_eq!(receipt.automatic_retry, vector.expected.automatic_retry);
        assert_eq!(receipt.follow_up, vector.expected.follow_up);
        assert_eq!(
            vector.observation.schema_version,
            SESSION_RUNNER_RECEIPT_OBSERVATION_SCHEMA_VERSION
        );
        assert_eq!(
            vector.observation.evaluation_mode,
            SESSION_RUNNER_RECEIPT_OBSERVATION_EVALUATION_MODE
        );
        assert!(vector.observation.selected_target_id.is_none());
    }
}

#[test]
fn session_runner_receipt_vectors_reject_wire_and_outcome_drift() {
    let duplicate = FIXTURE.replace(
        "\"schema_version\": \"forge.session-runner-receipt-vectors/v1\",",
        "\"schema_version\": \"forge.session-runner-receipt-vectors/v1\", \"schema_version\": \"forge.session-runner-receipt-vectors/v1\",",
    );
    assert!(serde_json::from_str::<SessionRunnerReceiptVectorsFixture>(&duplicate).is_err());

    let unknown = FIXTURE.replace(
        "\"evaluation_mode\": \"pure_session_runner_receipt_vectors_only\",",
        "\"evaluation_mode\": \"pure_session_runner_receipt_vectors_only\", \"unexpected\": true,",
    );
    assert!(serde_json::from_str::<SessionRunnerReceiptVectorsFixture>(&unknown).is_err());

    let trailing = format!("{FIXTURE} {{}}");
    let mut stream = serde_json::Deserializer::from_str(&trailing);
    let _: SessionRunnerReceiptVectorsFixture =
        SessionRunnerReceiptVectorsFixture::deserialize(&mut stream).expect("first JSON value");
    assert!(stream.end().is_err());

    let fixture: SessionRunnerReceiptVectorsFixture =
        serde_json::from_str(FIXTURE).expect("fixture");
    let mut foreign = fixture.vectors[0].observation.clone();
    foreign.receipt_observation.command_id = "command-foreign".into();
    assert!(foreign.validate().is_ok());
    assert_ne!(
        foreign.receipt_observation.command_id,
        fixture.vectors[0].expected.command_id
    );

    let mut retry = fixture.vectors[2].observation.clone();
    retry.receipt_observation.automatic_retry = true;
    assert!(retry.validate().is_err());

    let mut selected = fixture.vectors[2].observation.clone();
    selected.selected_target_id = Some("runner-1".into());
    assert!(selected.validate().is_err());
}
