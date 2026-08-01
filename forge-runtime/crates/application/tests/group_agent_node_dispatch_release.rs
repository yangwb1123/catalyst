#![allow(dead_code)]

mod group_agent_node_execution_support;

use std::sync::Arc;

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractInput, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchConsentRequirement, GroupAgentNodeDispatchCredentialPreflight,
    GroupAgentNodeDispatchDestinationPreflight, GroupAgentNodeDispatchPricingPreflight,
    GroupAgentNodeDispatchProjectLaneClaim, GroupAgentNodeDispatchProviderHealthCheck,
    GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchReleaseControlServiceError,
    GroupAgentNodeDispatchReleaseRequirements, GroupAgentNodeDispatchRequestCodec,
    GroupAgentNodeDispatchRequestService, GroupAgentNodeExecutionContractService,
    PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN, GroupAgentNodeDispatchReleaseControl,
    Message, ModelRequest, ProviderError, group_agent_node_destination_sha256,
    group_agent_node_dispatch_authorization_id,
};
use serde::Serialize;

use group_agent_node_execution_support::{FixtureBundle, MemoryContractHub, fixture};

#[derive(Serialize)]
struct ProviderBody<'a> {
    include: [&'static str; 1],
    input: [ProviderInput<'a>; 1],
    instructions: &'a str,
    max_output_tokens: u32,
    model: &'a str,
    store: bool,
    stream: bool,
    tools: [&'static str; 0],
}

#[derive(Serialize)]
struct ProviderInput<'a> {
    content: &'a str,
    role: &'static str,
    r#type: &'static str,
}

struct ExactJsonCodec;

impl GroupAgentNodeDispatchRequestCodec for ExactJsonCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        let Message::User { text } = &request.messages[0] else {
            return Err(provider_error("expected one user message"));
        };
        serde_json::to_vec(&ProviderBody {
            include: ["reasoning.encrypted_content"],
            input: [ProviderInput {
                content: text,
                role: "user",
                r#type: "message",
            }],
            instructions: &request.system_prompt,
            max_output_tokens: request.max_output_tokens,
            model,
            store: false,
            stream: true,
            tools: [],
        })
        .map_err(|_| provider_error("cannot encode request"))
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        (self.encode_request(model, expected)? == actual)
            .then_some(())
            .ok_or_else(|| provider_error("request bytes disagree"))
    }
}

