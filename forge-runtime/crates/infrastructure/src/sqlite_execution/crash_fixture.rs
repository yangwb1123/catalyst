use crate::runtime_domain::{
    execution::attempt::{
        AttemptBudget, AttemptRequest, AttemptRequestInput, ControlVersionBinding,
    },
    platform_core_contract::{
        ActorRef, ActorType, CANONICALIZATION, EntityRef, EntityType, EventEnvelope,
        ExecutorDescriptor, ScopeRef, SourceComponent,
    },
};
use serde_json::{Map, json};

pub(super) fn request() -> AttemptRequest {
    AttemptRequest::try_from_input(&AttemptRequestInput {
        scope_ref: scope(),
        attempt_ref: entity(EntityType::Attempt, "atm"),
        work_item_ref: entity(EntityType::WorkItem, "wki"),
        project_ref: entity(EntityType::Project, "prj"),
        project_snapshot_ref: entity(EntityType::ProjectSnapshot, "psn"),
        control_versions: ControlVersionBinding {
            objective_version: 1,
            change_version: 1,
            work_graph_version: 1,
            work_item_version: 1,
        },
        executor: ExecutorDescriptor {
            actor_ref: actor(),
            adapter_id: "forge.runtime.test_adapter".into(),
            adapter_version: "1.0.0".into(),
        },
        context_artifact_ref: None,
        workspace_capability_ref: None,
        grant_ref: None,
        approval_refs: vec![],
        requested_effects: vec![],
        budget: AttemptBudget {
            max_duration_ms: 1000,
            max_cost_usd_micros: 0,
            max_model_calls: 0,
            max_tool_calls: 0,
            max_input_tokens: 0,
            max_output_tokens: 0,
            max_output_bytes: 0,
            max_network_bytes: 0,
        },
        timeout_ms: 1000,
        idempotency_key: "crash-admission-request-0001".into(),
    })
    .expect("valid effect-free crash fixture")
}

fn scope() -> ScopeRef {
    ScopeRef {
        space_id: id("spc"),
        project_id: Some(id("prj")),
        project_snapshot_id: Some(id("psn")),
        objective_id: Some(id("obj")),
        change_id: Some(id("chg")),
        work_graph_id: Some(id("wgr")),
        work_item_id: Some(id("wki")),
        attempt_id: Some(id("atm")),
        session_id: None,
        turn_id: None,
        action_id: None,
    }
}

pub(super) fn event(request: &AttemptRequest) -> EventEnvelope {
    let payload = json!({"request_sha256": super::request_sha256(request).unwrap(),
        "state": "requested"});
    EventEnvelope {
        actor_ref: actor(),
        aggregate_ref: request.attempt_ref().clone(),
        aggregate_version: 1,
        canonicalization: CANONICALIZATION.into(),
        causation_id: None,
        correlation_id: id("cor"),
        envelope_version: 1,
        event_id: id("evt"),
        extensions: Map::new(),
        message_id: id("msg"),
        occurred_at_unix_ms: 1_700_000_000_001,
        payload: Some(payload.as_object().unwrap().clone()),
        payload_artifact_ref: None,
        schema_name: "forge.runtime.attempt_requested".into(),
        schema_version: 1,
        scope_ref: request.scope_ref().clone(),
        sequence: 1,
        source_component: SourceComponent::Runtime,
        source_snapshot_ref: Some(request.project_snapshot_ref().clone()),
    }
}

fn actor() -> ActorRef {
    ActorRef {
        actor_id: id("acr"),
        actor_type: ActorType::Service,
    }
}

fn entity(entity_type: EntityType, prefix: &str) -> EntityRef {
    EntityRef {
        entity_type,
        entity_id: id(prefix),
    }
}

fn id(prefix: &str) -> String {
    format!("{prefix}_00000000000000000000000001")
}
