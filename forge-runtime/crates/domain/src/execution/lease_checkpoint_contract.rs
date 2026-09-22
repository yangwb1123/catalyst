use super::lease::{
    EXECUTION_LEASE_CHECKPOINT_EVALUATION_MODE, EXECUTION_LEASE_CHECKPOINT_SCHEMA_VERSION,
    LeaseCheckpoint, LeaseError, LeaseState,
};
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-execution-lease-checkpoint-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    lease_issued: bool,
    terminal_persisted: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    checkpoint: LeaseCheckpoint,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    terminal: bool,
    #[serde(default)]
    uncertain: bool,
}

#[test]
fn lease_checkpoint_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("lease checkpoint fixture JSON");
    assert_eq!(
        fixture.schema_version,
        EXECUTION_LEASE_CHECKPOINT_SCHEMA_VERSION
    );
    assert_eq!(
        fixture.evaluation_mode,
        EXECUTION_LEASE_CHECKPOINT_EVALUATION_MODE
    );
    assert!(!fixture.authority.lease_issued);
    assert!(!fixture.authority.terminal_persisted);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert!(!fixture.authority.audit_published);
    assert_eq!(fixture.cases.len(), 4);

    for case in fixture.cases {
        let result = LeaseState::from_checkpoint(case.checkpoint);
        if case.expected.accepted {
            let state = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(
                state.terminal().is_some(),
                case.expected.terminal,
                "{}",
                case.name
            );
            assert_eq!(
                state
                    .terminal()
                    .is_some_and(|receipt| receipt.disposition.is_uncertain()),
                case.expected.uncertain,
                "{}",
                case.name
            );
        } else {
            let error = result.expect_err("checkpoint accepted unexpectedly");
            assert_eq!(error, LeaseError::InvalidCheckpoint, "{}", case.name);
            assert_eq!(
                error.code(),
                case.expected.error.as_deref().unwrap(),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn checkpoint_round_trip_keeps_uncertain_terminal_and_rejects_proof_drift() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("lease checkpoint fixture JSON");
    let case = fixture
        .cases
        .into_iter()
        .find(|case| case.name == "uncertain_receipt_remains_terminal")
        .expect("uncertain case");
    let state = LeaseState::from_checkpoint(case.checkpoint).expect("restore uncertain state");
    assert!(
        state
            .terminal()
            .expect("terminal")
            .disposition
            .is_uncertain()
    );
    let mut mutated = state.checkpoint();
    mutated.terminal.as_mut().expect("terminal").proof.target_id = "foreign-target".into();
    assert_eq!(
        LeaseState::from_checkpoint(mutated),
        Err(LeaseError::InvalidCheckpoint)
    );
}
