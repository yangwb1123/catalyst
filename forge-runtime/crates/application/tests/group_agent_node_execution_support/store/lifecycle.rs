use forge_runtime_domain::{
    ClaimGroupAgentNodeDispatch, ClaimGroupAgentNodeDispatchResult, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchAuthority, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeLifecycleStore, HubEntity, HubStoreError, TerminalizeGroupAgentNodeDispatch,
    TerminalizeGroupAgentNodeDispatchResult,
};

use super::MemoryContractHub;

impl GroupAgentNodeLifecycleStore for MemoryContractHub {
    fn claim_group_agent_node_dispatch(
        &self,
        request: &ClaimGroupAgentNodeDispatch,
    ) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError> {
        request.validate().map_err(invalid_request)?;
        let mut state = self.lock();
        if let Some(inspection) = &state.lifecycle {
            return Ok(ClaimGroupAgentNodeDispatchResult::AlreadyClaimed {
                inspection: inspection.clone(),
            });
        }
        let dispatch = state
            .dispatch
            .as_ref()
            .ok_or_else(|| conflict("dispatch request is missing"))?;
        let authority = GroupAgentNodeDispatchAuthority::new(
            &dispatch.inspection.record,
            request.claim.clone(),
            &request.event,
            dispatch.inspection.provider_request_body.clone(),
        )
        .map_err(invalid_request)?;
        let inspection = claimed_inspection(&state.run, request)?;
        state.run = inspection.graph_run.clone();
        state.lifecycle = Some(inspection);
        Ok(ClaimGroupAgentNodeDispatchResult::Claimed { authority })
    }

    fn terminalize_group_agent_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentNodeDispatch,
    ) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError> {
        request.validate().map_err(invalid_request)?;
        let mut state = self.lock();
        let current = state
            .lifecycle
            .as_ref()
            .ok_or_else(|| conflict("dispatch claim is missing"))?;
        if current.active_lane.as_ref() != Some(&request.control.active_lane) {
            return Err(conflict("active lane disagrees"));
        }
        let inspection = terminal_inspection(current, request)?;
        state.run = inspection.graph_run.clone();
        state.lifecycle = Some(inspection.clone());
        Ok(TerminalizeGroupAgentNodeDispatchResult {
            v: request.v,
            inspection,
        })
    }

    fn inspect_group_agent_node_lifecycle(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
        self.lock()
            .lifecycle
            .as_ref()
            .filter(|inspection| inspection.claim.graph_run_id == graph_run_id)
            .cloned()
            .ok_or_else(|| HubStoreError::NotFound {
                entity: HubEntity::GroupAgentNodeLifecycle,
                id: graph_run_id.into(),
            })
    }
}

fn claimed_inspection(
    run: &forge_runtime_domain::GroupAgentGraphRunInspection,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
    let mut graph_run = run.clone();
    graph_run.v = request.event.v;
    graph_run.run.v = request.event.v;
    graph_run.run.status = GroupAgentGraphRunStatus::DispatchUnknown;
    graph_run.run.dispatch_authority_released = true;
    graph_run.run.last_event_seq = 4;
    graph_run.run.journal_bytes += request.event_json.len();
    graph_run.events.push(request.event.clone());
    graph_run.event_jsons.push(request.event_json.clone());
    let inspection = GroupAgentNodeLifecycleInspection {
        v: request.v,
        graph_run,
        claim: request.claim.clone(),
        claim_json: request.claim_json.clone(),
        active_lane: Some(request.active_lane.clone()),
        active_lane_json: Some(request.active_lane_json.clone()),
        artifact: None,
        artifact_json: None,
        terminal_receipt: None,
        terminal_receipt_json: None,
    };
    inspection.validate().map_err(invalid_request)?;
    Ok(inspection)
}

fn terminal_inspection(
    current: &GroupAgentNodeLifecycleInspection,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
    let mut graph_run = current.graph_run.clone();
    graph_run.v = request.event.v;
    graph_run.run.v = request.event.v;
    graph_run.run.status = request.receipt.graph_status;
    graph_run.run.last_event_seq = 5;
    graph_run.run.journal_bytes += request.event_json.len();
    graph_run.events.push(request.event.clone());
    graph_run.event_jsons.push(request.event_json.clone());
    let inspection = GroupAgentNodeLifecycleInspection {
        v: request.v,
        graph_run,
        claim: current.claim.clone(),
        claim_json: current.claim_json.clone(),
        active_lane: None,
        active_lane_json: None,
        artifact: Some(request.control.artifact.clone()),
        artifact_json: Some(request.artifact_json.clone()),
        terminal_receipt: Some(request.receipt.clone()),
        terminal_receipt_json: Some(request.receipt_json.clone()),
    };
    inspection.validate().map_err(invalid_request)?;
    Ok(inspection)
}

fn invalid_request(error: impl std::fmt::Display) -> HubStoreError {
    HubStoreError::Corrupt {
        message: error.to_string(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeLifecycle,
        message: message.into(),
    }
}
