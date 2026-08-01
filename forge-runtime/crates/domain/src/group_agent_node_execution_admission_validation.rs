use super::validation::{invalid, is_digest, valid_identifier, valid_text};
use super::{
    AdmitGroupAgentNodeExecutionContract, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
    GroupAgentGraphControlSnapshot, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractInspection, GroupAgentNodeExecutionContractRecord,
    GroupAgentNodeExecutionValidationError, MAX_GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES, group_agent_node_system_prompt,
    group_agent_node_user_prompt,
};
use crate::{
    GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION, GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind,
    GroupAgentGraphRunStatus, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
};

pub(super) fn validate_record(
    record: &GroupAgentNodeExecutionContractRecord,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = record.v == GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION
        && valid_identifier(&record.contract_id)
        && valid_identifier(&record.graph_run_id)
        && valid_identifier(&record.node_id)
        && record.attempt == 1
        && is_digest(&record.control_snapshot_sha256)
        && is_digest(&record.contract_sha256)
        && record.contract_id == format!("node-contract-{}", record.contract_sha256)
        && (1..=MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES)
            .contains(&record.contract_bytes)
        && is_digest(&record.request_sha256)
        && is_digest(&record.project_lane_sha256)
        && record.expected_last_event_seq == 1
        && is_digest(&record.expected_last_event_sha256)
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid admitted Node Execution Contract record"))
}

pub(super) fn validate_admission(
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    validate_against_control(&request.contract, &request.control_snapshot)?;
    request
        .event
        .validate()
        .map_err(|error| invalid(&error.message))?;
    validate_admission_header(request)?;
    validate_exact_snapshot(&request.control_snapshot, &request.control_snapshot_json)?;
    validate_exact_contract(&request.contract, &request.contract_json)?;
    validate_exact_event(&request.event, &request.event_json)?;
    validate_admission_event(request)
}

pub(super) fn validate_against_control(
    contract: &GroupAgentNodeExecutionContract,
    snapshot: &GroupAgentGraphControlSnapshot,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    snapshot.validate()?;
    contract.validate()?;
    validate_control_contract(snapshot, contract)?;
    validate_selected_node(snapshot, contract)
}

fn validate_admission_header(
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = request.v == GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION
        && valid_identifier(&request.graph_run_id)
        && request.graph_run_id == request.control_snapshot.graph_run_id
        && request.graph_run_id == request.contract.graph_run_id
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.admitted_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract admission envelope"))
}

fn validate_control_contract(
    snapshot: &GroupAgentGraphControlSnapshot,
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = contract.graph_run_id == snapshot.graph_run_id
        && contract.graph_id == snapshot.graph_id
        && contract.source_snapshot_sha256 == snapshot.source_snapshot_sha256
        && contract.graph_manifest_sha256 == snapshot.graph_manifest_sha256
        && contract.core_plan_sha256 == snapshot.core_plan_sha256
        && contract.control_snapshot_sha256 == snapshot.snapshot_sha256
        && contract.expected_last_event_seq == snapshot.last_event_seq
        && contract.expected_last_event_sha256 == snapshot.last_event_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Node Execution Contract control bindings disagree"))
}

fn validate_selected_node(
    snapshot: &GroupAgentGraphControlSnapshot,
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let selected = snapshot
        .plan
        .waves
        .first()
        .and_then(|wave| wave.first())
        .ok_or_else(|| invalid("control snapshot has no first wave node"))?;
    let index = snapshot
        .manifest
        .nodes
        .iter()
        .position(|node| node.node_id == *selected)
        .ok_or_else(|| invalid("selected plan node is absent from the manifest"))?;
    let source = &snapshot.manifest.nodes[index];
    let node = &contract.node;
    let prompts_match = contract.request.system_prompt
        == group_agent_node_system_prompt(&snapshot.manifest.manager.instruction)
        && contract.request.user_prompt
            == group_agent_node_user_prompt(&source.node_id, &source.task, &source.acceptance)?;
    let valid = node.node_id == source.node_id
        && node.authored_node_index == index
        && node.topology_wave_index == 0
        && node.project_id == source.project_id
        && node.member_role == source.member_role
        && node.agent_profile == source.agent_profile
        && prompts_match;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Node Execution Contract did not select the exact first node"))
}

