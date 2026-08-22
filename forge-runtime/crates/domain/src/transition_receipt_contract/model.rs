use serde::{Deserialize, Serialize};

use crate::capability_grant_contract::{ApprovalRef, GrantTaskBinding, Principal};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionStateVocabulary {
    pub api_version: String,
    pub canonicalization: String,
    pub edges: Vec<TransitionEdge>,
    pub kind: String,
    pub rework_targets: Vec<TransitionState>,
    pub states: Vec<TransitionState>,
    pub terminal_states: Vec<TransitionState>,
    pub vocabulary_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionEdge {
    pub allowed_to_states: Vec<TransitionState>,
    pub from_state: TransitionState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransitionState {
    Draft,
    NeedsEvidence,
    Baselined,
    DesignDrafted,
    Assessed,
    Designed,
    Planned,
    Authorized,
    Implementing,
    Verifying,
    Reviewing,
    ChangesRequested,
    ReleaseReady,
    Releasing,
    Observing,
    Reflecting,
    Learning,
    Closed,
    NeedsInfo,
    Blocked,
    Quarantined,
    Rejected,
    Superseded,
}

impl TransitionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::NeedsEvidence => "NEEDS_EVIDENCE",
            Self::Baselined => "BASELINED",
            Self::DesignDrafted => "DESIGN_DRAFTED",
            Self::Assessed => "ASSESSED",
            Self::Designed => "DESIGNED",
            Self::Planned => "PLANNED",
            Self::Authorized => "AUTHORIZED",
            Self::Implementing => "IMPLEMENTING",
            Self::Verifying => "VERIFYING",
            Self::Reviewing => "REVIEWING",
            Self::ChangesRequested => "CHANGES_REQUESTED",
            Self::ReleaseReady => "RELEASE_READY",
            Self::Releasing => "RELEASING",
            Self::Observing => "OBSERVING",
            Self::Reflecting => "REFLECTING",
            Self::Learning => "LEARNING",
            Self::Closed => "CLOSED",
            Self::NeedsInfo => "NEEDS_INFO",
            Self::Blocked => "BLOCKED",
            Self::Quarantined => "QUARANTINED",
            Self::Rejected => "REJECTED",
            Self::Superseded => "SUPERSEDED",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionReceipt {
    pub actor: Principal,
    pub api_version: String,
    pub applicability: ApplicabilityDeclaration,
    pub approval_refs: Vec<ApprovalRef>,
    pub bindings: TransitionBindings,
    pub canonicalization: String,
    pub capability_grant_ref: CapabilityGrantRef,
    pub declared_controller: Principal,
    pub kind: String,
    pub preconditions: Vec<TransitionPrecondition>,
    pub previous_receipt_id: Option<String>,
    pub previous_receipt_sha256: Option<String>,
    pub reason_codes: Vec<String>,
    pub receipt_id: String,
    pub receipt_sha256: String,
    pub sequence: i64,
    pub task_binding: GrantTaskBinding,
    pub transition: TransitionDeclaration,
    pub transition_vocabulary_sha256: String,
    pub waiver_refs: Vec<WaiverRef>,
    pub work_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionDeclaredTarget {
    pub actor: Principal,
    pub applicability: ApplicabilityDeclaration,
    pub approval_refs: Vec<ApprovalRef>,
    pub bindings: TransitionBindings,
    pub capability_grant_ref: CapabilityGrantRef,
    pub declared_controller: Principal,
    pub preconditions: Vec<TransitionPrecondition>,
    pub previous_receipt_id: Option<String>,
    pub previous_receipt_sha256: Option<String>,
    pub reason_codes: Vec<String>,
    pub sequence: i64,
    pub task_binding: GrantTaskBinding,
    pub transition: TransitionDeclaration,
    pub transition_vocabulary_sha256: String,
    pub waiver_refs: Vec<WaiverRef>,
    pub work_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionAssessmentRequest {
    pub api_version: String,
    pub canonicalization: String,
    pub evaluated_at_unix_ms: i64,
    pub expected_target: TransitionDeclaredTarget,
    pub expected_target_sha256: String,
    pub previous_receipt: Option<TransitionReceipt>,
    pub request_sha256: String,
    pub transition_receipt: TransitionReceipt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionBindings {
    pub artifacts: Vec<TransitionArtifact>,
    pub context_sha256: String,
    pub impact_sha256: Option<String>,
    pub plan_sha256: Option<String>,
    pub policy_sha256: String,
    pub risk_sha256: Option<String>,
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionArtifact {
    pub artifact_kind: String,
    pub artifact_ref: String,
    pub artifact_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrantRef {
    pub authority_domain: String,
    pub grant_id: String,
    pub grant_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WaiverRef {
    pub authority_domain: String,
    pub waiver_id: String,
    pub waiver_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    pub canonical_sha256: String,
    pub record_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionPrecondition {
    pub evidence_refs: Vec<EvidenceRef>,
    pub precondition_id: String,
    pub reason_codes: Vec<String>,
    pub result: PreconditionResult,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreconditionResult {
    Pass,
    Fail,
    Na,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicabilityDeclaration {
    pub decision: ApplicabilityDecision,
    pub evidence_refs: Vec<EvidenceRef>,
    pub reason_codes: Vec<String>,
    pub stage_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityDecision {
    Applicable,
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionDeclaration {
    pub declared_at_unix_ms: i64,
    pub from_state: TransitionState,
    pub gate_id: Option<String>,
    pub resume_state: Option<TransitionState>,
    pub rework_target: Option<TransitionState>,
    pub to_state: TransitionState,
}
