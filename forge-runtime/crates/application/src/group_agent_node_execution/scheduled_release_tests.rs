use std::sync::Arc;

use serde_json::{Map, Value, json};

use crate::runtime_domain::{
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_CONSENT_CONTRACT_VERSION, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus, GroupAgentGraphRunStore,
    GroupAgentScheduledNodeDispatchAtomicTransitionRequirement,
    GroupAgentScheduledNodeDispatchAuthorization,
    GroupAgentScheduledNodeDispatchConsentRequirement,
    GroupAgentScheduledNodeDispatchCredentialPreflight,
    GroupAgentScheduledNodeDispatchDestinationPreflight,
    GroupAgentScheduledNodeDispatchPricingPreflight,
    GroupAgentScheduledNodeDispatchProjectLaneClaim,
    GroupAgentScheduledNodeDispatchProviderHealthCheck,
    GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchReleaseRequirements,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestRecord,
    GroupAgentScheduledNodeProviderRequestStore, GroupAgentScheduledNodeSuccessorRequirement,
    HubStoreError, ModelRequest, PrepareGroupAgentScheduledNodeProviderRequest,
    PrepareGroupAgentScheduledNodeProviderRequestResult, ProviderError,
    group_agent_scheduled_node_dispatch_authorization_id,
};

use super::{
    GroupAgentNodeDispatchRequestCodec, GroupAgentScheduledNodeDispatchReleaseControlService,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    GroupAgentScheduledNodeProviderRequestService,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
    scheduled_provider_request_tests::{SpyCodec, SpyHub},
};

const BODY: &[u8] = br#"{"model":"fixture"}"#;

struct PreparedRelease {
    hub: Arc<SpyHub>,
    request_id: String,
    service: GroupAgentScheduledNodeDispatchReleaseControlService,
}

#[test]
fn valid_export_and_verify_are_exact_bounded_and_effect_free() {
    let prepared = prepared_release();
    let export = prepared
        .service
        .export(&prepared.request_id)
        .expect("export");
    assert_eq!(
        export.release_control.canonical_json().unwrap(),
        export.canonical_json
    );
    assert_eq!(
        export.release_control.provider_request_json.as_bytes(),
        BODY
    );
    assert!(!export.canonical_json.ends_with('\n'));

    let authorization = authorization(&export.release_control);
    let json = authorization.canonical_json().expect("authorization JSON");
    let verified = prepared
        .service
        .verify(&prepared.request_id, &json)
        .expect("verify");
    assert_eq!(verified.authorization_id, authorization.authorization_id);
    assert_eq!(verified.scheduled_provider_request_id, prepared.request_id);
    assert_eq!(verified.request_body_bytes, BODY.len());
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn verify_rejects_requested_provider_request_identity_mismatch_before_storage() {
    let prepared = prepared_release();
    let control = prepared.service.export(&prepared.request_id).unwrap();
    let json = authorization(&control.release_control)
        .canonical_json()
        .unwrap();
    prepared.hub.reset_mutation_calls();
    let error = prepared
        .service
        .verify("scheduled-node-provider-request-different", &json)
        .expect_err("mismatched requested ID");
    assert!(matches!(
        error,
        GroupAgentScheduledNodeDispatchReleaseControlServiceError::InvalidInput { .. }
    ));
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn verify_rejects_malformed_noncanonical_unknown_and_oversize_authorization() {
    let prepared = prepared_release();
    let control = prepared.service.export(&prepared.request_id).unwrap();
    let canonical = authorization(&control.release_control)
        .canonical_json()
        .unwrap();
    let invalid = [
        "{".to_owned(),
        format!("{canonical}\n"),
        canonical.replacen("{\"v\":1", "{\"unknown\":0,\"v\":1", 1),
        "x".repeat(1024 * 1024 + 1),
    ];
    for json in invalid {
        assert!(matches!(
            prepared.service.verify(&prepared.request_id, &json),
            Err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::InvalidInput { .. })
        ));
    }
}

#[test]
fn stale_run_and_codec_divergence_fail_closed() {
    let prepared = prepared_release();
    let current = prepared.service.export(&prepared.request_id).unwrap();
    let authorization_json = authorization(&current.release_control)
        .canonical_json()
        .unwrap();
    let stale = Arc::new(StaleRunStore(Arc::clone(&prepared.hub)));
    let stale_service =
        service_with_run(&prepared.hub, stale, Arc::new(SpyCodec::new(BODY.to_vec())));
    assert!(matches!(
        stale_service.verify(&prepared.request_id, &authorization_json),
        Err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::Corrupt { .. })
    ));

    let bad_codec = service(&prepared.hub, Arc::new(RejectingCodec));
    assert!(matches!(
        bad_codec.export(&prepared.request_id),
        Err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::Corrupt { .. })
    ));
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

