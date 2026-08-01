use super::*;
use crate::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentNodeFailurePropagationOwner,
    GroupAgentNodePostClaimUncertainty, group_agent_node_destination_sha256,
    group_agent_project_lane_sha256,
};

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

#[test]
fn digest_domains_bounds_and_authorization_identity_are_frozen() {
    assert_eq!(
        GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
        b"forge.group-agent-node-dispatch-release-control.v1\0"
    );
    assert_eq!(
        GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
        b"forge.group-agent-node-dispatch-authorization.v1\0"
    );
    assert_eq!(
        MAX_GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
        48 * 1024 * 1024
    );
    assert_eq!(
        MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES,
        1024 * 1024
    );

    let value = authorization();
    assert_eq!(
        value.expected_sha256().unwrap(),
        "ad0b431fd7e51e500cea22414805246df498f22d9bba4548aa767bdb333f7233"
    );
    assert_eq!(
        value.authorization_id,
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256)
    );
    value.validate().expect("valid standalone authorization");
}

#[test]
fn canonical_field_and_flat_requirement_order_is_exact() {
    let value = authorization();
    let payload = value.canonical_payload_json().expect("payload JSON");
    let requirements = r#""release_requirements":{"consent":"fresh_off_machine","consent_contract_version":1,"credential_preflight":"header_safe_environment","destination_preflight":"exact_registered_destination","pricing_preflight":"exact_snapshot_within_max_cost","project_lane_claim":"global_exclusive_until_terminal","provider_health_check":"forbidden"}"#;

    assert!(payload.starts_with(
        r#"{"v":1,"scheduler_protocol_version":1,"dispatch_authorization_protocol_version":1,"graph_run_id":"graph-run-a","graph_id":"graph-a","group_run_id":"group-run-a""#
    ));
    assert!(payload.contains(requirements));
    assert!(payload.ends_with(
        r#""execution_contract_present":true,"dispatch_request_present":true,"dispatch_authority_release_authorized":true,"dispatch_authority_released":false}"#
    ));
    assert!(!payload.ends_with('\n'));
    let complete = value.canonical_json().expect("complete JSON");
    assert!(complete.ends_with(&format!(
        r#""authorization_id":"{}","authorization_sha256":"{}"}}"#,
        value.authorization_id, value.authorization_sha256
    )));
}

#[test]
fn every_authorization_payload_field_is_content_addressed() {
    let original = authorization();
    let digest = original.expected_sha256().expect("original digest");
    for (name, mutation) in mutations() {
        let mut changed = original.clone();
        mutation(&mut changed);
        assert_ne!(
            changed.expected_sha256().expect("changed digest"),
            digest,
            "field is not content addressed: {name}"
        );
        assert!(
            changed.validate().is_err(),
            "stale identity accepted: {name}"
        );
    }
}

#[test]
fn authorization_decoder_rejects_unknown_fields_and_noncanonical_escapes() {
    let value = authorization();
    let canonical = value.canonical_json().expect("authorization JSON");
    let unknown = canonical.replacen("{\"v\":1", "{\"v\":1,\"unknown\":0", 1);
    assert!(serde_json::from_str::<GroupAgentNodeDispatchAuthorization>(&unknown).is_err());
    let escaped = canonical.replacen("graph-run-a", "graph-run-\\u0061", 1);
    let decoded: GroupAgentNodeDispatchAuthorization =
        serde_json::from_str(&escaped).expect("semantically valid escape");
    assert_ne!(decoded.canonical_json().unwrap(), escaped);
}

fn authorization() -> GroupAgentNodeDispatchAuthorization {
    let mut value = unsigned_authorization();
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256);
    value
}

