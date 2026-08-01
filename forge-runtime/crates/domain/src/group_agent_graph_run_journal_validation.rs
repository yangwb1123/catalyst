use super::{
    BeginGroupAgentGraphRun, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION, GROUP_AGENT_GRAPH_RUN_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection, GroupAgentGraphRunRecord,
    GroupAgentGraphRunStatus, GroupAgentGraphRunValidationError,
    MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES, MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
    MAX_GROUP_AGENT_GRAPH_RUN_EVENTS, MAX_GROUP_AGENT_GRAPH_RUN_JOURNAL_BYTES,
};
use crate::{
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES, MAX_GROUP_AGENT_GRAPH_NODES,
};

pub(super) fn validate_record(
    record: &GroupAgentGraphRunRecord,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let valid = valid_identifier(&record.graph_run_id)
        && valid_identifier(&record.graph_id)
        && is_lower_hex_digest(&record.source_snapshot_sha256)
        && is_lower_hex_digest(&record.graph_manifest_sha256)
        && record.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && is_lower_hex_digest(&record.plan_sha256)
        && (1..=MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES).contains(&record.plan_bytes)
        && (1..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&record.node_count)
        && (1..=record.node_count).contains(&record.wave_count)
        && valid_record_state(record)
        && (1..=MAX_GROUP_AGENT_GRAPH_RUN_JOURNAL_BYTES).contains(&record.journal_bytes)
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid passive Group Agent Graph Run record"))
}

fn valid_record_state(record: &GroupAgentGraphRunRecord) -> bool {
    matches!(
        (
            record.v,
            record.status,
            record.execution_contract_present,
            record.dispatch_authority_released,
            record.last_event_seq,
        ),
        (
            GROUP_AGENT_GRAPH_RUN_VERSION,
            GroupAgentGraphRunStatus::AwaitingExecutionContract,
            false,
            false,
            1,
        ) | (
            GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
            GroupAgentGraphRunStatus::AwaitingCoreDispatch,
            true,
            false,
            2,
        )
    )
}

pub(super) fn validate_event(
    event: &GroupAgentGraphRunEvent,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let valid = match &event.kind {
        GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id,
            graph_manifest_sha256,
            plan_sha256,
            scheduler_protocol_version,
            prepared_at_ms,
        } => validate_prepared_event(
            event,
            graph_id,
            graph_manifest_sha256,
            plan_sha256,
            *scheduler_protocol_version,
            *prepared_at_ms,
        ),
        GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
            previous_event_sha256,
            control_snapshot_sha256,
            contract_id,
            contract_sha256,
            contract_bytes,
            node_id,
            attempt,
            request_sha256,
            project_lane_sha256,
            admitted_at_ms,
        } => validate_contract_event(
            event,
            previous_event_sha256,
            control_snapshot_sha256,
            contract_id,
            contract_sha256,
            *contract_bytes,
            node_id,
            *attempt,
            request_sha256,
            project_lane_sha256,
            *admitted_at_ms,
        ),
    };
    if valid && event.canonical_json()?.len() <= MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES {
        Ok(())
    } else {
        Err(invalid("invalid passive Group Agent Graph Run event"))
    }
}

fn validate_prepared_event(
    event: &GroupAgentGraphRunEvent,
    graph_id: &str,
    graph_manifest_sha256: &str,
    plan_sha256: &str,
    scheduler_protocol_version: u16,
    prepared_at_ms: u64,
) -> bool {
    event.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && event.seq == 1
        && valid_identifier(&event.graph_run_id)
        && valid_identifier(graph_id)
        && is_lower_hex_digest(graph_manifest_sha256)
        && is_lower_hex_digest(plan_sha256)
        && scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && i64::try_from(prepared_at_ms).is_ok()
}

#[allow(clippy::too_many_arguments)]
fn validate_contract_event(
    event: &GroupAgentGraphRunEvent,
    previous_event_sha256: &str,
    control_snapshot_sha256: &str,
    contract_id: &str,
    contract_sha256: &str,
    contract_bytes: usize,
    node_id: &str,
    attempt: u16,
    request_sha256: &str,
    project_lane_sha256: &str,
    admitted_at_ms: u64,
) -> bool {
    event.v == GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION
        && event.seq == 2
        && valid_identifier(&event.graph_run_id)
        && is_lower_hex_digest(previous_event_sha256)
        && is_lower_hex_digest(control_snapshot_sha256)
        && contract_id == format!("node-contract-{contract_sha256}")
        && is_lower_hex_digest(contract_sha256)
        && (1..=MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES).contains(&contract_bytes)
        && valid_identifier(node_id)
        && attempt == 1
        && is_lower_hex_digest(request_sha256)
        && is_lower_hex_digest(project_lane_sha256)
        && i64::try_from(admitted_at_ms).is_ok()
}

fn prepared_fields(event: &GroupAgentGraphRunEvent) -> Option<(&str, &str, &str, u16, u64)> {
    let GroupAgentGraphRunEventKind::GraphRunPrepared {
        graph_id,
        graph_manifest_sha256,
        plan_sha256,
        scheduler_protocol_version,
        prepared_at_ms,
    } = &event.kind
    else {
        return None;
    };
    Some((
        graph_id,
        graph_manifest_sha256,
        plan_sha256,
        *scheduler_protocol_version,
        *prepared_at_ms,
    ))
}

pub(super) fn validate_begin(
    request: &BeginGroupAgentGraphRun,
) -> Result<(), GroupAgentGraphRunValidationError> {
    request.plan.validate()?;
    request.event.validate()?;
    validate_begin_header(request)?;
    validate_exact_plan(&request.plan, &request.plan_json)?;
    validate_exact_event(&request.event, &request.event_json)?;
    validate_begin_binding(request)
}

