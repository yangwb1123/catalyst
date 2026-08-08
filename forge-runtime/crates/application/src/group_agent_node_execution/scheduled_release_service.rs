use std::sync::Arc;

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleStore,
    GroupAgentGraphRunInspection, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentScheduledNodeContractStore, GroupAgentScheduledNodeDispatchAuthorization,
    GroupAgentScheduledNodeDispatchReleaseControl, GroupAgentScheduledNodeLifecycleStore,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestStore,
    GroupAgentScheduledNodeSuccessorStore,
};

use super::{
    ExportGroupAgentGraphControl, GroupAgentNodeDispatchRequestCodec,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    GroupAgentScheduledNodeProviderRequestService,
    scheduled_release_error::{corrupt, invalid},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGroupAgentScheduledNodeDispatchReleaseControl {
    pub release_control: GroupAgentScheduledNodeDispatchReleaseControl,
    pub canonical_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGroupAgentScheduledNodeDispatchAuthorization {
    pub v: u16,
    pub authorization_id: String,
    pub authorization_sha256: String,
    pub graph_run_id: String,
    pub release_control_snapshot_sha256: String,
    pub schedule_id: String,
    pub scheduled_contract_id: String,
    pub scheduled_provider_request_id: String,
    pub execution_ordinal: usize,
    pub node_id: String,
    pub attempt: u16,
    pub project_id: String,
    pub project_lane_sha256: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub request_body_sha256: String,
    pub request_body_bytes: usize,
}

struct ScheduledReleaseSource {
    run: GroupAgentGraphRunInspection,
    control: ExportGroupAgentGraphControl,
    schedule: GroupAgentGraphExecutionScheduleInspection,
    request: GroupAgentScheduledNodeProviderRequestInspection,
}

pub struct GroupAgentScheduledNodeDispatchReleaseControlService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
    provider_requests: GroupAgentScheduledNodeProviderRequestService,
}

impl GroupAgentScheduledNodeDispatchReleaseControlService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
        scheduled_contracts: Arc<dyn GroupAgentScheduledNodeContractStore>,
        provider_requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
    ) -> Self {
        let request_service = GroupAgentScheduledNodeProviderRequestService::new(
            Arc::clone(&graphs),
            Arc::clone(&runs),
            Arc::clone(&schedules),
            scheduled_contracts,
            provider_requests,
            codec,
        );
        Self {
            graphs,
            runs,
            schedules,
            provider_requests: request_service,
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
        let request_service = GroupAgentScheduledNodeProviderRequestService::new_with_successors(
            Arc::clone(&graphs),
            Arc::clone(&runs),
            Arc::clone(&schedules),
            scheduled_contracts,
            successor_contracts,
            lifecycles,
            provider_requests,
            codec,
        );
        Self {
            graphs,
            runs,
            schedules,
            provider_requests: request_service,
        }
    }

    /// Exports one exact, passive scheduled-node release-control snapshot.
    ///
    /// This operation does not read credentials, release authority, claim a
    /// project lane, dispatch, access a workspace, or mutate Hub state.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, codec, or storage error.
    pub fn export(
        &self,
        provider_request_id: &str,
    ) -> Result<
        ExportGroupAgentScheduledNodeDispatchReleaseControl,
        GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    > {
        GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
        let request = self.provider_requests.inspect(provider_request_id)?;
        let source = self.load_current_source(request)?;
        build_release_control(source)
    }

    /// Verifies exact canonical authorization against freshly re-exported state.
    ///
    /// The result contains bounded identity and digest metadata only; it never
    /// returns provider-request bytes or Prompt plaintext.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, codec, or storage error.
    pub fn verify(
        &self,
        provider_request_id: &str,
        authorization_json: &str,
    ) -> Result<
        VerifiedGroupAgentScheduledNodeDispatchAuthorization,
        GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    > {
        GroupAgentScheduledNodeProviderRequestService::preflight_inspect(provider_request_id)?;
        let authorization = parse_authorization(authorization_json)?;
        if authorization.scheduled_provider_request_id != provider_request_id {
            return Err(invalid(
                "authorization scheduled provider-request binding disagrees",
            ));
        }
        let export = self.export(provider_request_id)?;
        authorization
            .validate_against_release_control(&export.release_control)
            .map_err(|error| invalid(&error.message))?;
        Ok(verified_metadata(authorization))
    }

    fn load_current_source(
        &self,
        request: GroupAgentScheduledNodeProviderRequestInspection,
    ) -> Result<ScheduledReleaseSource, GroupAgentScheduledNodeDispatchReleaseControlServiceError>
    {
        let run = self.load_run(&request.record.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        let control = super::snapshot::historical_base(&run, &graph)?;
        let schedule = self.load_schedule(&request.record.schedule_id)?;
        super::scheduled_contract_validation::validate_sources(
            &request.scheduled_contract.candidate,
            &control,
            &schedule,
        )?;
        Ok(ScheduledReleaseSource {
            run,
            control,
            schedule,
            request,
        })
    }

    fn load_run(
        &self,
        graph_run_id: &str,
    ) -> Result<
        GroupAgentGraphRunInspection,
        GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    > {
        let run = self
            .runs
            .inspect_group_agent_graph_run(graph_run_id)
            .map_err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::from)?;
        let run = super::validation::checked_run(run)?;
        super::scheduled_provider_request_validation::validate_pristine_run(
            graph_run_id,
            run.clone(),
        )?;
        Ok(run)
    }

    fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentGraphInspection,
        GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    > {
        let graph = self
            .graphs
            .inspect_group_agent_graph(graph_id)
            .map_err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::from)?;
        let graph = super::validation::checked_graph(graph)?;
        if graph.graph.graph_id != graph_id {
            return Err(corrupt("store returned a different Graph identity"));
        }
        Ok(graph)
    }

    fn load_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<
        GroupAgentGraphExecutionScheduleInspection,
        GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    > {
        let schedule = self
            .schedules
            .inspect_group_agent_graph_execution_schedule(schedule_id)
            .map_err(GroupAgentScheduledNodeDispatchReleaseControlServiceError::from)?;
        let schedule = super::scheduled_contract_validation::checked_schedule(schedule)?;
        if schedule.record.schedule_id != schedule_id {
            return Err(corrupt("store returned a different schedule identity"));
        }
        Ok(schedule)
    }
}

