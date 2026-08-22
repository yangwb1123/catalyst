use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredGrantTransitionCompatibility {
    pub reason_codes: Vec<GrantCompatibilityReason>,
    pub relations: DeclaredGrantTransitionRelations,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredGrantTransitionRelations {
    pub actor: GrantActorRelation,
    pub approval_refs: GrantApprovalRefsRelation,
    pub bindings: GrantBindingsRelation,
    pub declared_time: GrantDeclaredTimeRelation,
    pub grant_ref: GrantRefRelation,
    pub task_binding: GrantTaskBindingRelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredApprovalTransitionCompatibility {
    pub reason_codes: Vec<ApprovalCompatibilityReason>,
    pub relations: DeclaredApprovalTransitionRelations,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredApprovalTransitionRelations {
    pub ref_set: ApprovalRefSetRelation,
    pub scope: ApprovalTransitionScopeRelation,
}

macro_rules! relation_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

relation_enum!(GrantActorRelation {
    ActorMismatch,
    SameDeclaredActor
});
relation_enum!(GrantApprovalRefsRelation {
    ApprovalRefsMismatch,
    SameDeclaredApprovalRefs
});
relation_enum!(GrantBindingsRelation {
    BindingsMismatch,
    SameDeclaredBindings
});
relation_enum!(GrantDeclaredTimeRelation {
    DeclaredTimeMismatch,
    SameDeclaredTime
});
relation_enum!(GrantRefRelation {
    GrantRefMismatch,
    SameDeclaredGrantRef
});
relation_enum!(GrantTaskBindingRelation {
    SameDeclaredTaskBinding,
    TaskBindingMismatch
});
relation_enum!(ApprovalRefSetRelation {
    RefSetMismatch,
    SameDeclaredRefSet
});
relation_enum!(ApprovalTransitionScopeRelation {
    SameDeclaredScope,
    ScopeMismatch
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantCompatibilityReason {
    ActorMismatch,
    ApprovalRefsMismatch,
    BindingsMismatch,
    DeclaredTimeMismatch,
    GrantRefMismatch,
    TaskBindingMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalCompatibilityReason {
    RefSetMismatch,
    ScopeMismatch,
}
