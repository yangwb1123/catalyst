//! Safe JSON and human views for local prepared Group Agent Graphs.

use std::io::{self, Write};

use serde::Serialize;

use crate::{
    group_context_output::terminal_text,
    runtime_domain::{
        GroupAgentGraphInspection, GroupAgentGraphManifest, GroupAgentGraphRecord,
        PrepareGroupAgentGraphDisposition,
    },
};

#[derive(Debug, Serialize)]
pub struct GroupAgentGraphCliOutput {
    pub v: u16,
    #[serde(flatten)]
    pub kind: GroupAgentGraphOutputKind,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum GroupAgentGraphOutputKind {
    #[serde(rename = "group_agent_graph_prepared")]
    Prepared {
        disposition: PrepareGroupAgentGraphDisposition,
        inspection: GroupAgentGraphInspectionView,
    },
    #[serde(rename = "group_agent_graph")]
    Graph {
        inspection: GroupAgentGraphInspectionView,
    },
    #[serde(rename = "group_agent_graphs")]
    List {
        metadata_only: bool,
        source_and_manifest_validated: bool,
        graphs_are_prepared_only: bool,
        execution_performed: bool,
        manager_execution_performed: bool,
        node_execution_performed: bool,
        profiles_are_labels: bool,
        model_selected: bool,
        model_used: bool,
        capabilities_granted: bool,
        task_results_produced: bool,
        memory_written: bool,
        provider_used: bool,
        network_accessed: bool,
        workspace_scanned: bool,
        explicit_spec_file_read: bool,
        tools_used: bool,
        writeback_performed: bool,
        inspect_with: &'static str,
        graphs: Vec<GroupAgentGraphRecord>,
    },
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentGraphInspectionView {
    pub v: u16,
    pub graph_prepared: bool,
    pub source_and_manifest_validated: bool,
    pub execution_performed: bool,
    pub manager_execution_performed: bool,
    pub node_execution_performed: bool,
    pub profiles_are_labels: bool,
    pub model_selected: bool,
    pub model_used: bool,
    pub capabilities_granted: bool,
    pub task_results_produced: bool,
    pub memory_written: bool,
    pub provider_used: bool,
    pub network_accessed: bool,
    pub workspace_scanned: bool,
    pub explicit_spec_file_read: bool,
    pub tools_used: bool,
    pub writeback_performed: bool,
    pub spec_included: bool,
    pub graph: GroupAgentGraphRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<GroupAgentGraphManifest>,
}

impl GroupAgentGraphCliOutput {
    pub fn prepared(
        disposition: PrepareGroupAgentGraphDisposition,
        inspection: GroupAgentGraphInspection,
        explicit_spec_file_read: bool,
    ) -> Self {
        Self {
            v: 1,
            kind: GroupAgentGraphOutputKind::Prepared {
                disposition,
                inspection: GroupAgentGraphInspectionView::new(
                    inspection,
                    false,
                    explicit_spec_file_read,
                ),
            },
        }
    }

    pub fn graph(inspection: GroupAgentGraphInspection, include_spec: bool) -> Self {
        Self {
            v: 1,
            kind: GroupAgentGraphOutputKind::Graph {
                inspection: GroupAgentGraphInspectionView::new(inspection, include_spec, false),
            },
        }
    }

    pub fn list(graphs: Vec<GroupAgentGraphRecord>) -> Self {
        Self {
            v: 1,
            kind: GroupAgentGraphOutputKind::List {
                metadata_only: true,
                source_and_manifest_validated: false,
                graphs_are_prepared_only: true,
                execution_performed: false,
                manager_execution_performed: false,
                node_execution_performed: false,
                profiles_are_labels: true,
                model_selected: false,
                model_used: false,
                capabilities_granted: false,
                task_results_produced: false,
                memory_written: false,
                provider_used: false,
                network_accessed: false,
                workspace_scanned: false,
                explicit_spec_file_read: false,
                tools_used: false,
                writeback_performed: false,
                inspect_with: "group graph show GRAPH_ID",
                graphs,
            },
        }
    }
}

impl GroupAgentGraphInspectionView {
    fn new(
        inspection: GroupAgentGraphInspection,
        include_spec: bool,
        explicit_spec_file_read: bool,
    ) -> Self {
        Self {
            v: inspection.v,
            graph_prepared: true,
            source_and_manifest_validated: true,
            execution_performed: false,
            manager_execution_performed: false,
            node_execution_performed: false,
            profiles_are_labels: true,
            model_selected: false,
            model_used: false,
            capabilities_granted: false,
            task_results_produced: false,
            memory_written: false,
            provider_used: false,
            network_accessed: false,
            workspace_scanned: false,
            explicit_spec_file_read,
            tools_used: false,
            writeback_performed: false,
            spec_included: include_spec,
            graph: inspection.graph,
            manifest: include_spec.then_some(inspection.manifest),
        }
    }
}

pub fn write_output(
    output: &GroupAgentGraphCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    match &output.kind {
        GroupAgentGraphOutputKind::Prepared {
            disposition,
            inspection,
        } => {
            writeln!(
                writer,
                "prepared local Group Agent Graph {} — {}",
                terminal_text(&inspection.graph.graph_id),
                disposition_label(*disposition)
            )?;
            write_inspection(inspection, writer)
        }
        GroupAgentGraphOutputKind::Graph { inspection } => write_inspection(inspection, writer),
        GroupAgentGraphOutputKind::List { graphs, .. } => write_list(graphs, writer),
    }
}

fn write_inspection(
    inspection: &GroupAgentGraphInspectionView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let graph = &inspection.graph;
    writeln!(
        writer,
        "graph {} · group_run={} · nodes={} · edges={} · waves={}",
        terminal_text(&graph.graph_id),
        terminal_text(&graph.group_run_id),
        graph.node_count,
        graph.edge_count,
        graph.wave_count
    )?;
    writeln!(writer, "manifest sha256 {}", graph.manifest_sha256)?;
    if let Some(manifest) = &inspection.manifest {
        write_manifest(manifest, writer)?;
    } else {
        writeln!(
            writer,
            "spec hidden; use --include-spec to reveal instructions and tasks"
        )?;
    }
    write_boundaries(inspection.explicit_spec_file_read, writer)
}

fn write_boundaries(
    explicit_spec_file_read: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "prepared graph only: manager/node Agents not executed; no model selected or used; no provider, network, workspace scan, tools, or writeback"
    )?;
    writeln!(
        writer,
        "explicit caller-named spec file read: {}; no other workspace discovery or traversal",
        if explicit_spec_file_read { "yes" } else { "no" }
    )?;
    writeln!(
        writer,
        "Agent profiles are descriptive labels only; no capabilities were granted"
    )?;
    writeln!(writer, "task results: none; persistent memory: not written")
}

fn write_manifest(
    manifest: &GroupAgentGraphManifest,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "manager profile={} · instruction={}",
        terminal_text(&manifest.manager.agent_profile),
        terminal_text(&manifest.manager.instruction)
    )?;
    for node in &manifest.nodes {
        writeln!(
            writer,
            "node {} · project={} · role={} · profile={}",
            terminal_text(&node.node_id),
            terminal_text(&node.project_id),
            terminal_text(&node.member_role),
            terminal_text(&node.agent_profile)
        )?;
        writeln!(writer, "  task: {}", terminal_text(&node.task))?;
        writeln!(writer, "  acceptance: {}", terminal_text(&node.acceptance))?;
    }
    for edge in &manifest.edges {
        writeln!(
            writer,
            "edge {} -> {}",
            terminal_text(&edge.from_node_id),
            terminal_text(&edge.to_node_id)
        )?;
    }
    for (position, wave) in manifest.waves.iter().enumerate() {
        let nodes = wave
            .iter()
            .map(|node| terminal_text(node))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(writer, "wave {}: {nodes}", position + 1)?;
    }
    Ok(())
}

fn write_list(graphs: &[GroupAgentGraphRecord], writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "Group Agent Graphs: {} (metadata only; use show for integrity validation)",
        graphs.len()
    )?;
    for graph in graphs {
        writeln!(
            writer,
            "{}\tgroup_run={}\tnodes={}\tedges={}\twaves={}\tcreated={}",
            terminal_text(&graph.graph_id),
            terminal_text(&graph.group_run_id),
            graph.node_count,
            graph.edge_count,
            graph.wave_count,
            graph.created_at_ms
        )?;
    }
    write_boundaries(false, writer)
}

fn disposition_label(disposition: PrepareGroupAgentGraphDisposition) -> &'static str {
    match disposition {
        PrepareGroupAgentGraphDisposition::Created => "created",
        PrepareGroupAgentGraphDisposition::Replayed => "replayed",
    }
}
