use std::sync::Arc;

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION, GroupAgentGraphStore,
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchReleaseControl,
    GroupAgentNodeDispatchRequestStore, MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES,
};

use super::{
    GroupAgentNodeDispatchReleaseControlServiceError, GroupAgentNodeDispatchRequestCodec,
    dispatch_validation::model_request,
    release_error::{corrupt, invalid, not_found},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGroupAgentNodeDispatchReleaseControl {
    pub release_control: GroupAgentNodeDispatchReleaseControl,
    pub canonical_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGroupAgentNodeDispatchAuthorization {
    pub v: u16,
    pub authorization_id: String,
    pub authorization_sha256: String,
    pub graph_run_id: String,
    pub release_control_snapshot_sha256: String,
    pub contract_id: String,
    pub dispatch_request_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub project_id: String,
    pub project_lane_sha256: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub request_body_sha256: String,
    pub request_body_bytes: usize,
}

pub struct GroupAgentNodeDispatchReleaseControlService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
    codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
}

impl GroupAgentNodeDispatchReleaseControlService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
    ) -> Self {
        Self {
            graphs,
            requests,
            codec,
        }
    }

    /// Exports one canonical, self-validating passive release-control snapshot.
    ///
    /// This operation does not inspect credentials, authorize release, dispatch,
    /// call a provider, or mutate Hub state.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, codec, or storage error.
    pub fn export(
        &self,
        graph_run_id: &str,
    ) -> Result<
        ExportGroupAgentNodeDispatchReleaseControl,
        GroupAgentNodeDispatchReleaseControlServiceError,
    > {
        validate_graph_run_id(graph_run_id)?;
        let inspection = self.load_request(graph_run_id)?;
        self.validate_codec(&inspection)?;
        let graph_id = &inspection.contract.graph_run.run.graph_id;
        let graph = self
            .graphs
            .inspect_group_agent_graph(graph_id)
            .map_err(GroupAgentNodeDispatchReleaseControlServiceError::from)?;
        graph.validate().map_err(|error| corrupt(&error.message))?;
        validate_graph_binding(graph_id, &graph, &inspection)?;
        build_release_control(graph.manifest, inspection)
    }

    /// Verifies exact canonical authorization bytes against freshly exported state.
    ///
    /// This operation is read-only and returns only bounded release metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, codec, or storage error.
    pub fn verify(
        &self,
        graph_run_id: &str,
        authorization_json: &str,
    ) -> Result<
        VerifiedGroupAgentNodeDispatchAuthorization,
        GroupAgentNodeDispatchReleaseControlServiceError,
    > {
        validate_graph_run_id(graph_run_id)?;
        let authorization = parse_authorization(authorization_json)?;
        if authorization.graph_run_id != graph_run_id {
            return Err(invalid("authorization Graph Run binding disagrees"));
        }
        let export = self.export(graph_run_id)?;
        authorization
            .validate_against_release_control(&export.release_control)
            .map_err(|error| invalid(&error.message))?;
        Ok(verified_metadata(authorization))
    }

    fn load_request(
        &self,
        graph_run_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentNodeDispatchRequestInspection,
        GroupAgentNodeDispatchReleaseControlServiceError,
    > {
        let records = self
            .requests
            .list_group_agent_node_dispatch_requests(Some(graph_run_id), 2)
            .map_err(GroupAgentNodeDispatchReleaseControlServiceError::from)?;
        if records.len() > 1 {
            return Err(corrupt("Graph Run has multiple prepared dispatch requests"));
        }
        let record = records.first().ok_or_else(|| {
            not_found(&format!(
                "Graph Run {graph_run_id} has no prepared dispatch request"
            ))
        })?;
        record.validate().map_err(|error| corrupt(&error.message))?;
        if record.graph_run_id != graph_run_id {
            return Err(corrupt("request list returned a different Graph Run"));
        }
        let inspection = self
            .requests
            .inspect_group_agent_node_dispatch_request(&record.dispatch_request_id)
            .map_err(GroupAgentNodeDispatchReleaseControlServiceError::from)?;
        inspection
            .validate()
            .map_err(|error| corrupt(&error.message))?;
        if inspection.record != *record {
            return Err(corrupt("request metadata and deep inspection disagree"));
        }
        Ok(inspection)
    }

    fn validate_codec(
        &self,
        inspection: &crate::runtime_domain::GroupAgentNodeDispatchRequestInspection,
    ) -> Result<(), GroupAgentNodeDispatchReleaseControlServiceError> {
        let contract = &inspection.contract.contract;
        self.codec
            .validate_exact_request(
                &contract.provider.model,
                &model_request(contract),
                &inspection.provider_request_body,
            )
            .map_err(|error| {
                corrupt(&format!(
                    "stored provider request failed exact codec validation: {error}"
                ))
            })
    }
}

