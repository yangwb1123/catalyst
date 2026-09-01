use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::wire_enum::open_wire_enum;

open_wire_enum!(EntityType {
    Action => "action",
    Actor => "actor",
    Artifact => "artifact",
    Attempt => "attempt",
    Change => "change",
    Objective => "objective",
    Project => "project",
    ProjectSnapshot => "project_snapshot",
    Receipt => "receipt",
    Session => "session",
    Space => "space",
    Turn => "turn",
    WorkGraph => "work_graph",
    WorkItem => "work_item",
});

open_wire_enum!(ActorType {
    Agent => "agent",
    Harness => "harness",
    Human => "human",
    Service => "service",
    System => "system",
});

open_wire_enum!(SourceComponent {
    AppServer => "app_server",
    ControlPlane => "control_plane",
    Harness => "harness",
    LegacyImporter => "legacy_importer",
    Runtime => "runtime",
});

open_wire_enum!(RetentionClass {
    Audit => "audit",
    Durable => "durable",
    Ephemeral => "ephemeral",
    LegalHold => "legal_hold",
    Project => "project",
});

open_wire_enum!(Sensitivity {
    Confidential => "confidential",
    Internal => "internal",
    Public => "public",
    Secret => "secret",
});

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub entity_id: String,
    pub entity_type: EntityType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActorRef {
    pub actor_id: String,
    pub actor_type: ActorType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordRef {
    pub record_id: String,
    pub record_sha256: String,
    pub record_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeRef {
    pub action_id: Option<String>,
    pub attempt_id: Option<String>,
    pub change_id: Option<String>,
    pub objective_id: Option<String>,
    pub project_id: Option<String>,
    pub project_snapshot_id: Option<String>,
    pub session_id: Option<String>,
    pub space_id: String,
    pub turn_id: Option<String>,
    pub work_graph_id: Option<String>,
    pub work_item_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub artifact_kind: String,
    pub canonicalization: String,
    pub content_digest: String,
    pub content_id: String,
    pub created_at_unix_ms: i64,
    pub logical_id: String,
    pub media_type: String,
    pub producer_attempt_id: String,
    pub provenance_ref: RecordRef,
    pub retention_class: RetentionClass,
    pub sensitivity: Sensitivity,
    pub size_bytes: i64,
    pub source_snapshot_ref: EntityRef,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub actor_ref: ActorRef,
    pub authorization_ref: Option<RecordRef>,
    pub canonicalization: String,
    pub causation_id: Option<String>,
    pub command_id: String,
    pub correlation_id: String,
    pub deadline_unix_ms: Option<i64>,
    pub envelope_version: i64,
    pub expected_version: Option<i64>,
    pub extensions: Map<String, Value>,
    pub idempotency_key: String,
    pub issued_at_unix_ms: i64,
    pub message_id: String,
    pub payload: Option<Map<String, Value>>,
    pub payload_artifact_ref: Option<ArtifactRef>,
    pub schema_name: String,
    pub schema_version: i64,
    pub scope_ref: ScopeRef,
    pub target_ref: EntityRef,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub actor_ref: ActorRef,
    pub aggregate_ref: EntityRef,
    pub aggregate_version: i64,
    pub canonicalization: String,
    pub causation_id: Option<String>,
    pub correlation_id: String,
    pub envelope_version: i64,
    pub event_id: String,
    pub extensions: Map<String, Value>,
    pub message_id: String,
    pub occurred_at_unix_ms: i64,
    pub payload: Option<Map<String, Value>>,
    pub payload_artifact_ref: Option<ArtifactRef>,
    pub schema_name: String,
    pub schema_version: i64,
    pub scope_ref: ScopeRef,
    pub sequence: i64,
    pub source_component: SourceComponent,
    pub source_snapshot_ref: Option<EntityRef>,
}