#[test]
fn missing_and_unavailable_stores_preserve_error_categories_without_writes() {
    let prepared = prepared_release();
    assert!(matches!(
        prepared
            .service
            .export("scheduled-node-provider-request-missing"),
        Err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::NotFound { .. })
    ));
    let unavailable = service_with_requests(
        &prepared.hub,
        Arc::new(UnavailableRequestStore),
        Arc::new(SpyCodec::new(BODY.to_vec())),
    );
    assert!(matches!(
        unavailable.export(&prepared.request_id),
        Err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::Unavailable { .. })
    ));
    assert_eq!(prepared.hub.mutation_calls(), 0);
}

fn prepared_release() -> PreparedRelease {
    let hub = Arc::new(SpyHub::new());
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
            idempotency_key: "release-test-request".into(),
            prepared_at_ms: 91,
        })
        .expect("prepare request");
    hub.reset_mutation_calls();
    let service = service(&hub, codec);
    PreparedRelease {
        hub,
        request_id: result.inspection.record.provider_request_id,
        service,
    }
}

fn service(
    hub: &Arc<SpyHub>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
) -> GroupAgentScheduledNodeDispatchReleaseControlService {
    GroupAgentScheduledNodeDispatchReleaseControlService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec,
    )
}

fn service_with_run(
    hub: &Arc<SpyHub>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
) -> GroupAgentScheduledNodeDispatchReleaseControlService {
    GroupAgentScheduledNodeDispatchReleaseControlService::new(
        hub.clone(),
        runs,
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec,
    )
}

fn service_with_requests(
    hub: &Arc<SpyHub>,
    requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
) -> GroupAgentScheduledNodeDispatchReleaseControlService {
    GroupAgentScheduledNodeDispatchReleaseControlService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        requests,
        codec,
    )
}

