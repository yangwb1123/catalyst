use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleStore,
    GroupAgentGraphRunInspection, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractRecord, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodeLifecycleStore, GroupAgentScheduledNodeSuccessorStore,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalArtifactKind,
};

use super::{
    AdmitGroupAgentScheduledNodeContractInput, AdmitGroupAgentScheduledNodeContractResult,
    GroupAgentScheduledNodeContractServiceError,
    scheduled_contract_error::{corrupt, invalid},
    scheduled_contract_validation::{
        admission_request, checked_graph, checked_inspection, checked_run, checked_schedule,
        parse_admit_input, receipt_binding_matches, validate_admit_result, validate_identifier,
        validate_list_input, validate_sources, validate_successor_list,
        validated_terminal_result_artifact, verify_content_presence,
    },
    snapshot,
};

/// Input for one effect-free successor-candidate admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentScheduledNodeSuccessorInput {
    pub graph_run_id: String,
    pub contract_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
    /// Exact predecessor result text the candidate's prompt embeds; required
    /// exactly when `predecessor_content_included` is true.
    pub predecessor_content: Option<String>,
}

/// Result of one predecessor terminal receipt export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportedPredecessorReceipt {
    pub provider_request_id: String,
    pub receipt_json: String,
    pub receipt_sha256: String,
}

/// Effect-free successor-candidate service. It consumes verified predecessor
/// terminal receipts as evidence, persists one immutable candidate, and never
/// dispatches, claims a lane, reads a credential, or advances a wave.
pub struct GroupAgentScheduledNodeSuccessorService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
    successors: Arc<dyn GroupAgentScheduledNodeSuccessorStore>,
    lifecycles: Arc<dyn GroupAgentScheduledNodeLifecycleStore>,
}

