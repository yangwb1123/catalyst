use forge_runtime_domain::{
    execution::attempt::{
        APPROVAL_RECORD_TYPE, AttemptBudget, AttemptRequestInput, CAPABILITY_GRANT_RECORD_TYPE,
        ControlVersionBinding, WORKSPACE_CAPABILITY_RECORD_TYPE,
    },
    platform_core_contract::{
        ActorRef, ActorType, ArtifactRef, CANONICALIZATION, EntityRef, EntityType,
        ExecutorDescriptor, RecordRef, RetentionClass, ScopeRef, Sensitivity,
    },
};

pub fn input() -> AttemptRequestInput {
    AttemptRequestInput {
        scope_ref: scope(),
        attempt_ref: entity(EntityType::Attempt, "atm", '8'),
        work_item_ref: entity(EntityType::WorkItem, "wki", '7'),
        project_ref: entity(EntityType::Project, "prj", '2'),
        project_snapshot_ref: entity(EntityType::ProjectSnapshot, "psn", '3'),
        control_versions: versions(),
        executor: executor(),
        context_artifact_ref: Some(context_artifact()),
        workspace_capability_ref: Some(record(
            "workspace-capability-1",
            'a',
            WORKSPACE_CAPABILITY_RECORD_TYPE,
        )),
        grant_ref: Some(record(
            "capability-grant-1",
            'b',
            CAPABILITY_GRANT_RECORD_TYPE,
        )),
        approval_refs: vec![approval(2), approval(1)],
        requested_effects: vec!["repo.write".into(), "process.exec".into()],
        budget: budget(),
        timeout_ms: 30_000,
        idempotency_key: "attempt-request-key-0001".into(),
    }
}

pub fn scope() -> ScopeRef {
    ScopeRef {
        action_id: None,
        attempt_id: Some(platform_id("atm", '8')),
        change_id: Some(platform_id("chg", '5')),
        objective_id: Some(platform_id("obj", '4')),
        project_id: Some(platform_id("prj", '2')),
        project_snapshot_id: Some(platform_id("psn", '3')),
        session_id: None,
        space_id: platform_id("spc", '1'),
        turn_id: None,
        work_graph_id: Some(platform_id("wgr", '6')),
        work_item_id: Some(platform_id("wki", '7')),
    }
}

pub const fn versions() -> ControlVersionBinding {
    ControlVersionBinding {
        objective_version: 2,
        change_version: 3,
        work_graph_version: 4,
        work_item_version: 5,
    }
}

pub const fn budget() -> AttemptBudget {
    AttemptBudget {
        max_duration_ms: 60_000,
        max_cost_usd_micros: 1_000_000,
        max_model_calls: 4,
        max_tool_calls: 16,
        max_input_tokens: 100_000,
        max_output_tokens: 20_000,
        max_output_bytes: 1_000_000,
        max_network_bytes: 2_000_000,
    }
}

pub fn executor() -> ExecutorDescriptor {
    ExecutorDescriptor {
        actor_ref: ActorRef {
            actor_id: platform_id("acr", '9'),
            actor_type: ActorType::Service,
        },
        adapter_id: "forge.runtime.codex_adapter".into(),
        adapter_version: "1.2.3".into(),
    }
}

pub fn context_artifact() -> ArtifactRef {
    let digest = hash('c');
    ArtifactRef {
        artifact_kind: "attempt_context".into(),
        canonicalization: CANONICALIZATION.into(),
        content_digest: digest.clone(),
        content_id: format!("sha256:{digest}"),
        created_at_unix_ms: 1_700_000_000_000,
        logical_id: platform_id("art", 'a'),
        media_type: "application/json".into(),
        producer_attempt_id: platform_id("atm", 'b'),
        provenance_ref: record(
            "artifact-provenance-1",
            'd',
            "forge.runtime.artifact_provenance",
        ),
        retention_class: RetentionClass::Project,
        sensitivity: Sensitivity::Internal,
        size_bytes: 512,
        source_snapshot_ref: entity(EntityType::ProjectSnapshot, "psn", '3'),
    }
}

pub fn approval(index: usize) -> RecordRef {
    let digit = char::from_digit(u32::try_from(index).expect("small index"), 16)
        .expect("hex approval index");
    record(
        &format!("approval-record-{index}"),
        digit,
        APPROVAL_RECORD_TYPE,
    )
}

pub fn record(record_id: &str, hash_byte: char, record_type: &str) -> RecordRef {
    RecordRef {
        record_id: record_id.into(),
        record_sha256: hash(hash_byte),
        record_type: record_type.into(),
    }
}

pub fn entity(entity_type: EntityType, prefix: &str, suffix: char) -> EntityRef {
    EntityRef {
        entity_id: platform_id(prefix, suffix),
        entity_type,
    }
}

pub fn platform_id(prefix: &str, suffix: char) -> String {
    format!("{prefix}_{}{}", "0".repeat(25), suffix)
}

pub fn hash(value: char) -> String {
    value.to_string().repeat(64)
}
