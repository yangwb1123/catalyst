use serde::{Deserialize, Serialize};

use super::EffectId;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectVocabulary {
    pub api_version: String,
    pub canonicalization: String,
    pub effects: Vec<EffectDefinition>,
    pub kind: String,
    pub vocabulary_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDefinition {
    pub allowed_scope_kinds: Vec<ScopeKind>,
    pub effect_id: EffectId,
    pub production_restriction: ProductionRestriction,
    pub required_scope_kinds: Vec<ScopeKind>,
    pub scope_profile: ScopeProfile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Artifact,
    Command,
    Environment,
    GovernanceObject,
    NetworkOrigin,
    RepoPath,
    SecretRef,
    Target,
    TargetQuery,
}

impl ScopeKind {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Command => "command",
            Self::Environment => "environment",
            Self::GovernanceObject => "governance_object",
            Self::NetworkOrigin => "network_origin",
            Self::RepoPath => "repo_path",
            Self::SecretRef => "secret_ref",
            Self::Target => "target",
            Self::TargetQuery => "target_query",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionRestriction {
    ExternalOperatorOnly,
    PolicyControlledDefaultDeny,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeProfile {
    ApprovalObject,
    ArtifactEnvironment,
    Command,
    EnvironmentRepoEmit,
    KnowledgeObject,
    NetworkOrigin,
    PolicyObject,
    RepoEmitOptionalEnvironment,
    RepoRead,
    RepoWriteExact,
    SecretRef,
    Target,
    TargetQuery,
}

impl ScopeResourceKind for super::ScopeResource {
    fn scope_kind(&self) -> ScopeKind {
        match self {
            Self::Artifact { .. } => ScopeKind::Artifact,
            Self::Command { .. } => ScopeKind::Command,
            Self::Environment { .. } => ScopeKind::Environment,
            Self::GovernanceObject { .. } => ScopeKind::GovernanceObject,
            Self::NetworkOrigin { .. } => ScopeKind::NetworkOrigin,
            Self::RepoPath { .. } => ScopeKind::RepoPath,
            Self::SecretRef { .. } => ScopeKind::SecretRef,
            Self::Target { .. } => ScopeKind::Target,
            Self::TargetQuery { .. } => ScopeKind::TargetQuery,
        }
    }
}

pub(super) trait ScopeResourceKind {
    fn scope_kind(&self) -> ScopeKind;
}
