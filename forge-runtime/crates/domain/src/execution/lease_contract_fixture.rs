use super::{LeaseError, LeaseGrant, LeaseProof, LeaseState, TerminalDisposition};
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-runner-lease-fencing-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    grant: LeaseGrant,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    device_identity_verified: bool,
    command_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    operation: String,
    #[serde(default)]
    observed_at_ms: u64,
    #[serde(default)]
    fencing_token: Option<String>,
    #[serde(default)]
    ttl_ms: Option<u64>,
    #[serde(default)]
    proof: Option<LeaseProof>,
    #[serde(default)]
    disposition: Option<TerminalDisposition>,
    #[serde(default)]
    seed_disposition: Option<TerminalDisposition>,
    #[serde(default)]
    seed_observed_at_ms: u64,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    epoch: Option<u64>,
    #[serde(default)]
    issued_at_ms: Option<u64>,
    #[serde(default)]
    expires_at_ms: Option<u64>,
    #[serde(default)]
    replayed: bool,
    #[serde(default)]
    uncertain: bool,
    #[serde(default)]
    automatic_retry: Option<bool>,
}

#[test]
fn lease_fencing_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("lease fixture JSON");
    assert_eq!(fixture.schema_version, "forge.runner-lease-fencing/v1");
    assert_eq!(fixture.evaluation_mode, "pure_lease_fencing_only");
    assert!(!fixture.authority.device_identity_verified);
    assert!(!fixture.authority.command_persisted);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert!(!fixture.authority.audit_published);
    assert_eq!(fixture.cases.len(), 16);
    fixture.grant.validate().expect("fixture grant");

    for case in fixture.cases {
        run_case(&fixture.grant, case);
    }
}

fn run_case(grant: &LeaseGrant, case: Case) {
    match case.operation.as_str() {
        "active" => {
            assert_eq!(
                grant.is_active(case.observed_at_ms),
                case.expected.active.expect("active expectation"),
                "{}",
                case.name
            );
        }
        "renew" => {
            let result = grant.renew(
                case.observed_at_ms,
                case.fencing_token.expect("renew token"),
                case.ttl_ms.expect("renew ttl"),
            );
            assert_lease_result(
                &case.name,
                &case.expected,
                result.as_ref().map(|_| ()).map_err(|error| *error),
            );
            if let Ok(next) = result {
                assert_eq!(
                    next.epoch,
                    case.expected.epoch.expect("renew epoch"),
                    "{}",
                    case.name
                );
                assert_eq!(
                    next.issued_at_ms,
                    case.expected.issued_at_ms.expect("renew issued"),
                    "{}",
                    case.name
                );
                assert_eq!(
                    next.expires_at_ms,
                    case.expected.expires_at_ms.expect("renew expiry"),
                    "{}",
                    case.name
                );
            }
        }
        "proof" => {
            let result = grant.validate_proof(&case.proof.expect("proof"), case.observed_at_ms);
            assert_lease_result(&case.name, &case.expected, result);
        }
        "terminal" | "terminal_replay" | "terminal_conflict" => {
            let mut state = seeded_state(grant, &case);
            let result = state.submit_terminal(
                grant.proof(),
                case.disposition.expect("terminal disposition"),
                case.observed_at_ms,
            );
            assert_lease_result(
                &case.name,
                &case.expected,
                result.as_ref().map(|_| ()).map_err(|error| *error),
            );
            if let Ok(submission) = result {
                assert_eq!(submission.replayed, case.expected.replayed, "{}", case.name);
                assert_eq!(
                    submission.receipt.disposition.is_uncertain(),
                    case.expected.uncertain,
                    "{}",
                    case.name
                );
                if let Some(automatic_retry) = case.expected.automatic_retry {
                    assert!(!automatic_retry, "{}", case.name);
                }
            }
        }
        "renew_after_terminal" => {
            let mut state = seeded_state(grant, &case);
            let result = state.renew(
                case.observed_at_ms,
                case.fencing_token.expect("renew token"),
                case.ttl_ms.expect("renew ttl"),
            );
            assert_lease_result(&case.name, &case.expected, result);
        }
        operation => panic!("{}: unsupported operation {operation}", case.name),
    }
}

fn seeded_state(grant: &LeaseGrant, case: &Case) -> LeaseState {
    let mut state = LeaseState::new(grant.clone()).expect("fixture state");
    if let Some(disposition) = &case.seed_disposition {
        state
            .submit_terminal(grant.proof(), disposition.clone(), case.seed_observed_at_ms)
            .expect("fixture seed terminal");
    }
    state
}

fn assert_lease_result<T>(name: &str, expected: &Expected, result: Result<T, LeaseError>) {
    if expected.accepted {
        result.unwrap_or_else(|error| panic!("{name}: rejected: {error}"));
        return;
    }
    let error = match result {
        Ok(_) => panic!("{name}: accepted unexpectedly"),
        Err(error) => error,
    };
    assert_eq!(error.code(), expected.error.as_deref().unwrap(), "{name}");
}
