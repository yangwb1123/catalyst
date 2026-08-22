use super::{
    CapabilityGrantContractError, EffectId, GovernanceObjectKind, GrantScope, PathMatch,
    RequestedAction, ScopeClause, ScopeKind, ScopeResource, canonical, invalid, primitives,
    vocabulary::ScopeResourceKind, vocabulary_validation,
};

pub(super) fn validate_scope(scope: &GrantScope) -> Result<(), CapabilityGrantContractError> {
    if scope.allow.is_empty() || scope.allow.len() > 64 || scope.deny.len() > 64 {
        return Err(invalid("scope allow/deny cardinality is invalid"));
    }
    for clause in &scope.allow {
        validate_clause(clause, scope.effect_id)?;
    }
    let total = scope
        .allow
        .iter()
        .map(|clause| clause.resources.len())
        .sum::<usize>()
        + scope.deny.len();
    if total > 256 {
        return Err(invalid("scope resource count exceeds 256"));
    }
    for resource in &scope.deny {
        validate_resource(resource)?;
        require_allowed_kind(scope.effect_id, resource.scope_kind())?;
    }
    require_canonical_order(&scope.allow, "scope allow clauses")?;
    require_resource_order(&scope.deny, "scope deny resources")
}

pub(super) fn validate_action(
    action: &RequestedAction,
) -> Result<(), CapabilityGrantContractError> {
    if action.resources.is_empty() || action.resources.len() > 32 {
        return Err(invalid(
            "requested action resources must contain 1..32 entries",
        ));
    }
    for resource in &action.resources {
        validate_resource(resource)?;
        require_allowed_kind(action.effect_id, resource.scope_kind())?;
    }
    require_required_kinds(action.effect_id, &action.resources)?;
    validate_profile(action.effect_id, &action.resources, true)?;
    require_resource_order(&action.resources, "requested action resources")?;
    super::grant_validation::validate_requested_usage(&action.usage)?;
    validate_action_timeout_binding(action)
}

fn validate_action_timeout_binding(
    action: &RequestedAction,
) -> Result<(), CapabilityGrantContractError> {
    if action.effect_id != EffectId::ProcessExec {
        return Ok(());
    }
    let Some(ScopeResource::Command { timeout_ms, .. }) = action.resources.first() else {
        return Err(invalid("process.exec action requires exactly one command"));
    };
    if *timeout_ms == action.usage.timeout_ms {
        Ok(())
    } else {
        Err(invalid(
            "process.exec command timeout must equal requested usage timeout",
        ))
    }
}

fn validate_clause(
    clause: &ScopeClause,
    effect: EffectId,
) -> Result<(), CapabilityGrantContractError> {
    if clause.resources.is_empty() || clause.resources.len() > 32 {
        return Err(invalid("scope clause resources must contain 1..32 entries"));
    }
    for resource in &clause.resources {
        validate_resource(resource)?;
        require_allowed_kind(effect, resource.scope_kind())?;
    }
    require_required_kinds(effect, &clause.resources)?;
    validate_profile(effect, &clause.resources, false)?;
    require_resource_order(&clause.resources, "scope clause resources")
}

fn validate_profile(
    effect: EffectId,
    resources: &[ScopeResource],
    action: bool,
) -> Result<(), CapabilityGrantContractError> {
    let counts = |kind| {
        resources
            .iter()
            .filter(|item| item.scope_kind() == kind)
            .count()
    };
    let artifacts = counts(ScopeKind::Artifact);
    let environments = counts(ScopeKind::Environment);
    let paths = counts(ScopeKind::RepoPath);
    let valid = match effect {
        EffectId::MigrationApply | EffectId::ReleaseExecute => {
            resources.len() == 2 && artifacts == 1 && environments == 1
        }
        EffectId::ReleasePlan => {
            resources.len() == paths + 1 && environments == 1 && (1..=32).contains(&paths)
        }
        EffectId::MigrationGenerate => {
            resources.len() == paths + environments
                && environments <= 1
                && (1..=32).contains(&paths)
        }
        EffectId::RepoRead | EffectId::RepoWrite => {
            resources.len() == paths && (1..=32).contains(&paths)
        }
        _ => resources.len() == 1,
    };
    if !valid {
        return Err(invalid("scope resources do not match the effect profile"));
    }
    validate_profile_details(effect, resources, action)
}

fn validate_profile_details(
    effect: EffectId,
    resources: &[ScopeResource],
    action: bool,
) -> Result<(), CapabilityGrantContractError> {
    for resource in resources {
        if let ScopeResource::RepoPath { path_match, .. } = resource
            && (action || effect != EffectId::RepoRead)
            && *path_match != PathMatch::Exact
        {
            return Err(invalid("effect profile requires exact repo paths"));
        }
        if let ScopeResource::GovernanceObject { object_kind, .. } = resource
            && !governance_kind_matches(effect, *object_kind)
        {
            return Err(invalid("governance object kind does not match effect"));
        }
    }
    Ok(())
}