fn unsigned_authorization() -> GroupAgentNodeDispatchAuthorization {
    let endpoint = "https://api.example.com/v1/responses".to_owned();
    let model = "gpt-test".to_owned();
    GroupAgentNodeDispatchAuthorization {
        v: GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        dispatch_authorization_protocol_version:
            GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
        graph_run_id: "graph-run-a".into(),
        graph_id: "graph-a".into(),
        group_run_id: "group-run-a".into(),
        source_snapshot_sha256: A.into(),
        graph_manifest_sha256: B.into(),
        core_plan_sha256: C.into(),
        release_control_snapshot_sha256: D.into(),
        expected_last_event_seq: 3,
        expected_last_event_sha256: E.into(),
        contract_id: format!("node-contract-{A}"),
        contract_sha256: A.into(),
        dispatch_request_id: format!("node-dispatch-request-{B}"),
        dispatch_request_sha256: B.into(),
        logical_request_sha256: C.into(),
        request_body_sha256: D.into(),
        request_body_bytes: 321,
        node_id: "node-a".into(),
        attempt: 1,
        project_id: "project-a".into(),
        project_lane_sha256: group_agent_project_lane_sha256("project-a"),
        same_project_policy: GroupAgentNodeSameProjectPolicy::ExclusiveUntilTerminal,
        provider_kind: GroupAgentNodeProviderKind::OpenAiResponses,
        endpoint: endpoint.clone(),
        model: model.clone(),
        destination_sha256: group_agent_node_destination_sha256(
            GroupAgentNodeProviderKind::OpenAiResponses,
            &endpoint,
            &model,
        ),
        pricing_snapshot_sha256: A.into(),
        budgets: budgets(),
        release_requirements: requirements(),
        failure: failure(),
        execution_contract_present: true,
        dispatch_request_present: true,
        dispatch_authority_release_authorized: true,
        dispatch_authority_released: false,
        authorization_id: String::new(),
        authorization_sha256: String::new(),
    }
}

fn budgets() -> GroupAgentNodeExecutionBudgets {
    GroupAgentNodeExecutionBudgets {
        max_turns: 1,
        max_tool_calls: 0,
        max_output_tokens: 100,
        max_model_output_bytes: 1_000,
        max_model_events: 10,
        timeout_ms: 1_000,
        max_cost_usd_micros: 100,
        pricing_snapshot_sha256: A.into(),
    }
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

fn failure() -> GroupAgentNodeExecutionFailurePolicy {
    GroupAgentNodeExecutionFailurePolicy {
        automatic_retry: false,
        lease_retry: false,
        post_claim_uncertainty: GroupAgentNodePostClaimUncertainty::DispatchUnknown,
        failure_propagation_owner: GroupAgentNodeFailurePropagationOwner::ForgeCore,
    }
}

type Mutation = (&'static str, fn(&mut GroupAgentNodeDispatchAuthorization));

fn mutations() -> Vec<Mutation> {
    vec![
        ("v", |v| v.v += 1),
        ("scheduler", |v| v.scheduler_protocol_version += 1),
        ("authorization protocol", |v| {
            v.dispatch_authorization_protocol_version += 1;
        }),
        ("Graph Run", |v| v.graph_run_id.push('x')),
        ("graph", |v| v.graph_id.push('x')),
        ("Group Run", |v| v.group_run_id.push('x')),
        ("source", |v| v.source_snapshot_sha256 = B.into()),
        ("manifest", |v| v.graph_manifest_sha256 = C.into()),
        ("plan", |v| v.core_plan_sha256 = D.into()),
        ("release control", |v| {
            v.release_control_snapshot_sha256 = E.into();
        }),
        ("event seq", |v| v.expected_last_event_seq += 1),
        ("event head", |v| v.expected_last_event_sha256 = A.into()),
        ("contract ID", |v| v.contract_id.push('x')),
        ("contract digest", |v| v.contract_sha256 = B.into()),
        ("dispatch ID", |v| v.dispatch_request_id.push('x')),
        ("dispatch digest", |v| v.dispatch_request_sha256 = C.into()),
        ("logical request", |v| v.logical_request_sha256 = D.into()),
        ("body digest", |v| v.request_body_sha256 = E.into()),
        ("body bytes", |v| v.request_body_bytes += 1),
        ("node", |v| v.node_id.push('x')),
        ("attempt", |v| v.attempt += 1),
        ("project", |v| v.project_id.push('x')),
        ("lane", |v| v.project_lane_sha256 = A.into()),
        ("endpoint", |v| v.endpoint.push('/')),
        ("model", |v| v.model.push('x')),
        ("destination", |v| v.destination_sha256 = B.into()),
        ("pricing", |v| v.pricing_snapshot_sha256 = C.into()),
        ("budgets", |v| v.budgets.max_cost_usd_micros += 1),
        ("consent contract", |v| {
            v.release_requirements.consent_contract_version += 1;
        }),
        ("failure", |v| v.failure.automatic_retry = true),
        ("contract present", |v| v.execution_contract_present = false),
        ("request present", |v| v.dispatch_request_present = false),
        ("release authorized", |v| {
            v.dispatch_authority_release_authorized = false;
        }),
        ("authority released", |v| {
            v.dispatch_authority_released = true;
        }),
    ]
}
