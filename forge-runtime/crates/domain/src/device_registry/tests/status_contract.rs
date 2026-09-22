use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-status-contract-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    stale_after_ms: u64,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    input: Observation,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    approval_state: String,
    cordon_state: String,
    liveness: String,
    reservation_state: String,
    snapshot_observed_at_ms: u64,
    lease_expires_at_ms: u64,
    evaluated_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    fresh: Option<bool>,
    #[serde(default)]
    declared_eligible: Option<bool>,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn inventory_status_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-inventory-status-contract/v1"
    );
    assert_eq!(fixture.evaluation_mode, "pure_projection_only");
    assert_eq!(fixture.stale_after_ms, DEFAULT_STALE_AFTER_MS);
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.cases.len(), 11);
    for case in fixture.cases {
        let observation = InventoryStatusObservation {
            approval_state: case.input.approval_state,
            cordon_state: case.input.cordon_state,
            liveness: case.input.liveness,
            reservation_state: case.input.reservation_state,
            snapshot_observed_at_ms: case.input.snapshot_observed_at_ms,
            lease_expires_at_ms: case.input.lease_expires_at_ms,
            evaluated_at_ms: case.input.evaluated_at_ms,
        };
        let result = project_inventory_status(&observation, fixture.stale_after_ms);
        assert_case(&case.name, &case.expected, result);
    }
}

fn assert_case(
    name: &str,
    expected: &Expected,
    result: Result<InventoryStatusProjection, InventoryStatusError>,
) {
    if let Some(error) = expected.error.as_deref() {
        assert_eq!(result.expect_err(name).to_string(), error, "{name}");
        return;
    }
    let projection = result.unwrap_or_else(|error| panic!("{name} rejected: {error}"));
    assert_eq!(
        projection.status.as_str(),
        expected.status.as_deref().unwrap(),
        "{name}"
    );
    assert_eq!(projection.fresh, expected.fresh.unwrap(), "{name}");
    assert_eq!(
        projection.declared_eligible,
        expected.declared_eligible.unwrap(),
        "{name}"
    );
}
