use super::{
    ActorRef, ActorType, EntityRef, EntityType, PlatformCoreContractError, RecordRef,
    RejectionCode, ScopeRef, identity, invalid, reject, wire,
};

/// Validates one supplied typed entity reference without resolving it.
///
/// # Errors
/// Returns an error when the entity type or Platform ID is invalid.
pub(crate) fn validate_entity_ref(
    value: &EntityRef,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    identity::validate_entity_id(&value.entity_type, &value.entity_id, label)
}

pub(super) fn validate_actor_ref(value: &ActorRef) -> Result<(), PlatformCoreContractError> {
    identity::validate_typed_id(&value.actor_id, "acr", "actor_ref.actor_id")?;
    if matches!(&value.actor_type, ActorType::Unknown(_)) {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "actor_ref.actor_type is unsupported",
        ));
    }
    Ok(())
}

/// Validates one supplied opaque record reference without resolving it.
///
/// # Errors
/// Returns an error when its identifier, digest, or record type is invalid.
pub(crate) fn validate_record_ref(
    value: &RecordRef,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    validate_record_id(&value.record_id, &format!("{label}.record_id"))?;
    wire::validate_hash(&value.record_sha256, &format!("{label}.record_sha256"))?;
    wire::validate_schema_name(&value.record_type, &format!("{label}.record_type"))
}

/// Validates supplied scope identities and their structural ancestry.
///
/// # Errors
/// Returns an error when an ID is invalid or a child lacks its required parent.
pub(crate) fn validate_scope_ref(value: &ScopeRef) -> Result<(), PlatformCoreContractError> {
    validate_scope_values(value)?;
    validate_scope_references(value)
}

pub(super) fn validate_scope_values(value: &ScopeRef) -> Result<(), PlatformCoreContractError> {
    identity::validate_typed_id(&value.space_id, "spc", "scope_ref.space_id")?;
    let checks = [
        (&value.action_id, "act", "action_id"),
        (&value.attempt_id, "atm", "attempt_id"),
        (&value.change_id, "chg", "change_id"),
        (&value.objective_id, "obj", "objective_id"),
        (&value.project_id, "prj", "project_id"),
        (&value.project_snapshot_id, "psn", "project_snapshot_id"),
        (&value.session_id, "ses", "session_id"),
        (&value.turn_id, "trn", "turn_id"),
        (&value.work_graph_id, "wgr", "work_graph_id"),
        (&value.work_item_id, "wki", "work_item_id"),
    ];
    for (candidate, prefix, label) in checks {
        if let Some(candidate) = candidate {
            identity::validate_typed_id(candidate, prefix, &format!("scope_ref.{label}"))?;
        }
    }
    Ok(())
}

pub(super) fn validate_scope_references(value: &ScopeRef) -> Result<(), PlatformCoreContractError> {
    validate_scope_ancestry(value)
}

pub(super) fn scope_contains(scope: &ScopeRef, reference: &EntityRef) -> bool {
    match &reference.entity_type {
        EntityType::Space => reference.entity_id == scope.space_id,
        EntityType::Project => matches_scope(&reference.entity_id, scope.project_id.as_deref()),
        EntityType::ProjectSnapshot => {
            matches_scope(&reference.entity_id, scope.project_snapshot_id.as_deref())
        }
        EntityType::Objective => matches_scope(&reference.entity_id, scope.objective_id.as_deref()),
        EntityType::Change => matches_scope(&reference.entity_id, scope.change_id.as_deref()),
        EntityType::WorkGraph => {
            matches_scope(&reference.entity_id, scope.work_graph_id.as_deref())
        }
        EntityType::WorkItem => matches_scope(&reference.entity_id, scope.work_item_id.as_deref()),
        EntityType::Attempt => matches_scope(&reference.entity_id, scope.attempt_id.as_deref()),
        EntityType::Session => matches_scope(&reference.entity_id, scope.session_id.as_deref()),
        EntityType::Turn => matches_scope(&reference.entity_id, scope.turn_id.as_deref()),
        EntityType::Action => matches_scope(&reference.entity_id, scope.action_id.as_deref()),
        EntityType::Actor | EntityType::Artifact | EntityType::Receipt | EntityType::Unknown(_) => {
            false
        }
    }
}

fn validate_record_id(value: &str, label: &str) -> Result<(), PlatformCoreContractError> {
    wire::validate_text(value, label, 160, true)?;
    let valid = value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_lowercase()
            || index > 0
                && (byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-'))
    });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} has invalid opaque record identifier text"
        )))
    }
}

fn validate_scope_ancestry(value: &ScopeRef) -> Result<(), PlatformCoreContractError> {
    require_parent(
        value.project_snapshot_id.is_some(),
        value.project_id.is_some(),
        "project_snapshot_id requires project_id",
    )?;
    require_parent(
        value.change_id.is_some(),
        value.objective_id.is_some(),
        "change_id requires objective_id",
    )?;
    require_parent(
        value.work_graph_id.is_some(),
        value.change_id.is_some(),
        "work_graph_id requires change_id",
    )?;
    require_parent(
        value.work_item_id.is_some(),
        value.work_graph_id.is_some(),
        "work_item_id requires work_graph_id",
    )?;
    require_parent(
        value.attempt_id.is_some(),
        value.work_item_id.is_some(),
        "attempt_id requires work_item_id",
    )?;
    require_parent(
        value.session_id.is_some(),
        value.attempt_id.is_some(),
        "session_id requires attempt_id",
    )?;
    require_parent(
        value.turn_id.is_some(),
        value.session_id.is_some(),
        "turn_id requires session_id",
    )?;
    require_parent(
        value.action_id.is_some(),
        value.turn_id.is_some(),
        "action_id requires turn_id",
    )?;
    Ok(())
}

fn require_parent(child: bool, parent: bool, label: &str) -> Result<(), PlatformCoreContractError> {
    if child && !parent {
        Err(reject(
            RejectionCode::ReferenceMismatch,
            format!("scope_ref.{label}"),
        ))
    } else {
        Ok(())
    }
}

fn matches_scope(expected: &str, candidate: Option<&str>) -> bool {
    candidate == Some(expected)
}