impl GroupAgentScheduledNodeSuccessorService {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
        successors: Arc<dyn GroupAgentScheduledNodeSuccessorStore>,
        lifecycles: Arc<dyn GroupAgentScheduledNodeLifecycleStore>,
    ) -> Self {
        Self {
            graphs,
            runs,
            schedules,
            successors,
            lifecycles,
        }
    }

    /// Validates every pure admission input before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid identifier, key,
    /// envelope, byte bound, time, or successor scope.
    pub fn preflight_admit(
        input: &AdmitGroupAgentScheduledNodeSuccessorInput,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        let candidate = parse_admit_input(&AdmitGroupAgentScheduledNodeContractInput {
            graph_run_id: input.graph_run_id.clone(),
            contract_json: input.contract_json.clone(),
            idempotency_key: input.idempotency_key.clone(),
            admitted_at_ms: input.admitted_at_ms,
        })?;
        if candidate.contract_scope != GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly {
            return Err(invalid(
                "successor admission requires a schedule_successor_only candidate",
            ));
        }
        verify_content_presence(&candidate, input.predecessor_content.as_deref())
    }

    /// Validates an inspection identifier before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid successor identifier.
    pub fn preflight_inspect(
        contract_id: &str,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        validate_identifier(contract_id, "successor contract ID")
    }

    /// Validates list filters and bounds before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid Run filter or limit.
    pub fn preflight_list(
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        validate_list_input(graph_run_id, limit)
    }

    /// Exports one exact predecessor terminal receipt from the durable v16
    /// scheduled lifecycle sidecar.
    ///
    /// # Errors
    ///
    /// Returns a structured input, not-found, or corruption error when the
    /// lifecycle is missing, not terminalized, or its receipt is not exact.
    pub fn export_predecessor_receipt(
        &self,
        provider_request_id: &str,
    ) -> Result<ExportedPredecessorReceipt, GroupAgentScheduledNodeContractServiceError> {
        validate_identifier(provider_request_id, "provider request ID")?;
        let inspection = self
            .lifecycles
            .inspect_group_agent_scheduled_node_lifecycle(provider_request_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let (receipt_json, receipt_sha256) = terminal_receipt_evidence(&inspection)?;
        Ok(ExportedPredecessorReceipt {
            provider_request_id: provider_request_id.to_owned(),
            receipt_json,
            receipt_sha256,
        })
    }

    /// Admits one passive successor candidate after verifying every consumed
    /// predecessor receipt against the durable terminal lifecycle.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, or storage
    /// error. A nonterminal predecessor or a receipt whose identities drift
    /// from the durable sidecar fails closed.
    pub fn admit(
        &self,
        input: &AdmitGroupAgentScheduledNodeSuccessorInput,
    ) -> Result<
        AdmitGroupAgentScheduledNodeContractResult,
        GroupAgentScheduledNodeContractServiceError,
    > {
        Self::preflight_admit(input)?;
        let candidate = parse_admit_input(&AdmitGroupAgentScheduledNodeContractInput {
            graph_run_id: input.graph_run_id.clone(),
            contract_json: input.contract_json.clone(),
            idempotency_key: input.idempotency_key.clone(),
            admitted_at_ms: input.admitted_at_ms,
        })?;
        self.verify_predecessor_evidence(&candidate)?;
        self.verify_predecessor_content(&candidate, input.predecessor_content.as_deref())?;
        let run = self.load_run(&input.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        let control = snapshot::export(&run, &graph)?;
        let schedule = self.load_schedule(&candidate.schedule_id)?;
        let request = admission_request(
            &AdmitGroupAgentScheduledNodeContractInput {
                graph_run_id: input.graph_run_id.clone(),
                contract_json: input.contract_json.clone(),
                idempotency_key: input.idempotency_key.clone(),
                admitted_at_ms: input.admitted_at_ms,
            },
            candidate,
            control,
            &schedule,
        )?;
        let result = self
            .successors
            .admit_group_agent_scheduled_node_successor(&request)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        validate_admit_result(&request, result)
    }

    /// Loads and fully revalidates one successor candidate against its source
    /// and schedule.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, or storage error.
    pub fn inspect(
        &self,
        contract_id: &str,
    ) -> Result<
        GroupAgentScheduledNodeContractInspection,
        GroupAgentScheduledNodeContractServiceError,
    > {
        validate_identifier(contract_id, "successor contract ID")?;
        let inspection = self
            .successors
            .inspect_group_agent_scheduled_node_successor(contract_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let inspection = checked_inspection(inspection)?;
        if inspection.record.contract_id != contract_id {
            return Err(corrupt(
                "store returned a different scheduled successor identity",
            ));
        }
        let run = self.load_run(&inspection.record.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        let control = snapshot::historical_base(&run, &graph)?;
        let schedule = self.load_schedule(&inspection.record.schedule_id)?;
        validate_sources(&inspection.candidate, &control, &schedule)?;
        self.verify_predecessor_evidence(&inspection.candidate)?;
        self.verify_stored_predecessor_content(&inspection.candidate)?;
        Ok(inspection)
    }

    /// Lists bounded successor metadata without Prompt or contract plaintext.
    ///
    /// # Errors
    ///
    /// Returns a structured input, corruption, or storage error.
    pub fn list(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<
        Vec<GroupAgentScheduledNodeContractRecord>,
        GroupAgentScheduledNodeContractServiceError,
    > {
        validate_list_input(graph_run_id, limit)?;
        let records = self
            .successors
            .list_group_agent_scheduled_node_successors(graph_run_id, limit)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        validate_successor_list(&records, graph_run_id, limit)?;
        Ok(records)
    }

    /// Verifies the disclosed predecessor content: the candidate's prompt
    /// must embed exactly the caller-supplied bytes, which must equal the
    /// durable result-class artifact text of the receipt's lifecycle.
    fn verify_predecessor_content(
        &self,
        candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
        supplied: Option<&str>,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        if !candidate.request.predecessor_content_included {
            if supplied.is_some() {
                return Err(invalid(
                    "predecessor content supplied for a candidate that excludes it",
                ));
            }
            return Ok(());
        }
        let supplied = supplied.ok_or_else(|| {
            invalid(
                "successor candidate embeds predecessor content; --predecessor-content is required",
            )
        })?;
        let embedded = crate::runtime_domain::group_agent_scheduled_node_predecessor_output(
            &candidate.request.user_prompt,
        )
        .map_err(|error| corrupt(&error.to_string()))?;
        match embedded.as_deref() {
            Some(value) if value == supplied => {}
            _ => {
                return Err(corrupt(
                    "candidate prompt predecessor output disagrees with supplied content",
                ));
            }
        }
        self.verify_durable_artifact_matches(candidate, supplied)
    }

    /// Re-extracts persisted predecessor content and binds it to the same
    /// durable first-direct result artifact checked during admission.
    fn verify_stored_predecessor_content(
        &self,
        candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        if !candidate.request.predecessor_content_included {
            return stored_predecessor_content_matches(candidate, None);
        }
        let receipt = candidate
            .request
            .predecessor_terminal_receipts
            .first()
            .ok_or_else(|| corrupt("successor candidate has no predecessor receipt"))?;
        let lifecycle = self
            .lifecycles
            .inspect_group_agent_scheduled_node_lifecycle(&receipt.provider_request_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let artifact = validated_terminal_result_artifact(&lifecycle)?;
        stored_predecessor_content_matches(candidate, Some(artifact))
    }

    /// The disclosed content must equal the durable result-class artifact
    /// text of the schedule's first direct predecessor receipt lifecycle.
    fn verify_durable_artifact_matches(
        &self,
        candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
        supplied: &str,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        let receipt = candidate
            .request
            .predecessor_terminal_receipts
            .first()
            .ok_or_else(|| corrupt("successor candidate has no predecessor receipt"))?;
        let lifecycle = self
            .lifecycles
            .inspect_group_agent_scheduled_node_lifecycle(&receipt.provider_request_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let artifact = validated_terminal_result_artifact(&lifecycle)?;
        result_artifact_text_matches(artifact, supplied)
    }

    /// Every consumed predecessor receipt must correspond to a durable
    /// terminalized lifecycle whose stored receipt identities match exactly.
    fn verify_predecessor_evidence(
        &self,
        candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        for receipt in &candidate.request.predecessor_terminal_receipts {
            let inspection = self
                .lifecycles
                .inspect_group_agent_scheduled_node_lifecycle(&receipt.provider_request_id)
                .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
            // Stage-02 Finding 2: the durable lifecycle must belong to the
            // SAME Graph Run as the candidate — a genuine receipt from
            // another run of the same graph must not satisfy this run's
            // predecessor set.
            if inspection.graph_run.run.graph_run_id != candidate.graph_run_id {
                return Err(invalid(
                    "predecessor lifecycle belongs to a different Graph Run",
                ));
            }
            verify_receipt_binding(&inspection, receipt)?;
        }
        Ok(())
    }

    fn load_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, GroupAgentScheduledNodeContractServiceError> {
        checked_run(
            self.runs
                .inspect_group_agent_graph_run(graph_run_id)
                .map_err(GroupAgentScheduledNodeContractServiceError::from)?,
        )
    }

    fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentGraphInspection,
        GroupAgentScheduledNodeContractServiceError,
    > {
        checked_graph(
            self.graphs
                .inspect_group_agent_graph(graph_id)
                .map_err(GroupAgentScheduledNodeContractServiceError::from)?,
        )
    }

    fn load_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<
        GroupAgentGraphExecutionScheduleInspection,
        GroupAgentScheduledNodeContractServiceError,
    > {
        checked_schedule(
            self.schedules
                .inspect_group_agent_graph_execution_schedule(schedule_id)
                .map_err(GroupAgentScheduledNodeContractServiceError::from)?,
        )
    }
}

pub(super) fn stored_predecessor_content_matches(
    candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
    artifact: Option<&GroupAgentScheduledNodeTerminalArtifact>,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    let embedded = crate::runtime_domain::group_agent_scheduled_node_predecessor_output(
        &candidate.request.user_prompt,
    )
    .map_err(|error| corrupt(&error.to_string()))?;
    if !candidate.request.predecessor_content_included {
        return embedded
            .is_none()
            .then_some(())
            .ok_or_else(|| corrupt("candidate prompt unexpectedly embeds predecessor output"));
    }
    let embedded = embedded.as_deref().ok_or_else(|| {
        corrupt("candidate declares predecessor content without embedding predecessor output")
    })?;
    let artifact = artifact
        .ok_or_else(|| corrupt("terminalized lifecycle has no artifact for predecessor content"))?;
    result_artifact_text_matches(artifact, embedded)
}

fn result_artifact_text_matches(
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
    supplied: &str,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    if artifact.artifact_kind != GroupAgentScheduledNodeTerminalArtifactKind::Result {
        return Err(invalid(
            "predecessor content can only be disclosed from a result-class artifact",
        ));
    }
    if artifact.output_text != supplied {
        return Err(corrupt(
            "supplied predecessor content disagrees with the durable artifact",
        ));
    }
    Ok(())
}

/// Extracts the exact canonical receipt JSON from a terminalized lifecycle.
fn terminal_receipt_evidence(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(String, String), GroupAgentScheduledNodeContractServiceError> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    if inspection.status != GroupAgentScheduledNodeLifecycleStatus::Terminalized {
        return Err(invalid("predecessor lifecycle is not terminalized"));
    }
    let receipt = inspection
        .terminal_receipt
        .as_ref()
        .ok_or_else(|| corrupt("terminalized lifecycle has no persisted receipt"))?;
    if receipt.node_outcome != GroupAgentNodeTerminalOutcome::Completed {
        return Err(invalid(
            "only a completed predecessor lifecycle can advance a successor",
        ));
    }
    let receipt_json = inspection
        .terminal_receipt_json
        .as_ref()
        .ok_or_else(|| corrupt("terminalized lifecycle has no receipt JSON"))?;
    Ok((receipt_json.clone(), receipt.receipt_sha256.clone()))
}

/// Verifies one consumed receipt against the durable terminal lifecycle:
/// every candidate identity must equal the persisted evidence exactly.
fn verify_receipt_binding(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
    receipt: &crate::runtime_domain::GroupAgentScheduledNodePredecessorReceipt,
) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    if inspection.status != GroupAgentScheduledNodeLifecycleStatus::Terminalized {
        return Err(invalid("predecessor lifecycle is not terminalized"));
    }
    let stored = inspection
        .terminal_receipt
        .as_ref()
        .ok_or_else(|| corrupt("terminalized lifecycle has no persisted receipt"))?;
    // Stage-02 Finding 2: predecessor evidence must belong to the SAME
    // Graph Run — a genuine receipt from another run of the same graph must
    // not satisfy this candidate's predecessor set.
    let valid = receipt_binding_matches(stored, receipt);
    valid
        .then_some(())
        .ok_or_else(|| corrupt("predecessor receipt disagrees with durable lifecycle"))
}