fn authorization(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> GroupAgentScheduledNodeDispatchAuthorization {
    let mut object = Map::new();
    merge(&mut object, authorization_source(control));
    merge(&mut object, authorization_execution(control));
    merge(&mut object, authorization_state());
    let mut value: GroupAgentScheduledNodeDispatchAuthorization =
        serde_json::from_value(Value::Object(object)).expect("authorization fields");
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_scheduled_node_dispatch_authorization_id(&value.authorization_sha256);
    value
}

fn authorization_source(control: &GroupAgentScheduledNodeDispatchReleaseControl) -> Value {
    let source = &control.control_snapshot;
    let request = &control.provider_request;
    json!({
        "v": GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_VERSION,
        "scheduler_protocol_version": source.scheduler_protocol_version,
        "dispatch_authorization_protocol_version": GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
        "graph_run_id": source.graph_run_id, "graph_id": source.graph_id,
        "group_run_id": source.manifest.source.group_run_id, "group_id": source.manifest.source.group_id,
        "source_snapshot_sha256": source.source_snapshot_sha256,
        "graph_manifest_sha256": source.graph_manifest_sha256, "core_plan_sha256": source.core_plan_sha256,
        "control_snapshot_sha256": source.snapshot_sha256,
        "release_control_snapshot_sha256": control.snapshot_sha256,
        "schedule_id": control.schedule.schedule_id, "schedule_sha256": control.schedule.schedule_sha256,
        "scheduled_contract_id": control.scheduled_contract.contract_id,
        "scheduled_contract_sha256": control.scheduled_contract.contract_sha256,
        "scheduled_provider_request_id": request.provider_request_id,
        "scheduled_provider_request_sha256": request.prepared_request_sha256,
        "logical_request_id": request.logical_request_id, "logical_request_sha256": request.logical_request_sha256,
        "request_body_sha256": request.provider_request_sha256, "request_body_bytes": request.provider_request_bytes,
        "expected_last_event_seq": source.last_event_seq, "expected_last_event_sha256": source.last_event_sha256,
        "authorization_id": "", "authorization_sha256": ""
    })
}

fn authorization_execution(control: &GroupAgentScheduledNodeDispatchReleaseControl) -> Value {
    let contract = &control.scheduled_contract;
    let request = &control.provider_request;
    json!({
        "execution_ordinal": contract.node.execution_ordinal, "node_id": contract.node.node_id,
        "attempt": contract.node.attempt, "project_id": contract.node.project_id,
        "project_lane_sha256": contract.node.project_lane_sha256,
        "same_project_policy": contract.node.same_project_policy,
        "provider_kind": contract.provider.kind, "endpoint": contract.provider.endpoint,
        "model": contract.provider.model, "destination_sha256": request.destination_sha256,
        "pricing_snapshot_sha256": request.pricing_snapshot_sha256,
        "budgets": contract.budgets, "failure": contract.failure,
        "release_requirements": release_requirements()
    })
}

fn authorization_state() -> Value {
    json!({
        "lifecycle_contract_admission_authorized": true,
        "execution_authority_release_authorized": true,
        "dispatch_authority_release_authorized": true,
        "scheduled_contract_candidate_present": true, "provider_request_prepared": true,
        "lifecycle_contract_admitted": false, "execution_authority_released": false,
        "dispatch_authority_released": false, "project_lane_claimed": false,
        "provider_request_sent": false, "progress_observed": false,
        "terminal_receipt_recorded": false, "successor_advance_authorized": false
    })
}

fn merge(target: &mut Map<String, Value>, source: Value) {
    let Value::Object(source) = source else {
        panic!("authorization object");
    };
    target.extend(source);
}

fn release_requirements() -> GroupAgentScheduledNodeDispatchReleaseRequirements {
    GroupAgentScheduledNodeDispatchReleaseRequirements {
        consent: GroupAgentScheduledNodeDispatchConsentRequirement::FreshOffMachine,
        consent_contract_version: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
        credential_preflight: GroupAgentScheduledNodeDispatchCredentialPreflight::HeaderSafeEnvironment,
        destination_preflight: GroupAgentScheduledNodeDispatchDestinationPreflight::ExactRegisteredDestination,
        pricing_preflight: GroupAgentScheduledNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost,
        project_lane_claim: GroupAgentScheduledNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal,
        provider_health_check: GroupAgentScheduledNodeDispatchProviderHealthCheck::Forbidden,
        atomic_transition: GroupAgentScheduledNodeDispatchAtomicTransitionRequirement::ExactPristineHeadAdmissionReleaseAndLaneClaim,
        successor: GroupAgentScheduledNodeSuccessorRequirement::VerifiedIntermediateTerminalReceiptBeforeSuccessor,
    }
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

struct RejectingCodec;

impl GroupAgentNodeDispatchRequestCodec for RejectingCodec {
    fn encode_request(
        &self,
        _model: &str,
        _request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        Err(codec_error())
    }

    fn validate_exact_request(
        &self,
        _model: &str,
        _expected: &ModelRequest,
        _actual: &[u8],
    ) -> Result<(), ProviderError> {
        Err(codec_error())
    }
}

struct UnavailableRequestStore;

impl GroupAgentScheduledNodeProviderRequestStore for UnavailableRequestStore {
    fn prepare_group_agent_scheduled_node_provider_request(
        &self,
        _request: &PrepareGroupAgentScheduledNodeProviderRequest,
    ) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError> {
        Err(unavailable("unexpected provider-request mutation"))
    }

    fn inspect_group_agent_scheduled_node_provider_request(
        &self,
        _provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
        Err(unavailable("scheduled provider-request store offline"))
    }

    fn list_group_agent_scheduled_node_provider_requests(
        &self,
        _graph_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeProviderRequestRecord>, HubStoreError> {
        Err(unavailable("scheduled provider-request store offline"))
    }
}

fn codec_error() -> ProviderError {
    ProviderError::new("release_test_codec", "exact bytes diverged", false)
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
