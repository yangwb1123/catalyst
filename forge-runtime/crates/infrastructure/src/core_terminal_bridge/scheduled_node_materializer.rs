use std::{path::PathBuf, time::Duration};

use crate::runtime_domain::{
    GroupAgentNodeProviderKind, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractScope, MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
    ScheduledGraphNodeMaterialization, ScheduledGraphNodeMaterializationInput,
    ScheduledGraphNodeMaterializationPort, ScheduledGraphNodeMaterializationPortError,
};

use super::{
    CORE_DECISION_TIMEOUT, CORE_PREFLIGHT_TIMEOUT, PinnedCoreTerminalBridge, invalid,
    validate_digest,
};

const MATERIALIZATION_PROTOCOL_VERSION: &[u8] = b"2";
const RECEIPT_FILE_PREFIX: &str = "predecessor-receipt-";

#[derive(Clone, Debug)]
pub struct PinnedScheduledNodeMaterializationBridge {
    inner: PinnedCoreTerminalBridge,
}

impl PinnedScheduledNodeMaterializationBridge {
    /// Validates and handshakes one explicitly pinned candidate materializer.
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, super::CoreTerminalBridgeError> {
        validate_digest(&sha256)?;
        let inner = PinnedCoreTerminalBridge { path, sha256 };
        let output = inner.invoke(
            &["graph-scheduled-node-contract", "--protocol-version"],
            b"",
            CORE_PREFLIGHT_TIMEOUT,
        )?;
        if output != MATERIALIZATION_PROTOCOL_VERSION {
            return Err(invalid("Core scheduled materialization handshake failed"));
        }
        Ok(Self { inner })
    }

    fn invoke(
        &self,
        input: &ScheduledGraphNodeMaterializationInput,
    ) -> Result<Vec<u8>, ScheduledGraphNodeMaterializationPortError> {
        validate_input(input)?;
        let directory = tempfile::tempdir()
            .map_err(|_| ScheduledGraphNodeMaterializationPortError::Unavailable)?;
        let receipt_paths = write_receipts(directory.path(), input)?;
        let arguments = arguments(input, &receipt_paths);
        let argument_refs = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        let control = input
            .control_snapshot
            .canonical_json()
            .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)?;
        self.inner
            .invoke_with_stdout_limit(
                &argument_refs,
                control.as_bytes(),
                materialization_timeout(input),
                MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
            )
            .map_err(|_| ScheduledGraphNodeMaterializationPortError::Unavailable)
    }
}

impl ScheduledGraphNodeMaterializationPort for PinnedScheduledNodeMaterializationBridge {
    fn materialize(
        &self,
        input: &ScheduledGraphNodeMaterializationInput,
    ) -> Result<ScheduledGraphNodeMaterialization, ScheduledGraphNodeMaterializationPortError> {
        let output = self.invoke(input)?;
        let candidate = GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(&output)
            .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)?;
        validate_candidate(input, &candidate)?;
        let candidate_json = String::from_utf8(output)
            .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)?;
        Ok(ScheduledGraphNodeMaterialization {
            candidate,
            candidate_json,
        })
    }
}

fn validate_input(
    input: &ScheduledGraphNodeMaterializationInput,
) -> Result<(), ScheduledGraphNodeMaterializationPortError> {
    input
        .control_snapshot
        .validate()
        .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)?;
    input
        .execution_profile
        .validate()
        .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)?;
    let valid = input.schedule_sha256.len() == 64
        && input.schedule_sha256.bytes().all(is_lower_hex)
        && !input.node_id.trim().is_empty()
        && input.execution_ordinal < input.control_snapshot.plan.authored_node_ids.len()
        && input
            .control_snapshot
            .plan
            .authored_node_ids
            .iter()
            .any(|node_id| node_id == &input.node_id)
        && input.predecessor_receipts.len() <= input.execution_ordinal;
    if !valid {
        return Err(ScheduledGraphNodeMaterializationPortError::InvalidCandidate);
    }
    input.predecessor_receipts.iter().try_for_each(|receipt| {
        receipt
            .validate()
            .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)
    })
}

