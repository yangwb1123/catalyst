use super::{
    MAX_SCHEDULED_GRAPH_CONTROLLER_EVENT_BYTES, MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS,
    MAX_SCHEDULED_GRAPH_CONTROLLER_JOURNAL_BYTES, SCHEDULED_GRAPH_CONTROLLER_VERSION,
    ScheduledGraphControllerEvent, ScheduledGraphControllerEventPayload,
    ScheduledGraphControllerHeader, ScheduledGraphControllerJournal,
    ScheduledGraphControllerStopReason, ScheduledGraphControllerValidationError, codec,
};
use crate::{MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES};

use super::{lineage::validate_dispatch_lineage, transitions::allowed_transition};

mod profile_header;
pub(super) use profile_header::{validate_header, validate_profile};

pub(super) fn invalid(message: &str) -> ScheduledGraphControllerValidationError {
    ScheduledGraphControllerValidationError {
        message: message.into(),
    }
}

pub(super) fn validate_event_shape(
    value: &ScheduledGraphControllerEvent,
) -> Result<(), ScheduledGraphControllerValidationError> {
    let valid = value.v == SCHEDULED_GRAPH_CONTROLLER_VERSION
        && valid_identifier(&value.controller_id)
        && valid_identifier(&value.graph_run_id)
        && (1..=MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS).contains(&value.sequence)
        && value.previous_event_sha256.as_deref().is_none_or(is_digest)
        && i64::try_from(value.created_at_ms).is_ok()
        && is_digest(&value.event_sha256)
        && value.event_sha256 == codec::event_digest(value)?
        && codec::canonical_json(value)?.len() <= MAX_SCHEDULED_GRAPH_CONTROLLER_EVENT_BYTES;
    if !valid {
        return Err(invalid("scheduled Graph controller event is invalid"));
    }
    validate_payload(&value.payload)
}

pub(super) fn validate_journal(
    value: &ScheduledGraphControllerJournal,
) -> Result<(), ScheduledGraphControllerValidationError> {
    value.header.validate()?;
    if value.events.is_empty() || value.events.len() > MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS {
        return Err(invalid("scheduled Graph controller event count is invalid"));
    }
    let mut previous: Option<&ScheduledGraphControllerEvent> = None;
    let mut reservations = 0_u16;
    let mut reserved_cost = 0_u64;
    let mut pending_dispatch = None;
    let mut bytes = value.header.canonical_json()?.len();
    for event in &value.events {
        validate_event_shape(event)?;
        validate_event_binding(event, &value.header, previous)?;
        validate_dispatch_lineage(&mut pending_dispatch, &event.payload)?;
        if let ScheduledGraphControllerEventPayload::DispatchPlanned {
            effectful_step_reservation,
            reserved_cost_usd_micros,
            ..
        } = event.payload
        {
            reservations = reservations
                .checked_add(1)
                .ok_or_else(|| invalid("controller reservation count overflowed"))?;
            if effectful_step_reservation != reservations
                || reservations > value.header.max_effectful_steps
            {
                return Err(invalid("controller effectful reservation is invalid"));
            }
            reserved_cost = reserved_cost
                .checked_add(reserved_cost_usd_micros)
                .ok_or_else(|| invalid("controller cost reservation overflowed"))?;
            if reserved_cost > value.header.max_total_cost_usd_micros {
                return Err(invalid("controller aggregate cost budget is exceeded"));
            }
        }
        bytes = bytes
            .checked_add(event.canonical_json()?.len())
            .ok_or_else(|| invalid("controller journal byte count overflowed"))?;
        previous = Some(event);
    }
    if bytes > MAX_SCHEDULED_GRAPH_CONTROLLER_JOURNAL_BYTES {
        return Err(invalid(
            "scheduled Graph controller journal exceeds its byte bound",
        ));
    }
    Ok(())
}

fn validate_event_binding(
    event: &ScheduledGraphControllerEvent,
    header: &ScheduledGraphControllerHeader,
    previous: Option<&ScheduledGraphControllerEvent>,
) -> Result<(), ScheduledGraphControllerValidationError> {
    if event.controller_id != header.controller_id || event.graph_run_id != header.graph_run_id {
        return Err(invalid("controller event source binding is invalid"));
    }
    match previous {
        None => {
            if event.sequence != 1
                || event.previous_event_sha256.is_some()
                || !matches!(
                    event.payload,
                    ScheduledGraphControllerEventPayload::Started { .. }
                )
                || event.created_at_ms < header.created_at_ms
            {
                return Err(invalid("controller first event is invalid"));
            }
        }
        Some(old) => {
            if event.sequence != old.sequence + 1
                || event.previous_event_sha256.as_deref() != Some(&old.event_sha256)
                || event.created_at_ms < old.created_at_ms
                || !allowed_transition(&old.payload, &event.payload)
                || completes_before_last_node(&old.payload, &event.payload, header.node_count)
            {
                return Err(invalid("controller event transition is invalid"));
            }
        }
    }
    validate_payload_against_header(&event.payload, header)
}

