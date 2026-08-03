use forge_runtime_domain::{
    GroupAgentNodePricingSnapshot, GroupAgentScheduledNodeDispatchAuthorization,
    group_agent_node_destination_sha256, group_agent_scheduled_node_dispatch_authorization_id,
};
use serde::Deserialize;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Deserialize)]
struct PricingFixture {
    expected: PricingExpected,
}

#[derive(Deserialize)]
struct PricingExpected {
    canonical_pricing_snapshot_json: String,
    worst_cost: WorstCost,
}

#[derive(Deserialize)]
struct WorstCost {
    max_output_tokens: u32,
    usd_micros: u64,
}

#[derive(Deserialize)]
struct ScheduledAuthorizationFixture {
    canonical_authorization_json: String,
}

#[test]
fn scheduled_authorization_reuses_the_exact_shared_snapshot_quote() {
    let (snapshot, authorization, expected_cost) = artifacts();
    let quote = snapshot
        .verify_scheduled_authorization(&authorization)
        .expect("scheduled authorization quote");

    assert_eq!(
        quote.pricing_snapshot_sha256,
        snapshot.pricing_snapshot_sha256
    );
    assert_eq!(quote.destination_sha256, snapshot.destination_sha256);
    assert_eq!(quote.max_input_tokens, 400_000);
    assert_eq!(quote.max_output_tokens, 4_096);
    assert_eq!(quote.max_cost_usd_micros, expected_cost);
    assert_eq!(expected_cost, 840_960);
}

#[test]
fn scheduled_quote_rejects_registered_metadata_and_pricing_drift() {
    let (snapshot, baseline, _) = artifacts();
    let mut endpoint = baseline.clone();
    endpoint.endpoint = "https://api.example.com/v1/responses".into();
    rebind_destination_and_sign(&mut endpoint);
    let mut model = baseline.clone();
    model.model = "gpt-other".into();
    rebind_destination_and_sign(&mut model);
    let mut pricing = baseline;
    pricing.pricing_snapshot_sha256 = A.into();
    pricing.budgets.pricing_snapshot_sha256 = A.into();
    sign_authorization(&mut pricing);

    for authorization in [endpoint, model, pricing] {
        authorization
            .validate()
            .expect("internally valid scheduled authorization");
        assert!(
            snapshot
                .verify_scheduled_authorization(&authorization)
                .is_err()
        );
    }
}

#[test]
fn scheduled_quote_fails_closed_below_the_conservative_cost() {
    let (snapshot, mut authorization, expected_cost) = artifacts();
    authorization.budgets.max_cost_usd_micros = expected_cost - 1;
    sign_authorization(&mut authorization);
    authorization.validate().expect("valid smaller budget");

    assert!(
        snapshot
            .verify_scheduled_authorization(&authorization)
            .is_err()
    );
}

fn artifacts() -> (
    GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeDispatchAuthorization,
    u64,
) {
    let fixture: PricingFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-pricing-snapshot-v1.json"
    )))
    .expect("pricing fixture");
    let snapshot = GroupAgentNodePricingSnapshot::decode_exact(
        &fixture.expected.canonical_pricing_snapshot_json,
    )
    .expect("pricing snapshot");
    let authorization = authorization(
        &snapshot,
        fixture.expected.worst_cost.usd_micros,
        fixture.expected.worst_cost.max_output_tokens,
    );
    (
        snapshot,
        authorization,
        fixture.expected.worst_cost.usd_micros,
    )
}

fn authorization(
    snapshot: &GroupAgentNodePricingSnapshot,
    max_cost_usd_micros: u64,
    max_output_tokens: u32,
) -> GroupAgentScheduledNodeDispatchAuthorization {
    let fixture: ScheduledAuthorizationFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("scheduled authorization fixture");
    let mut value = GroupAgentScheduledNodeDispatchAuthorization::decode_exact(
        &fixture.canonical_authorization_json,
    )
    .expect("scheduled authorization");
    value.provider_kind = snapshot.provider_kind;
    value.endpoint.clone_from(&snapshot.endpoint);
    value.model.clone_from(&snapshot.model);
    value
        .destination_sha256
        .clone_from(&snapshot.destination_sha256);
    value
        .pricing_snapshot_sha256
        .clone_from(&snapshot.pricing_snapshot_sha256);
    value
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&snapshot.pricing_snapshot_sha256);
    value.budgets.max_cost_usd_micros = max_cost_usd_micros;
    value.budgets.max_output_tokens = max_output_tokens;
    sign_authorization(&mut value);
    value.validate().expect("valid rebound authorization");
    value
}

fn rebind_destination_and_sign(value: &mut GroupAgentScheduledNodeDispatchAuthorization) {
    value.destination_sha256 =
        group_agent_node_destination_sha256(value.provider_kind, &value.endpoint, &value.model);
    sign_authorization(value);
}

fn sign_authorization(value: &mut GroupAgentScheduledNodeDispatchAuthorization) {
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_scheduled_node_dispatch_authorization_id(&value.authorization_sha256);
}