fn validate_begin_header(
    request: &BeginGroupAgentGraphRun,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let valid = request.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && valid_identifier(&request.graph_run_id)
        && valid_identifier(&request.graph_id)
        && is_lower_hex_digest(&request.source_snapshot_sha256)
        && is_lower_hex_digest(&request.graph_manifest_sha256)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid passive Group Agent Graph Run request"))
}

fn validate_begin_binding(
    request: &BeginGroupAgentGraphRun,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let Some((
        graph_id,
        graph_manifest_sha256,
        plan_sha256,
        scheduler_protocol_version,
        prepared_at_ms,
    )) = prepared_fields(&request.event)
    else {
        return Err(invalid("Graph Run begin requires its preparation event"));
    };
    let valid = request.plan.graph_id == request.graph_id
        && request.plan.graph_manifest_sha256 == request.graph_manifest_sha256
        && request.event.graph_run_id == request.graph_run_id
        && graph_id == request.graph_id
        && graph_manifest_sha256 == request.graph_manifest_sha256
        && plan_sha256 == request.plan.plan_sha256
        && scheduler_protocol_version == request.plan.scheduler_protocol_version
        && prepared_at_ms == request.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Graph Run request bindings disagree"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentGraphRunValidationError> {
    inspection.run.validate()?;
    inspection.plan.validate()?;
    validate_exact_plan(&inspection.plan, &inspection.plan_json)?;
    validate_event_journal(inspection)?;
    validate_inspection_binding(inspection)
}

fn validate_inspection_binding(
    inspection: &GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let run = &inspection.run;
    let plan = &inspection.plan;
    let Some(prepared) = inspection.events.first().and_then(prepared_fields) else {
        return Err(invalid("Graph Run journal has no preparation event"));
    };
    let valid = inspection.v == run.v
        && run.graph_id == plan.graph_id
        && run.graph_id == prepared.0
        && run.graph_manifest_sha256 == plan.graph_manifest_sha256
        && run.graph_manifest_sha256 == prepared.1
        && run.plan_sha256 == plan.plan_sha256
        && run.plan_sha256 == prepared.2
        && run.scheduler_protocol_version == plan.scheduler_protocol_version
        && run.scheduler_protocol_version == prepared.3
        && run.graph_run_id == inspection.events[0].graph_run_id
        && run.created_at_ms == prepared.4
        && run.plan_bytes == inspection.plan_json.len()
        && run.node_count == plan.authored_node_ids.len()
        && run.wave_count == plan.waves.len();
    valid
        .then_some(())
        .ok_or_else(|| invalid("Graph Run inspection bindings disagree"))
}

fn validate_event_journal(
    inspection: &GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let expected_count = usize::try_from(inspection.run.last_event_seq)
        .map_err(|_| invalid("Graph Run event count is invalid"))?;
    if expected_count == 0
        || expected_count > MAX_GROUP_AGENT_GRAPH_RUN_EVENTS
        || inspection.events.len() != expected_count
        || inspection.event_jsons.len() != expected_count
    {
        return Err(invalid(
            "Graph Run journal length disagrees with its record",
        ));
    }
    let mut bytes = 0_usize;
    for (index, (event, json)) in inspection
        .events
        .iter()
        .zip(&inspection.event_jsons)
        .enumerate()
    {
        event.validate()?;
        validate_exact_event(event, json)?;
        let expected_seq = u64::try_from(index + 1).map_err(|_| invalid("event index overflow"))?;
        if event.seq != expected_seq || event.graph_run_id != inspection.run.graph_run_id {
            return Err(invalid("Graph Run journal envelope is not contiguous"));
        }
        bytes = bytes
            .checked_add(json.len())
            .ok_or_else(|| invalid("Graph Run journal byte count overflowed"))?;
    }
    validate_journal_head(inspection)?;
    if bytes != inspection.run.journal_bytes {
        return Err(invalid("Graph Run journal bytes disagree with its record"));
    }
    Ok(())
}

fn validate_journal_head(
    inspection: &GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentGraphRunValidationError> {
    if inspection.events.len() == 1 {
        return Ok(());
    }
    let previous = inspection.events[0].expected_sha256()?;
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
        previous_event_sha256,
        ..
    } = &inspection.events[1].kind
    else {
        return Err(invalid("Graph Run seq-2 event kind is invalid"));
    };
    if previous == *previous_event_sha256 {
        Ok(())
    } else {
        Err(invalid("Graph Run event hash chain is invalid"))
    }
}

fn validate_exact_plan(
    plan: &GroupAgentGraphCorePlan,
    json: &str,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let valid = !json.is_empty()
        && json.len() <= MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES
        && plan.canonical_json()?.as_bytes() == json.as_bytes();
    valid
        .then_some(())
        .ok_or_else(|| invalid("Core Plan JSON is not its exact canonical encoding"))
}

fn validate_exact_event(
    event: &GroupAgentGraphRunEvent,
    json: &str,
) -> Result<(), GroupAgentGraphRunValidationError> {
    let valid = !json.is_empty()
        && json.len() <= MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES
        && event.canonical_json()?.as_bytes() == json.as_bytes();
    valid
        .then_some(())
        .ok_or_else(|| invalid("Graph Run event JSON is not its exact canonical encoding"))
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

fn valid_text(value: &str, maximum: usize) -> bool {
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

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> GroupAgentGraphRunValidationError {
    GroupAgentGraphRunValidationError {
        message: message.into(),
    }
}
