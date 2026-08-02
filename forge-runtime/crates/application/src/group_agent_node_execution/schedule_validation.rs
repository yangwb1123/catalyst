use std::collections::BTreeSet;

use crate::runtime_domain::{
    AdmitGroupAgentGraphExecutionSchedule, AdmitGroupAgentGraphExecutionScheduleDisposition,
    AdmitGroupAgentGraphExecutionScheduleResult, GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
    GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleRecord, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES,
    MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_LIST_LIMIT,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
};

use super::{
    AdmitGroupAgentGraphExecutionScheduleInput, ExportGroupAgentGraphControl,
    GroupAgentGraphExecutionScheduleServiceError,
    schedule_error::{corrupt, invalid},
};

pub(super) fn parse_admit_input(
    input: &AdmitGroupAgentGraphExecutionScheduleInput,
) -> Result<GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleServiceError> {
    validate_identifier(&input.graph_run_id, "Graph Run ID")?;
    validate_text(
        &input.idempotency_key,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        "idempotency key",
    )?;
    if i64::try_from(input.admitted_at_ms).is_err()
        || !(1..=MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES)
            .contains(&input.schedule_json.len())
    {
        return Err(invalid("schedule byte bound or admission time is invalid"));
    }
    let schedule = GroupAgentGraphExecutionSchedule::decode_exact(&input.schedule_json)
        .map_err(|error| invalid(&error.to_string()))?;
    if schedule.graph_run_id != input.graph_run_id {
        return Err(invalid("schedule Graph Run binding disagrees"));
    }
    Ok(schedule)
}

pub(super) fn admission_request(
    input: &AdmitGroupAgentGraphExecutionScheduleInput,
    schedule: GroupAgentGraphExecutionSchedule,
    control: ExportGroupAgentGraphControl,
) -> Result<AdmitGroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleServiceError> {
    let request = AdmitGroupAgentGraphExecutionSchedule {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        control_snapshot: control.snapshot,
        control_snapshot_json: control.snapshot_json,
        schedule,
        schedule_json: input.schedule_json.clone(),
        idempotency_key: input.idempotency_key.clone(),
        admitted_at_ms: input.admitted_at_ms,
    };
    request
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    Ok(request)
}

pub(super) fn checked_run(
    inspection: GroupAgentGraphRunInspection,
) -> Result<GroupAgentGraphRunInspection, GroupAgentGraphExecutionScheduleServiceError> {
    super::validation::checked_run(inspection).map_err(Into::into)
}

pub(super) fn checked_graph(
    inspection: GroupAgentGraphInspection,
) -> Result<GroupAgentGraphInspection, GroupAgentGraphExecutionScheduleServiceError> {
    super::validation::checked_graph(inspection).map_err(Into::into)
}

pub(super) fn checked_inspection(
    inspection: GroupAgentGraphExecutionScheduleInspection,
) -> Result<GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleServiceError>
{
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_admit_result(
    request: &AdmitGroupAgentGraphExecutionSchedule,
    result: AdmitGroupAgentGraphExecutionScheduleResult,
) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, GroupAgentGraphExecutionScheduleServiceError>
{
    if result.v != GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION {
        return Err(corrupt("store returned an unsupported schedule version"));
    }
    let inspection = checked_inspection(result.inspection)?;
    if inspection.schedule != request.schedule
        || inspection.schedule_json != request.schedule_json
        || inspection.record.graph_run_id != request.graph_run_id
    {
        return Err(corrupt("store returned different schedule semantics"));
    }
    if result.disposition == AdmitGroupAgentGraphExecutionScheduleDisposition::Created
        && inspection.record.created_at_ms != request.admitted_at_ms
    {
        return Err(corrupt(
            "created schedule admission does not preserve its admission time",
        ));
    }
    Ok(AdmitGroupAgentGraphExecutionScheduleResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

pub(super) fn validate_inspection_source(
    inspection: &GroupAgentGraphExecutionScheduleInspection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), GroupAgentGraphExecutionScheduleServiceError> {
    let control = super::snapshot::for_admission(run, graph)?;
    inspection
        .schedule
        .validate_against_control(&control.snapshot)
        .map_err(|error| corrupt(&error.to_string()))
}

pub(super) fn validate_list_input(
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentGraphExecutionScheduleServiceError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_LIST_LIMIT).contains(&limit) {
        return Err(invalid("schedule list limit is outside its bounds"));
    }
    if let Some(id) = graph_run_id {
        validate_identifier(id, "Graph Run ID")?;
    }
    Ok(())
}

pub(super) fn validate_list(
    records: &[GroupAgentGraphExecutionScheduleRecord],
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentGraphExecutionScheduleServiceError> {
    if records.len() > limit {
        return Err(corrupt(
            "store returned more schedule records than requested",
        ));
    }
    let mut schedules = BTreeSet::new();
    let mut runs = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|error| corrupt(&error.to_string()))?;
        if graph_run_id.is_some_and(|id| id != record.graph_run_id)
            || !schedules.insert(record.schedule_id.as_str())
            || !runs.insert(record.graph_run_id.as_str())
        {
            return Err(corrupt(
                "store returned unfiltered or duplicate schedule metadata",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_identifier(
    value: &str,
    subject: &str,
) -> Result<(), GroupAgentGraphExecutionScheduleServiceError> {
    super::validation::validate_identifier(value, subject).map_err(Into::into)
}

fn validate_text(
    value: &str,
    maximum: usize,
    subject: &str,
) -> Result<(), GroupAgentGraphExecutionScheduleServiceError> {
    let valid =
        !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(unsupported);
    valid
        .then_some(())
        .ok_or_else(|| invalid(&format!("{subject} is invalid")))
}

fn unsupported(value: char) -> bool {
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
