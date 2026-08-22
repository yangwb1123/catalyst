use serde::{Deserialize, Serialize};

use super::{
    CapabilityGrant, CapabilityIdentity, GrantBindings, GrantTaskBinding, Principal,
    RequestedAction,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredAssessmentRequest {
    pub api_version: String,
    pub canonicalization: String,
    pub evaluated_at_unix_ms: i64,
    pub expected: ExpectedGrantBinding,
    pub grant: CapabilityGrant,
    pub request_sha256: String,
    pub requested_action: RequestedAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedGrantBinding {
    pub bindings: GrantBindings,
    pub capability: CapabilityIdentity,
    pub subject: Principal,
    pub task_binding: GrantTaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredAssessment {
    pub api_version: String,
    pub approval_state: NotEvaluated,
    pub assessment_mode: String,
    pub assessment_sha256: String,
    pub authority_proof_state: NotEvaluated,
    pub authorization_decision: NoAuthorizationDecision,
    pub canonicalization: String,
    pub effect_attestation: bool,
    pub grant_id: String,
    pub grant_sha256: String,
    pub permission_attestation: bool,
    pub reason_codes: Vec<ReasonCode>,
    pub relations: DeclaredRelations,
    pub request_sha256: String,
    pub requested_action_sha256: String,
    pub result: String,
    pub revocation_state: NotEvaluated,
    pub usage_state: NotEvaluated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotEvaluated {
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoAuthorizationDecision {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredRelations {
    pub binding: BindingRelation,
    pub budget: BudgetRelation,
    pub capability: CapabilityRelation,
    pub effect: EffectRelation,
    pub scope: ScopeRelation,
    pub subject: SubjectRelation,
    pub task: TaskRelation,
    pub temporal: TemporalRelation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingRelation {
    SameDeclaredBinding,
    BindingMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetRelation {
    AtOrBelowDeclaredCeiling,
    ExceedsDeclaredCeiling,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRelation {
    SameDeclaredCapability,
    CapabilityMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectRelation {
    SameDeclaredEffect,
    EffectMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeRelation {
    CoveredByDeclaration,
    DeniedByDeclaration,
    OutsideDeclaredScope,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectRelation {
    SameDeclaredSubject,
    SubjectMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRelation {
    SameDeclaredTask,
    TaskMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalRelation {
    InsideDeclaredWindow,
    OutsideDeclaredWindow,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    BindingMismatch,
    BudgetExceeded,
    CapabilityMismatch,
    DenyMatched,
    EffectMismatch,
    ScopeNotCovered,
    SubjectMismatch,
    TaskMismatch,
    TemporalWindowMismatch,
}

impl ReasonCode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::BindingMismatch => "binding_mismatch",
            Self::BudgetExceeded => "budget_exceeded",
            Self::CapabilityMismatch => "capability_mismatch",
            Self::DenyMatched => "deny_matched",
            Self::EffectMismatch => "effect_mismatch",
            Self::ScopeNotCovered => "scope_not_covered",
            Self::SubjectMismatch => "subject_mismatch",
            Self::TaskMismatch => "task_mismatch",
            Self::TemporalWindowMismatch => "temporal_window_mismatch",
        }
    }
}
