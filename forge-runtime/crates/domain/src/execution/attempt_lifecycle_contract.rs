use super::attempt_lifecycle::{AttemptLifecycle, AttemptTransitionRequest};
use crate::platform_core_contract::{AttemptState, validate_attempt_transition};
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-attempt-lifecycle-v1.json");

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
    from: String,
    to: String,
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn attempt_lifecycle_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("strict Attempt lifecycle fixture");
    assert_eq!(fixture.schema_version, "forge.attempt-lifecycle/v1");
    assert_eq!(fixture.evaluation_mode, "pure_attempt_lifecycle_only");
    assert!(!fixture.authority.device_identity_verified);
    assert!(!fixture.authority.command_persisted);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert!(!fixture.authority.audit_published);
    assert_eq!(fixture.cases.len(), 19);

    for case in fixture.cases {
        let result = validate_attempt_transition(&state(&case.from), &state(&case.to));
        if case.accepted {
            result.unwrap_or_else(|error| panic!("{}: edge rejected: {error}", case.name));
        } else {
            let error = result.expect_err("edge accepted unexpectedly");
            assert_eq!(
                error.code.as_str(),
                case.error.as_deref().expect("rejection code"),
                "{}",
                case.name
            );
        }
    }

    let initial = AttemptLifecycle::requested();
    assert_eq!(initial.state(), &AttemptState::Requested);
    let next = initial
        .reduce(&AttemptTransitionRequest::Accept)
        .expect("requested -> accepted");
    assert_eq!(next.state(), &AttemptState::Accepted);
    assert_eq!(initial.state(), &AttemptState::Requested);
}

fn state(value: &str) -> AttemptState {
    match value {
        "requested" => AttemptState::Requested,
        "accepted" => AttemptState::Accepted,
        "starting" => AttemptState::Starting,
        "running" => AttemptState::Running,
        "interrupted" => AttemptState::Interrupted,
        "completed" => AttemptState::Completed,
        "failed" => AttemptState::Failed,
        "uncertain" => AttemptState::Uncertain,
        other => AttemptState::Unknown(other.to_owned()),
    }
}
