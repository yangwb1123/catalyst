use crate::runtime_domain::{
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeLifecycleStatus, ScheduledGraphControllerEventPayload as Payload,
    ScheduledGraphControllerHeader, ScheduledGraphControllerJournal,
    ScheduledGraphNodeMaterializationInput, ScheduledGraphProgressNode,
    ScheduledGraphReconcileDisposition,
};
use crate::{
    AdmitGroupAgentScheduledNodeContractInput, AdmitGroupAgentScheduledNodeSuccessorInput,
    GroupAgentNodeExecutionContractService, GroupAgentScheduledNodeContractService,
    GroupAgentScheduledNodeProviderRequestService, GroupAgentScheduledNodeSuccessorService,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
};

use super::{
    ScheduledGraphControllerService, ScheduledGraphControllerServiceError, map_hub_error,
    source_validation::{validate_candidate_inspection, validate_materialized_candidate},
};

impl ScheduledGraphControllerService {
    pub(super) fn materialize_ready(
        &self,
        journal: &ScheduledGraphControllerJournal,
        observation: &crate::ScheduledGraphReconcileObservation,
        node: &ScheduledGraphProgressNode,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        if let Some(contract_id) = node.candidate_id.as_deref() {
            return self.observe_materialized_candidate(journal, node, contract_id, observed_at_ms);
        }
        if !matches!(journal.head().payload, Payload::MaterializePlanned { .. }) {
            let key = materialize_key(journal, node.execution_ordinal);
            return self.append_event(
                journal,
                Payload::MaterializePlanned {
                    execution_ordinal: node.execution_ordinal,
                    node_id: node.node_id.clone(),
                    snapshot_sha256: observation.snapshot.snapshot_sha256.clone(),
                    decision_sha256: observation.decision.decision_sha256.clone(),
                    idempotency_key: key,
                },
                observed_at_ms,
            );
        }
        let key = validate_materialize_plan(journal, node)?;
        validate_materialize_source(journal, observation)?;
        let call_at_ms = self.event_time(Some(journal), observed_at_ms);
        let contract_id =
            self.perform_materialization(journal, observation, node, &key, call_at_ms)?;
        self.append_event(
            journal,
            Payload::MaterializeObserved {
                execution_ordinal: node.execution_ordinal,
                node_id: node.node_id.clone(),
                contract_id,
            },
            observed_at_ms,
        )
    }

    pub(super) fn prepare_ready(
        &self,
        journal: &ScheduledGraphControllerJournal,
        _observation: &crate::ScheduledGraphReconcileObservation,
        node: &ScheduledGraphProgressNode,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        let contract_id = node
            .candidate_id
            .as_deref()
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
        self.validate_existing_contract(&journal.header, node, contract_id)?;
        if let Some(provider_request_id) = node.provider_request_id.as_deref() {
            return self.observe_existing_request(
                journal,
                node,
                contract_id,
                provider_request_id,
                observed_at_ms,
            );
        }
        if !matches!(journal.head().payload, Payload::PreparePlanned { .. }) {
            return self.plan_prepare(journal, node, contract_id, observed_at_ms);
        }
        let key = validate_prepare_plan(journal, node, contract_id)?;
        let call_at_ms = self.event_time(Some(journal), observed_at_ms);
        let result = self
            .provider_requests()
            .prepare(&PrepareGroupAgentScheduledNodeProviderRequestInput {
                scheduled_contract_id: contract_id.into(),
                idempotency_key: key,
                prepared_at_ms: call_at_ms,
            })
            .map_err(|_| ScheduledGraphControllerServiceError::PreparationFailed)?;
        let provider_request_id = result.inspection.record.provider_request_id;
        append_prepare_observed(
            self,
            journal,
            node,
            contract_id,
            &provider_request_id,
            observed_at_ms,
        )
    }

    fn observe_materialized_candidate(
        &self,
        journal: &ScheduledGraphControllerJournal,
        node: &ScheduledGraphProgressNode,
        contract_id: &str,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        validate_materialize_plan(journal, node)?;
        self.validate_existing_contract(&journal.header, node, contract_id)?;
        self.append_event(
            journal,
            Payload::MaterializeObserved {
                execution_ordinal: node.execution_ordinal,
                node_id: node.node_id.clone(),
                contract_id: contract_id.into(),
            },
            observed_at_ms,
        )
    }

    fn plan_prepare(
        &self,
        journal: &ScheduledGraphControllerJournal,
        node: &ScheduledGraphProgressNode,
        contract_id: &str,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        self.append_event(
            journal,
            Payload::PreparePlanned {
                execution_ordinal: node.execution_ordinal,
                node_id: node.node_id.clone(),
                contract_id: contract_id.into(),
                idempotency_key: prepare_key(journal, node.execution_ordinal),
            },
            observed_at_ms,
        )
    }

