use forge_runtime_domain::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    group_agent_node_destination_sha256, group_agent_node_dispatch_authorization_id,
};
use forge_runtime_infrastructure::RegisteredGroupAgentNodeProviderFactory;
use serde::Deserialize;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Deserialize)]
struct PricingFixture {
    expected: PricingExpected,
}

#[derive(Deserialize)]
struct PricingExpected {
    canonical_pricing_snapshot_json: String,
    pricing_snapshot_sha256: String,
    worst_cost: WorstCost,
}

#[derive(Deserialize)]
struct WorstCost {
    max_output_tokens: u32,
    usd_micros: u64,
}

#[derive(Deserialize)]
struct AuthorizationFixture {
    canonical_authorization_json: String,
}

#[test]
fn resolve_is_effect_free_and_binds_exact_registered_metadata() {
    let (snapshot, authorization) = artifacts();
    let readiness = RegisteredGroupAgentNodeProviderFactory::new()
        .resolve(&authorization, &snapshot)
        .expect("registered readiness");

    assert_eq!(
        readiness.authorization_sha256(),
        authorization.authorization_sha256
    );
    assert_eq!(
        readiness.provider_kind(),
        GroupAgentNodeProviderKind::OpenAiResponses
    );
    assert_eq!(readiness.endpoint(), "https://api.openai.com/v1/responses");
    assert_eq!(readiness.model(), "gpt-5.6-sol");
    assert_eq!(readiness.destination_sha256(), snapshot.destination_sha256);
    assert_eq!(
        readiness.pricing_snapshot_sha256(),
        snapshot.pricing_snapshot_sha256
    );
    assert_eq!(readiness.quote().max_cost_usd_micros, 840_960);
}

#[test]
fn resolve_rejects_authorization_destination_model_and_pricing_drift() {
    let factory = RegisteredGroupAgentNodeProviderFactory::new();
    let (snapshot, baseline) = artifacts();
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
            .expect("valid drifted authorization");
        assert!(factory.resolve(&authorization, &snapshot).is_err());
    }
}

#[test]
fn explicit_credential_build_uses_only_the_registered_adapter_configuration() {
    let factory = RegisteredGroupAgentNodeProviderFactory::new();
    let (snapshot, authorization) = artifacts();
    let readiness = factory.resolve(&authorization, &snapshot).unwrap();
    let provider = factory
        .build(readiness, "explicit-test-credential".into())
        .expect("local construction does not perform network I/O");

    assert_eq!(provider.endpoint(), "https://api.openai.com/v1/responses");
    assert_eq!(provider.model(), "gpt-5.6-sol");
    assert_eq!(provider.readiness().quote().max_cost_usd_micros, 840_960);
}

#[test]
fn credential_construction_errors_are_redacted() {
    let factory = RegisteredGroupAgentNodeProviderFactory::new();
    let (snapshot, authorization) = artifacts();
    for secret in [
        "",
        " ",
        " leading-secret",
        "trailing-secret ",
        "secret-must-not-leak\nheader",
    ] {
        let readiness = factory.resolve(&authorization, &snapshot).unwrap();
        let Err(error) = factory.build(readiness, secret.into()) else {
            panic!("unsafe credential was accepted");
        };
        if !secret.trim().is_empty() {
            assert!(!error.message.contains(secret));
        }
        assert_eq!(
            error.message,
            "registered Group Agent Node provider credential is invalid"
        );
    }
}

fn artifacts() -> (
    GroupAgentNodePricingSnapshot,
    GroupAgentNodeDispatchAuthorization,
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
    assert_eq!(
        snapshot.pricing_snapshot_sha256,
        fixture.expected.pricing_snapshot_sha256
    );
    let authorization = authorization(
        &snapshot,
        fixture.expected.worst_cost.usd_micros,
        fixture.expected.worst_cost.max_output_tokens,
    );
    (snapshot, authorization)
}

fn authorization(
    snapshot: &GroupAgentNodePricingSnapshot,
    max_cost_usd_micros: u64,
    max_output_tokens: u32,
) -> GroupAgentNodeDispatchAuthorization {
    let fixture: AuthorizationFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-dispatch-authorization-v1.json"
    )))
    .expect("authorization fixture");
    let mut value: GroupAgentNodeDispatchAuthorization =
        serde_json::from_str(&fixture.canonical_authorization_json).expect("authorization");
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

fn rebind_destination_and_sign(value: &mut GroupAgentNodeDispatchAuthorization) {
    value.destination_sha256 =
        group_agent_node_destination_sha256(value.provider_kind, &value.endpoint, &value.model);
    sign_authorization(value);
}

fn sign_authorization(value: &mut GroupAgentNodeDispatchAuthorization) {
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256);
}
