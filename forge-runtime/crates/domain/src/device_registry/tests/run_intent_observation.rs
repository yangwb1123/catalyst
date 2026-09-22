use super::*;
use serde::Deserialize;

const RUN_INTENT_FIXTURE: &str =
    include_str!("../../../../../../docs/contracts/fixtures/forge-run-intent-observation-v1.json");
const SESSION_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-session-placement-observation-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunFixture {
    api_version: String,
    placement_contract_fixture: String,
    owner: OwnerFixture,
    conversation_id: String,
    prompt_receipt: PromptFixture,
    run_reference: RunReferenceFixture,
    expected: ExpectedFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptFixture {
    prompt_id: String,
    conversation_id: String,
    role: String,
    accepted_at_ms: u64,
    intent_id: String,
    initial_event_id: String,
    initial_event_sequence: u64,
    initial_event_type: String,
    replayed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunReferenceFixture {
    run_id: String,
    conversation_id: String,
    prompt_id: String,
    created_at_ms: u64,
    latest_sequence: u64,
    status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct ExpectedFixture {
    evaluation_mode: String,
    prompt_accepted: bool,
    run_reference_observed: bool,
    prompt_run_binding_valid: bool,
    placement_observation_bound: bool,
    preview_only: bool,
    intent_replayed: bool,
    run_status: String,
    run_latest_sequence: u64,
    prompt_accepted_at_ms: u64,
    placement_evaluated_at_ms: u64,
    placement_decision_count: usize,
    eligible_instance_count: usize,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: AuthorityFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct AuthorityFixture {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionFixture {
    api_version: String,
    owner: OwnerFixture,
    conversation_id: String,
    run_id: String,
    placement: serde_json::Value,
    expected: SessionExpectedFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionExpectedFixture {
    evaluation_mode: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    decisions: Vec<SessionDecisionFixture>,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: AuthorityFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDecisionFixture {
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[test]
#[allow(clippy::too_many_lines)]
fn shared_run_intent_fixture_binds_prompt_receipt_to_placement_preview() {
    let fixture: RunFixture = serde_json::from_str(RUN_INTENT_FIXTURE).unwrap();
    assert_eq!(
        fixture.api_version,
        "forgeos.run-intent-observation-contract/v1"
    );
    assert_eq!(
        fixture.placement_contract_fixture,
        "forge-session-placement-observation-v1"
    );
    let session: SessionFixture = serde_json::from_str(SESSION_FIXTURE).unwrap();
    assert_eq!(
        session.api_version,
        "forgeos.session-placement-observation-contract/v1"
    );
    assert!(!session.placement.is_null());
    assert_eq!(fixture.owner.tenant_id, session.owner.tenant_id);
    assert_eq!(fixture.conversation_id, session.conversation_id);
    let placement = session_observation(&session);
    let owner = SessionPlacementOwner {
        issuer: fixture.owner.issuer.clone(),
        subject: fixture.owner.subject.clone(),
        tenant_id: TenantId::parse(fixture.owner.tenant_id.clone()).unwrap(),
    };
    let observation = observe_run_intent(RunIntentObservationRequest {
        owner,
        conversation_id: fixture.conversation_id,
        prompt: RunIntentPromptReceipt {
            prompt_id: fixture.prompt_receipt.prompt_id,
            conversation_id: fixture.prompt_receipt.conversation_id,
            role: fixture.prompt_receipt.role,
            accepted_at_ms: fixture.prompt_receipt.accepted_at_ms,
            intent_id: fixture.prompt_receipt.intent_id,
            initial_event_id: fixture.prompt_receipt.initial_event_id,
            initial_event_sequence: fixture.prompt_receipt.initial_event_sequence,
            initial_event_type: fixture.prompt_receipt.initial_event_type,
            replayed: fixture.prompt_receipt.replayed,
        },
        run: RunIntentRunReference {
            run_id: fixture.run_reference.run_id,
            conversation_id: fixture.run_reference.conversation_id,
            prompt_id: fixture.run_reference.prompt_id,
            created_at_ms: fixture.run_reference.created_at_ms,
            latest_sequence: fixture.run_reference.latest_sequence,
            status: fixture.run_reference.status,
        },
        placement,
    })
    .unwrap();
    let expected = fixture.expected;
    assert_eq!(
        observation.schema_version,
        RUN_INTENT_OBSERVATION_SCHEMA_VERSION
    );
    assert_eq!(observation.evaluation_mode, expected.evaluation_mode);
    assert_eq!(observation.prompt_accepted, expected.prompt_accepted);
    assert_eq!(
        observation.run_reference_observed,
        expected.run_reference_observed
    );
    assert_eq!(
        observation.prompt_run_binding_valid,
        expected.prompt_run_binding_valid
    );
    assert_eq!(
        observation.placement_observation_bound,
        expected.placement_observation_bound
    );
    assert_eq!(observation.preview_only, expected.preview_only);
    assert_eq!(observation.intent_replayed, expected.intent_replayed);
    assert_eq!(observation.run_status, expected.run_status);
    assert_eq!(
        observation.run_latest_sequence,
        expected.run_latest_sequence
    );
    assert_eq!(
        observation.prompt_accepted_at_ms,
        expected.prompt_accepted_at_ms
    );
    assert_eq!(
        observation.placement_evaluated_at_ms,
        expected.placement_evaluated_at_ms
    );
    assert_eq!(
        observation.placement_decision_count,
        expected.placement_decision_count
    );
    assert_eq!(
        observation.eligible_instance_count,
        expected.eligible_instance_count
    );
    assert_eq!(
        observation.owner_declaration_unverified,
        expected.owner_declaration_unverified
    );
    assert_eq!(
        observation.device_attributes_unverified,
        expected.device_attributes_unverified
    );
    assert_eq!(observation.selected_device_id, expected.selected_device_id);
    assert_eq!(
        observation.selected_instance_id,
        expected.selected_instance_id
    );
    assert_eq!(
        observation.authority,
        SessionPlacementAuthority {
            identity_verified: expected.authority.identity_verified,
            heartbeat_persisted: expected.authority.heartbeat_persisted,
            inventory_authoritative: expected.authority.inventory_authoritative,
            reservation_created: expected.authority.reservation_created,
            execution_authorized: expected.authority.execution_authorized,
            dispatch_performed: expected.authority.dispatch_performed,
        }
    );
    assert_eq!(observation.authority, SessionPlacementAuthority::default());
}

fn session_observation(session: &SessionFixture) -> SessionPlacementObservation {
    let owner = SessionPlacementOwner {
        issuer: session.owner.issuer.clone(),
        subject: session.owner.subject.clone(),
        tenant_id: TenantId::parse(session.owner.tenant_id.clone()).unwrap(),
    };
    SessionPlacementObservation {
        schema_version: SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION,
        evaluation_mode: match session.expected.evaluation_mode.as_str() {
            "offline_static_only" => "offline_static_only",
            value => panic!("unexpected session evaluation mode {value:?}"),
        },
        owner,
        conversation_id: session.conversation_id.clone(),
        run_id: session.run_id.clone(),
        evaluated_at_ms: session.expected.evaluated_at_ms,
        owner_declaration_unverified: session.expected.owner_declaration_unverified,
        device_attributes_unverified: session.expected.device_attributes_unverified,
        decisions: session
            .expected
            .decisions
            .iter()
            .map(|decision| SessionPlacementDecision {
                device_id: decision.device_id.clone(),
                instance_id: decision.instance_id.clone(),
                matches_requirements: decision.matches_requirements,
                exclusion_reasons: decision.exclusion_reasons.clone(),
            })
            .collect(),
        selected_device_id: session.expected.selected_device_id.clone(),
        selected_instance_id: session.expected.selected_instance_id.clone(),
        authority: SessionPlacementAuthority {
            identity_verified: session.expected.authority.identity_verified,
            heartbeat_persisted: session.expected.authority.heartbeat_persisted,
            inventory_authoritative: session.expected.authority.inventory_authoritative,
            reservation_created: session.expected.authority.reservation_created,
            execution_authorized: session.expected.authority.execution_authorized,
            dispatch_performed: session.expected.authority.dispatch_performed,
        },
    }
}
