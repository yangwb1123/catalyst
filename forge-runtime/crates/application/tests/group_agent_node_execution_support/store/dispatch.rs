use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION, GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
    GroupAgentGraphRunStatus, GroupAgentNodeDispatchRequestInspection,
    GroupAgentNodeDispatchRequestRecord, GroupAgentNodeDispatchRequestStore, HubEntity,
    HubStoreError, PrepareGroupAgentNodeDispatchRequest,
    PrepareGroupAgentNodeDispatchRequestDisposition, PrepareGroupAgentNodeDispatchRequestResult,
};

use super::MemoryContractHub;

#[derive(Clone)]
pub(super) struct StoredDispatch {
    pub(super) key: String,
    pub(super) inspection: GroupAgentNodeDispatchRequestInspection,
}

impl GroupAgentNodeDispatchRequestStore for MemoryContractHub {
    fn prepare_group_agent_node_dispatch_request(
        &self,
        request: &PrepareGroupAgentNodeDispatchRequest,
    ) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError> {
        request
            .validate()
            .map_err(|error| conflict(&error.to_string()))?;
        let mut state = self.lock();
        if let Some(stored) = &state.dispatch {
            return replay(stored, request);
        }
        let mut contract = state
            .admission
            .as_ref()
            .map(|stored| stored.inspection.clone())
            .ok_or_else(|| conflict("contract was not admitted"))?;
        advance_run(&mut contract, request);
        let inspection = inspection(request, contract)?;
        state.run = inspection.contract.graph_run.clone();
        state
            .admission
            .as_mut()
            .expect("admission checked above")
            .inspection = inspection.contract.clone();
        state.dispatch = Some(StoredDispatch {
            key: request.idempotency_key.clone(),
            inspection: inspection.clone(),
        });
        Ok(result(
            PrepareGroupAgentNodeDispatchRequestDisposition::Created,
            inspection,
        ))
    }

    fn inspect_group_agent_node_dispatch_request(
        &self,
        dispatch_request_id: &str,
    ) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
        self.lock()
            .dispatch
            .as_ref()
            .filter(|stored| stored.inspection.record.dispatch_request_id == dispatch_request_id)
            .map(|stored| stored.inspection.clone())
            .ok_or_else(|| HubStoreError::NotFound {
                entity: HubEntity::GroupAgentNodeDispatchRequest,
                id: dispatch_request_id.into(),
            })
    }

    fn list_group_agent_node_dispatch_requests(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeDispatchRequestRecord>, HubStoreError> {
        Ok(self
            .lock()
            .dispatch
            .iter()
            .map(|stored| stored.inspection.record.clone())
            .filter(|record| graph_run_id.is_none_or(|id| id == record.graph_run_id))
            .take(limit)
            .collect())
    }
}

fn replay(
    stored: &StoredDispatch,
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError> {
    let exact = stored.key == request.idempotency_key
        && stored.inspection.record.dispatch_request_id == request.dispatch_request_id
        && stored.inspection.record.dispatch_request_sha256 == request.dispatch_request_sha256
        && stored.inspection.provider_request_body == request.provider_request_body;
    if !exact {
        return Err(conflict("dispatch replay semantics disagree"));
    }
    Ok(result(
        PrepareGroupAgentNodeDispatchRequestDisposition::Replayed,
        stored.inspection.clone(),
    ))
}

fn advance_run(
    contract: &mut forge_runtime_domain::GroupAgentNodeExecutionContractInspection,
    request: &PrepareGroupAgentNodeDispatchRequest,
) {
    let run = &mut contract.graph_run;
    run.v = GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION;
    run.run.v = GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION;
    run.run.status = GroupAgentGraphRunStatus::AwaitingDispatchAuthorization;
    run.run.dispatch_request_present = true;
    run.run.last_event_seq = 3;
    run.run.journal_bytes += request.event_json.len();
    run.events.push(request.event.clone());
    run.event_jsons.push(request.event_json.clone());
}

fn inspection(
    request: &PrepareGroupAgentNodeDispatchRequest,
    contract: forge_runtime_domain::GroupAgentNodeExecutionContractInspection,
) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
    let inspection = GroupAgentNodeDispatchRequestInspection {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        record: record(request),
        provider_request_body: request.provider_request_body.clone(),
        preparation_event_json: request.event_json.clone(),
        preparation_event: request.event.clone(),
        contract,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

fn record(request: &PrepareGroupAgentNodeDispatchRequest) -> GroupAgentNodeDispatchRequestRecord {
    GroupAgentNodeDispatchRequestRecord {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: request.dispatch_request_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        contract_id: request.contract_id.clone(),
        node_id: request.node_id.clone(),
        attempt: request.attempt,
        contract_sha256: request.contract_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        project_lane_sha256: request.project_lane_sha256.clone(),
        provider: request.provider,
        endpoint: request.endpoint.clone(),
        model: request.model.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        provider_request_sha256: request.provider_request_sha256.clone(),
        provider_request_bytes: request.provider_request_body.len(),
        destination_sha256: request.destination_sha256.clone(),
        dispatch_request_sha256: request.dispatch_request_sha256.clone(),
        codec_protocol_version: request.codec_protocol_version,
        expected_last_event_seq: request.expected_last_event_seq,
        expected_last_event_sha256: request.expected_last_event_sha256.clone(),
        created_at_ms: request.prepared_at_ms,
    }
}

fn result(
    disposition: PrepareGroupAgentNodeDispatchRequestDisposition,
    inspection: GroupAgentNodeDispatchRequestInspection,
) -> PrepareGroupAgentNodeDispatchRequestResult {
    PrepareGroupAgentNodeDispatchRequestResult {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        disposition,
        inspection,
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeDispatchRequest,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
