use serde::{Deserialize, Serialize};

pub use crate::capability_grant_contract::{Principal, PrincipalType};
pub use crate::governance_contract::{SnapshotType, SourceSnapshot};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkIntentKind {
    WorkIntent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkIntentFreshness {
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkIntentStatus {
    DeclaredUnassessed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkType {
    ArchitectureEvolution,
    Defect,
    Feature,
    IncidentResponse,
    Migration,
    Question,
    Refactor,
    SmallChange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    Incident,
    OperatorRequest,
    Other,
    ReflectionProposal,
    RuntimeSignal,
    TechnicalDebt,
    UserRequest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MaterialityLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
    #[serde(rename = "materiality_not_bound")]
    NotBound,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct WorkIntentAttestations {
    pub approval_attestation: bool,
    pub authentication_attestation: bool,
    pub authority_attestation: bool,
    pub completion_attestation: bool,
    pub effect_attestation: bool,
    pub execution_attestation: bool,
    pub freshness_attestation: bool,
    pub materiality_attestation: bool,
    pub ownership_attestation: bool,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub reference_resolution_attestation: bool,
    pub scope_attestation: bool,
    pub truth_attestation: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkIntentBinding {
    pub change_id: String,
    pub project_id: String,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDeclaration {
    pub deadline_unix_ms: Option<i64>,
    pub external_constraints: Vec<String>,
    pub goal: String,
    pub non_goals: Vec<String>,
    pub open_questions: Vec<String>,
    pub scope: Vec<String>,
    pub success_signals: Vec<String>,
    pub work_type: WorkType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialityDeclaration {
    pub basis: String,
    pub level: MaterialityLevel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OriginDeclaration {
    pub origin_kind: OriginKind,
    pub origin_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordReference {
    pub canonical_sha256: String,
    pub record_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalArtifactDeclaration {
    pub artifact_kind: String,
    pub artifact_ref: String,
    pub artifact_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkIntentReferences {
    pub claim_record_refs: Vec<RecordReference>,
    pub evidence_record_refs: Vec<RecordReference>,
    pub local_artifact_declarations: Vec<LocalArtifactDeclaration>,
    pub local_source_snapshot_declaration: Option<SourceSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkIntent {
    pub api_version: String,
    pub attestations: WorkIntentAttestations,
    pub binding: WorkIntentBinding,
    pub canonicalization: String,
    pub declared_at_unix_ms: i64,
    pub declared_owner: Option<Principal>,
    pub freshness: WorkIntentFreshness,
    pub intent: IntentDeclaration,
    pub kind: WorkIntentKind,
    pub materiality: MaterialityDeclaration,
    pub origin: OriginDeclaration,
    pub references: WorkIntentReferences,
    pub requester: Principal,
    pub status: WorkIntentStatus,
    pub work_intent_id: String,
    pub work_intent_sha256: String,
}
