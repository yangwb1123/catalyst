use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphRunStore, GroupAgentGraphStore, GroupAgentNodeDispatchRequestInspection,
    GroupAgentNodeDispatchRequestRecord, GroupAgentNodeDispatchRequestStore,
    GroupAgentNodeExecutionContractStore, ModelRequest, ProviderError,
};

use super::{
    GroupAgentNodeDispatchRequestServiceError, GroupAgentNodeExecutionContractService,
    PrepareGroupAgentNodeDispatchRequestResult,
    dispatch_validation::{
        checked_inspection, model_request, prepare_request, validate_input, validate_list,
        validate_list_input, validate_result,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentNodeDispatchRequestInput {
    pub graph_run_id: String,
    pub idempotency_key: String,
    pub prepared_at_ms: u64,
}

pub trait GroupAgentNodeDispatchRequestCodec: Send + Sync {
    /// Deterministically encodes one logical request into exact provider body bytes.
    ///
    /// Implementations must be side-effect-free. Encoding must not read credentials,
    /// construct or invoke a provider or transport, access the network, filesystem,
    /// or workspace, or perform any write.
    ///
    /// # Errors
    ///
    /// Returns a provider-protocol error when the request cannot be encoded.
    fn encode_request(&self, model: &str, request: &ModelRequest)
    -> Result<Vec<u8>, ProviderError>;

    /// Deterministically re-encodes and compares one stored request byte-for-byte.
    ///
    /// Implementations must obey the same side-effect-free contract as
    /// [`Self::encode_request`]. Validation must not read credentials, construct or
    /// invoke a provider or transport, access the network, filesystem, or workspace,
    /// or perform any write.
    ///
    /// # Errors
    ///
    /// Returns a provider-protocol error when encoding fails or bytes differ.
    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError>;
}

pub struct GroupAgentNodeDispatchRequestService {
    contracts: GroupAgentNodeExecutionContractService,
    requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
}

impl GroupAgentNodeDispatchRequestService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        contracts: Arc<dyn GroupAgentNodeExecutionContractStore>,
        requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
    ) -> Self {
        Self {
            contracts: GroupAgentNodeExecutionContractService::new(graphs, runs, contracts),
            requests,
            codec,
        }
    }

    /// Deterministically encodes and atomically persists one provider request.
    ///
    /// This operation does not read credentials, release authority, or dispatch.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, codec, or storage error.
    pub fn prepare(
        &self,
        input: &PrepareGroupAgentNodeDispatchRequestInput,
    ) -> Result<PrepareGroupAgentNodeDispatchRequestResult, GroupAgentNodeDispatchRequestServiceError>
    {
        validate_input(input)?;
        let contract = self.contract_for_run(&input.graph_run_id)?;
        let logical = model_request(&contract.contract);
        let body = self
            .codec
            .encode_request(&contract.contract.provider.model, &logical)
            .map_err(|error| super::dispatch_error::codec_error(&error))?;
        let request = prepare_request(input, &contract, body)?;
        let result = self
            .requests
            .prepare_group_agent_node_dispatch_request(&request)
            .map_err(GroupAgentNodeDispatchRequestServiceError::from)?;
        let result = validate_result(&request, result)?;
        self.validate_source_and_codec(&result.inspection)?;
        Ok(result)
    }

    /// Loads and fully revalidates one exact prepared provider request.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, codec, or storage error.
    pub fn inspect(
        &self,
        dispatch_request_id: &str,
    ) -> Result<GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchRequestServiceError>
    {
        super::validation::validate_identifier(dispatch_request_id, "dispatch request ID")?;
        let inspection = self
            .requests
            .inspect_group_agent_node_dispatch_request(dispatch_request_id)
            .map_err(GroupAgentNodeDispatchRequestServiceError::from)?;
        let inspection = checked_inspection(inspection)?;
        if inspection.record.dispatch_request_id != dispatch_request_id {
            return Err(super::dispatch_error::corrupt(
                "store returned a different dispatch request identity",
            ));
        }
        self.validate_source_and_codec(&inspection)?;
        Ok(inspection)
    }

    /// Lists bounded prepared-request metadata without exact request bodies.
    ///
    /// # Errors
    ///
    /// Returns a structured input, corruption, or storage error.
    pub fn list(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeDispatchRequestRecord>, GroupAgentNodeDispatchRequestServiceError>
    {
        validate_list_input(graph_run_id, limit)?;
        let records = self
            .requests
            .list_group_agent_node_dispatch_requests(graph_run_id, limit)
            .map_err(GroupAgentNodeDispatchRequestServiceError::from)?;
        validate_list(&records, graph_run_id, limit)?;
        Ok(records)
    }

    fn validate_source_and_codec(
        &self,
        inspection: &GroupAgentNodeDispatchRequestInspection,
    ) -> Result<(), GroupAgentNodeDispatchRequestServiceError> {
        let contract = self.contracts.inspect(&inspection.record.contract_id)?;
        if contract != inspection.contract {
            return Err(super::dispatch_error::corrupt(
                "dispatch request and source contract inspections disagree",
            ));
        }
        self.codec
            .validate_exact_request(
                &contract.contract.provider.model,
                &model_request(&contract.contract),
                &inspection.provider_request_body,
            )
            .map_err(|error| {
                super::dispatch_error::corrupt(&format!(
                    "stored provider request bytes failed exact codec validation: {error}"
                ))
            })
    }

    fn contract_for_run(
        &self,
        graph_run_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentNodeExecutionContractInspection,
        GroupAgentNodeDispatchRequestServiceError,
    > {
        let run = self.contracts.load_run(graph_run_id)?;
        let records = self.contracts.list(Some(graph_run_id), 2)?;
        if records.len() > 1 || (!run.run.execution_contract_present && !records.is_empty()) {
            return Err(super::dispatch_error::corrupt(
                "Graph Run execution-contract projection disagrees with stored metadata",
            ));
        }
        let Some(record) = records.first() else {
            return if run.run.execution_contract_present {
                Err(super::dispatch_error::corrupt(
                    "Graph Run declares an execution contract but its stored contract is missing",
                ))
            } else {
                Err(GroupAgentNodeDispatchRequestServiceError::NotFound {
                    message: format!("Graph Run {graph_run_id} has no admitted execution contract"),
                })
            };
        };
        let inspection = match self.contracts.inspect(&record.contract_id) {
            Err(super::GroupAgentNodeExecutionContractServiceError::NotFound { .. }) => {
                return Err(super::dispatch_error::corrupt(
                    "Graph Run declares an execution contract but its stored contract is missing",
                ));
            }
            result => result?,
        };
        if inspection.record != *record || inspection.record.graph_run_id != graph_run_id {
            return Err(super::dispatch_error::corrupt(
                "execution-contract metadata disagrees with its inspection",
            ));
        }
        Ok(inspection)
    }
}
