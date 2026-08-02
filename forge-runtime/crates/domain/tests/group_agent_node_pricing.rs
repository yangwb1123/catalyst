use forge_runtime_domain::{
    GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT, GROUP_AGENT_NODE_PRICING_COST_ALGORITHM,
    GROUP_AGENT_NODE_PRICING_CURRENCY, GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_PRICING_PROVENANCE, GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
    GROUP_AGENT_NODE_PRICING_TOKEN_UNIT, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind, MAX_GROUP_AGENT_NODE_MODEL_BYTES,
    MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS, MAX_GROUP_AGENT_NODE_PRICING_INPUT_TOKENS,
    MAX_GROUP_AGENT_NODE_PRICING_RATE_USD_MICROS, MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
    group_agent_node_destination_sha256, group_agent_node_dispatch_authorization_id,
};
use serde::Deserialize;
use serde_json::Value;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Deserialize)]
struct PricingFixture {
    v: u16,
    cost_vectors: Vec<CostVector>,
    expected: PricingExpected,
}

#[derive(Deserialize)]
struct CostVector {
    name: String,
    input_usd_micros_per_token_unit: u64,
    output_usd_micros_per_token_unit: u64,
    max_input_tokens: u64,
    max_output_tokens: u32,
    expected_usd_micros: u64,
}

#[derive(Deserialize)]
struct PricingExpected {
    canonical_pricing_payload_json: String,
    canonical_pricing_snapshot_json: String,
    destination_sha256: String,
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
fn shared_fixture_has_exact_cross_language_identity_and_quote() {
    let fixture = pricing_fixture();
    let snapshot = GroupAgentNodePricingSnapshot::decode_exact(
        &fixture.expected.canonical_pricing_snapshot_json,
    )
    .expect("shared pricing snapshot");
    let authorization = authorization(
        &snapshot,
        fixture.expected.worst_cost.usd_micros,
        fixture.expected.worst_cost.max_output_tokens,
    );
    let quote = snapshot
        .verify_authorization(&authorization)
        .expect("shared worst-cost quote");

    assert_eq!(fixture.v, 1);
    assert_eq!(
        snapshot.canonical_payload_json().unwrap(),
        fixture.expected.canonical_pricing_payload_json
    );
    assert_eq!(
        snapshot.expected_sha256().unwrap(),
        fixture.expected.pricing_snapshot_sha256
    );
    assert_eq!(
        snapshot.destination_sha256,
        fixture.expected.destination_sha256
    );
    assert_eq!(
        quote.max_cost_usd_micros,
        fixture.expected.worst_cost.usd_micros
    );
    assert_eq!(quote.max_input_tokens, 400_000);
    assert_eq!(quote.max_output_tokens, 4_096);
}

#[test]
fn shared_cost_vectors_recompute_every_expected_micro_usd_total() {
    let fixture = pricing_fixture();
    assert_required_cost_vectors(&fixture.cost_vectors);
    for vector in fixture.cost_vectors {
        let mut candidate = snapshot();
        candidate.input_usd_micros_per_token_unit = vector.input_usd_micros_per_token_unit;
        candidate.output_usd_micros_per_token_unit = vector.output_usd_micros_per_token_unit;
        candidate.max_input_tokens = vector.max_input_tokens;
        sign_snapshot(&mut candidate);
        assert_eq!(
            candidate
                .maximum_cost_usd_micros(vector.max_output_tokens)
                .expect("valid shared cost vector"),
            vector.expected_usd_micros,
            "shared cost vector {}",
            vector.name
        );
    }
}

fn assert_required_cost_vectors(vectors: &[CostVector]) {
    for required in [
        "exact_division",
        "input_round_up_one_unit",
        "output_round_up_one_unit",
        "both_components_round_up_one_unit",
        "current_golden",
        "protocol_maximum_supported_values",
    ] {
        assert!(
            vectors.iter().any(|vector| vector.name == required),
            "shared cost matrix omits {required}"
        );
    }
}

#[test]
fn exact_decoder_rejects_noncanonical_or_ambiguous_json() {
    let canonical = pricing_fixture().expected.canonical_pricing_snapshot_json;
    let reordered =
        serde_json::to_string(&serde_json::from_str::<Value>(&canonical).expect("JSON value"))
            .expect("reordered JSON");
    let duplicate = canonical.replacen("{\"v\":1", "{\"v\":1,\"v\":1", 1);
    let unknown = canonical.replacen("{\"v\":1", "{\"unknown\":1,\"v\":1", 1);
    let escaped = canonical.replacen("gpt-5.6-sol", "gpt-5.6-\\u0073ol", 1);

    for candidate in [
        format!("{canonical}\n"),
        format!(" {canonical}"),
        reordered,
        duplicate,
        unknown,
        escaped,
    ] {
        assert!(GroupAgentNodePricingSnapshot::decode_exact(&candidate).is_err());
    }
}

#[test]
fn exact_decoder_rejects_missing_null_wrong_type_and_oversize() {
    let canonical = pricing_fixture().expected.canonical_pricing_snapshot_json;
    let mut missing = serde_json::from_str::<Value>(&canonical).expect("JSON value");
    missing.as_object_mut().unwrap().remove("currency");
    let mut null = serde_json::from_str::<Value>(&canonical).expect("JSON value");
    null["model"] = Value::Null;
    let mut wrong_type = serde_json::from_str::<Value>(&canonical).expect("JSON value");
    wrong_type["token_unit"] = Value::String("1000000".into());

    for candidate in [missing, null, wrong_type] {
        let encoded = serde_json::to_string(&candidate).expect("mutated JSON");
        assert!(GroupAgentNodePricingSnapshot::decode_exact(&encoded).is_err());
    }
    let oversized = "x".repeat(MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES + 1);
    assert!(GroupAgentNodePricingSnapshot::decode_exact(&oversized).is_err());
}

#[test]
fn decoder_errors_never_echo_untrusted_input() {
    let untrusted_input = "untrusted-pricing-material-must-not-leak";
    let error = GroupAgentNodePricingSnapshot::decode_exact(&format!("{{{untrusted_input}"))
        .expect_err("invalid JSON");
    assert!(!error.message.contains(untrusted_input));
}

#[test]
fn snapshot_header_and_identity_are_closed_world() {
    let baseline = snapshot();
    assert_invalid_snapshot(&mutate(&baseline, |value| value.v += 1));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.pricing_protocol_version += 1;
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.endpoint = "https://example.com/v1/responses".into();
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| value.model.clear()));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.model = "x".repeat(MAX_GROUP_AGENT_NODE_MODEL_BYTES + 1);
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.model = "bad\nmodel".into();
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.destination_sha256 = A.into();
    }));

    let mut bad_digest = baseline;
    bad_digest.pricing_snapshot_sha256 = A.into();
    assert!(bad_digest.validate().is_err());
}