#[test]
fn export_reconstructs_exact_utf8_control_without_a_trailing_lf() {
    let fixture = fixture();
    let service = prepared_release_service(&fixture);
    let export = service
        .export(&fixture.run.run.graph_run_id)
        .expect("export");

    export
        .release_control
        .validate()
        .expect("valid release control");
    assert_eq!(
        export.release_control.snapshot_sha256,
        export
            .release_control
            .expected_sha256()
            .expect("snapshot digest")
    );
    assert_eq!(
        export.canonical_json,
        export.release_control.canonical_json().unwrap()
    );
    assert!(!export.canonical_json.ends_with('\n'));
    assert!(
        export
            .canonical_json
            .contains(r#""provider_request_json":"{\"include\""#)
    );
    assert!(export.canonical_json.contains("café"));
    assert_eq!(
        GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
        b"forge.group-agent-node-dispatch-release-control.v1\0"
    );
}

#[test]
fn verify_accepts_only_the_exact_current_authorization_and_returns_metadata() {
    let fixture = fixture();
    let service = prepared_release_service(&fixture);
    let export = service
        .export(&fixture.run.run.graph_run_id)
        .expect("export");
    let authorization = authorization(&export.release_control);
    let json = authorization.canonical_json().expect("authorization JSON");

    let verified = service
        .verify(&fixture.run.run.graph_run_id, &json)
        .expect("verify");
    assert_eq!(verified.authorization_id, authorization.authorization_id);
    assert_eq!(
        verified.authorization_sha256,
        authorization.authorization_sha256
    );
    assert_eq!(verified.graph_run_id, fixture.run.run.graph_run_id);
    assert_eq!(
        verified.request_body_bytes,
        export.release_control.provider_request_json.len()
    );
    assert!(!json.ends_with('\n'));
    assert!(
        authorization
            .canonical_payload_json()
            .unwrap()
            .contains(requirements_json())
    );
}

#[test]
fn every_security_binding_tamper_fails_closed_even_when_resigned() {
    let fixture = fixture();
    let service = prepared_release_service(&fixture);
    let export = service
        .export(&fixture.run.run.graph_run_id)
        .expect("export");
    let source = authorization(&export.release_control);

    for (name, mutation) in authorization_mutations() {
        let mut changed = source.clone();
        mutation(&mut changed);
        resign(&mut changed);
        let json = changed.canonical_json().expect("changed JSON");
        assert!(
            matches!(
                service.verify(&fixture.run.run.graph_run_id, &json),
                Err(GroupAgentNodeDispatchReleaseControlServiceError::InvalidInput { .. })
            ),
            "tampered field must fail closed: {name}"
        );
    }
}

#[test]
fn every_release_control_component_tamper_fails_closed_even_when_resigned() {
    let fixture = fixture();
    let service = prepared_release_service(&fixture);
    let original = service
        .export(&fixture.run.run.graph_run_id)
        .expect("export")
        .release_control;
    for (name, mutation) in release_control_mutations() {
        let mut changed = original.clone();
        mutation(&mut changed);
        resign_control(&mut changed);
        assert!(changed.validate().is_err(), "tampered component: {name}");
    }
    let mut digest = original;
    digest.snapshot_sha256 = "a".repeat(64);
    assert!(digest.validate().is_err(), "tampered snapshot digest");
}

#[test]
fn verify_rejects_noncanonical_unknown_duplicate_and_escaped_input() {
    let fixture = fixture();
    let service = prepared_release_service(&fixture);
    let control = service.export(&fixture.run.run.graph_run_id).unwrap();
    let canonical = authorization(&control.release_control)
        .canonical_json()
        .expect("authorization JSON");
    let inputs = [
        format!("{canonical}\n"),
        canonical.replacen("{\"v\":1", "{\"unknown\":0,\"v\":1", 1),
        canonical.replacen("{\"v\":1", "{\"v\":1,\"v\":1", 1),
        canonical.replacen("graph-run-fixture-v1", "graph-run-fixture-\\u00761", 1),
    ];
    for json in inputs {
        assert!(matches!(
            service.verify(&fixture.run.run.graph_run_id, &json),
            Err(GroupAgentNodeDispatchReleaseControlServiceError::InvalidInput { .. })
        ));
    }
}

fn prepared_release_service(
    fixture: &FixtureBundle,
) -> GroupAgentNodeDispatchReleaseControlService {
    let hub = Arc::new(MemoryContractHub::new(fixture));
    let codec = Arc::new(ExactJsonCodec);
    let contracts =
        GroupAgentNodeExecutionContractService::new(hub.clone(), hub.clone(), hub.clone());
    contracts
        .admit(&AdmitGroupAgentNodeExecutionContractInput {
            graph_run_id: fixture.run.run.graph_run_id.clone(),
            contract_json: fixture.contract_json.clone(),
            idempotency_key: "contract-key".into(),
            admitted_at_ms: 80,
        })
        .expect("admit contract");
    let dispatch = GroupAgentNodeDispatchRequestService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec.clone(),
    );
    dispatch
        .prepare(&PrepareGroupAgentNodeDispatchRequestInput {
            graph_run_id: fixture.run.run.graph_run_id.clone(),
            idempotency_key: "dispatch-key".into(),
            prepared_at_ms: 90,
        })
        .expect("prepare request");
    GroupAgentNodeDispatchReleaseControlService::new(hub.clone(), hub, codec)
}

