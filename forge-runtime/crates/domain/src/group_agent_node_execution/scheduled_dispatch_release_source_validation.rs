use super::{
    GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchReleaseValidationError,
};
use crate::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphRunInspection,
    GroupAgentGraphRunStatus, GroupAgentScheduledNodeContractInspection,
    group_agent_node_provider_request_sha256,
};

pub(super) fn validate_sources(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    validate_graph_source(control)?;
    validate_schedule_source(control)?;
    validate_contract_source(control)?;
    validate_request_source(control)
}

fn validate_graph_source(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    control
        .control_snapshot
        .validate()
        .map_err(|_| invalid("scheduled release Graph control is invalid"))?;
    let event_jsons = control
        .journal_events
        .iter()
        .map(|event| {
            event
                .canonical_json()
                .map_err(|_| invalid("invalid Graph journal"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inspection = GroupAgentGraphRunInspection {
        v: control.graph_run.v,
        run: control.graph_run.clone(),
        plan_json: control
            .control_snapshot
            .plan
            .canonical_json()
            .map_err(|_| invalid("invalid Graph plan"))?,
        plan: control.control_snapshot.plan.clone(),
        event_jsons,
        events: control.journal_events.clone(),
    };
    inspection
        .validate()
        .map_err(|_| invalid("scheduled release Graph Run is not exact current v1 state"))?;
    validate_control_run_bindings(control)
}

fn validate_control_run_bindings(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    let snapshot = &control.control_snapshot;
    let run = &control.graph_run;
    let event = control
        .journal_events
        .first()
        .ok_or_else(|| invalid("scheduled release Graph journal is empty"))?;
    let event_sha256 = event
        .expected_sha256()
        .map_err(|_| invalid("scheduled release Graph journal head is invalid"))?;
    let valid = run.status == GroupAgentGraphRunStatus::AwaitingExecutionContract
        && control.journal_events.len() == 1
        && snapshot.graph_run_version == run.v
        && snapshot.graph_run_id == run.graph_run_id
        && snapshot.graph_id == run.graph_id
        && snapshot.source_snapshot_sha256 == run.source_snapshot_sha256
        && snapshot.graph_manifest_sha256 == run.graph_manifest_sha256
        && snapshot.scheduler_protocol_version == run.scheduler_protocol_version
        && snapshot.core_plan_sha256 == run.plan_sha256
        && snapshot.last_event_seq == run.last_event_seq
        && snapshot.last_event_sha256 == event_sha256
        && !run.execution_contract_present
        && !run.dispatch_request_present
        && !run.dispatch_authority_released;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled release Graph control, record, and journal disagree"))
}

fn validate_schedule_source(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    control
        .schedule
        .validate_against_control(&control.control_snapshot)
        .map_err(|_| invalid("scheduled release execution schedule source is invalid"))?;
    let inspection = GroupAgentGraphExecutionScheduleInspection {
        v: control.schedule.v,
        record: control.schedule_record.clone(),
        schedule_json: control
            .schedule
            .canonical_json()
            .map_err(|_| invalid("scheduled release schedule bytes are invalid"))?,
        schedule: control.schedule.clone(),
    };
    inspection
        .validate()
        .map_err(|_| invalid("scheduled release schedule record disagrees"))
}

fn validate_contract_source(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    control
        .scheduled_contract
        .validate_against_control_and_schedule(&control.control_snapshot, &control.schedule)
        .map_err(|_| invalid("scheduled release contract source is invalid"))?;
    let inspection = GroupAgentScheduledNodeContractInspection {
        v: control.scheduled_contract.v,
        record: control.scheduled_contract_record.clone(),
        candidate_json: control
            .scheduled_contract
            .canonical_json()
            .map_err(|_| invalid("scheduled release contract bytes are invalid"))?,
        candidate: control.scheduled_contract.clone(),
    };
    inspection
        .validate()
        .map_err(|_| invalid("scheduled release contract record disagrees"))
}

fn validate_request_source(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    let request = &control.provider_request;
    request
        .validate()
        .map_err(|_| invalid("scheduled release provider request record is invalid"))?;
    let body = control.provider_request_json.as_bytes();
    if serde_json::from_str::<serde_json::Value>(&control.provider_request_json).is_err()
        || request.provider_request_bytes != body.len()
        || request.provider_request_sha256 != group_agent_node_provider_request_sha256(body)
    {
        return Err(invalid(
            "scheduled release exact UTF-8 provider request JSON disagrees",
        ));
    }
    validate_request_contract_bindings(control)
}

fn validate_request_contract_bindings(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
    let record = &control.provider_request;
    let contract = &control.scheduled_contract;
    let valid = record.graph_run_id == contract.graph_run_id
        && record.schedule_id == contract.schedule_id
        && record.schedule_sha256 == contract.schedule_sha256
        && record.scheduled_contract_id == contract.contract_id
        && record.scheduled_contract_sha256 == contract.contract_sha256
        && record.execution_ordinal == contract.node.execution_ordinal
        && record.node_id == contract.node.node_id
        && record.attempt == contract.node.attempt
        && record.logical_request_id == contract.request.request_id
        && record.logical_request_sha256 == contract.request.request_sha256
        && record.project_lane_sha256 == contract.node.project_lane_sha256
        && record.provider == contract.provider.kind
        && record.endpoint == contract.provider.endpoint
        && record.model == contract.provider.model
        && record.pricing_snapshot_sha256 == contract.budgets.pricing_snapshot_sha256
        && record.expected_last_event_seq == contract.expected_last_event_seq
        && record.expected_last_event_sha256 == contract.expected_last_event_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled release provider request bindings disagree"))
}

fn invalid(message: &str) -> GroupAgentScheduledNodeDispatchReleaseValidationError {
    GroupAgentScheduledNodeDispatchReleaseValidationError {
        message: message.into(),
    }
}
