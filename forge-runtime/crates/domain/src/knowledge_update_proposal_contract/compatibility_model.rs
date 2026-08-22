use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantKnowledgeUpdateCompatibility {
    pub reason_codes: Vec<GrantKnowledgeUpdateReason>,
    pub relations: GrantKnowledgeUpdateRelations,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantKnowledgeUpdateRelations {
    pub bindings: GrantBindingsRelation,
    pub declared_time: GrantDeclaredTimeRelation,
    pub effect: GrantEffectRelation,
    pub grant_ref: GrantReferenceRelation,
    pub proposer: GrantProposerRelation,
    pub scope: GrantScopeRelation,
    pub task_binding: GrantTaskRelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextKnowledgeUpdateCompatibility {
    pub reason_codes: Vec<ContextKnowledgeUpdateReason>,
    pub relations: ContextKnowledgeUpdateRelations,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextKnowledgeUpdateRelations {
    pub context: ContextDigestRelation,
    pub freshness: ContextFreshnessRelation,
    pub policy: ContextPolicyRelation,
    pub source: ContextSourceRelation,
    pub task_binding: ContextTaskRelation,
}

macro_rules! relation_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

relation_enum!(GrantBindingsRelation {
    BindingsMismatch,
    SameDeclaredBindings
});
relation_enum!(GrantDeclaredTimeRelation {
    DeclaredTimeMismatch,
    SameDeclaredTime
});
relation_enum!(GrantEffectRelation {
    EffectMismatch,
    SameDeclaredEffect
});
relation_enum!(GrantReferenceRelation {
    GrantRefMismatch,
    SameDeclaredGrantRef
});
relation_enum!(GrantProposerRelation {
    ProposerMismatch,
    SameDeclaredProposer
});
relation_enum!(GrantScopeRelation {
    CoveredByDeclaration,
    DeniedByDeclaration,
    OutsideDeclaredScope
});
relation_enum!(GrantTaskRelation {
    SameDeclaredTaskBinding,
    TaskBindingMismatch
});
relation_enum!(ContextDigestRelation {
    ContextMismatch,
    SameDeclaredContext
});
relation_enum!(ContextFreshnessRelation {
    InsideDeclaredFreshness,
    OutsideDeclaredFreshness
});
relation_enum!(ContextPolicyRelation {
    PolicyMismatch,
    SameDeclaredPolicy
});
relation_enum!(ContextSourceRelation {
    SameDeclaredSource,
    SourceMismatch
});
relation_enum!(ContextTaskRelation {
    SameDeclaredTaskBinding,
    TaskBindingMismatch
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantKnowledgeUpdateReason {
    BindingsMismatch,
    DeclaredTimeMismatch,
    DenyMatched,
    EffectMismatch,
    GrantRefMismatch,
    ProposerMismatch,
    ScopeNotCovered,
    TaskBindingMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextKnowledgeUpdateReason {
    ContextMismatch,
    FreshnessMismatch,
    PolicyMismatch,
    SourceMismatch,
    TaskBindingMismatch,
}