fn completes_before_last_node(
    old: &ScheduledGraphControllerEventPayload,
    new: &ScheduledGraphControllerEventPayload,
    node_count: usize,
) -> bool {
    let ScheduledGraphControllerEventPayload::NodeCompleted {
        execution_ordinal, ..
    } = old
    else {
        return false;
    };
    matches!(new, ScheduledGraphControllerEventPayload::Completed { .. })
        && execution_ordinal.checked_add(1) != Some(node_count)
}

fn validate_payload(
    payload: &ScheduledGraphControllerEventPayload,
) -> Result<(), ScheduledGraphControllerValidationError> {
    valid_payload(payload)
        .then_some(())
        .ok_or_else(|| invalid("scheduled Graph controller event payload is invalid"))
}

fn valid_payload(payload: &ScheduledGraphControllerEventPayload) -> bool {
    use ScheduledGraphControllerEventPayload as Payload;
    match payload {
        Payload::Started {
            snapshot_sha256,
            decision_sha256,
        }
        | Payload::Completed {
            snapshot_sha256,
            decision_sha256,
        } => is_digest(snapshot_sha256) && is_digest(decision_sha256),
        Payload::MaterializePlanned { .. } => valid_materialize_plan(payload),
        Payload::MaterializeObserved {
            node_id,
            contract_id,
            ..
        } => valid_identifier(node_id) && valid_identifier(contract_id),
        Payload::PreparePlanned { .. } | Payload::PrepareObserved { .. } => valid_prepare(payload),
        Payload::AwaitingFreshConsent { .. } | Payload::DispatchPlanned { .. } => {
            valid_dispatch(payload)
        }
        Payload::NodeCompleted {
            node_id,
            provider_request_id,
            terminal_receipt_sha256,
            ..
        } => {
            valid_identifier(node_id)
                && valid_identifier(provider_request_id)
                && is_digest(terminal_receipt_sha256)
        }
        Payload::RetryablePreclaimFailure {
            node_id,
            provider_request_id,
            ..
        } => valid_identifier(node_id) && valid_identifier(provider_request_id),
        Payload::Stopped {
            reason,
            provider_request_id,
            snapshot_sha256,
            decision_sha256,
        } => {
            provider_request_id.as_deref().is_none_or(valid_identifier)
                && optional_digest_pair(snapshot_sha256.as_ref(), decision_sha256.as_ref())
                && stop_provider_shape(*reason, provider_request_id.as_deref())
                && stop_evidence_shape(*reason, snapshot_sha256.as_ref())
        }
    }
}

fn valid_materialize_plan(payload: &ScheduledGraphControllerEventPayload) -> bool {
    let ScheduledGraphControllerEventPayload::MaterializePlanned {
        node_id,
        snapshot_sha256,
        decision_sha256,
        idempotency_key,
        ..
    } = payload
    else {
        return false;
    };
    valid_identifier(node_id)
        && is_digest(snapshot_sha256)
        && is_digest(decision_sha256)
        && valid_idempotency_key(idempotency_key)
}

fn valid_prepare(payload: &ScheduledGraphControllerEventPayload) -> bool {
    use ScheduledGraphControllerEventPayload as Payload;
    match payload {
        Payload::PreparePlanned {
            node_id,
            contract_id,
            idempotency_key,
            ..
        } => {
            valid_identifier(node_id)
                && valid_identifier(contract_id)
                && valid_idempotency_key(idempotency_key)
        }
        Payload::PrepareObserved {
            node_id,
            contract_id,
            provider_request_id,
            ..
        } => {
            valid_identifier(node_id)
                && valid_identifier(contract_id)
                && valid_identifier(provider_request_id)
        }
        _ => false,
    }
}

fn valid_dispatch(payload: &ScheduledGraphControllerEventPayload) -> bool {
    use ScheduledGraphControllerEventPayload as Payload;
    let (Payload::AwaitingFreshConsent {
        node_id,
        provider_request_id: request_id,
        authorization_sha256: authorization,
        snapshot_sha256: snapshot,
        decision_sha256: decision,
        ..
    }
    | Payload::DispatchPlanned {
        node_id,
        provider_request_id: request_id,
        authorization_sha256: authorization,
        snapshot_sha256: snapshot,
        decision_sha256: decision,
        ..
    }) = payload
    else {
        return false;
    };
    valid_identifier(node_id)
        && valid_identifier(request_id)
        && is_digest(authorization)
        && is_digest(snapshot)
        && is_digest(decision)
}

