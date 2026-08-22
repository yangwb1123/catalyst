use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct TransitionDeclaredAssessment {
    pub api_version: String,
    pub approval_state: NotEvaluated,
    pub assessment_mode: String,
    pub assessment_sha256: String,
    pub authorization_decision: NoDecision,
    pub canonicalization: String,
    pub completion_attestation: bool,
    pub controller_authentication_state: NotEvaluated,
    pub effect_attestation: bool,
    pub evidence_state: NotEvaluated,
    pub execution_attestation: bool,
    pub expected_target_sha256: String,
    pub grant_state: NotEvaluated,
    pub ledger_state: NotEvaluated,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub policy_decision: NoDecision,
    pub precondition_truth_state: NotEvaluated,
    pub reason_codes: Vec<TransitionReasonCode>,
    pub receipt_id: String,
    pub receipt_sha256: String,
    pub relations: TransitionDeclaredRelations,
    pub request_sha256: String,
    pub result: String,
    pub transition_attestation: bool,
    pub transition_vocabulary_sha256: String,
    pub waiver_state: NotEvaluated,
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
pub struct TransitionDeclaredRelations {
    pub applicability: ApplicabilityRelation,
    pub chain: ChainRelation,
    pub continuity: ContinuityRelation,
    pub edge: EdgeRelation,
    pub preconditions: PreconditionsRelation,
    pub recovery: RecoveryRelation,
    pub target: TargetRelation,
    pub temporal: TransitionTemporalRelation,
}

macro_rules! relation_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

relation_enum!(ApplicabilityRelation {
    InternallyConsistentDeclaredApplicability
});
relation_enum!(ChainRelation {
    InitialDeclaredChain,
    PredecessorMismatch,
    SameDeclaredPredecessor
});
relation_enum!(ContinuityRelation {
    SameDeclaredStateContinuity,
    StateContinuityMismatch
});
relation_enum!(EdgeRelation {
    ListedDeclaredEdge,
    UnlistedDeclaredEdge
});
relation_enum!(PreconditionsRelation {
    DeclaredFailOrUnknownPresent,
    DeclaredPassOrNaOnly
});
relation_enum!(RecoveryRelation {
    InternallyConsistentDeclaredRecovery,
    ReworkOrResumeMismatch
});
relation_enum!(TargetRelation {
    SameDeclaredTarget,
    TargetMismatch
});
relation_enum!(TransitionTemporalRelation {
    NondecreasingDeclaredTime,
    TemporalDeclarationMismatch
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionReasonCode {
    DeclaredFailOrUnknownPresent,
    PredecessorMismatch,
    ReworkOrResumeMismatch,
    StateContinuityMismatch,
    TargetMismatch,
    TemporalDeclarationMismatch,
    UnlistedDeclaredEdge,
}

impl TransitionReasonCode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredFailOrUnknownPresent => "declared_fail_or_unknown_present",
            Self::PredecessorMismatch => "predecessor_mismatch",
            Self::ReworkOrResumeMismatch => "rework_or_resume_mismatch",
            Self::StateContinuityMismatch => "state_continuity_mismatch",
            Self::TargetMismatch => "target_mismatch",
            Self::TemporalDeclarationMismatch => "temporal_declaration_mismatch",
            Self::UnlistedDeclaredEdge => "unlisted_declared_edge",
        }
    }
}