fn validate_candidate(
    input: &ScheduledGraphNodeMaterializationInput,
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), ScheduledGraphNodeMaterializationPortError> {
    let profile = &input.execution_profile;
    let expected_scope = if input.execution_ordinal == 0 {
        GroupAgentScheduledNodeContractScope::ScheduleInitialNodeOnly
    } else {
        GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly
    };
    let valid = candidate.graph_run_id == input.control_snapshot.graph_run_id
        && candidate.schedule_sha256 == input.schedule_sha256
        && candidate.node.execution_ordinal == input.execution_ordinal
        && candidate.node.node_id == input.node_id
        && candidate.contract_scope == expected_scope
        && candidate.provider.kind == GroupAgentNodeProviderKind::OpenAiResponses
        && candidate.provider.endpoint == profile.endpoint
        && candidate.provider.model == profile.model
        && u64::from(candidate.budgets.max_output_tokens) == profile.max_output_tokens
        && u64::try_from(candidate.budgets.max_model_output_bytes).ok()
            == Some(profile.max_model_output_bytes)
        && u64::from(candidate.budgets.max_model_events) == profile.max_model_events
        && candidate.budgets.timeout_ms == profile.timeout_ms
        && candidate.budgets.max_cost_usd_micros == profile.max_cost_usd_micros
        && candidate.budgets.pricing_snapshot_sha256 == profile.pricing_snapshot_sha256
        && u64::try_from(candidate.result.max_result_bytes).ok() == Some(profile.max_result_bytes);
    valid
        .then_some(())
        .ok_or(ScheduledGraphNodeMaterializationPortError::InvalidCandidate)
}

fn arguments(input: &ScheduledGraphNodeMaterializationInput, receipts: &[PathBuf]) -> Vec<String> {
    let profile = &input.execution_profile;
    let mut values = vec![
        "graph-scheduled-node-contract".into(),
        "--control".into(),
        "-".into(),
        "--schedule-sha256".into(),
        input.schedule_sha256.clone(),
        "--endpoint".into(),
        profile.endpoint.clone(),
        "--model".into(),
        profile.model.clone(),
    ];
    push_budget_arguments(&mut values, profile);
    if input.execution_ordinal > 0 {
        values.extend(["--target-node".into(), input.node_id.clone()]);
    }
    for receipt in receipts {
        values.extend([
            "--predecessor-receipt".into(),
            receipt.to_string_lossy().into_owned(),
        ]);
    }
    values
}

fn push_budget_arguments(
    values: &mut Vec<String>,
    profile: &crate::runtime_domain::ScheduledGraphControllerExecutionProfile,
) {
    for (flag, value) in [
        ("--max-output-tokens", profile.max_output_tokens),
        ("--max-model-output-bytes", profile.max_model_output_bytes),
        ("--max-model-events", profile.max_model_events),
        ("--timeout-ms", profile.timeout_ms),
        ("--max-cost-usd-micros", profile.max_cost_usd_micros),
        ("--max-result-bytes", profile.max_result_bytes),
    ] {
        values.extend([flag.into(), value.to_string()]);
    }
    values.extend([
        "--pricing-snapshot-sha256".into(),
        profile.pricing_snapshot_sha256.clone(),
    ]);
}

fn write_receipts(
    directory: &std::path::Path,
    input: &ScheduledGraphNodeMaterializationInput,
) -> Result<Vec<PathBuf>, ScheduledGraphNodeMaterializationPortError> {
    input
        .predecessor_receipts
        .iter()
        .enumerate()
        .map(|(index, receipt)| {
            let path = directory.join(format!("{RECEIPT_FILE_PREFIX}{index}.json"));
            let encoded = receipt
                .canonical_json()
                .map_err(|_| ScheduledGraphNodeMaterializationPortError::InvalidCandidate)?;
            std::fs::write(&path, encoded)
                .map_err(|_| ScheduledGraphNodeMaterializationPortError::Unavailable)?;
            Ok(path)
        })
        .collect()
}

fn materialization_timeout(input: &ScheduledGraphNodeMaterializationInput) -> Duration {
    CORE_DECISION_TIMEOUT.max(Duration::from_millis(
        input.execution_profile.timeout_ms.min(60_000),
    ))
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}