fn authorization(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> GroupAgentNodeDispatchAuthorization {
    let contract = &control.contract;
    let request = &control.dispatch_request;
    let value = GroupAgentNodeDispatchAuthorization {
        v: GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        dispatch_authorization_protocol_version:
            GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        graph_id: control.graph_run.graph_id.clone(),
        group_run_id: control.manifest.source.group_run_id.clone(),
        source_snapshot_sha256: control.graph_run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: control.graph_run.graph_manifest_sha256.clone(),
        core_plan_sha256: control.graph_run.plan_sha256.clone(),
        release_control_snapshot_sha256: control.snapshot_sha256.clone(),
        expected_last_event_seq: 3,
        expected_last_event_sha256: control.journal_events[2].expected_sha256().unwrap(),
        contract_id: contract.contract_id.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        dispatch_request_id: request.dispatch_request_id.clone(),
        dispatch_request_sha256: request.dispatch_request_sha256.clone(),
        logical_request_sha256: request.request_sha256.clone(),
        request_body_sha256: request.provider_request_sha256.clone(),
        request_body_bytes: request.provider_request_bytes,
        node_id: contract.node.node_id.clone(),
        attempt: contract.node.attempt,
        project_id: contract.node.project_id.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        same_project_policy: contract.node.same_project_policy,
        provider_kind: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        destination_sha256: request.destination_sha256.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        budgets: contract.budgets.clone(),
        release_requirements: requirements(),
        failure: contract.failure.clone(),
        execution_contract_present: true,
        dispatch_request_present: true,
        dispatch_authority_release_authorized: true,
        dispatch_authority_released: false,
        authorization_id: String::new(),
        authorization_sha256: String::new(),
    };
    finish_authorization(control, value)
}

fn finish_authorization(
    control: &GroupAgentNodeDispatchReleaseControl,
    mut value: GroupAgentNodeDispatchAuthorization,
) -> GroupAgentNodeDispatchAuthorization {
    resign(&mut value);
    value
        .validate_against_release_control(control)
        .expect("valid authorization");
    value
}

fn requirements() -> GroupAgentNodeDispatchReleaseRequirements {
    GroupAgentNodeDispatchReleaseRequirements {
        consent: GroupAgentNodeDispatchConsentRequirement::FreshOffMachine,
        consent_contract_version: GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
        credential_preflight: GroupAgentNodeDispatchCredentialPreflight::HeaderSafeEnvironment,
        destination_preflight:
            GroupAgentNodeDispatchDestinationPreflight::ExactRegisteredDestination,
        pricing_preflight: GroupAgentNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost,
        project_lane_claim: GroupAgentNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal,
        provider_health_check: GroupAgentNodeDispatchProviderHealthCheck::Forbidden,
    }
}

fn requirements_json() -> &'static str {
    r#""release_requirements":{"consent":"fresh_off_machine","consent_contract_version":1,"credential_preflight":"header_safe_environment","destination_preflight":"exact_registered_destination","pricing_preflight":"exact_snapshot_within_max_cost","project_lane_claim":"global_exclusive_until_terminal","provider_health_check":"forbidden"}"#
}

type AuthorizationMutation = (&'static str, fn(&mut GroupAgentNodeDispatchAuthorization));
type ReleaseControlMutation = (&'static str, fn(&mut GroupAgentNodeDispatchReleaseControl));

fn release_control_mutations() -> Vec<ReleaseControlMutation> {
    vec![
        ("v", |value| value.v += 1),
        ("scheduler", |value| value.scheduler_protocol_version += 1),
        ("release protocol", |value| {
            value.release_control_protocol_version += 1;
        }),
        ("Graph Run", |value| {
            value.graph_run.source_snapshot_sha256 = "a".repeat(64);
        }),
        ("plan", |value| value.plan.graph_id.push('x')),
        ("manifest", |value| {
            value.manifest.manager.instruction.push('x');
        }),
        ("journal", |value| value.journal_events[2].seq = 2),
        ("contract record", |value| {
            value.contract_record.created_at_ms += 1;
        }),
        ("contract", |value| {
            value.contract.failure.automatic_retry = true;
        }),
        ("dispatch request", |value| {
            value.dispatch_request.created_at_ms += 1;
        }),
        ("provider body", |value| {
            value.provider_request_json.push(' ');
        }),
    ]
}

fn authorization_mutations() -> Vec<AuthorizationMutation> {
    vec![
        ("source", |value| {
            value.source_snapshot_sha256 = "a".repeat(64);
        }),
        ("head", |value| {
            value.expected_last_event_sha256 = "a".repeat(64);
        }),
        ("request", |value| {
            value.dispatch_request_sha256 = "a".repeat(64);
            value.dispatch_request_id = format!("node-dispatch-request-{}", "a".repeat(64));
        }),
        ("body digest", |value| {
            value.request_body_sha256 = "a".repeat(64);
        }),
        ("body bytes", |value| value.request_body_bytes += 1),
        ("lane", |value| value.project_lane_sha256 = "a".repeat(64)),
        ("destination", |value| {
            value.endpoint = "https://example.com/v1/responses".into();
            value.destination_sha256 = group_agent_node_destination_sha256(
                value.provider_kind,
                &value.endpoint,
                &value.model,
            );
        }),
        ("pricing", |value| {
            value.pricing_snapshot_sha256 = "a".repeat(64);
            value.budgets.pricing_snapshot_sha256 = "a".repeat(64);
        }),
        ("budget", |value| value.budgets.max_cost_usd_micros += 1),
        ("failure", |value| value.failure.automatic_retry = true),
    ]
}

fn resign(value: &mut GroupAgentNodeDispatchAuthorization) {
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256);
}

fn resign_control(value: &mut GroupAgentNodeDispatchReleaseControl) {
    value.snapshot_sha256 = value.expected_sha256().expect("release-control digest");
}

fn provider_error(message: &str) -> ProviderError {
    ProviderError::new("test_codec", message, false)
}
