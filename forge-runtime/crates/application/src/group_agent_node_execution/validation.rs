use std::collections::BTreeSet;

use crate::runtime_domain::{
    AdmitGroupAgentNodeExecutionContract, AdmitGroupAgentNodeExecutionContractDisposition,
    AdmitGroupAgentNodeExecutionContractResult, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
    GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION, GroupAgentGraphCorePlan,
    GroupAgentGraphInspection, GroupAgentGraphManifest, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractInspection, GroupAgentNodeExecutionContractRecord,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES,
    MAX_GROUP_AGENT_NODE_EXECUTION_CONTRACT_LIST_LIMIT,
};

use super::{
    AdmitGroupAgentNodeExecutionContractInput, ExportGroupAgentGraphControl,
    GroupAgentNodeExecutionContractServiceError,
    error::{corrupt, invalid},
    snapshot,
};

pub(super) fn parse_admit_input(
    input: &AdmitGroupAgentNodeExecutionContractInput,
) -> Result<GroupAgentNodeExecutionContract, GroupAgentNodeExecutionContractServiceError> {
    validate_identifier(&input.graph_run_id, "Graph Run ID")?;
    validate_text(
        &input.idempotency_key,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        "idempotency key",
    )?;
    if i64::try_from(input.admitted_at_ms).is_err()
        || !(1..=MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES)
            .contains(&input.contract_json.len())
    {
        return Err(invalid("contract byte bound or admission time is invalid"));
    }
    let contract: GroupAgentNodeExecutionContract = serde_json::from_str(&input.contract_json)
        .map_err(|_| invalid("contract JSON is malformed or has unknown fields"))?;
    contract
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    if contract.graph_run_id != input.graph_run_id
        || contract.canonical_json().as_deref() != Ok(input.contract_json.as_str())
    {
        return Err(invalid(
            "contract Graph Run binding or canonical bytes disagree",
        ));
    }
    Ok(contract)
}

pub(super) fn admission_request(
    input: &AdmitGroupAgentNodeExecutionContractInput,
    contract: GroupAgentNodeExecutionContract,
    control: ExportGroupAgentGraphControl,
) -> Result<AdmitGroupAgentNodeExecutionContract, GroupAgentNodeExecutionContractServiceError> {
    let event = admission_event(input, &contract);
    let event_json = event
        .canonical_json()
        .map_err(|error| invalid(&error.to_string()))?;
    let request = AdmitGroupAgentNodeExecutionContract {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        control_snapshot: control.snapshot,
        control_snapshot_json: control.snapshot_json,
        contract,
        contract_json: input.contract_json.clone(),
        event,
        event_json,
        idempotency_key: input.idempotency_key.clone(),
        admitted_at_ms: input.admitted_at_ms,
    };
    request
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    Ok(request)
}

fn admission_event(
    input: &AdmitGroupAgentNodeExecutionContractInput,
    contract: &GroupAgentNodeExecutionContract,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
        graph_run_id: input.graph_run_id.clone(),
        seq: 2,
        kind: GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
            previous_event_sha256: contract.expected_last_event_sha256.clone(),
            control_snapshot_sha256: contract.control_snapshot_sha256.clone(),
            contract_id: contract.contract_id.clone(),
            contract_sha256: contract.contract_sha256.clone(),
            contract_bytes: input.contract_json.len(),
            node_id: contract.node.node_id.clone(),
            attempt: contract.node.attempt,
            request_sha256: contract.request.request_sha256.clone(),
            project_lane_sha256: contract.node.project_lane_sha256.clone(),
            admitted_at_ms: input.admitted_at_ms,
        },
    }
}

pub(super) fn checked_run(
    inspection: GroupAgentGraphRunInspection,
) -> Result<GroupAgentGraphRunInspection, GroupAgentNodeExecutionContractServiceError> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    let plan: GroupAgentGraphCorePlan = serde_json::from_str(&inspection.plan_json)
        .map_err(|_| corrupt("stored Core Plan JSON cannot be decoded"))?;
    let events = inspection
        .event_jsons
        .iter()
        .map(|json| serde_json::from_str::<GroupAgentGraphRunEvent>(json))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| corrupt("stored Graph Run event JSON cannot be decoded"))?;
    if plan != inspection.plan || events != inspection.events {
        return Err(corrupt(
            "stored Graph Run decoded values disagree with exact bytes",
        ));
    }
    Ok(inspection)
}