fn validate_graph_run_id(
    graph_run_id: &str,
) -> Result<(), GroupAgentNodeDispatchReleaseControlServiceError> {
    crate::group_agent_graph_validation::validate_identifier(graph_run_id)
        .map_err(|_| invalid("Graph Run ID is invalid"))
}

fn validate_graph_binding(
    requested_graph_id: &str,
    graph: &crate::runtime_domain::GroupAgentGraphInspection,
    request: &crate::runtime_domain::GroupAgentNodeDispatchRequestInspection,
) -> Result<(), GroupAgentNodeDispatchReleaseControlServiceError> {
    let run = &request.contract.graph_run.run;
    let valid = graph.graph.graph_id == requested_graph_id
        && graph.graph.graph_id == run.graph_id
        && graph.graph.manifest_sha256 == run.graph_manifest_sha256
        && graph.graph.source_snapshot_sha256 == run.source_snapshot_sha256;
    if valid {
        Ok(())
    } else {
        Err(corrupt(
            "graph inspection and dispatch request source disagree",
        ))
    }
}

fn build_release_control(
    manifest: crate::runtime_domain::GroupAgentGraphManifest,
    inspection: crate::runtime_domain::GroupAgentNodeDispatchRequestInspection,
) -> Result<
    ExportGroupAgentNodeDispatchReleaseControl,
    GroupAgentNodeDispatchReleaseControlServiceError,
> {
    let provider_request_json = String::from_utf8(inspection.provider_request_body.clone())
        .map_err(|_| corrupt("stored provider request is not exact UTF-8"))?;
    let mut release_control = GroupAgentNodeDispatchReleaseControl {
        v: GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: inspection.contract.graph_run.run.clone(),
        plan: inspection.contract.graph_run.plan.clone(),
        manifest,
        journal_events: inspection.contract.graph_run.events.clone(),
        contract_record: inspection.contract.record.clone(),
        contract: inspection.contract.contract.clone(),
        dispatch_request: inspection.record,
        provider_request_json,
        snapshot_sha256: String::new(),
    };
    release_control.snapshot_sha256 = release_control
        .expected_sha256()
        .map_err(|error| corrupt(&error.message))?;
    release_control
        .validate()
        .map_err(|error| corrupt(&error.message))?;
    let canonical_json = release_control
        .canonical_json()
        .map_err(|error| corrupt(&error.message))?;
    Ok(ExportGroupAgentNodeDispatchReleaseControl {
        release_control,
        canonical_json,
    })
}

fn parse_authorization(
    json: &str,
) -> Result<GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchReleaseControlServiceError> {
    if !(1..=MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES).contains(&json.len()) {
        return Err(invalid("authorization JSON byte bound is invalid"));
    }
    let authorization: GroupAgentNodeDispatchAuthorization = serde_json::from_str(json)
        .map_err(|_| invalid("authorization JSON is malformed or has unknown fields"))?;
    authorization
        .validate()
        .map_err(|error| invalid(&error.message))?;
    if authorization.canonical_json().as_deref() != Ok(json) {
        return Err(invalid(
            "authorization JSON is not its exact canonical encoding",
        ));
    }
    Ok(authorization)
}

fn verified_metadata(
    value: GroupAgentNodeDispatchAuthorization,
) -> VerifiedGroupAgentNodeDispatchAuthorization {
    VerifiedGroupAgentNodeDispatchAuthorization {
        v: value.v,
        authorization_id: value.authorization_id,
        authorization_sha256: value.authorization_sha256,
        graph_run_id: value.graph_run_id,
        release_control_snapshot_sha256: value.release_control_snapshot_sha256,
        contract_id: value.contract_id,
        dispatch_request_id: value.dispatch_request_id,
        node_id: value.node_id,
        attempt: value.attempt,
        project_id: value.project_id,
        project_lane_sha256: value.project_lane_sha256,
        destination_sha256: value.destination_sha256,
        pricing_snapshot_sha256: value.pricing_snapshot_sha256,
        request_body_sha256: value.request_body_sha256,
        request_body_bytes: value.request_body_bytes,
    }
}
