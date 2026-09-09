mod bindings;
mod capacity;
mod connections;
mod writers;

use std::path::{Path, PathBuf};

use forge_runtime_domain::{
    execution::attempt::{
        APPROVAL_RECORD_TYPE, AttemptBudget, AttemptRequest, AttemptRequestInput,
        CAPABILITY_GRANT_RECORD_TYPE, ControlVersionBinding, WORKSPACE_CAPABILITY_RECORD_TYPE,
    },
    platform_core_contract::{
        ActorRef, ActorType, ArtifactRef, CANONICALIZATION, EntityRef, EntityType, EventEnvelope,
        ExecutorDescriptor, RecordRef, RetentionClass, ScopeRef, Sensitivity, SourceComponent,
    },
};
use forge_runtime_infrastructure::sqlite_execution::{SqliteAttemptJournal, request_sha256};
use rusqlite::Connection;
use serde_json::{Map, json};
use tempfile::TempDir;

pub fn database() -> (TempDir, PathBuf, SqliteAttemptJournal) {
    let directory = tempfile::tempdir().expect("test directory");
    let path = directory.path().join("attempts.sqlite");
    let journal = reopen(&path);
    (directory, path, journal)
}

pub fn reopen(path: &Path) -> SqliteAttemptJournal {
    SqliteAttemptJournal::from_connection(Connection::open(path).expect("test connection"))
        .expect("validated journal")
}

pub fn platform_id(prefix: &str, index: usize) -> String {
    format!("{prefix}_{index:026}")
}

pub fn request(index: usize) -> AttemptRequest {
    AttemptRequest::try_from_input(&input(index)).expect("valid request fixture")
}

pub fn input(index: usize) -> AttemptRequestInput {
    AttemptRequestInput {
        scope_ref: scope(index),
        attempt_ref: entity(EntityType::Attempt, "atm", index),
        work_item_ref: entity(EntityType::WorkItem, "wki", 7),
        project_ref: entity(EntityType::Project, "prj", 2),
        project_snapshot_ref: entity(EntityType::ProjectSnapshot, "psn", 3),
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
        approval_refs: vec![
            record("approval-record-2", '2', APPROVAL_RECORD_TYPE),
            record("approval-record-1", '1', APPROVAL_RECORD_TYPE),
        ],
        requested_effects: vec!["repo.write".into(), "process.exec".into()],
        budget: budget(),
        timeout_ms: 30_000,
        idempotency_key: format!("attempt-request-key-{index:04}"),
    }
}

fn scope(index: usize) -> ScopeRef {
    ScopeRef {
        action_id: None,
        attempt_id: Some(platform_id("atm", index)),
        change_id: Some(platform_id("chg", 5)),
        objective_id: Some(platform_id("obj", 4)),
        project_id: Some(platform_id("prj", 2)),
        project_snapshot_id: Some(platform_id("psn", 3)),
        session_id: None,
        space_id: platform_id("spc", 1),
        turn_id: None,
        work_graph_id: Some(platform_id("wgr", 6)),
        work_item_id: Some(platform_id("wki", 7)),
    }
}

const fn versions() -> ControlVersionBinding {
    ControlVersionBinding {
        objective_version: 2,
        change_version: 3,
        work_graph_version: 4,
        work_item_version: 5,
    }
}

const fn budget() -> AttemptBudget {
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

fn executor() -> ExecutorDescriptor {
    ExecutorDescriptor {
        actor_ref: ActorRef {
            actor_id: platform_id("acr", 9),
            actor_type: ActorType::Service,
        },
        adapter_id: "forge.runtime.codex_adapter".into(),
        adapter_version: "1.2.3".into(),
    }
}

fn context_artifact() -> ArtifactRef {
    let digest = "c".repeat(64);
    ArtifactRef {
        artifact_kind: "attempt_context".into(),
        canonicalization: CANONICALIZATION.into(),
        content_digest: digest.clone(),
        content_id: format!("sha256:{digest}"),
        created_at_unix_ms: 1_700_000_000_000,
        logical_id: platform_id("art", 10),
        media_type: "application/json".into(),
        producer_attempt_id: platform_id("atm", 999_999),
        provenance_ref: record(
            "artifact-provenance-1",
            'd',
            "forge.runtime.artifact_provenance",
        ),
        retention_class: RetentionClass::Project,
        sensitivity: Sensitivity::Internal,
        size_bytes: 512,
        source_snapshot_ref: entity(EntityType::ProjectSnapshot, "psn", 3),
    }
}

fn record(record_id: &str, digest_byte: char, record_type: &str) -> RecordRef {
    RecordRef {
        record_id: record_id.into(),
        record_sha256: digest_byte.to_string().repeat(64),
        record_type: record_type.into(),
    }
}

fn entity(entity_type: EntityType, prefix: &str, index: usize) -> EntityRef {
    EntityRef {
        entity_id: platform_id(prefix, index),
        entity_type,
    }
}

pub fn event(request: &AttemptRequest, index: usize) -> EventEnvelope {
    let payload = json!({
        "request_sha256": request_sha256(request).expect("request digest"),
        "state": "requested",
    });
    EventEnvelope {
        actor_ref: executor().actor_ref,
        aggregate_ref: request.attempt_ref().clone(),
        aggregate_version: 1,
        canonicalization: CANONICALIZATION.into(),
        causation_id: None,
        correlation_id: platform_id("cor", index),
        envelope_version: 1,
        event_id: platform_id("evt", index),
        extensions: Map::new(),
        message_id: platform_id("msg", index),
        occurred_at_unix_ms: 1_700_000_000_001,
        payload: Some(payload.as_object().expect("object").clone()),
        payload_artifact_ref: None,
        schema_name: "forge.runtime.attempt_requested".into(),
        schema_version: 1,
        scope_ref: request.scope_ref().clone(),
        sequence: 1,
        source_component: SourceComponent::Runtime,
        source_snapshot_ref: Some(request.project_snapshot_ref().clone()),
    }
}

pub fn counts(path: &Path) -> (i64, i64, i64) {
    Connection::open(path)
        .expect("inspect test database")
        .query_row(
            "SELECT (SELECT COUNT(*) FROM attempt_admissions), \
             (SELECT COUNT(*) FROM attempt_events), (SELECT COUNT(*) FROM attempt_outbox)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("row counts")
}
