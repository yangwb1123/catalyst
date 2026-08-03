use forge_runtime_domain::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchProviderFactory,
    GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    GroupAgentScheduledNodeDestinationRegistry, GroupAgentScheduledNodeDispatchAuthorization,
    group_agent_node_destination_sha256, group_agent_node_dispatch_authorization_id,
    group_agent_scheduled_node_dispatch_authorization_id,
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

#[derive(Deserialize)]
struct ScheduledAuthorizationFixture {
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
fn scheduled_registry_is_effect_free_and_returns_only_the_exact_quote() {
    let (snapshot, authorization) = scheduled_artifacts();
    let factory = RegisteredGroupAgentNodeProviderFactory::new();
    let registry: &dyn GroupAgentScheduledNodeDestinationRegistry = &factory;
    let quote = registry
        .resolve(&authorization, &snapshot)
        .expect("registered scheduled quote");

    assert_eq!(quote.destination_sha256, snapshot.destination_sha256);
    assert_eq!(
        quote.pricing_snapshot_sha256,
        snapshot.pricing_snapshot_sha256
    );
    assert_eq!(quote.max_input_tokens, 400_000);
    assert_eq!(quote.max_output_tokens, 4_096);
    assert_eq!(quote.max_cost_usd_micros, 840_960);
}

#[test]
fn scheduled_destination_registry_rejects_the_complete_representable_drift_matrix() {
    let factory = RegisteredGroupAgentNodeProviderFactory::new();
    let registry: &dyn GroupAgentScheduledNodeDestinationRegistry = &factory;
    let (snapshot, baseline) = scheduled_artifacts();
    let quote = registry
        .resolve(&baseline, &snapshot)
        .expect("scheduled registry quote");
    assert_eq!(quote.max_cost_usd_micros, 840_960);

    for authorization in scheduled_authorization_drifts(&baseline) {
        assert!(registry.resolve(&authorization, &snapshot).is_err());
    }
    let (input_token_snapshot, input_token_authorization) =
        scheduled_input_token_drift(snapshot, baseline);
    assert!(
        registry
            .resolve(&input_token_authorization, &input_token_snapshot)
            .is_err()
    );
}

#[test]
fn scheduled_provider_kind_drift_is_closed_before_the_registry() {
    let fixture: ScheduledAuthorizationFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("scheduled authorization fixture");
    let drifted = fixture.canonical_authorization_json.replacen(
        "\"provider_kind\":\"openai_responses\"",
        "\"provider_kind\":\"unregistered_provider\"",
        1,
    );
    assert_ne!(drifted, fixture.canonical_authorization_json);
    assert!(GroupAgentScheduledNodeDispatchAuthorization::decode_exact(&drifted).is_err());
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

#[test]
fn lifecycle_factory_trait_resolves_and_builds_without_provider_io() {
    let factory = RegisteredGroupAgentNodeProviderFactory::new();
    let lifecycle_factory: &dyn GroupAgentNodeDispatchProviderFactory = &factory;
    let (snapshot, authorization) = artifacts();
    let resolved = lifecycle_factory
        .resolve(&authorization, &snapshot)
        .expect("trait readiness");

    assert_eq!(
        resolved.authorization_sha256,
        authorization.authorization_sha256
    );
    assert_eq!(resolved.destination_sha256, snapshot.destination_sha256);
    assert_eq!(
        resolved.pricing_snapshot_sha256,
        snapshot.pricing_snapshot_sha256
    );
    assert_eq!(resolved.max_cost_usd_micros, 840_960);
    lifecycle_factory
        .build(resolved.clone(), "explicit-test-credential".into())
        .expect("trait construction is local");

    let mut forged = resolved;
    forged.destination_sha256 = A.into();
    let Err(error) = lifecycle_factory.build(forged, "secret-must-not-leak".into()) else {
        panic!("forged readiness accepted");
    };
    assert_eq!(
        error.message,
        "registered Group Agent Node provider is unavailable"
    );
    assert!(!error.message.contains("secret-must-not-leak"));
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

fn scheduled_artifacts() -> (
    GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeDispatchAuthorization,
) {
    let (snapshot, _) = artifacts();
    let fixture: ScheduledAuthorizationFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("scheduled authorization fixture");
    let mut authorization = GroupAgentScheduledNodeDispatchAuthorization::decode_exact(
        &fixture.canonical_authorization_json,
    )
    .expect("scheduled authorization");
    authorization.provider_kind = snapshot.provider_kind;
    authorization.endpoint.clone_from(&snapshot.endpoint);
    authorization.model.clone_from(&snapshot.model);
    authorization
        .destination_sha256
        .clone_from(&snapshot.destination_sha256);
    authorization
        .pricing_snapshot_sha256
        .clone_from(&snapshot.pricing_snapshot_sha256);
    authorization
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&snapshot.pricing_snapshot_sha256);
    authorization.budgets.max_cost_usd_micros = 840_960;
    authorization.budgets.max_output_tokens = 4_096;
    sign_scheduled_authorization(&mut authorization);
    authorization
        .validate()
        .expect("valid rebound scheduled authorization");
    (snapshot, authorization)
}

fn scheduled_authorization_drifts(
    baseline: &GroupAgentScheduledNodeDispatchAuthorization,
) -> Vec<GroupAgentScheduledNodeDispatchAuthorization> {
    let mut endpoint = baseline.clone();
    endpoint.endpoint = "https://api.example.com/v1/responses".into();
    rebind_scheduled_destination_and_sign(&mut endpoint);
    let mut model = baseline.clone();
    model.model = "gpt-other".into();
    rebind_scheduled_destination_and_sign(&mut model);
    let mut destination = baseline.clone();
    destination.destination_sha256 = A.into();
    sign_scheduled_authorization(&mut destination);
    let mut pricing = baseline.clone();
    pricing.pricing_snapshot_sha256 = A.into();
    pricing.budgets.pricing_snapshot_sha256 = A.into();
    sign_scheduled_authorization(&mut pricing);
    let mut output_tokens = baseline.clone();
    output_tokens.budgets.max_output_tokens += 1;
    sign_scheduled_authorization(&mut output_tokens);
    let mut insufficient = baseline.clone();
    insufficient.budgets.max_cost_usd_micros -= 1;
    sign_scheduled_authorization(&mut insufficient);
    vec![
        endpoint,
        model,
        destination,
        pricing,
        output_tokens,
        insufficient,
    ]
}

fn scheduled_input_token_drift(
    mut snapshot: GroupAgentNodePricingSnapshot,
    mut authorization: GroupAgentScheduledNodeDispatchAuthorization,
) -> (
    GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeDispatchAuthorization,
) {
    snapshot.max_input_tokens += 1;
    snapshot.pricing_snapshot_sha256 = snapshot.expected_sha256().expect("pricing digest");
    snapshot
        .validate()
        .expect("valid input-token drift snapshot");
    authorization
        .pricing_snapshot_sha256
        .clone_from(&snapshot.pricing_snapshot_sha256);
    authorization
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&snapshot.pricing_snapshot_sha256);
    sign_scheduled_authorization(&mut authorization);
    authorization
        .validate()
        .expect("valid input-token drift authorization");
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

fn rebind_scheduled_destination_and_sign(value: &mut GroupAgentScheduledNodeDispatchAuthorization) {
    value.destination_sha256 =
        group_agent_node_destination_sha256(value.provider_kind, &value.endpoint, &value.model);
    sign_scheduled_authorization(value);
}

fn sign_scheduled_authorization(value: &mut GroupAgentScheduledNodeDispatchAuthorization) {
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_scheduled_node_dispatch_authorization_id(&value.authorization_sha256);
}