    fn observe_existing_request(
        &self,
        journal: &ScheduledGraphControllerJournal,
        node: &ScheduledGraphProgressNode,
        contract_id: &str,
        provider_request_id: &str,
        observed_at_ms: u64,
    ) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
        validate_prepare_plan(journal, node, contract_id)?;
        self.validate_existing_request(&journal.header, node, contract_id, provider_request_id)?;
        append_prepare_observed(
            self,
            journal,
            node,
            contract_id,
            provider_request_id,
            observed_at_ms,
        )
    }

    fn perform_materialization(
        &self,
        journal: &ScheduledGraphControllerJournal,
        observation: &crate::ScheduledGraphReconcileObservation,
        node: &ScheduledGraphProgressNode,
        idempotency_key: &str,
        observed_at_ms: u64,
    ) -> Result<String, ScheduledGraphControllerServiceError> {
        let control = GroupAgentNodeExecutionContractService::new(
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
        )
        .export_control(&journal.header.graph_run_id)
        .map_err(|_| ScheduledGraphControllerServiceError::MaterializationFailed)?;
        let receipts = self.completed_prefix_receipts(observation, node.execution_ordinal)?;
        let output = self
            .materializer
            .materialize(&ScheduledGraphNodeMaterializationInput {
                control_snapshot: control.snapshot,
                schedule_sha256: journal.header.schedule_sha256.clone(),
                execution_ordinal: node.execution_ordinal,
                node_id: node.node_id.clone(),
                predecessor_receipts: receipts,
                execution_profile: journal.header.execution_profile.clone(),
            })
            .map_err(|_| ScheduledGraphControllerServiceError::MaterializationFailed)?;
        validate_materialized_candidate(&journal.header, node, &output.candidate)?;
        self.admit_materialized(node, output.candidate_json, idempotency_key, observed_at_ms)
    }

    fn admit_materialized(
        &self,
        node: &ScheduledGraphProgressNode,
        candidate_json: String,
        idempotency_key: &str,
        observed_at_ms: u64,
    ) -> Result<String, ScheduledGraphControllerServiceError> {
        let graph_run_id = candidate_graph_run_id(&candidate_json)?;
        let inspection = if node.execution_ordinal == 0 {
            self.initial_contracts()
                .admit(&AdmitGroupAgentScheduledNodeContractInput {
                    graph_run_id,
                    contract_json: candidate_json,
                    idempotency_key: idempotency_key.into(),
                    admitted_at_ms: observed_at_ms,
                })
                .map(|value| value.inspection)
        } else {
            self.successor_contracts()
                .admit(&AdmitGroupAgentScheduledNodeSuccessorInput {
                    graph_run_id,
                    contract_json: candidate_json,
                    idempotency_key: idempotency_key.into(),
                    admitted_at_ms: observed_at_ms,
                    predecessor_content: None,
                })
                .map(|value| value.inspection)
        }
        .map_err(|_| ScheduledGraphControllerServiceError::AdmissionFailed)?;
        Ok(inspection.record.contract_id)
    }

    fn completed_prefix_receipts(
        &self,
        observation: &crate::ScheduledGraphReconcileObservation,
        ordinal: usize,
    ) -> Result<
        Vec<crate::runtime_domain::GroupAgentScheduledNodeTerminalReceipt>,
        ScheduledGraphControllerServiceError,
    > {
        observation.snapshot.nodes[..ordinal]
            .iter()
            .map(|node| self.completed_receipt(node))
            .collect()
    }

    fn completed_receipt(
        &self,
        node: &ScheduledGraphProgressNode,
    ) -> Result<
        crate::runtime_domain::GroupAgentScheduledNodeTerminalReceipt,
        ScheduledGraphControllerServiceError,
    > {
        let provider_request_id = node
            .provider_request_id
            .as_deref()
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
        let lifecycle = self
            .hub
            .inspect_group_agent_scheduled_node_lifecycle_any_family(provider_request_id)
            .map_err(|error| map_hub_error(&error))?;
        let receipt = lifecycle
            .terminal_receipt()
            .filter(|receipt| {
                lifecycle.status() == GroupAgentScheduledNodeLifecycleStatus::Terminalized
                    && receipt.node_outcome == GroupAgentNodeTerminalOutcome::Completed
                    && receipt.node_id == node.node_id
                    && receipt.provider_request_id == provider_request_id
            })
            .cloned()
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
        receipt
            .validate()
            .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)?;
        Ok(receipt)
    }

    fn validate_existing_contract(
        &self,
        header: &ScheduledGraphControllerHeader,
        node: &ScheduledGraphProgressNode,
        contract_id: &str,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        let inspection = if node.execution_ordinal == 0 {
            self.initial_contracts().inspect(contract_id)
        } else {
            self.successor_contracts().inspect(contract_id)
        }
        .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)?;
        validate_candidate_inspection(header, node, &inspection)
    }

    fn validate_existing_request(
        &self,
        header: &ScheduledGraphControllerHeader,
        node: &ScheduledGraphProgressNode,
        contract_id: &str,
        provider_request_id: &str,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        let inspection = self
            .provider_requests()
            .inspect(provider_request_id)
            .map_err(|_| ScheduledGraphControllerServiceError::CorruptEvidence)?;
        validate_candidate_inspection(header, node, &inspection.scheduled_contract)?;
        let record = &inspection.record;
        let valid = record.scheduled_contract_id == contract_id
            && record.execution_ordinal == node.execution_ordinal
            && record.node_id == node.node_id;
        valid
            .then_some(())
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
    }

    pub(super) fn validate_start_source(
        &self,
        header: &ScheduledGraphControllerHeader,
        observation: &crate::ScheduledGraphReconcileObservation,
    ) -> Result<(), ScheduledGraphControllerServiceError> {
        if observation.decision.disposition != ScheduledGraphReconcileDisposition::Ready {
            return Ok(());
        }
        let ordinal = observation
            .decision
            .next_execution_ordinal
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
        let node = observation
            .snapshot
            .nodes
            .get(ordinal)
            .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)?;
        if let Some(contract_id) = node.candidate_id.as_deref() {
            self.validate_existing_contract(header, node, contract_id)?;
            if let Some(provider_request_id) = node.provider_request_id.as_deref() {
                self.validate_existing_request(header, node, contract_id, provider_request_id)?;
            }
        }
        Ok(())
    }

    fn initial_contracts(&self) -> GroupAgentScheduledNodeContractService {
        GroupAgentScheduledNodeContractService::new(
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
        )
    }

    fn successor_contracts(&self) -> GroupAgentScheduledNodeSuccessorService {
        GroupAgentScheduledNodeSuccessorService::new_with_any_lifecycles(
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
        )
    }

    fn provider_requests(&self) -> GroupAgentScheduledNodeProviderRequestService {
        GroupAgentScheduledNodeProviderRequestService::new_with_any_lifecycle_successors(
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
            self.codec.clone(),
        )
    }
}

