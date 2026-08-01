use std::io::{self, Write};

use forge_runtime_application::{
    BeginGroupAgentGraphRunDisposition, GroupAgentGraphCorePlan, GroupAgentGraphRunEvent,
    GroupAgentGraphRunInspection, GroupAgentGraphRunRecord, GroupAgentGraphRunStatus,
};
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentGraphRunCliOutput {
    #[serde(rename = "group_agent_graph_run_prepared")]
    Prepared {
        v: u16,
        disposition: BeginGroupAgentGraphRunDisposition,
        inspection: GroupAgentGraphRunInspectionView,
    },
    #[serde(rename = "group_agent_graph_run")]
    GraphAgentRun {
        v: u16,
        inspection: GroupAgentGraphRunInspectionView,
    },
    #[serde(rename = "group_agent_graph_runs")]
    GraphAgentRuns {
        v: u16,
        metadata_only: bool,
        plan_and_journal_validated: bool,
        source_graph_validated: bool,
        execution_contract_present: bool,
        dispatch_authority_released: bool,
        execution_performed: bool,
        manager_execution_performed: bool,
        node_execution_performed: bool,
        model_selected: bool,
        model_used: bool,
        capabilities_granted: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_accessed: bool,
        tools_used: bool,
        task_results_produced: bool,
        conversation_or_prompt_written: bool,
        memory_written: bool,
        writeback_performed: bool,
        explicit_plan_file_read: bool,
        plan_included: bool,
        runs: Vec<GroupAgentGraphRunRecord>,
    },
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentGraphRunInspectionView {
    v: u16,
    plan_admitted: bool,
    source_graph_validated: bool,
    plan_and_journal_validated: bool,
    execution_contract_present: bool,
    dispatch_authority_released: bool,
    execution_performed: bool,
    manager_execution_performed: bool,
    node_execution_performed: bool,
    model_selected: bool,
    model_used: bool,
    capabilities_granted: bool,
    provider_used: bool,
    network_accessed: bool,
    workspace_accessed: bool,
    tools_used: bool,
    task_results_produced: bool,
    conversation_or_prompt_written: bool,
    memory_written: bool,
    writeback_performed: bool,
    explicit_plan_file_read: bool,
    plan_included: bool,
    run: GroupAgentGraphRunRecord,
    events: Vec<GroupAgentGraphRunEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<GroupAgentGraphCorePlan>,
}

impl GroupAgentGraphRunCliOutput {
    pub fn prepared(
        disposition: BeginGroupAgentGraphRunDisposition,
        inspection: GroupAgentGraphRunInspection,
        include_plan: bool,
        explicit_plan_file_read: bool,
    ) -> Self {
        Self::Prepared {
            v: inspection.v,
            disposition,
            inspection: GroupAgentGraphRunInspectionView::new(
                inspection,
                include_plan,
                explicit_plan_file_read,
            ),
        }
    }

    pub fn run(inspection: GroupAgentGraphRunInspection, include_plan: bool) -> Self {
        Self::GraphAgentRun {
            v: inspection.v,
            inspection: GroupAgentGraphRunInspectionView::new(inspection, include_plan, false),
        }
    }

    pub fn list(v: u16, runs: Vec<GroupAgentGraphRunRecord>) -> Self {
        let execution_contract_present = runs.iter().any(|run| run.execution_contract_present);
        Self::GraphAgentRuns {
            v,
            metadata_only: true,
            plan_and_journal_validated: false,
            source_graph_validated: false,
            execution_contract_present,
            dispatch_authority_released: false,
            execution_performed: false,
            manager_execution_performed: false,
            node_execution_performed: false,
            model_selected: execution_contract_present,
            model_used: false,
            capabilities_granted: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            task_results_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            explicit_plan_file_read: false,
            plan_included: false,
            runs,
        }
    }
}

impl GroupAgentGraphRunInspectionView {
    fn new(
        inspection: GroupAgentGraphRunInspection,
        include_plan: bool,
        explicit_plan_file_read: bool,
    ) -> Self {
        let execution_contract_present = inspection.run.execution_contract_present;
        Self {
            v: inspection.v,
            plan_admitted: true,
            source_graph_validated: true,
            plan_and_journal_validated: true,
            execution_contract_present,
            dispatch_authority_released: inspection.run.dispatch_authority_released,
            execution_performed: false,
            manager_execution_performed: false,
            node_execution_performed: false,
            model_selected: execution_contract_present,
            model_used: false,
            capabilities_granted: false,
            provider_used: false,
            network_accessed: false,
            workspace_accessed: false,
            tools_used: false,
            task_results_produced: false,
            conversation_or_prompt_written: false,
            memory_written: false,
            writeback_performed: false,
            explicit_plan_file_read,
            plan_included: include_plan,
            run: inspection.run,
            events: inspection.events,
            plan: include_plan.then_some(inspection.plan),
        }
    }
}

pub fn write_output(
    output: &GroupAgentGraphRunCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match output {
        GroupAgentGraphRunCliOutput::Prepared {
            disposition,
            inspection,
            ..
        } => {
            writeln!(
                writer,
                "prepared passive Group Agent Graph Run {} — {}",
                terminal_text(&inspection.run.graph_run_id),
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentGraphRunCliOutput::GraphAgentRun { inspection, .. } => {
            write_inspection(inspection, writer)
        }
        GroupAgentGraphRunCliOutput::GraphAgentRuns { runs, .. } => write_list(runs, writer),
    }
}

fn write_inspection(
    inspection: &GroupAgentGraphRunInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let run = &inspection.run;
    writeln!(
        writer,
        "graph_run {} · graph={} · status={} · nodes={} · waves={}",
        terminal_text(&run.graph_run_id),
        terminal_text(&run.graph_id),
        status_label(run.status),
        run.node_count,
        run.wave_count
    )?;
    writeln!(writer, "core plan sha256 {}", run.plan_sha256)?;
    if let Some(plan) = &inspection.plan {
        write_plan(plan, writer)?;
    } else {
        writeln!(writer, "plan hidden; use --include-plan to reveal topology")?;
    }
    write_boundaries(
        inspection.explicit_plan_file_read,
        inspection.execution_contract_present,
        writer,
    )
}

fn write_plan(plan: &GroupAgentGraphCorePlan, writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "core plan v={} · scheduler protocol={} · graph version={}",
        plan.v, plan.scheduler_protocol_version, plan.graph_version
    )?;
    writeln!(
        writer,
        "graph={} · graph manifest={} · plan sha256={}",
        terminal_text(&plan.graph_id),
        plan.graph_manifest_sha256,
        plan.plan_sha256
    )?;
    let authored_nodes = plan
        .authored_node_ids
        .iter()
        .map(|node| terminal_text(node))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(writer, "authored nodes: {authored_nodes}")?;
    if plan.edges.is_empty() {
        writeln!(writer, "edges: none")?;
    }
    for edge in &plan.edges {
        writeln!(
            writer,
            "edge {} -> {}",
            terminal_text(&edge.from_node_id),
            terminal_text(&edge.to_node_id)
        )?;
    }
    for (index, wave) in plan.waves.iter().enumerate() {
        let nodes = wave
            .iter()
            .map(|node| terminal_text(node))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(writer, "topology wave {}: {nodes}", index + 1)?;
    }
    writeln!(
        writer,
        "execution contract present={} · dispatch authority released={}",
        plan.execution_contract_present, plan.dispatch_authority_released
    )
}

fn write_list(runs: &[GroupAgentGraphRunRecord], writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Group Agent Graph Runs: {} (metadata only; use show for integrity validation)",
        runs.len()
    )?;
    for run in runs {
        writeln!(
            writer,
            "{}\tgraph={}\tstatus={}\tcreated={}",
            terminal_text(&run.graph_run_id),
            terminal_text(&run.graph_id),
            status_label(run.status),
            run.created_at_ms
        )?;
    }
    write_boundaries(
        false,
        runs.iter().any(|run| run.execution_contract_present),
        writer,
    )
}

fn write_boundaries(
    explicit_plan_file_read: bool,
    execution_contract_present: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if execution_contract_present {
        writeln!(
            writer,
            "execution contract present; dispatch authority not released"
        )?;
    } else {
        writeln!(
            writer,
            "plan receipt only: execution contract absent; dispatch authority not released"
        )?;
    }
    writeln!(
        writer,
        "manager/node Agents not executed; model configuration selected={execution_contract_present}; model not used"
    )?;
    writeln!(
        writer,
        "no provider/network/workspace/tools; no task results"
    )?;
    writeln!(
        writer,
        "no Conversation/Prompt/memory/writeback operation occurred"
    )?;
    writeln!(
        writer,
        "explicit caller-named plan file read: {}",
        if explicit_plan_file_read { "yes" } else { "no" }
    )
}

fn status_label(status: GroupAgentGraphRunStatus) -> &'static str {
    match status {
        GroupAgentGraphRunStatus::AwaitingExecutionContract => "awaiting_execution_contract",
        GroupAgentGraphRunStatus::AwaitingCoreDispatch => "awaiting_core_dispatch",
    }
}

fn disposition_label(disposition: BeginGroupAgentGraphRunDisposition) -> &'static str {
    match disposition {
        BeginGroupAgentGraphRunDisposition::Created => "created",
        BeginGroupAgentGraphRunDisposition::Replayed => "replayed",
    }
}
