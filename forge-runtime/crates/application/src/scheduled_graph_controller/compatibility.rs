use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, ScheduledGraphControllerHeader,
    ScheduledGraphControllerStopReason as StopReason,
};
use crate::{
    GroupAgentGraphExecutionScheduleService, GroupAgentGraphExecutionScheduleServiceError,
    ScheduledGraphReconcileObservation, ScheduledGraphReconcileService,
    ScheduledGraphReconcileServiceError,
};

use super::{ScheduledGraphControllerService, ScheduledGraphControllerServiceError};

impl ScheduledGraphControllerService {
    pub(super) fn validate_reentry_schedule(
        &self,
        header: &ScheduledGraphControllerHeader,
    ) -> Result<Option<StopReason>, ScheduledGraphControllerServiceError> {
        let service = GroupAgentGraphExecutionScheduleService::new(
            self.hub.clone(),
            self.hub.clone(),
            self.hub.clone(),
        );
        let inspection = match service.inspect(&header.schedule_id) {
            Ok(value) => value,
            Err(
                GroupAgentGraphExecutionScheduleServiceError::Unavailable { .. }
                | GroupAgentGraphExecutionScheduleServiceError::NotFound { .. },
            ) => return Err(ScheduledGraphControllerServiceError::StoreUnavailable),
            Err(GroupAgentGraphExecutionScheduleServiceError::InvalidInput { .. }) => {
                return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
            }
            Err(
                GroupAgentGraphExecutionScheduleServiceError::Conflict { .. }
                | GroupAgentGraphExecutionScheduleServiceError::Corrupt { .. },
            ) => return Err(ScheduledGraphControllerServiceError::CorruptEvidence),
        };
        if !schedule_binding_matches(header, &inspection) {
            return Err(ScheduledGraphControllerServiceError::CorruptEvidence);
        }
        if schedule_supported(&inspection) {
            Ok(None)
        } else {
            Ok(Some(StopReason::IncompatibleSchedule))
        }
    }

    pub(super) fn observe_for_drive(
        &self,
        graph_run_id: &str,
    ) -> Result<Box<ScheduledGraphReconcileObservation>, ScheduledGraphControllerServiceError> {
        let result = ScheduledGraphReconcileService::new(self.hub.clone(), self.reconcile.clone())
            .observe(graph_run_id);
        match result {
            Ok(value) => Ok(Box::new(value)),
            Err(
                ScheduledGraphReconcileServiceError::CorruptProgress
                | ScheduledGraphReconcileServiceError::InvalidInput,
            ) => Err(ScheduledGraphControllerServiceError::CorruptEvidence),
            Err(
                ScheduledGraphReconcileServiceError::StorageUnavailable
                | ScheduledGraphReconcileServiceError::NotFound,
            ) => Err(ScheduledGraphControllerServiceError::StoreUnavailable),
            Err(
                ScheduledGraphReconcileServiceError::CoreUnavailable
                | ScheduledGraphReconcileServiceError::InvalidCoreDecision,
            ) => Err(ScheduledGraphControllerServiceError::ReconcileFailed),
        }
    }
}

fn schedule_binding_matches(
    header: &ScheduledGraphControllerHeader,
    inspection: &GroupAgentGraphExecutionScheduleInspection,
) -> bool {
    let record = &inspection.record;
    let schedule = &inspection.schedule;
    record.schedule_id == header.schedule_id
        && record.schedule_sha256 == header.schedule_sha256
        && record.graph_run_id == header.graph_run_id
        && record.node_count == header.node_count
        && schedule.schedule_id == header.schedule_id
        && schedule.schedule_sha256 == header.schedule_sha256
        && schedule.graph_run_id == header.graph_run_id
        && schedule.node_count == header.node_count
}

fn schedule_supported(inspection: &GroupAgentGraphExecutionScheduleInspection) -> bool {
    inspection.schedule.v == crate::GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && inspection.schedule.max_in_flight_nodes == 1
}
