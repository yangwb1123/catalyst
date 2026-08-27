use super::ScheduledGraphControllerEventPayload;

pub(super) fn allowed_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    use ScheduledGraphControllerEventPayload as Payload;
    if matches!(old, Payload::Stopped { .. } | Payload::Completed { .. }) {
        return false;
    }
    if matches!(new, Payload::Stopped { .. }) {
        return true;
    }
    match old {
        Payload::Started { .. } => matches!(
            new,
            Payload::MaterializePlanned { .. }
                | Payload::PreparePlanned { .. }
                | Payload::AwaitingFreshConsent { .. }
                | Payload::Completed { .. }
        ),
        Payload::MaterializePlanned { .. } => materialize_transition(old, new),
        Payload::MaterializeObserved { .. } => materialized_transition(old, new),
        Payload::PreparePlanned { .. } => prepare_transition(old, new),
        Payload::PrepareObserved { .. } => prepared_transition(old, new),
        Payload::AwaitingFreshConsent { .. } => awaiting_transition(old, new),
        Payload::DispatchPlanned { .. } => dispatch_transition(old, new),
        Payload::RetryablePreclaimFailure { .. } => retryable_transition(old, new),
        Payload::NodeCompleted { .. } => completed_node_transition(old, new),
        Payload::Stopped { .. } | Payload::Completed { .. } => false,
    }
}

fn materialize_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    matches!(
        new,
        ScheduledGraphControllerEventPayload::MaterializeObserved { .. }
    ) && same_node(old, new)
}

fn materialized_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    (matches!(
        new,
        ScheduledGraphControllerEventPayload::PreparePlanned { .. }
    ) && same_node(old, new)
        && same_contract(old, new))
        || (matches!(
            new,
            ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
        ) && same_node(old, new))
}

fn prepare_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    matches!(
        new,
        ScheduledGraphControllerEventPayload::PrepareObserved { .. }
    ) && same_node(old, new)
        && same_contract(old, new)
}

fn prepared_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    matches!(
        new,
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
    ) && same_node(old, new)
        && same_provider(old, new)
}

fn awaiting_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    (matches!(
        new,
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
            | ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    ) && same_node(old, new)
        && same_provider(old, new))
        || (matches!(
            new,
            ScheduledGraphControllerEventPayload::DispatchPlanned { .. }
        ) && exact_dispatch_anchor(old, new))
}

fn dispatch_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    matches!(
        new,
        ScheduledGraphControllerEventPayload::RetryablePreclaimFailure { .. }
            | ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    ) && same_node(old, new)
        && same_provider(old, new)
}

fn retryable_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    matches!(
        new,
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
            | ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    ) && same_node(old, new)
        && same_provider(old, new)
}

fn completed_node_transition(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    matches!(new, ScheduledGraphControllerEventPayload::Completed { .. })
        || (matches!(
            new,
            ScheduledGraphControllerEventPayload::MaterializePlanned { .. }
                | ScheduledGraphControllerEventPayload::PreparePlanned { .. }
                | ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
        ) && next_node(old, new))
}

fn same_node(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    event_node(old)
        .zip(event_node(new))
        .is_some_and(|values| values.0 == values.1)
}

fn next_node(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    event_node(old)
        .zip(event_node(new))
        .is_some_and(|((ordinal, _), (next, _))| ordinal.checked_add(1) == Some(next))
}

fn event_node(payload: &ScheduledGraphControllerEventPayload) -> Option<(usize, &str)> {
    use ScheduledGraphControllerEventPayload as Payload;
    match payload {
        Payload::MaterializePlanned {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::MaterializeObserved {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::PreparePlanned {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::PrepareObserved {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::AwaitingFreshConsent {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::DispatchPlanned {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::NodeCompleted {
            execution_ordinal,
            node_id,
            ..
        }
        | Payload::RetryablePreclaimFailure {
            execution_ordinal,
            node_id,
            ..
        } => Some((*execution_ordinal, node_id)),
        Payload::Started { .. } | Payload::Stopped { .. } | Payload::Completed { .. } => None,
    }
}

fn same_contract(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    event_contract(old)
        .zip(event_contract(new))
        .is_some_and(|values| values.0 == values.1)
}

fn event_contract(payload: &ScheduledGraphControllerEventPayload) -> Option<&str> {
    match payload {
        ScheduledGraphControllerEventPayload::MaterializeObserved { contract_id, .. }
        | ScheduledGraphControllerEventPayload::PreparePlanned { contract_id, .. }
        | ScheduledGraphControllerEventPayload::PrepareObserved { contract_id, .. } => {
            Some(contract_id)
        }
        _ => None,
    }
}

fn same_provider(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    event_provider(old)
        .zip(event_provider(new))
        .is_some_and(|values| values.0 == values.1)
}

fn event_provider(payload: &ScheduledGraphControllerEventPayload) -> Option<&str> {
    match payload {
        ScheduledGraphControllerEventPayload::PrepareObserved {
            provider_request_id,
            ..
        }
        | ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
            provider_request_id,
            ..
        }
        | ScheduledGraphControllerEventPayload::DispatchPlanned {
            provider_request_id,
            ..
        }
        | ScheduledGraphControllerEventPayload::NodeCompleted {
            provider_request_id,
            ..
        }
        | ScheduledGraphControllerEventPayload::RetryablePreclaimFailure {
            provider_request_id,
            ..
        } => Some(provider_request_id),
        _ => None,
    }
}

fn exact_dispatch_anchor(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
) -> bool {
    let ScheduledGraphControllerEventPayload::AwaitingFreshConsent {
        execution_ordinal,
        node_id,
        provider_request_id,
        authorization_sha256,
        snapshot_sha256,
        decision_sha256,
        predecessor_content_included,
        ..
    } = old
    else {
        return false;
    };
    let ScheduledGraphControllerEventPayload::DispatchPlanned {
        execution_ordinal: new_ordinal,
        node_id: new_node,
        provider_request_id: new_provider,
        authorization_sha256: new_authorization,
        snapshot_sha256: new_snapshot,
        decision_sha256: new_decision,
        predecessor_content_consent_observed,
        ..
    } = new
    else {
        return false;
    };
    execution_ordinal == new_ordinal
        && node_id == new_node
        && provider_request_id == new_provider
        && authorization_sha256 == new_authorization
        && snapshot_sha256 == new_snapshot
        && decision_sha256 == new_decision
        && (!predecessor_content_included || *predecessor_content_consent_observed)
}
