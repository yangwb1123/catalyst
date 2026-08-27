use std::io::{self, Write};

use forge_runtime_application::ExecuteGroupAgentScheduledNodeDispatchResult;
use serde::Serialize;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome,
        GroupAgentScheduledNodeAnyLifecycleInspection, GroupAgentScheduledNodeDispatchClaim,
        GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStatus,
        GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalReceipt,
    },
};

/// Public CLI projection for the scheduled effectful lifecycle.
///
/// The projection deliberately contains no request, authorization, pricing,
/// credential, or Core-control bytes. The optional result is only emitted when
/// the caller explicitly asks for it.
#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeDispatchExecutionCliOutput {
    pub v: u16,
    pub r#type: &'static str,
    pub status: GroupAgentScheduledNodeLifecycleStatus,
    pub provider_request_id: String,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub artifact_kind: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalArtifactKind>,
    pub classification: Option<GroupAgentNodeTerminalClassification>,
    pub outcome: Option<GroupAgentNodeTerminalOutcome>,
    pub provider_poll_started: bool,
    pub remote_provider_request_observation: &'static str,
    pub terminal_seen: bool,
    pub stream_eof_seen: bool,
    pub lane_active: bool,
    pub retry_authorized: bool,
    pub lane_release_authorized: bool,
    pub successor_advance_authorized: bool,
    pub dispatch_performed_this_invocation: bool,
    pub database_written_this_invocation: bool,
    pub owner_sidecar_cleanup_observation: &'static str,
    pub owner_sidecar_left_active_by_this_invocation: Option<bool>,
    pub metadata_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
}

impl GroupAgentScheduledNodeDispatchExecutionCliOutput {
    pub fn from_result(
        result: ExecuteGroupAgentScheduledNodeDispatchResult,
        include_result: bool,
    ) -> Self {
        let (inspection, dispatch_performed, database_written) = match result {
            ExecuteGroupAgentScheduledNodeDispatchResult::Terminalized(inspection)
            | ExecuteGroupAgentScheduledNodeDispatchResult::Quarantined(inspection) => {
                (inspection, true, true)
            }
            ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(inspection) => {
                (inspection, false, false)
            }
        };
        Self::from_inspection(
            &inspection,
            dispatch_performed,
            database_written,
            include_result,
        )
        .with_owner_cleanup(ScheduledExecutorOwnerCleanup::NotApplicable)
    }

    pub(super) fn from_result_with_owner_cleanup(
        result: ExecuteGroupAgentScheduledNodeDispatchResult,
        include_result: bool,
        cleanup: ScheduledExecutorOwnerCleanup,
    ) -> Self {
        Self::from_result(result, include_result).with_owner_cleanup(cleanup)
    }

    pub(super) fn from_inspection(
        inspection: &GroupAgentScheduledNodeLifecycleInspection,
        dispatch_performed: bool,
        database_written: bool,
        include_result: bool,
    ) -> Self {
        Self::from_parts(
            inspection.v,
            inspection.status,
            &inspection.claim,
            inspection.active_lane.is_some(),
            inspection.artifact.as_ref(),
            inspection.terminal_receipt.as_ref(),
            dispatch_performed,
            database_written,
            include_result,
        )
    }