pub(super) fn checked_graph(
    inspection: GroupAgentGraphInspection,
) -> Result<GroupAgentGraphInspection, GroupAgentNodeExecutionContractServiceError> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    let manifest: GroupAgentGraphManifest = serde_json::from_str(&inspection.manifest_json)
        .map_err(|_| corrupt("stored Graph manifest JSON cannot be decoded"))?;
    if manifest != inspection.manifest {
        return Err(corrupt(
            "stored Graph manifest value disagrees with exact bytes",
        ));
    }
    Ok(inspection)
}

pub(super) fn validate_admit_result(
    request: &AdmitGroupAgentNodeExecutionContract,
    result: AdmitGroupAgentNodeExecutionContractResult,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, GroupAgentNodeExecutionContractServiceError>
{
    if result.v != GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION {
        return Err(corrupt("store returned an unsupported admission version"));
    }
    let inspection = checked_inspection(result.inspection)?;
    if inspection.contract != request.contract
        || inspection.contract_json != request.contract_json
        || inspection.record.graph_run_id != request.graph_run_id
    {
        return Err(corrupt("store returned different contract semantics"));
    }
    validate_created_result(request, result.disposition, &inspection)?;
    Ok(AdmitGroupAgentNodeExecutionContractResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

fn validate_created_result(
    request: &AdmitGroupAgentNodeExecutionContract,
    disposition: AdmitGroupAgentNodeExecutionContractDisposition,
    inspection: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    let matches = disposition != AdmitGroupAgentNodeExecutionContractDisposition::Created
        || (inspection.record.created_at_ms == request.admitted_at_ms
            && inspection.admission_event == request.event
            && inspection.admission_event_json == request.event_json);
    matches
        .then_some(())
        .ok_or_else(|| corrupt("created admission does not match its exact candidate event"))
}

pub(super) fn checked_inspection(
    inspection: GroupAgentNodeExecutionContractInspection,
) -> Result<GroupAgentNodeExecutionContractInspection, GroupAgentNodeExecutionContractServiceError>
{
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_list_input(
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    if !(1..=MAX_GROUP_AGENT_NODE_EXECUTION_CONTRACT_LIST_LIMIT).contains(&limit) {
        return Err(invalid("contract list limit is outside its bounds"));
    }
    if let Some(id) = graph_run_id {
        validate_identifier(id, "Graph Run ID")?;
    }
    Ok(())
}

pub(super) fn validate_list(
    records: &[GroupAgentNodeExecutionContractRecord],
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    if records.len() > limit {
        return Err(corrupt(
            "store returned more contract records than requested",
        ));
    }
    let mut contracts = BTreeSet::new();
    let mut runs = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|error| corrupt(&error.to_string()))?;
        if graph_run_id.is_some_and(|id| id != record.graph_run_id)
            || !contracts.insert(record.contract_id.as_str())
            || !runs.insert(record.graph_run_id.as_str())
        {
            return Err(corrupt(
                "store returned unfiltered or duplicate contract metadata",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_identifier(
    value: &str,
    subject: &str,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    crate::group_agent_graph_validation::validate_identifier(value)
        .map_err(|_| invalid(&format!("{subject} is invalid")))
}

fn validate_text(
    value: &str,
    maximum: usize,
    subject: &str,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
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

pub(super) fn validate_inspection_source_and_control(
    inspection: &GroupAgentNodeExecutionContractInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), GroupAgentNodeExecutionContractServiceError> {
    snapshot::validate_source_binding(&inspection.graph_run, graph)?;
    let control = snapshot::for_admission(&inspection.graph_run, graph)?;
    inspection
        .contract
        .validate_against_control(&control.snapshot)
        .map_err(|error| corrupt(&error.to_string()))
}