#[test]
fn snapshot_pricing_policy_is_closed_world_and_bounded() {
    let baseline = snapshot();
    assert_invalid_snapshot(&mutate(&baseline, |value| value.currency = "usd".into()));
    assert_invalid_snapshot(&mutate(&baseline, |value| value.token_unit += 1));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.input_usd_micros_per_token_unit = 0;
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.input_usd_micros_per_token_unit = MAX_GROUP_AGENT_NODE_PRICING_RATE_USD_MICROS + 1;
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.output_usd_micros_per_token_unit = 0;
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.output_usd_micros_per_token_unit = MAX_GROUP_AGENT_NODE_PRICING_RATE_USD_MICROS + 1;
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| value.max_input_tokens = 0));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.max_input_tokens = MAX_GROUP_AGENT_NODE_PRICING_INPUT_TOKENS + 1;
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.cost_algorithm = "floor_v1".into();
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.provenance = "vendor".into();
    }));
    assert_invalid_snapshot(&mutate(&baseline, |value| {
        value.vendor_attestation_present = true;
    }));
}

#[test]
fn quote_uses_component_ceiling_and_accepts_only_an_exact_budget() {
    let mut tiny = snapshot();
    tiny.max_input_tokens = 1;
    tiny.input_usd_micros_per_token_unit = 1;
    tiny.output_usd_micros_per_token_unit = 1;
    sign_snapshot(&mut tiny);
    let mut authorization = authorization(&tiny, 2, 1);

    let quote = tiny
        .verify_authorization(&authorization)
        .expect("exact budget");
    assert_eq!(quote.max_cost_usd_micros, 2);
    authorization.budgets.max_cost_usd_micros = 1;
    sign_authorization(&mut authorization);
    assert!(tiny.verify_authorization(&authorization).is_err());
}

