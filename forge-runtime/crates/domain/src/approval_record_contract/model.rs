use serde::{Deserialize, Serialize};

use crate::capability_grant_contract::{AuthorityClass, EffectId, EnvironmentClass, Principal};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRecord {
    pub api_version: String,
    pub approval_id: String,
    pub approval_sha256: String,
    pub approver: Principal,
    pub authority_proof: ApprovalAuthorityProof,
    pub bindings: ApprovalBindings,
    pub canonicalization: String,
    pub conditions: Vec<ApprovalCondition>,
    pub decision: ApprovalDecision,
    pub decision_basis: ApprovalDecisionBasis,
    pub effect_vocabulary_sha256: String,
    pub kind: String,
    pub risk_acceptance_refs: Vec<RiskAcceptanceRef>,
    pub scope: ApprovalScope,
    pub separation_of_duty: ApprovalSeparationOfDuty,
    pub subject: Principal,
    pub validity: ApprovalValidity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalAuthoritySource {
    pub authority_class: AuthorityClass,
    pub authority_domain: String,
    pub principal_id: String,
    pub principal_type: crate::capability_grant_contract::PrincipalType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalAuthorityBinding {
    pub authority_source: ApprovalAuthoritySource,
    pub key_id: String,
    pub proof_kind: ApprovalProofKind,
    pub proof_profile_id: String,
    pub proof_profile_sha256: String,
    pub trust_domain: String,
    pub trust_epoch: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalAuthorityProof {
    pub authority_source: ApprovalAuthoritySource,
    pub key_id: String,
    pub proof_base64url: String,
    pub proof_kind: ApprovalProofKind,
    pub proof_profile_id: String,
    pub proof_profile_sha256: String,
    pub trust_domain: String,
    pub trust_epoch: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalProofKind {
    Attestation,
    Signature,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalArtifact {
    pub artifact_kind: String,
    pub artifact_ref: String,
    pub artifact_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalBindings {
    pub artifacts: Vec<ApprovalArtifact>,
    pub context_sha256: String,
    pub impact_sha256: String,
    pub plan_sha256: String,
    pub policy_sha256: String,
    pub risk_sha256: String,
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalCondition {
    pub condition_id: String,
    pub condition_ref: String,
    pub condition_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiskAcceptanceRef {
    pub authority_domain: String,
    pub risk_acceptance_id: String,
    pub risk_acceptance_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Abstain,
    Approve,
    Reject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDecisionBasis {
    pub rationale_ref: String,
    pub rationale_sha256: String,
    pub reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalScope {
    pub change_id: String,
    pub effect_id: Option<EffectId>,
    pub environment_class: EnvironmentClass,
    pub environment_id: String,
    pub gate_id: Option<String>,
    pub materiality_level: MaterialityLevel,
    pub project_id: String,
    pub scope_type: ApprovalScopeType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MaterialityLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScopeType {
    Effect,
    Gate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalSeparationOfDutyDeclaration {
    pub implementers: Vec<Principal>,
    pub proof_profile_id: String,
    pub proof_profile_sha256: String,
    pub requester: Principal,
    pub required_distinctions: Vec<ApprovalRequiredDistinction>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalSeparationOfDuty {
    pub implementers: Vec<Principal>,
    pub proof_base64url: String,
    pub proof_profile_id: String,
    pub proof_profile_sha256: String,
    pub requester: Principal,
    pub required_distinctions: Vec<ApprovalRequiredDistinction>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequiredDistinction {
    ApproverNotImplementer,
    ApproverNotRequester,
    ApproverNotSubject,
}

impl ApprovalRequiredDistinction {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ApproverNotImplementer => "approver_not_implementer",
            Self::ApproverNotRequester => "approver_not_requester",
            Self::ApproverNotSubject => "approver_not_subject",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalValidity {
    pub expires_at_unix_ms: i64,
    pub issued_at_unix_ms: i64,
    pub not_before_unix_ms: i64,
    pub revoked_at_unix_ms: Option<i64>,
    pub transferable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDeclaredTarget {
    pub approver: Principal,
    pub authority_binding: ApprovalAuthorityBinding,
    pub bindings: ApprovalBindings,
    pub conditions: Vec<ApprovalCondition>,
    pub decision: ApprovalDecision,
    pub risk_acceptance_refs: Vec<RiskAcceptanceRef>,
    pub scope: ApprovalScope,
    pub separation_of_duty_declaration: ApprovalSeparationOfDutyDeclaration,
    pub subject: Principal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalAssessmentRequest {
    pub api_version: String,
    pub approval_record: ApprovalRecord,
    pub canonicalization: String,
    pub evaluated_at_unix_ms: i64,
    pub expected_target: ApprovalDeclaredTarget,
    pub expected_target_sha256: String,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct ApprovalDeclaredAssessment {
    pub api_version: String,
    pub approval_id: String,
    pub approval_sha256: String,
    pub approver_identity_state: NotEvaluated,
    pub assessment_mode: String,
    pub assessment_sha256: String,
    pub authority_proof_state: NotEvaluated,
    pub authorization_decision: NoDecision,
    pub canonicalization: String,
    pub condition_satisfaction_state: NotEvaluated,
    pub effect_attestation: bool,
    pub effective_approval_state: NotEvaluated,
    pub expected_target_sha256: String,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub policy_decision: NoDecision,
    pub reason_codes: Vec<ApprovalReasonCode>,
    pub relations: ApprovalDeclaredRelations,
    pub request_sha256: String,
    pub result: String,
    pub revocation_registry_state: NotEvaluated,
    pub risk_acceptance_state: NotEvaluated,
    pub separation_of_duty_proof_state: NotEvaluated,
    pub transition_attestation: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotEvaluated {
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoDecision {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDeclaredRelations {
    pub approver: ApproverRelation,
    pub authority_binding: AuthorityBindingRelation,
    pub binding: ApprovalBindingRelation,
    pub conditions: ConditionsRelation,
    pub decision: DecisionRelation,
    pub revocation: RevocationRelation,
    pub risk_acceptance: RiskAcceptanceRelation,
    pub scope: ApprovalScopeRelation,
    pub separation_of_duty: SeparationOfDutyRelation,
    pub subject: ApprovalSubjectRelation,
    pub temporal: ApprovalTemporalRelation,
}

macro_rules! relation_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

relation_enum!(ApproverRelation {
    ApproverMismatch,
    SameDeclaredApprover
});
relation_enum!(AuthorityBindingRelation {
    AuthorityBindingMismatch,
    SameDeclaredAuthorityBinding
});
relation_enum!(ApprovalBindingRelation {
    BindingMismatch,
    SameDeclaredBinding
});
relation_enum!(ConditionsRelation {
    ConditionsMismatch,
    SameDeclaredConditions
});
relation_enum!(DecisionRelation {
    DecisionMismatch,
    SameDeclaredDecision
});
relation_enum!(RevocationRelation {
    DeclaredRevocationTimeNotReached,
    DeclaredRevocationTimeReached
});
relation_enum!(RiskAcceptanceRelation {
    RiskAcceptanceMismatch,
    SameDeclaredRiskAcceptanceRefs
});
relation_enum!(ApprovalScopeRelation {
    SameDeclaredScope,
    ScopeMismatch
});
relation_enum!(SeparationOfDutyRelation {
    SameDeclaredSeparationOfDuty,
    SeparationOfDutyMismatch
});
relation_enum!(ApprovalSubjectRelation {
    SameDeclaredSubject,
    SubjectMismatch
});
relation_enum!(ApprovalTemporalRelation {
    InsideDeclaredWindow,
    OutsideDeclaredWindow
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalReasonCode {
    ApproverMismatch,
    AuthorityBindingMismatch,
    BindingMismatch,
    ConditionsMismatch,
    DeclaredRevocationTimeReached,
    DecisionMismatch,
    RiskAcceptanceMismatch,
    ScopeMismatch,
    SeparationOfDutyMismatch,
    SubjectMismatch,
    TemporalWindowMismatch,
}

impl ApprovalReasonCode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ApproverMismatch => "approver_mismatch",
            Self::AuthorityBindingMismatch => "authority_binding_mismatch",
            Self::BindingMismatch => "binding_mismatch",
            Self::ConditionsMismatch => "conditions_mismatch",
            Self::DeclaredRevocationTimeReached => "declared_revocation_time_reached",
            Self::DecisionMismatch => "decision_mismatch",
            Self::RiskAcceptanceMismatch => "risk_acceptance_mismatch",
            Self::ScopeMismatch => "scope_mismatch",
            Self::SeparationOfDutyMismatch => "separation_of_duty_mismatch",
            Self::SubjectMismatch => "subject_mismatch",
            Self::TemporalWindowMismatch => "temporal_window_mismatch",
        }
    }
}