fn validate_materialize_plan(
    journal: &ScheduledGraphControllerJournal,
    node: &ScheduledGraphProgressNode,
) -> Result<String, ScheduledGraphControllerServiceError> {
    let Payload::MaterializePlanned {
        execution_ordinal,
        node_id,
        idempotency_key,
        ..
    } = &journal.head().payload
    else {
        return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
    };
    let valid = *execution_ordinal == node.execution_ordinal
        && node_id == &node.node_id
        && idempotency_key == &materialize_key(journal, node.execution_ordinal);
    valid
        .then(|| idempotency_key.clone())
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn validate_materialize_source(
    journal: &ScheduledGraphControllerJournal,
    observation: &crate::ScheduledGraphReconcileObservation,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let Payload::MaterializePlanned {
        snapshot_sha256,
        decision_sha256,
        ..
    } = &journal.head().payload
    else {
        return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
    };
    (snapshot_sha256 == &observation.snapshot.snapshot_sha256
        && decision_sha256 == &observation.decision.decision_sha256)
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn validate_prepare_plan(
    journal: &ScheduledGraphControllerJournal,
    node: &ScheduledGraphProgressNode,
    contract_id: &str,
) -> Result<String, ScheduledGraphControllerServiceError> {
    let Payload::PreparePlanned {
        execution_ordinal,
        node_id,
        contract_id: planned_contract_id,
        idempotency_key,
    } = &journal.head().payload
    else {
        return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
    };
    (*execution_ordinal == node.execution_ordinal
        && node_id == &node.node_id
        && planned_contract_id == contract_id
        && idempotency_key == &prepare_key(journal, node.execution_ordinal))
        .then(|| idempotency_key.clone())
        .ok_or(ScheduledGraphControllerServiceError::CorruptEvidence)
}

fn append_prepare_observed(
    service: &ScheduledGraphControllerService,
    journal: &ScheduledGraphControllerJournal,
    node: &ScheduledGraphProgressNode,
    contract_id: &str,
    provider_request_id: &str,
    observed_at_ms: u64,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    service.append_event(
        journal,
        Payload::PrepareObserved {
            execution_ordinal: node.execution_ordinal,
            node_id: node.node_id.clone(),
            contract_id: contract_id.into(),
            provider_request_id: provider_request_id.into(),
        },
        observed_at_ms,
    )
}

fn materialize_key(journal: &ScheduledGraphControllerJournal, ordinal: usize) -> String {
    format!(
        "controller-{}-materialize-{ordinal}",
        journal.header.controller_sha256
    )
}

fn prepare_key(journal: &ScheduledGraphControllerJournal, ordinal: usize) -> String {
    format!(
        "controller-{}-prepare-{ordinal}",
        journal.header.controller_sha256
    )
}

fn candidate_graph_run_id(
    candidate_json: &str,
) -> Result<String, ScheduledGraphControllerServiceError> {
    GroupAgentScheduledNodeContractCandidate::decode_exact(candidate_json)
        .map(|candidate| candidate.graph_run_id)
        .map_err(|_| ScheduledGraphControllerServiceError::MaterializationFailed)
}
