use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::runtime_domain::{
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT, GROUP_AGENT_NODE_PRICING_COST_ALGORITHM,
    GROUP_AGENT_NODE_PRICING_CURRENCY, GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_PRICING_PROVENANCE, GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
    GROUP_AGENT_NODE_PRICING_TOKEN_UNIT, GroupAgentGraphRunInspection, GroupAgentGraphRunRecord,
    GroupAgentGraphRunStatus, GroupAgentGraphRunStore, GroupAgentNodeDestinationRegistryError,
    GroupAgentNodePricingQuote, GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    GroupAgentScheduledNodeDestinationRegistry, GroupAgentScheduledNodeDispatchAuthorization,
    HubStoreError, group_agent_node_destination_sha256,
};

use super::{
    GroupAgentNodeDispatchRequestCodec, GroupAgentScheduledNodeDispatchReadinessService,
    GroupAgentScheduledNodeDispatchReadinessServiceError,
    GroupAgentScheduledNodeDispatchReleaseControlService,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    GroupAgentScheduledNodeProviderRequestService,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
    scheduled_provider_request_tests::{SpyCodec, SpyHub},
    scheduled_release_tests::authorization,
};

const BODY: &[u8] = br#"{"model":"fixture"}"#;
const EXACT_MAX_COST_USD_MICROS: u64 = 1_000_000;

struct PreparedReadiness {
    hub: Arc<SpyHub>,
    codec: Arc<SpyCodec>,
    provider_request_id: String,
    authorization: GroupAgentScheduledNodeDispatchAuthorization,
    authorization_json: String,
    pricing_json: String,
}

#[derive(Default)]
struct VerifyingDestinationRegistry {
    calls: AtomicUsize,
}

impl VerifyingDestinationRegistry {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl GroupAgentScheduledNodeDestinationRegistry for VerifyingDestinationRegistry {
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        pricing
            .verify_scheduled_authorization(authorization)
            .map_err(|_| GroupAgentNodeDestinationRegistryError::Rejected)
    }
}

struct RejectingDestinationRegistry;

