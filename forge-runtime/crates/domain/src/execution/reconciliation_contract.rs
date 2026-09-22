use super::reconciliation::{
    EXECUTION_RECONCILIATION_EVALUATION_MODE, EXECUTION_RECONCILIATION_SCHEMA_VERSION,
    ReconciliationAuthority, ReconciliationInput, ReconciliationObservation, observe,
};
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-execution-reconciliation-observation-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: ReconciliationAuthority,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    input: ReconciliationInput,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    next_observation: Option<String>,
    lease_active: Option<bool>,
    terminal_observed: Option<bool>,
    terminal_disposition: Option<String>,
    terminal_state_aligned: Option<bool>,
    reconciliation_required: Option<bool>,
}

#[test]
fn reconciliation_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("reconciliation fixture JSON");
    assert_eq!(
        fixture.schema_version,
        EXECUTION_RECONCILIATION_SCHEMA_VERSION
    );
    assert_eq!(
        fixture.evaluation_mode,
        EXECUTION_RECONCILIATION_EVALUATION_MODE
    );
    assert_eq!(fixture.authority, ReconciliationAuthority::default());
    assert_eq!(fixture.cases.len(), 6);

    for case in fixture.cases {
        let result = observe(case.input);
        if !case.expected.accepted {
            assert!(result.is_err(), "{} accepted unexpectedly", case.name);
            continue;
        }
        let observation = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
        observation
            .validate()
            .unwrap_or_else(|error| panic!("{} output invalid: {error}", case.name));
        assert_eq!(
            observation.next_observation,
            case.expected.next_observation.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            observation.lease_active,
            case.expected.lease_active.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            observation.terminal_observed,
            case.expected.terminal_observed.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            observation.terminal_disposition,
            case.expected.terminal_disposition.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            observation.terminal_state_aligned,
            case.expected.terminal_state_aligned.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            observation.reconciliation_required,
            case.expected.reconciliation_required.unwrap(),
            "{}",
            case.name
        );
    }
}

#[test]
fn reconciliation_observation_rejects_authority_and_retry_claims() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("reconciliation fixture JSON");
    let input = fixture.cases.into_iter().next().expect("active case").input;
    let observation = observe(input).expect("active observation");
    let mut mutated: ReconciliationObservation = observation.clone();
    mutated.authority.dispatch_performed = true;
    assert!(mutated.validate().is_err());
    mutated = observation.clone();
    mutated.automatic_retry = true;
    assert!(mutated.validate().is_err());
    mutated.next_observation = "terminal_completed".into();
    assert!(mutated.validate().is_err());
}

#[test]
fn reconciliation_rejects_unsafe_lease_epoch() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("reconciliation fixture JSON");
    let mut input = fixture.cases.into_iter().next().expect("active case").input;
    input.lease.epoch = 9_007_199_254_740_992;
    assert!(observe(input).is_err());
}
