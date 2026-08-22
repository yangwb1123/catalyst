use serde::{Deserialize, Serialize};

use crate::{
    capability_grant_contract::{GrantTaskBinding, Principal},
    governance_contract::GovernanceRecord,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeUpdateProposal {
    pub api_version: String,
    pub bindings: KnowledgeUpdateBindings,
    pub canonicalization: String,
    pub capability_grant_ref: CapabilityGrantRef,
    pub kind: String,
    pub knowledge_scope: KnowledgeScope,
    pub mutations: Vec<KnowledgeMutation>,
    pub proposal_id: String,
    pub proposal_sha256: String,
    pub proposer: Principal,
    pub record_set_sha256: String,
    pub records: Vec<GovernanceRecord>,
    pub submitted_at_unix_ms: i64,
    pub task_binding: GrantTaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeUpdateBindings {
    pub artifacts: Vec<KnowledgeArtifact>,
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
pub struct KnowledgeArtifact {
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
pub struct KnowledgeScope {
    pub object_kind: KnowledgeObjectKind,
    pub object_ref: String,
    pub object_scope_sha256: String,
    pub scope_kind: GovernanceScopeKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeObjectKind {
    Knowledge,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceScopeKind {
    GovernanceObject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeMutation {
    pub after_claim_ref: ClaimRef,
    pub before_claim_ref: Option<ClaimRef>,
    pub operation: MutationOperation,
    pub rationale: String,
    pub reason_codes: Vec<String>,
    pub target_aggregate_id: String,
    pub target_kind: MutationTargetKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRef {
    pub canonical_sha256: String,
    pub record_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationOperation {
    Create,
    Supersede,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MutationTargetKind {
    KnowledgeClaim,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeUpdateDeclaredTarget {
    pub bindings: KnowledgeUpdateBindings,
    pub capability_grant_ref: CapabilityGrantRef,
    pub knowledge_scope: KnowledgeScope,
    pub mutations: Vec<KnowledgeMutation>,
    pub proposer: Principal,
    pub record_set_sha256: String,
    pub task_binding: GrantTaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeUpdateAssessmentRequest {
    pub api_version: String,
    pub canonicalization: String,
    pub evaluated_at_unix_ms: i64,
    pub expected_target: KnowledgeUpdateDeclaredTarget,
    pub expected_target_sha256: String,
    pub knowledge_update_proposal: KnowledgeUpdateProposal,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct KnowledgeUpdateDeclaredAssessment {
    pub api_version: String,
    pub assessment_mode: String,
    pub assessment_sha256: String,
    pub authorization_decision: NoDecision,
    pub canonicalization: String,
    pub conflict_state: NotEvaluated,
    pub context_state: NotEvaluated,
    pub current_knowledge_state: NotEvaluated,
    pub effect_attestation: bool,
    pub evidence_state: NotEvaluated,
    pub execution_attestation: bool,
    pub expected_target_sha256: String,
    pub freshness_state: NotEvaluated,
    pub grant_state: NotEvaluated,
    pub knowledge_adoption_attestation: bool,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub policy_decision: NoDecision,
    pub proposal_id: String,
    pub proposal_sha256: String,
    pub proposer_authentication_state: NotEvaluated,
    pub reason_codes: Vec<KnowledgeUpdateReasonCode>,
    pub relations: KnowledgeUpdateDeclaredRelations,
    pub request_sha256: String,
    pub result: String,
    pub truth_attestation: bool,
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
pub struct KnowledgeUpdateDeclaredRelations {
    pub binding: BindingRelation,
    pub grant_ref: GrantRefRelation,
    pub mutations: MutationsRelation,
    pub proposer: ProposerRelation,
    pub record_set: RecordSetRelation,
    pub scope: ScopeRelation,
    pub task_binding: TaskBindingRelation,
    pub temporal: TemporalRelation,
}

macro_rules! relation_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

relation_enum!(BindingRelation {
    SameDeclaredBinding,
    BindingMismatch
});
relation_enum!(GrantRefRelation {
    SameDeclaredGrantRef,
    GrantRefMismatch
});
relation_enum!(MutationsRelation {
    SameDeclaredMutations,
    MutationsMismatch
});
relation_enum!(ProposerRelation {
    SameDeclaredProposer,
    ProposerMismatch
});
relation_enum!(RecordSetRelation {
    SameDeclaredRecordSet,
    RecordSetMismatch
});
relation_enum!(ScopeRelation {
    SameDeclaredScope,
    ScopeMismatch
});
relation_enum!(TaskBindingRelation {
    SameDeclaredTaskBinding,
    TaskBindingMismatch
});
relation_enum!(TemporalRelation {
    NonfutureDeclaredSubmission,
    FutureDeclaredSubmission
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeUpdateReasonCode {
    BindingMismatch,
    GrantRefMismatch,
    MutationsMismatch,
    ProposerMismatch,
    RecordSetMismatch,
    ScopeMismatch,
    TaskBindingMismatch,
    TemporalDeclarationMismatch,
}

impl KnowledgeUpdateReasonCode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::BindingMismatch => "binding_mismatch",
            Self::GrantRefMismatch => "grant_ref_mismatch",
            Self::MutationsMismatch => "mutations_mismatch",
            Self::ProposerMismatch => "proposer_mismatch",
            Self::RecordSetMismatch => "record_set_mismatch",
            Self::ScopeMismatch => "scope_mismatch",
            Self::TaskBindingMismatch => "task_binding_mismatch",
            Self::TemporalDeclarationMismatch => "temporal_declaration_mismatch",
        }
    }
}