fn validate_admission_event(
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
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
    } = &request.event.kind
    else {
        return Err(invalid("admission requires a contract-admitted event"));
    };
    let contract = &request.contract;
    let valid = request.event.graph_run_id == request.graph_run_id
        && previous_event_sha256 == &contract.expected_last_event_sha256
        && control_snapshot_sha256 == &contract.control_snapshot_sha256
        && contract_id == &contract.contract_id
        && contract_sha256 == &contract.contract_sha256
        && *contract_bytes == request.contract_json.len()
        && node_id == &contract.node.node_id
        && *attempt == contract.node.attempt
        && request_sha256 == &contract.request.request_sha256
        && project_lane_sha256 == &contract.node.project_lane_sha256
        && *admitted_at_ms == request.admitted_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Node Execution Contract admission event bindings disagree"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    inspection.record.validate()?;
    inspection.contract.validate()?;
    inspection
        .admission_event
        .validate()
        .map_err(|error| invalid(&error.message))?;
    inspection
        .graph_run
        .validate()
        .map_err(|error| invalid(&error.message))?;
    validate_exact_contract(&inspection.contract, &inspection.contract_json)?;
    validate_exact_event(
        &inspection.admission_event,
        &inspection.admission_event_json,
    )?;
    validate_inspection_record(inspection)?;
    validate_inspection_event(inspection)?;
    validate_inspection_run(inspection)
}

fn validate_inspection_record(
    inspection: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let record = &inspection.record;
    let contract = &inspection.contract;
    let valid = inspection.v == GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION
        && record.contract_id == contract.contract_id
        && record.graph_run_id == contract.graph_run_id
        && record.node_id == contract.node.node_id
        && record.attempt == contract.node.attempt
        && record.control_snapshot_sha256 == contract.control_snapshot_sha256
        && record.contract_sha256 == contract.contract_sha256
        && record.contract_bytes == inspection.contract_json.len()
        && record.request_sha256 == contract.request.request_sha256
        && record.project_lane_sha256 == contract.node.project_lane_sha256
        && record.expected_last_event_seq == contract.expected_last_event_seq
        && record.expected_last_event_sha256 == contract.expected_last_event_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("contract inspection record bindings disagree"))
}

fn validate_inspection_event(
    inspection: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let record = &inspection.record;
    let event = &inspection.admission_event;
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
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
    } = &event.kind
    else {
        return Err(invalid("contract inspection has the wrong event kind"));
    };
    let valid = event.graph_run_id == record.graph_run_id
        && previous_event_sha256 == &record.expected_last_event_sha256
        && control_snapshot_sha256 == &record.control_snapshot_sha256
        && contract_id == &record.contract_id
        && contract_sha256 == &record.contract_sha256
        && *contract_bytes == record.contract_bytes
        && node_id == &record.node_id
        && *attempt == record.attempt
        && request_sha256 == &record.request_sha256
        && project_lane_sha256 == &record.project_lane_sha256
        && *admitted_at_ms == record.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("contract inspection event bindings disagree"))
}

fn validate_inspection_run(
    inspection: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let run = &inspection.graph_run;
    let contract = &inspection.contract;
    let valid = run.run.v == GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION
        && run.run.status == GroupAgentGraphRunStatus::AwaitingCoreDispatch
        && run.run.graph_run_id == contract.graph_run_id
        && run.run.graph_id == contract.graph_id
        && run.run.source_snapshot_sha256 == contract.source_snapshot_sha256
        && run.run.graph_manifest_sha256 == contract.graph_manifest_sha256
        && run.run.plan_sha256 == contract.core_plan_sha256
        && run.events.get(1) == Some(&inspection.admission_event)
        && run.event_jsons.get(1) == Some(&inspection.admission_event_json);
    valid
        .then_some(())
        .ok_or_else(|| invalid("contract inspection Graph Run bindings disagree"))
}

fn validate_exact_snapshot(
    snapshot: &GroupAgentGraphControlSnapshot,
    json: &str,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    exact_json(
        &snapshot.canonical_json()?,
        json,
        MAX_GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_BYTES,
        "control snapshot JSON is not its exact canonical encoding",
    )
}

fn validate_exact_contract(
    contract: &GroupAgentNodeExecutionContract,
    json: &str,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    exact_json(
        &contract.canonical_json()?,
        json,
        MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES,
        "contract JSON is not its exact canonical encoding",
    )
}

fn validate_exact_event(
    event: &GroupAgentGraphRunEvent,
    json: &str,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    exact_json(
        &event
            .canonical_json()
            .map_err(|error| invalid(&error.message))?,
        json,
        MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES,
        "admission event JSON is not its exact canonical encoding",
    )
}

fn exact_json(
    expected: &str,
    actual: &str,
    maximum: usize,
    message: &str,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid =
        !actual.is_empty() && actual.len() <= maximum && actual.as_bytes() == expected.as_bytes();
    valid.then_some(()).ok_or_else(|| invalid(message))
}
