use super::{
    CANONICALIZATION, CapabilityGrantContractError, EFFECT_VOCABULARY_API_VERSION,
    EFFECT_VOCABULARY_KIND, EffectDefinition, EffectId, EffectVocabulary, ProductionRestriction,
    ScopeKind, ScopeProfile, codec, invalid,
};

const ARTIFACT_ENVIRONMENT: &[ScopeKind] = &[ScopeKind::Artifact, ScopeKind::Environment];
const COMMAND: &[ScopeKind] = &[ScopeKind::Command];
const ENVIRONMENT_REPO: &[ScopeKind] = &[ScopeKind::Environment, ScopeKind::RepoPath];
const GOVERNANCE_OBJECT: &[ScopeKind] = &[ScopeKind::GovernanceObject];
const NETWORK_ORIGIN: &[ScopeKind] = &[ScopeKind::NetworkOrigin];
const REPO_PATH: &[ScopeKind] = &[ScopeKind::RepoPath];
const SECRET_REF: &[ScopeKind] = &[ScopeKind::SecretRef];
const TARGET: &[ScopeKind] = &[ScopeKind::Target];
const TARGET_QUERY: &[ScopeKind] = &[ScopeKind::TargetQuery];

pub(super) const ALL_EFFECTS: &[EffectId] = &[
    EffectId::ApprovalDecide,
    EffectId::ApprovalRequest,
    EffectId::KnowledgeApply,
    EffectId::KnowledgePropose,
    EffectId::MigrationApply,
    EffectId::MigrationGenerate,
    EffectId::NetworkRead,
    EffectId::NetworkWrite,
    EffectId::PlacementPlan,
    EffectId::PolicyPropose,
    EffectId::PolicyWrite,
    EffectId::ProcessExec,
    EffectId::ReleaseExecute,
    EffectId::ReleasePlan,
    EffectId::RepoRead,
    EffectId::RepoWrite,
    EffectId::SecretsRead,
    EffectId::TargetExecute,
    EffectId::TargetInventory,
    EffectId::TargetProbe,
    EffectId::TargetReserve,
];

pub(super) fn validate(vocabulary: &EffectVocabulary) -> Result<(), CapabilityGrantContractError> {
    if vocabulary.api_version != EFFECT_VOCABULARY_API_VERSION
        || vocabulary.canonicalization != CANONICALIZATION
        || vocabulary.kind != EFFECT_VOCABULARY_KIND
        || vocabulary.effects.len() != ALL_EFFECTS.len()
    {
        return Err(invalid("effect vocabulary envelope does not match v1"));
    }
    for (definition, expected_effect) in vocabulary.effects.iter().zip(ALL_EFFECTS) {
        validate_definition(definition, *expected_effect)?;
    }
    super::primitives::sha256(&vocabulary.vocabulary_sha256, "vocabulary_sha256")?;
    if codec::effect_vocabulary_sha256(vocabulary)? != vocabulary.vocabulary_sha256 {
        return Err(invalid("effect vocabulary self digest does not match"));
    }
    Ok(())
}

fn validate_definition(
    definition: &EffectDefinition,
    effect: EffectId,
) -> Result<(), CapabilityGrantContractError> {
    let spec = specification(effect);
    if definition.effect_id != effect
        || definition.allowed_scope_kinds != spec.allowed
        || definition.required_scope_kinds != spec.required
        || definition.production_restriction != spec.restriction
        || definition.scope_profile != spec.profile
    {
        return Err(invalid("effect vocabulary definition does not match v1"));
    }
    Ok(())
}

pub(super) struct EffectSpecification {
    pub(super) allowed: &'static [ScopeKind],
    pub(super) required: &'static [ScopeKind],
    restriction: ProductionRestriction,
    profile: ScopeProfile,
}

pub(super) fn specification(effect: EffectId) -> EffectSpecification {
    use EffectId as E;
    use ScopeProfile as P;
    match effect {
        E::ApprovalDecide | E::ApprovalRequest => {
            default_spec(GOVERNANCE_OBJECT, P::ApprovalObject)
        }
        E::KnowledgeApply | E::KnowledgePropose => {
            default_spec(GOVERNANCE_OBJECT, P::KnowledgeObject)
        }
        E::MigrationApply | E::ReleaseExecute => {
            external_spec(ARTIFACT_ENVIRONMENT, P::ArtifactEnvironment)
        }
        E::MigrationGenerate => {
            custom_spec(ENVIRONMENT_REPO, REPO_PATH, P::RepoEmitOptionalEnvironment)
        }
        E::NetworkRead | E::NetworkWrite => default_spec(NETWORK_ORIGIN, P::NetworkOrigin),
        E::PlacementPlan | E::TargetInventory => default_spec(TARGET_QUERY, P::TargetQuery),
        E::PolicyPropose | E::PolicyWrite => default_spec(GOVERNANCE_OBJECT, P::PolicyObject),
        E::ProcessExec => default_spec(COMMAND, P::Command),
        E::ReleasePlan => default_spec(ENVIRONMENT_REPO, P::EnvironmentRepoEmit),
        E::RepoRead => default_spec(REPO_PATH, P::RepoRead),
        E::RepoWrite => default_spec(REPO_PATH, P::RepoWriteExact),
        E::SecretsRead => default_spec(SECRET_REF, P::SecretRef),
        E::TargetExecute | E::TargetProbe | E::TargetReserve => default_spec(TARGET, P::Target),
    }
}

fn default_spec(kinds: &'static [ScopeKind], profile: ScopeProfile) -> EffectSpecification {
    custom_spec(kinds, kinds, profile)
}

fn external_spec(kinds: &'static [ScopeKind], profile: ScopeProfile) -> EffectSpecification {
    EffectSpecification {
        allowed: kinds,
        required: kinds,
        restriction: ProductionRestriction::ExternalOperatorOnly,
        profile,
    }
}

fn custom_spec(
    allowed: &'static [ScopeKind],
    required: &'static [ScopeKind],
    profile: ScopeProfile,
) -> EffectSpecification {
    EffectSpecification {
        allowed,
        required,
        restriction: ProductionRestriction::PolicyControlledDefaultDeny,
        profile,
    }
}