    pub(super) fn from_any_inspection(
        inspection: &GroupAgentScheduledNodeAnyLifecycleInspection,
        dispatch_performed: bool,
        database_written: bool,
        include_result: bool,
        cleanup: ScheduledExecutorOwnerCleanup,
    ) -> Self {
        let output = match inspection {
            GroupAgentScheduledNodeAnyLifecycleInspection::Legacy(value) => {
                Self::from_inspection(value, dispatch_performed, database_written, include_result)
            }
            GroupAgentScheduledNodeAnyLifecycleInspection::Ready(value) => Self::from_parts(
                value.v,
                value.status,
                &value.claim,
                value.active_lane.is_some(),
                value.artifact.as_ref(),
                value.terminal_receipt.as_ref(),
                dispatch_performed,
                database_written,
                include_result,
            ),
        };
        output.with_owner_cleanup(cleanup)
    }

    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    fn from_parts(
        v: u16,
        status: GroupAgentScheduledNodeLifecycleStatus,
        claim: &GroupAgentScheduledNodeDispatchClaim,
        lane_active: bool,
        artifact: Option<&GroupAgentScheduledNodeTerminalArtifact>,
        receipt: Option<&GroupAgentScheduledNodeTerminalReceipt>,
        dispatch_performed: bool,
        database_written: bool,
        include_result: bool,
    ) -> Self {
        let result_text = include_result
            .then(|| artifact.map(|value| value.output_text.clone()))
            .flatten();
        Self {
            v,
            r#type: "group_agent_scheduled_node_dispatch_execution",
            status,
            provider_request_id: claim.provider_request_id.clone(),
            graph_run_id: claim.graph_run_id.clone(),
            node_id: claim.node_id.clone(),
            attempt: claim.attempt,
            dispatch_id: claim.dispatch_id.clone(),
            artifact_kind: artifact.map(|value| value.artifact_kind),
            classification: artifact.map(|value| value.classification),
            outcome: receipt.map(|value| value.node_outcome),
            provider_poll_started: artifact.is_some_and(|value| value.provider_poll_started),
            remote_provider_request_observation: "not_attested",
            terminal_seen: artifact.is_some_and(|value| value.terminal_seen),
            stream_eof_seen: artifact.is_some_and(|value| value.stream_eof_seen),
            lane_active,
            retry_authorized: artifact.is_some_and(|value| value.retry_authorized)
                || receipt.is_some_and(|value| value.retry_authorized),
            lane_release_authorized: receipt.is_some_and(|value| value.lane_release_authorized),
            successor_advance_authorized: receipt
                .is_some_and(|value| value.successor_advance_authorized),
            dispatch_performed_this_invocation: dispatch_performed,
            database_written_this_invocation: database_written,
            owner_sidecar_cleanup_observation: "not_applicable",
            owner_sidecar_left_active_by_this_invocation: Some(false),
            metadata_only: result_text.is_none(),
            result_text,
        }
    }

    fn with_owner_cleanup(mut self, cleanup: ScheduledExecutorOwnerCleanup) -> Self {
        (
            self.owner_sidecar_cleanup_observation,
            self.owner_sidecar_left_active_by_this_invocation,
        ) = match cleanup {
            ScheduledExecutorOwnerCleanup::NotApplicable => ("not_applicable", Some(false)),
            ScheduledExecutorOwnerCleanup::Succeeded => ("succeeded", Some(false)),
            ScheduledExecutorOwnerCleanup::Failed => ("failed", None),
        };
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScheduledExecutorOwnerCleanup {
    NotApplicable,
    Succeeded,
    Failed,
}

pub fn write_dispatch_execution_output(
    output: &GroupAgentScheduledNodeDispatchExecutionCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer(&mut *writer, output)?;
        writeln!(writer)
    } else {
        writeln!(
            writer,
            "scheduled graph dispatch {} · provider_request={} · graph_run={} · node={} · attempt={} · dispatch={} · owner_cleanup={} · remote_observation={} · lane_active={} · retry={}",
            status_text(output.status),
            terminal_text(&output.provider_request_id),
            terminal_text(&output.graph_run_id),
            terminal_text(&output.node_id),
            output.attempt,
            terminal_text(&output.dispatch_id),
            output.owner_sidecar_cleanup_observation,
            output.remote_provider_request_observation,
            output.lane_active,
            output.retry_authorized,
        )?;
        if let Some(result) = &output.result_text {
            writeln!(writer, "result: {}", terminal_text(result))?;
        }
        Ok(())
    }
}

fn status_text(status: GroupAgentScheduledNodeLifecycleStatus) -> &'static str {
    match status {
        GroupAgentScheduledNodeLifecycleStatus::Claimed => "claimed",
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => "terminalized",
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => "quarantined",
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated => "adjudicated",
    }
}

#[cfg(test)]
#[path = "scheduled_dispatch_execution_output_tests.rs"]
mod tests;
