use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeLifecycleStore, GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeProviderRequestRecord, GroupAgentScheduledNodeProviderRequestStore,
    GroupAgentScheduledNodeSuccessorStore,
};

use super::{
    GroupAgentNodeDispatchRequestCodec, GroupAgentScheduledNodeContractService,
    GroupAgentScheduledNodeProviderRequestServiceError, GroupAgentScheduledNodeSuccessorService,
    PrepareGroupAgentScheduledNodeProviderRequestResult,
    scheduled_provider_request_error::{codec_input_error, corrupt},
    scheduled_provider_request_validation::{
        checked_inspection, model_request, prepare_request, validate_input, validate_list,
        validate_list_input, validate_pristine_run, validate_result,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentScheduledNodeProviderRequestInput {
    pub scheduled_contract_id: String,
    pub idempotency_key: String,
    pub prepared_at_ms: u64,
}

pub struct GroupAgentScheduledNodeProviderRequestService {
    runs: Arc<dyn GroupAgentGraphRunStore>,
    scheduled_contracts: GroupAgentScheduledNodeContractService,
    successor_contracts: Option<GroupAgentScheduledNodeSuccessorService>,
    provider_requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
}

impl GroupAgentScheduledNodeProviderRequestService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
        scheduled_contracts: Arc<dyn GroupAgentScheduledNodeContractStore>,
        provider_requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
    ) -> Self {
        Self {
            runs: Arc::clone(&runs),
            scheduled_contracts: GroupAgentScheduledNodeContractService::new(
                graphs,
                runs,
                schedules,
                scheduled_contracts,
            ),
            successor_contracts: None,
            provider_requests,
            codec,
        }
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_successors(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
        scheduled_contracts: Arc<dyn GroupAgentScheduledNodeContractStore>,
        successor_contracts: Arc<dyn GroupAgentScheduledNodeSuccessorStore>,
        lifecycles: Arc<dyn GroupAgentScheduledNodeLifecycleStore>,
        provider_requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
    ) -> Self {
        Self {
            runs: Arc::clone(&runs),
            scheduled_contracts: GroupAgentScheduledNodeContractService::new(
                Arc::clone(&graphs),
                Arc::clone(&runs),
                Arc::clone(&schedules),
                scheduled_contracts,
            ),
            successor_contracts: Some(GroupAgentScheduledNodeSuccessorService::new(
                graphs,
                runs,
                schedules,
                successor_contracts,
                lifecycles,
            )),
            provider_requests,
            codec,
        }
    }

    /// Validates pure preparation input before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid contract ID, key, or time.
    pub fn preflight_prepare(
        input: &PrepareGroupAgentScheduledNodeProviderRequestInput,
    ) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
        validate_input(input)
    }

    /// Validates an inspection identifier before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid provider-request ID.
    pub fn preflight_inspect(
        provider_request_id: &str,
    ) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
        super::scheduled_contract_validation::validate_identifier(
            provider_request_id,
            "scheduled provider request ID",
        )
        .map_err(Into::into)
    }

    /// Validates list filters and bounds before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid Run filter or limit.
    pub fn preflight_list(
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
        validate_list_input(graph_run_id, limit)
    }

    /// Deterministically encodes and persists one passive provider request.
    ///
    /// This operation does not release authority, claim a lane, or dispatch.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, codec, or storage error.
    pub fn prepare(
        &self,
        input: &PrepareGroupAgentScheduledNodeProviderRequestInput,
    ) -> Result<
        PrepareGroupAgentScheduledNodeProviderRequestResult,
        GroupAgentScheduledNodeProviderRequestServiceError,
    > {
        Self::preflight_prepare(input)?;
        let source = self.inspect_source(&input.scheduled_contract_id)?;
        self.require_pristine_run(&source)?;
        let logical = model_request(&source.candidate);
        let body = self
            .codec
            .encode_request(&source.candidate.provider.model, &logical)
            .map_err(|error| codec_input_error(&error))?;
        self.codec
            .validate_exact_request(&source.candidate.provider.model, &logical, &body)
            .map_err(|error| codec_input_error(&error))?;
        let request = prepare_request(input, &source, body)?;
        let result = self
            .provider_requests
            .prepare_group_agent_scheduled_node_provider_request(&request)
            .map_err(GroupAgentScheduledNodeProviderRequestServiceError::from)?;
        let result = validate_result(&request, &source, result)?;
        self.validate_source_and_codec(&result.inspection)?;
        Ok(result)
    }

    /// Loads and fully revalidates one exact passive provider request.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, codec, or storage error.
    pub fn inspect(
        &self,
        provider_request_id: &str,
    ) -> Result<
        GroupAgentScheduledNodeProviderRequestInspection,
        GroupAgentScheduledNodeProviderRequestServiceError,
    > {
        Self::preflight_inspect(provider_request_id)?;
        let inspection = self
            .provider_requests
            .inspect_group_agent_scheduled_node_provider_request(provider_request_id)
            .map_err(GroupAgentScheduledNodeProviderRequestServiceError::from)?;
        let inspection = checked_inspection(inspection)?;
        if inspection.record.provider_request_id != provider_request_id {
            return Err(corrupt(
                "store returned a different scheduled provider request identity",
            ));
        }
        self.validate_source_and_codec(&inspection)?;
        Ok(inspection)
    }

    /// Lists bounded metadata without exact request body bytes.
    ///
    /// # Errors
    ///
    /// Returns a structured input, corruption, or storage error.
    pub fn list(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<
        Vec<GroupAgentScheduledNodeProviderRequestRecord>,
        GroupAgentScheduledNodeProviderRequestServiceError,
    > {
        Self::preflight_list(graph_run_id, limit)?;
        let records = self
            .provider_requests
            .list_group_agent_scheduled_node_provider_requests(graph_run_id, limit)
            .map_err(GroupAgentScheduledNodeProviderRequestServiceError::from)?;
        validate_list(&records, graph_run_id, limit)?;
        Ok(records)
    }

    fn validate_source_and_codec(
        &self,
        inspection: &GroupAgentScheduledNodeProviderRequestInspection,
    ) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
        let source = self.inspect_source(&inspection.record.scheduled_contract_id)?;
        self.require_pristine_run(&source)?;
        if source != inspection.scheduled_contract {
            return Err(corrupt(
                "scheduled provider request and source contract inspections disagree",
            ));
        }
        self.codec
            .validate_exact_request(
                &source.candidate.provider.model,
                &model_request(&source.candidate),
                &inspection.provider_request_body,
            )
            .map_err(|error| {
                corrupt(&format!(
                    "stored scheduled provider request bytes failed exact codec validation: {error}"
                ))
            })
    }

    fn require_pristine_run(
        &self,
        source: &GroupAgentScheduledNodeContractInspection,
    ) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
        let run = self
            .runs
            .inspect_group_agent_graph_run(&source.record.graph_run_id)
            .map_err(GroupAgentScheduledNodeProviderRequestServiceError::from)?;
        validate_pristine_run(&source.record.graph_run_id, run)
    }

    fn inspect_source(
        &self,
        contract_id: &str,
    ) -> Result<
        GroupAgentScheduledNodeContractInspection,
        GroupAgentScheduledNodeProviderRequestServiceError,
    > {
        match self.scheduled_contracts.inspect(contract_id) {
            Ok(source) => Ok(source),
            Err(super::GroupAgentScheduledNodeContractServiceError::NotFound { .. }) => self
                .successor_contracts
                .as_ref()
                .ok_or_else(
                    || GroupAgentScheduledNodeProviderRequestServiceError::NotFound {
                        message: format!("scheduled contract '{contract_id}' was not found"),
                    },
                )?
                .inspect(contract_id)
                .map_err(Into::into),
            Err(error) => Err(error.into()),
        }
    }
}