fn governance_kind_matches(effect: EffectId, kind: GovernanceObjectKind) -> bool {
    match effect {
        EffectId::ApprovalDecide | EffectId::ApprovalRequest => {
            kind == GovernanceObjectKind::Approval
        }
        EffectId::KnowledgeApply | EffectId::KnowledgePropose => {
            kind == GovernanceObjectKind::Knowledge
        }
        EffectId::PolicyPropose | EffectId::PolicyWrite => kind == GovernanceObjectKind::Policy,
        _ => true,
    }
}

fn require_allowed_kind(
    effect: EffectId,
    kind: ScopeKind,
) -> Result<(), CapabilityGrantContractError> {
    if vocabulary_validation::specification(effect)
        .allowed
        .contains(&kind)
    {
        Ok(())
    } else {
        Err(invalid(
            "scope kind is not admitted for the declared effect",
        ))
    }
}

fn require_required_kinds(
    effect: EffectId,
    resources: &[ScopeResource],
) -> Result<(), CapabilityGrantContractError> {
    let spec = vocabulary_validation::specification(effect);
    if spec.required.iter().all(|kind| {
        resources
            .iter()
            .any(|resource| resource.scope_kind() == *kind)
    }) {
        Ok(())
    } else {
        Err(invalid(
            "scope omits a required kind for the declared effect",
        ))
    }
}

pub(super) fn validate_resource(
    resource: &ScopeResource,
) -> Result<(), CapabilityGrantContractError> {
    match resource {
        ScopeResource::Artifact { .. } => validate_artifact_resource(resource),
        ScopeResource::Command { .. } => validate_command_resource(resource),
        ScopeResource::Environment { .. } => validate_environment_resource(resource),
        ScopeResource::GovernanceObject { .. } => validate_governance_resource(resource),
        ScopeResource::NetworkOrigin { .. } => validate_network_resource(resource),
        ScopeResource::RepoPath { path_match, path } => validate_repo_path(*path_match, path),
        ScopeResource::SecretRef { .. } => validate_secret_resource(resource),
        ScopeResource::Target { .. } => validate_target_resource(resource),
        ScopeResource::TargetQuery { .. } => validate_query_resource(resource),
    }
}

fn validate_artifact_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::Artifact {
        artifact_kind,
        artifact_ref,
        artifact_sha256,
    } = value
    else {
        unreachable!()
    };
    primitives::text(artifact_kind, 160, "artifact_kind")?;
    primitives::text(artifact_ref, 4_096, "artifact_ref")?;
    primitives::sha256(artifact_sha256, "artifact_sha256")
}

fn validate_command_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::Command {
        argv,
        cwd,
        environment_sha256,
        stdin_bytes,
        stdin_sha256,
        timeout_ms,
        tool_snapshot_sha256,
    } = value
    else {
        unreachable!()
    };
    validate_command(argv, cwd, *stdin_bytes, *timeout_ms)?;
    primitives::sha256(environment_sha256, "command environment_sha256")?;
    primitives::sha256(stdin_sha256, "command stdin_sha256")?;
    if *stdin_bytes == 0
        && stdin_sha256 != "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    {
        return Err(invalid("zero-byte stdin must bind SHA256(empty)"));
    }
    primitives::sha256(tool_snapshot_sha256, "tool_snapshot_sha256")
}

fn validate_environment_resource(
    value: &ScopeResource,
) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::Environment {
        environment_class,
        environment_id,
        environment_sha256,
    } = value
    else {
        unreachable!()
    };
    if *environment_class == super::EnvironmentClass::Local {
        return Err(invalid("scope environment_class cannot be local"));
    }
    primitives::text(environment_id, 160, "environment_id")?;
    primitives::sha256(environment_sha256, "environment_sha256")
}

fn validate_governance_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::GovernanceObject {
        object_ref,
        object_scope_sha256,
        ..
    } = value
    else {
        unreachable!()
    };
    primitives::text(object_ref, 4_096, "object_ref")?;
    primitives::sha256(object_scope_sha256, "object_scope_sha256")
}

fn validate_network_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::NetworkOrigin {
        host,
        host_kind,
        port,
        ..
    } = value
    else {
        unreachable!()
    };
    primitives::host(host, *host_kind)?;
    primitives::integer(*port, 1, 65_535, "network port")
}

fn validate_secret_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::SecretRef {
        broker_id,
        secret_ref,
        version_ref,
    } = value
    else {
        unreachable!()
    };
    primitives::text(broker_id, 160, "broker_id")?;
    primitives::text(secret_ref, 4_096, "secret_ref")?;
    validate_version_ref(version_ref)
}

fn validate_target_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::Target {
        target_attestation_sha256,
        target_id,
    } = value
    else {
        unreachable!()
    };
    primitives::sha256(target_attestation_sha256, "target_attestation_sha256")?;
    primitives::text(target_id, 160, "target_id")
}