#[test]
fn actual_cost_checks_observed_tokens_component_rounding_and_authorized_bounds() {
    let snapshot = snapshot();
    let authorization = authorization(&snapshot, 840_960, 4_096);
    assert_eq!(
        snapshot
            .actual_cost_usd_micros(5, 3, &authorization)
            .expect("checked actual usage"),
        40
    );
    for (input_tokens, output_tokens) in [(0, 3), (5, 0), (400_001, 3), (5, 4_097)] {
        assert!(
            snapshot
                .actual_cost_usd_micros(input_tokens, output_tokens, &authorization)
                .is_err()
        );
    }

    let mut insufficient = authorization;
    insufficient.budgets.max_cost_usd_micros = 39;
    sign_authorization(&mut insufficient);
    assert!(
        snapshot
            .actual_cost_usd_micros(5, 3, &insufficient)
            .is_err()
    );
}

#[test]
fn pure_cost_rejects_public_values_before_out_of_protocol_arithmetic() {
    let baseline = snapshot();
    assert!(baseline.maximum_cost_usd_micros(0).is_err());
    assert!(
        baseline
            .maximum_cost_usd_micros(MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS + 1)
            .is_err()
    );

    let mut outside_snapshot_bounds = baseline;
    outside_snapshot_bounds.max_input_tokens = u64::MAX;
    outside_snapshot_bounds.input_usd_micros_per_token_unit = u64::MAX;
    outside_snapshot_bounds.output_usd_micros_per_token_unit = u64::MAX;
    sign_snapshot(&mut outside_snapshot_bounds);
    let error = outside_snapshot_bounds
        .maximum_cost_usd_micros(1)
        .expect_err("snapshot bounds reject out-of-protocol arithmetic first");
    assert_eq!(error.message, "invalid Group Agent Node pricing snapshot");
}

#[test]
fn quote_rejects_each_authorization_binding_drift() {
    let snapshot = snapshot();
    let baseline = authorization(&snapshot, 1_000_000, 4_096);
    let mut endpoint = baseline.clone();
    endpoint.endpoint = "https://api.example.com/v1/responses".into();
    rebind_destination_and_sign(&mut endpoint);
    let mut model = baseline.clone();
    model.model = "gpt-other".into();
    rebind_destination_and_sign(&mut model);
    let mut pricing = baseline.clone();
    pricing.pricing_snapshot_sha256 = A.into();
    pricing.budgets.pricing_snapshot_sha256 = A.into();
    sign_authorization(&mut pricing);

    for candidate in [endpoint, model, pricing] {
        candidate
            .validate()
            .expect("internally valid authorization");
        assert!(snapshot.verify_authorization(&candidate).is_err());
    }
}

fn pricing_fixture() -> PricingFixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-pricing-snapshot-v1.json"
    )))
    .expect("pricing fixture wrapper")
}

fn snapshot() -> GroupAgentNodePricingSnapshot {
    GroupAgentNodePricingSnapshot::decode_exact(
        &pricing_fixture().expected.canonical_pricing_snapshot_json,
    )
    .expect("valid snapshot")
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
    .expect("authorization fixture wrapper");
    let mut value = serde_json::from_str::<GroupAgentNodeDispatchAuthorization>(
        &fixture.canonical_authorization_json,
    )
    .expect("authorization");
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
    value.validate().expect("rebound authorization");
    value
}

fn mutate(
    snapshot: &GroupAgentNodePricingSnapshot,
    mutation: impl FnOnce(&mut GroupAgentNodePricingSnapshot),
) -> GroupAgentNodePricingSnapshot {
    let mut candidate = snapshot.clone();
    mutation(&mut candidate);
    sign_snapshot(&mut candidate);
    candidate
}

fn assert_invalid_snapshot(snapshot: &GroupAgentNodePricingSnapshot) {
    assert!(snapshot.validate().is_err());
}

fn sign_snapshot(snapshot: &mut GroupAgentNodePricingSnapshot) {
    snapshot.pricing_snapshot_sha256 = snapshot.expected_sha256().expect("pricing digest");
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

#[test]
fn public_pricing_constants_match_the_protocol() {
    assert_eq!(GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION, 1);
    assert_eq!(GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION, 1);
    assert_eq!(
        GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT,
        "https://api.openai.com/v1/responses"
    );
    assert_eq!(GROUP_AGENT_NODE_PRICING_CURRENCY, "usd_micros");
    assert_eq!(GROUP_AGENT_NODE_PRICING_TOKEN_UNIT, 1_000_000);
    assert_eq!(
        GROUP_AGENT_NODE_PRICING_COST_ALGORITHM,
        "ceil_each_token_component_v1"
    );
    assert_eq!(GROUP_AGENT_NODE_PRICING_PROVENANCE, "operator_asserted");
    assert_eq!(
        snapshot().provider_kind,
        GroupAgentNodeProviderKind::OpenAiResponses
    );
}