impl GroupAgentScheduledNodeDestinationRegistry for RejectingDestinationRegistry {
    fn resolve(
        &self,
        _authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        _pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError> {
        Err(GroupAgentNodeDestinationRegistryError::Rejected)
    }
}

struct MisquotingDestinationRegistry;

impl GroupAgentScheduledNodeDestinationRegistry for MisquotingDestinationRegistry {
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError> {
        let mut quote = pricing
            .verify_scheduled_authorization(authorization)
            .map_err(|_| GroupAgentNodeDestinationRegistryError::Rejected)?;
        quote.max_cost_usd_micros -= 1;
        Ok(quote)
    }
}

#[test]
fn valid_quote_accepts_the_exact_budget_boundary_without_effects() {
    let prepared = prepared_readiness(EXACT_MAX_COST_USD_MICROS);
    let registry = Arc::new(VerifyingDestinationRegistry::default());
    let service = service(&prepared, prepared.hub.clone(), registry.clone());

    let verified = service
        .verify(
            &prepared.provider_request_id,
            &prepared.authorization_json,
            &prepared.pricing_json,
        )
        .expect("scheduled readiness");

    assert_eq!(verified.v, GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION);
    assert_eq!(
        verified.quote.max_cost_usd_micros,
        EXACT_MAX_COST_USD_MICROS
    );
    assert_eq!(
        verified.quote.max_cost_usd_micros,
        prepared.authorization.budgets.max_cost_usd_micros
    );
    assert_eq!(
        verified.authorization.authorization_sha256,
        prepared.authorization.authorization_sha256
    );
    assert_eq!(registry.calls(), 1);
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn one_micro_below_the_exact_budget_boundary_fails_before_registry() {
    let prepared = prepared_readiness(EXACT_MAX_COST_USD_MICROS - 1);
    let registry = Arc::new(VerifyingDestinationRegistry::default());
    let service = service(&prepared, prepared.hub.clone(), registry.clone());

    assert!(matches!(
        service.verify(
            &prepared.provider_request_id,
            &prepared.authorization_json,
            &prepared.pricing_json,
        ),
        Err(GroupAgentScheduledNodeDispatchReadinessServiceError::InvalidInput { .. })
    ));
    assert_eq!(registry.calls(), 0);
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn registry_rejection_and_quote_disagreement_fail_closed() {
    let prepared = prepared_readiness(EXACT_MAX_COST_USD_MICROS);
    for registry in [
        Arc::new(RejectingDestinationRegistry)
            as Arc<dyn GroupAgentScheduledNodeDestinationRegistry>,
        Arc::new(MisquotingDestinationRegistry),
    ] {
        let service = service(&prepared, prepared.hub.clone(), registry);
        assert!(matches!(
            service.verify(
                &prepared.provider_request_id,
                &prepared.authorization_json,
                &prepared.pricing_json,
            ),
            Err(GroupAgentScheduledNodeDispatchReadinessServiceError::InvalidInput { .. })
        ));
    }
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn fresh_current_state_succeeds_and_stale_state_fails_closed() {
    let prepared = prepared_readiness(EXACT_MAX_COST_USD_MICROS);
    let current = service(
        &prepared,
        prepared.hub.clone(),
        Arc::new(VerifyingDestinationRegistry::default()),
    );
    current
        .verify(
            &prepared.provider_request_id,
            &prepared.authorization_json,
            &prepared.pricing_json,
        )
        .expect("current state");

    let stale = service(
        &prepared,
        Arc::new(StaleRunStore(prepared.hub.clone())),
        Arc::new(VerifyingDestinationRegistry::default()),
    );
    assert!(matches!(
        stale.verify(
            &prepared.provider_request_id,
            &prepared.authorization_json,
            &prepared.pricing_json,
        ),
        Err(
            GroupAgentScheduledNodeDispatchReadinessServiceError::Release(
                GroupAgentScheduledNodeDispatchReleaseControlServiceError::Corrupt { .. }
            )
        )
    ));
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn malformed_and_noncanonical_authorization_and_pricing_are_rejected() {
    let prepared = prepared_readiness(EXACT_MAX_COST_USD_MICROS);
    let registry = Arc::new(VerifyingDestinationRegistry::default());
    let service = service(&prepared, prepared.hub.clone(), registry.clone());

    for authorization_json in [
        "{".to_owned(),
        format!("{}\n", prepared.authorization_json),
        prepared
            .authorization_json
            .replacen("{\"v\":1", "{\"unknown\":0,\"v\":1", 1),
    ] {
        assert!(matches!(
            service.verify(
                &prepared.provider_request_id,
                &authorization_json,
                &prepared.pricing_json,
            ),
            Err(GroupAgentScheduledNodeDispatchReadinessServiceError::InvalidInput { .. })
        ));
    }

    for pricing_json in [
        "{".to_owned(),
        format!("{}\n", prepared.pricing_json),
        prepared
            .pricing_json
            .replacen("{\"v\":1", "{\"unknown\":0,\"v\":1", 1),
    ] {
        assert!(matches!(
            service.verify(
                &prepared.provider_request_id,
                &prepared.authorization_json,
                &pricing_json,
            ),
            Err(GroupAgentScheduledNodeDispatchReadinessServiceError::InvalidInput { .. })
        ));
    }
    assert_eq!(registry.calls(), 0);
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

fn prepared_readiness(max_cost_usd_micros: u64) -> PreparedReadiness {
    let pricing = pricing_snapshot();
    let hub = Arc::new(SpyHub::new_with_pricing_policy(
        &pricing.pricing_snapshot_sha256,
        max_cost_usd_micros,
    ));
    let codec = Arc::new(SpyCodec::new(BODY.to_vec()));
    let prepare = GroupAgentScheduledNodeProviderRequestService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec.clone(),
    );
    let result = prepare
        .prepare(&PrepareGroupAgentScheduledNodeProviderRequestInput {
            scheduled_contract_id: hub.contract_id(),
            idempotency_key: "scheduled-readiness-request".into(),
            prepared_at_ms: 91,
        })
        .expect("prepare request");
    let provider_request_id = result.inspection.record.provider_request_id;
    let release = GroupAgentScheduledNodeDispatchReleaseControlService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec.clone(),
    );
    let control = release
        .export(&provider_request_id)
        .expect("release control");
    let authorization = authorization(&control.release_control);
    let authorization_json = authorization.canonical_json().expect("authorization JSON");
    let pricing_json = pricing.canonical_json().expect("pricing JSON");
    hub.reset_mutation_calls();
    PreparedReadiness {
        hub,
        codec,
        provider_request_id,
        authorization,
        authorization_json,
        pricing_json,
    }
}

fn pricing_snapshot() -> GroupAgentNodePricingSnapshot {
    let model = "gpt-5.6-sol".to_owned();
    let endpoint = GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT.to_owned();
    let mut pricing = GroupAgentNodePricingSnapshot {
        v: GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
        pricing_protocol_version: GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
        provider_kind: GroupAgentNodeProviderKind::OpenAiResponses,
        destination_sha256: group_agent_node_destination_sha256(
            GroupAgentNodeProviderKind::OpenAiResponses,
            &endpoint,
            &model,
        ),
        endpoint,
        model,
        currency: GROUP_AGENT_NODE_PRICING_CURRENCY.into(),
        token_unit: GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
        input_usd_micros_per_token_unit: 1,
        output_usd_micros_per_token_unit: 244_140_380,
        max_input_tokens: 1,
        cost_algorithm: GROUP_AGENT_NODE_PRICING_COST_ALGORITHM.into(),
        provenance: GROUP_AGENT_NODE_PRICING_PROVENANCE.into(),
        vendor_attestation_present: false,
        pricing_snapshot_sha256: String::new(),
    };
    pricing.pricing_snapshot_sha256 = pricing.expected_sha256().expect("pricing digest");
    pricing.validate().expect("pricing snapshot");
    pricing
}

fn service(
    prepared: &PreparedReadiness,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    destinations: Arc<dyn GroupAgentScheduledNodeDestinationRegistry>,
) -> GroupAgentScheduledNodeDispatchReadinessService {
    GroupAgentScheduledNodeDispatchReadinessService::new(
        prepared.hub.clone(),
        runs,
        prepared.hub.clone(),
        prepared.hub.clone(),
        prepared.hub.clone(),
        prepared.codec.clone() as Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        destinations,
    )
}

struct StaleRunStore(Arc<SpyHub>);

impl GroupAgentGraphRunStore for StaleRunStore {
    fn begin_group_agent_graph_run(
        &self,
        _request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
        Err(unavailable("unexpected Run mutation"))
    }

    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
        let mut value = self.0.inspect_group_agent_graph_run(graph_run_id)?;
        value.run.status = GroupAgentGraphRunStatus::AwaitingCoreDispatch;
        Ok(value)
    }

    fn list_group_agent_graph_runs(
        &self,
        graph_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
        self.0.list_group_agent_graph_runs(graph_id, limit)
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