fn validate_query_resource(value: &ScopeResource) -> Result<(), CapabilityGrantContractError> {
    let ScopeResource::TargetQuery {
        query_ref,
        query_sha256,
    } = value
    else {
        unreachable!()
    };
    primitives::text(query_ref, 4_096, "query_ref")?;
    primitives::sha256(query_sha256, "query_sha256")
}

fn validate_command(
    argv: &[String],
    cwd: &str,
    stdin_bytes: i64,
    timeout_ms: i64,
) -> Result<(), CapabilityGrantContractError> {
    if argv.is_empty() || argv.len() > 64 {
        return Err(invalid("command argv must contain 1..64 entries"));
    }
    let mut total = 0_usize;
    for argument in argv {
        primitives::text(argument, 4_096, "command argument")?;
        total += argument.len();
    }
    if total > 32_768 {
        return Err(invalid("command argv exceeds 32768 total bytes"));
    }
    primitives::canonical_path(cwd, true)?;
    primitives::integer(stdin_bytes, 0, i64::MAX, "stdin_bytes")?;
    primitives::integer(timeout_ms, 1, 86_400_000, "command timeout_ms")
}

fn validate_version_ref(value: &str) -> Result<(), CapabilityGrantContractError> {
    primitives::text(value, 4_096, "version_ref")?;
    let lexical = value.as_bytes().split_first().is_some_and(|(first, rest)| {
        first.is_ascii_alphanumeric()
            && rest.iter().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(*byte, b'.' | b'_' | b':' | b'/' | b'@' | b'+' | b'-')
            })
    });
    let lowercase = value.to_ascii_lowercase();
    if !lexical
        || matches!(lowercase.as_str(), "latest" | "current" | "active")
        || value.contains('*')
    {
        Err(invalid("version_ref must be immutable and exact"))
    } else {
        Ok(())
    }
}

fn validate_repo_path(
    path_match: PathMatch,
    path: &str,
) -> Result<(), CapabilityGrantContractError> {
    primitives::canonical_path(path, true)?;
    if path == "." && path_match == PathMatch::Exact {
        return Err(invalid("exact repo path cannot name the repository root"));
    }
    Ok(())
}

fn require_canonical_order<T: serde::Serialize>(
    values: &[T],
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    let encoded = values
        .iter()
        .map(|value| canonical::encode(value, super::MAX_GRANT_BYTES, label))
        .collect::<Result<Vec<_>, _>>()?;
    if encoded
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly ordered and unique"
        )))
    }
}

fn require_resource_order(
    values: &[ScopeResource],
    label: &str,
) -> Result<(), CapabilityGrantContractError> {
    let encoded = values
        .iter()
        .map(|value| {
            let json = canonical::encode(value, super::MAX_GRANT_BYTES, label)?;
            Ok((value.scope_kind().as_str(), json))
        })
        .collect::<Result<Vec<_>, CapabilityGrantContractError>>()?;
    if encoded.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be strictly ordered and unique"
        )))
    }
}

pub(super) fn scope_relation(scope: &GrantScope, action: &RequestedAction) -> super::ScopeRelation {
    if scope.effect_id != action.effect_id {
        return super::ScopeRelation::OutsideDeclaredScope;
    }
    if action
        .resources
        .iter()
        .any(|requested| scope.deny.iter().any(|deny| covers(deny, requested)))
    {
        return super::ScopeRelation::DeniedByDeclaration;
    }
    if scope
        .allow
        .iter()
        .any(|clause| clause_covers(scope.effect_id, clause, action))
    {
        super::ScopeRelation::CoveredByDeclaration
    } else {
        super::ScopeRelation::OutsideDeclaredScope
    }
}

fn clause_covers(effect: EffectId, clause: &ScopeClause, action: &RequestedAction) -> bool {
    if effect == EffectId::MigrationGenerate
        && has_environment(&clause.resources) != has_environment(&action.resources)
    {
        return false;
    }
    action.resources.iter().all(|requested| {
        clause
            .resources
            .iter()
            .any(|allow| covers(allow, requested))
    })
}

fn has_environment(resources: &[ScopeResource]) -> bool {
    resources
        .iter()
        .any(|resource| matches!(resource, ScopeResource::Environment { .. }))
}

fn covers(declared: &ScopeResource, requested: &ScopeResource) -> bool {
    match (declared, requested) {
        (
            ScopeResource::RepoPath {
                path_match: declared_match,
                path: declared_path,
            },
            ScopeResource::RepoPath {
                path_match: requested_match,
                path: requested_path,
            },
        ) => repo_path_covers(
            *declared_match,
            declared_path,
            *requested_match,
            requested_path,
        ),
        _ => declared == requested,
    }
}

fn repo_path_covers(
    declared_match: PathMatch,
    declared_path: &str,
    requested_match: PathMatch,
    requested_path: &str,
) -> bool {
    if declared_match == PathMatch::Exact {
        return requested_match == PathMatch::Exact && declared_path == requested_path;
    }
    declared_path == "."
        || requested_path == declared_path
        || requested_path
            .strip_prefix(declared_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
