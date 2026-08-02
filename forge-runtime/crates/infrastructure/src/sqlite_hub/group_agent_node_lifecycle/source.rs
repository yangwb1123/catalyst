use rusqlite::Connection;

use crate::runtime_domain::{
    GroupAgentGraphInspection, GroupAgentGraphRunInspection, GroupAgentNodeDispatchReleaseControl,
    GroupAgentNodeDispatchRequestInspection, GroupAgentNodeTerminalControl, HubStoreError,
};

use super::{
    super::{
        group_agent_graph, group_agent_graph_run,
        group_agent_node_execution_contract::dispatch_request,
    },
    codec::{conflict, corrupt},
};

pub(super) struct LoadedSource {
    pub graph: GroupAgentGraphInspection,
    pub run: GroupAgentGraphRunInspection,
    pub dispatch: GroupAgentNodeDispatchRequestInspection,
}

pub(super) fn load(
    connection: &Connection,
    graph_run_id: &str,
    dispatch_request_id: &str,
) -> Result<LoadedSource, HubStoreError> {
    let run = group_agent_graph_run::read::inspect_in_snapshot(connection, graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(connection, &run.run.graph_id)
        .map_err(stored_graph_error)?;
    let dispatch = dispatch_request::read::inspect_in_snapshot(connection, dispatch_request_id)
        .map_err(stored_dispatch_error)?;
    if dispatch.record.graph_run_id != graph_run_id {
        return Err(corrupt(
            "stored lifecycle dispatch request belongs to another Graph Run",
        ));
    }
    Ok(LoadedSource {
        graph,
        run,
        dispatch,
    })
}

pub(super) fn validate_release_source(
    source: &LoadedSource,
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), HubStoreError> {
    let exact = source.run.run == control.graph_run
        && source.run.plan == control.plan
        && source.run.events == control.journal_events
        && source.graph.manifest == control.manifest
        && source.dispatch.record == control.dispatch_request
        && source.dispatch.provider_request_body == control.provider_request_json.as_bytes()
        && source.dispatch.contract.record == control.contract_record
        && source.dispatch.contract.contract == control.contract;
    exact
        .then_some(())
        .ok_or_else(|| conflict("dispatch claim source changed since release control was built"))?;
    validate_single_node(source)
}

pub(super) fn validate_terminal_source(
    source: &LoadedSource,
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), HubStoreError> {
    let exact = source.run.run == control.graph_run
        && source.run.plan == control.plan
        && source.run.events == control.journal_events
        && source.graph.manifest == control.manifest
        && source.dispatch.record == control.dispatch_request
        && source.dispatch.provider_request_body == control.provider_request_json.as_bytes()
        && source.dispatch.contract.record == control.contract_record
        && source.dispatch.contract.contract == control.contract;
    exact
        .then_some(())
        .ok_or_else(|| conflict("terminal source changed since control was built"))?;
    validate_single_node(source)
}

fn validate_single_node(source: &LoadedSource) -> Result<(), HubStoreError> {
    let manifest = &source.graph.manifest;
    let node_id = &source.dispatch.record.node_id;
    let exact_wave = manifest.waves.as_slice() == [vec![node_id.clone()]];
    let valid = source.run.run.node_count == 1
        && source.run.run.wave_count == 1
        && manifest.nodes.len() == 1
        && manifest.nodes[0].node_id == *node_id
        && manifest.edges.is_empty()
        && exact_wave;
    valid
        .then_some(())
        .ok_or_else(|| conflict("effectful dispatch requires an exact single-node Graph"))
}

fn stored_graph_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => corrupt("stored lifecycle Graph is missing"),
        other => other,
    }
}

fn stored_dispatch_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => corrupt("stored lifecycle dispatch request is missing"),
        other => other,
    }
}