fn validate_payload_against_header(
    payload: &ScheduledGraphControllerEventPayload,
    header: &ScheduledGraphControllerHeader,
) -> Result<(), ScheduledGraphControllerValidationError> {
    let ordinal = payload_ordinal(payload);
    if ordinal.is_some_and(|value| value >= header.node_count) {
        return Err(invalid("controller event ordinal is outside the schedule"));
    }
    if let ScheduledGraphControllerEventPayload::DispatchPlanned {
        effectful_step_reservation,
        reserved_cost_usd_micros,
        off_machine_consent_observed,
        ..
    } = payload
        && (*effectful_step_reservation == 0
            || *effectful_step_reservation > header.max_effectful_steps
            || *reserved_cost_usd_micros != header.execution_profile.max_cost_usd_micros
            || !off_machine_consent_observed)
    {
        return Err(invalid(
            "controller dispatch plan consent or budget is invalid",
        ));
    }
    validate_planned_idempotency(payload, header)
}

fn validate_planned_idempotency(
    payload: &ScheduledGraphControllerEventPayload,
    header: &ScheduledGraphControllerHeader,
) -> Result<(), ScheduledGraphControllerValidationError> {
    let expected_key = match payload {
        ScheduledGraphControllerEventPayload::MaterializePlanned {
            execution_ordinal,
            idempotency_key,
            ..
        } => Some((
            idempotency_key,
            format!(
                "controller-{}-materialize-{execution_ordinal}",
                header.controller_sha256
            ),
        )),
        ScheduledGraphControllerEventPayload::PreparePlanned {
            execution_ordinal,
            idempotency_key,
            ..
        } => Some((
            idempotency_key,
            format!(
                "controller-{}-prepare-{execution_ordinal}",
                header.controller_sha256
            ),
        )),
        _ => None,
    };
    if expected_key.is_some_and(|(actual, expected)| actual != &expected) {
        return Err(invalid(
            "controller planned idempotency key is not source-bound",
        ));
    }
    Ok(())
}

fn payload_ordinal(payload: &ScheduledGraphControllerEventPayload) -> Option<usize> {
    use ScheduledGraphControllerEventPayload as Payload;
    match payload {
        Payload::MaterializePlanned {
            execution_ordinal, ..
        }
        | Payload::MaterializeObserved {
            execution_ordinal, ..
        }
        | Payload::PreparePlanned {
            execution_ordinal, ..
        }
        | Payload::PrepareObserved {
            execution_ordinal, ..
        }
        | Payload::AwaitingFreshConsent {
            execution_ordinal, ..
        }
        | Payload::DispatchPlanned {
            execution_ordinal, ..
        }
        | Payload::NodeCompleted {
            execution_ordinal, ..
        }
        | Payload::RetryablePreclaimFailure {
            execution_ordinal, ..
        } => Some(*execution_ordinal),
        Payload::Started { .. } | Payload::Stopped { .. } | Payload::Completed { .. } => None,
    }
}

fn stop_provider_shape(reason: ScheduledGraphControllerStopReason, value: Option<&str>) -> bool {
    match reason {
        ScheduledGraphControllerStopReason::ClaimedUnknown
        | ScheduledGraphControllerStopReason::Quarantined
        | ScheduledGraphControllerStopReason::Adjudicated
        | ScheduledGraphControllerStopReason::Failed
        | ScheduledGraphControllerStopReason::FailedUncertain
        | ScheduledGraphControllerStopReason::BudgetExhausted => value.is_some(),
        ScheduledGraphControllerStopReason::IncompatibleProgress => true,
        ScheduledGraphControllerStopReason::IncompatibleSchedule => value.is_none(),
    }
}

fn stop_evidence_shape(
    reason: ScheduledGraphControllerStopReason,
    snapshot_sha256: Option<&String>,
) -> bool {
    match reason {
        ScheduledGraphControllerStopReason::IncompatibleSchedule => snapshot_sha256.is_none(),
        ScheduledGraphControllerStopReason::IncompatibleProgress
        | ScheduledGraphControllerStopReason::BudgetExhausted => snapshot_sha256.is_some(),
        ScheduledGraphControllerStopReason::ClaimedUnknown
        | ScheduledGraphControllerStopReason::Quarantined
        | ScheduledGraphControllerStopReason::Adjudicated
        | ScheduledGraphControllerStopReason::Failed
        | ScheduledGraphControllerStopReason::FailedUncertain => true,
    }
}

fn optional_digest_pair(left: Option<&String>, right: Option<&String>) -> bool {
    match (left.map(String::as_str), right.map(String::as_str)) {
        (None, None) => true,
        (Some(left), Some(right)) => is_digest(left) && is_digest(right),
        _ => false,
    }
}

pub(super) fn in_u64_bound(value: u64, maximum: u64) -> bool {
    value > 0 && value <= maximum
}

fn valid_idempotency_key(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES)
}

pub(super) fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

pub(super) fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(unsupported_character)
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
