use std::io::{self, Write};

use crate::runtime_domain::{
    GroupAgentGraphRunStatus, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeTerminalClassification,
};
use forge_runtime_application::ExecuteGroupAgentNodeDispatchResult;
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeDispatchExecutionCliOutput {
    pub v: u16,
    pub r#type: &'static str,
    pub graph_run_id: String,
    pub dispatch_id: String,
    pub node_id: String,
    pub graph_status: GroupAgentGraphRunStatus,
    pub classification: Option<GroupAgentNodeTerminalClassification>,
    pub provider_poll_started: bool,
    pub terminal_seen: bool,
    pub stream_eof_seen: bool,
    pub lane_active: bool,
    pub retry_authorized: bool,
    pub dispatch_performed_this_invocation: bool,
    pub database_written_this_invocation: bool,
    pub metadata_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
}

impl GroupAgentNodeDispatchExecutionCliOutput {
    pub fn from_result(result: ExecuteGroupAgentNodeDispatchResult, include_result: bool) -> Self {
        let (inspection, performed) = match result {
            ExecuteGroupAgentNodeDispatchResult::Terminalized(inspection) => (inspection, true),
            ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection) => (inspection, false),
        };
        Self::from_inspection(inspection, performed, include_result)
    }

    fn from_inspection(
        inspection: GroupAgentNodeLifecycleInspection,
        performed: bool,
        include_result: bool,
    ) -> Self {
        let artifact = inspection.artifact.as_ref();
        let result_text = include_result
            .then(|| artifact.map(|value| value.output_text.clone()))
            .flatten();
        Self {
            v: 1,
            r#type: "group_agent_node_dispatch_execution",
            graph_run_id: inspection.graph_run.run.graph_run_id,
            dispatch_id: inspection.claim.dispatch_id,
            node_id: inspection.claim.node_id,
            graph_status: inspection.graph_run.run.status,
            classification: artifact.map(|value| value.classification),
            provider_poll_started: artifact.is_some_and(|value| value.provider_poll_started),
            terminal_seen: artifact.is_some_and(|value| value.terminal_seen),
            stream_eof_seen: artifact.is_some_and(|value| value.stream_eof_seen),
            lane_active: inspection.active_lane.is_some(),
            retry_authorized: false,
            dispatch_performed_this_invocation: performed,
            database_written_this_invocation: performed,
            metadata_only: result_text.is_none(),
            result_text,
        }
    }
}

pub fn write_output(
    output: &GroupAgentNodeDispatchExecutionCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer(&mut *writer, output)?;
        writeln!(writer)
    } else {
        writeln!(
            writer,
            "graph dispatch {} · graph_run={} · node={} · dispatch={} · lane_active={} · retry=false",
            status_text(output.graph_status),
            terminal_text(&output.graph_run_id),
            terminal_text(&output.node_id),
            terminal_text(&output.dispatch_id),
            output.lane_active,
        )?;
        if let Some(result) = &output.result_text {
            writeln!(writer, "result: {}", terminal_text(result))?;
        }
        Ok(())
    }
}

fn status_text(status: GroupAgentGraphRunStatus) -> &'static str {
    match status {
        GroupAgentGraphRunStatus::AwaitingExecutionContract => "awaiting_execution_contract",
        GroupAgentGraphRunStatus::AwaitingCoreDispatch => "awaiting_core_dispatch",
        GroupAgentGraphRunStatus::AwaitingDispatchAuthorization => {
            "awaiting_dispatch_authorization"
        }
        GroupAgentGraphRunStatus::DispatchUnknown => "dispatch_unknown",
        GroupAgentGraphRunStatus::Completed => "completed",
        GroupAgentGraphRunStatus::Failed => "failed",
        GroupAgentGraphRunStatus::FailedUncertain => "failed_uncertain",
    }
}

#[cfg(test)]
#[path = "dispatch_execution_output_tests.rs"]
mod tests;