fn build_release_control(
    source: ScheduledReleaseSource,
) -> Result<
    ExportGroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
> {
    let provider_request_json = String::from_utf8(source.request.provider_request_body)
        .map_err(|_| corrupt("stored scheduled provider request is not exact UTF-8"))?;
    let mut release_control = GroupAgentScheduledNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: source.run.run,
        journal_events: source.run.events,
        control_snapshot: source.control.snapshot,
        schedule_record: source.schedule.record,
        schedule: source.schedule.schedule,
        scheduled_contract_record: source.request.scheduled_contract.record,
        scheduled_contract: source.request.scheduled_contract.candidate,
        provider_request: source.request.record,
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
    Ok(ExportGroupAgentScheduledNodeDispatchReleaseControl {
        release_control,
        canonical_json,
    })
}

fn parse_authorization(
    json: &str,
) -> Result<
    GroupAgentScheduledNodeDispatchAuthorization,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
> {
    GroupAgentScheduledNodeDispatchAuthorization::decode_exact(json)
        .map_err(|error| invalid(&error.message))
}

fn verified_metadata(
    value: GroupAgentScheduledNodeDispatchAuthorization,
) -> VerifiedGroupAgentScheduledNodeDispatchAuthorization {
    VerifiedGroupAgentScheduledNodeDispatchAuthorization {
        v: value.v,
        authorization_id: value.authorization_id,
        authorization_sha256: value.authorization_sha256,
        graph_run_id: value.graph_run_id,
        release_control_snapshot_sha256: value.release_control_snapshot_sha256,
        schedule_id: value.schedule_id,
        scheduled_contract_id: value.scheduled_contract_id,
        scheduled_provider_request_id: value.scheduled_provider_request_id,
        execution_ordinal: value.execution_ordinal,
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
