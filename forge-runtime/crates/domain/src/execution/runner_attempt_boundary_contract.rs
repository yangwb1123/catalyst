use super::runner_attempt_boundary::{
    RUNNER_ATTEMPT_BOUNDARY_EVALUATION_MODE, RUNNER_ATTEMPT_BOUNDARY_SCHEMA_VERSION,
    RunnerAttemptBoundaryAuthority, RunnerAttemptBoundaryObservation, decode,
};

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-runner-attempt-boundary-v1.json");

#[test]
fn runner_attempt_boundary_contract_fixture_matches_canonical_wire() {
    let observation: RunnerAttemptBoundaryObservation =
        decode(FIXTURE.as_bytes()).expect("strict Runner Attempt boundary fixture");
    observation
        .validate()
        .expect("valid Runner Attempt boundary");
    assert_eq!(
        observation.schema_version,
        RUNNER_ATTEMPT_BOUNDARY_SCHEMA_VERSION
    );
    assert_eq!(
        observation.evaluation_mode,
        RUNNER_ATTEMPT_BOUNDARY_EVALUATION_MODE
    );
    assert!(observation.attempt_boundary_ready);
    assert!(observation.rejection_reasons.is_empty());
    assert_eq!(
        observation.authority,
        RunnerAttemptBoundaryAuthority::default()
    );
}
