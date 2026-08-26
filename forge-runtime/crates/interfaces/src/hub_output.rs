#[path = "hub_output_group_human.rs"]
mod group_human;
#[path = "prompt_receipt.rs"]
mod prompt_receipt;
#[path = "run_explain_output.rs"]
mod run_explain_output;

use crate::run_lineage_output::{RunLineageView, write as write_run_lineage};
use prompt_receipt::PromptReceipt;
pub(crate) use run_explain_output::{RunExplanationView, write_run_explanation};

use crate::runtime_domain::{
    BeginGroupExecutionDisposition, Conversation, ConversationScope, GroupProjectMember,
    GroupRunRecord, HubSnapshot, PrepareGroupRunDisposition, PromptRecord, RunInspection,
    RunRecord, RunRecoveryState, RuntimeEventKind, SessionGroup,
};
use crate::{
    group_analysis_panel_output::{
        GroupAnalysisPanelInspectionView, write_list as write_group_analysis_panel_list,
        write_panel as write_group_analysis_panel,
        write_prepared as write_group_analysis_panel_prepared,
    },
    group_context_output::{GroupContextView, write_group_context},
    group_execution_output::{
        GroupExecutionInspectionView, write_group_execution, write_group_execution_list,
        write_group_execution_started,
    },
    group_model_analysis_output::{
        GroupModelAnalysisInspectionView, GroupModelAnalysisSendDisposition,
        write_analysis as write_group_model_analysis,
        write_list as write_group_model_analysis_list,
        write_prepared as write_group_model_analysis_prepared,
        write_sent as write_group_model_analysis_sent,
    },
    group_panel_synthesis_output::{
        GroupPanelSynthesisInspectionView, GroupPanelSynthesisListItemView,
        GroupPanelSynthesisSendDisposition, write_list as write_group_panel_synthesis_list,
        write_prepared as write_group_panel_synthesis_prepared,
        write_sent as write_group_panel_synthesis_sent,
        write_synthesis as write_group_panel_synthesis,
    },
    group_run_output::{
        GroupRunSnapshotView, write_group_run, write_group_run_list, write_group_run_prepared,
    },
};
use serde::Serialize;
use std::io::{self, Write};
#[derive(Debug, Serialize)]
pub struct CliOutput {
    pub v: u16,
    #[serde(flatten)]
    pub kind: OutputKind,
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputKind {
    HubStatus {
        schema_version: i64,
        expected_schema_version: i64,
        migration_pending: bool,
        backups: usize,
        healthy: bool,
    },
    Hub {
        snapshot: HubSnapshot,
        remote: RemoteStatus,
    },
    Sessions {
        scope: ConversationScope,
        sessions: Vec<Conversation>,
    },
    SessionCreated {
        session: Conversation,
    },
    PromptAdded {
        prompt: PromptReceipt,
    },
    Prompts {
        prompts: Vec<PromptRecord>,
    },
    GroupCreated {
        group: SessionGroup,
    },
    GroupLinked {
        member: GroupProjectMember,
    },
    GroupContext {
        context: GroupContextView,
    },
    GroupRunPrepared {
        disposition: PrepareGroupRunDisposition,
        snapshot: GroupRunSnapshotView,
    },
    GroupRun {
        snapshot: GroupRunSnapshotView,
    },
    GroupRuns {
        runs: Vec<GroupRunRecord>,
    },
    GroupExecutionStarted {
        disposition: BeginGroupExecutionDisposition,
        inspection: GroupExecutionInspectionView,
    },
    GroupExecution {
        inspection: GroupExecutionInspectionView,
    },
    GroupExecutions {
        metadata_only: bool,
        source_and_journal_validated: bool,
        inspect_with: &'static str,
        executions: Vec<crate::runtime_domain::GroupExecutionRecord>,
    },
    GroupModelAnalysisPrepared {
        disposition: crate::runtime_domain::PrepareGroupModelAnalysisDisposition,
        inspection: GroupModelAnalysisInspectionView,
    },
    GroupModelAnalysisSent {
        disposition: GroupModelAnalysisSendDisposition,
        inspection: GroupModelAnalysisInspectionView,
    },
    GroupModelAnalysis {
        inspection: GroupModelAnalysisInspectionView,
    },
    GroupModelAnalyses {
        metadata_only: bool,
        source_and_journal_validated: bool,
        inspect_with: &'static str,
        analyses: Vec<crate::runtime_domain::GroupModelAnalysisRecord>,
    },
    GroupAnalysisPanelPrepared {
        disposition: crate::runtime_domain::PrepareGroupAnalysisPanelDisposition,
        panel: GroupAnalysisPanelInspectionView,
    },
    GroupAnalysisPanel {
        panel: GroupAnalysisPanelInspectionView,
    },
    GroupAnalysisPanels {
        metadata_only: bool,
        source_and_results_validated: bool,
        inspect_with: &'static str,
        panels: Vec<crate::runtime_domain::GroupAnalysisPanelRecord>,
    },
    GroupPanelSynthesisPrepared {
        disposition: crate::runtime_domain::PrepareGroupPanelSynthesisDisposition,
        inspection: GroupPanelSynthesisInspectionView,
    },
    GroupPanelSynthesisSent {
        disposition: GroupPanelSynthesisSendDisposition,
        inspection: GroupPanelSynthesisInspectionView,
    },
    GroupPanelSynthesis {
        inspection: GroupPanelSynthesisInspectionView,
    },
    GroupPanelSyntheses {
        metadata_only: bool,
        source_and_journal_validated: bool,
        inspect_with: &'static str,
        syntheses: Vec<GroupPanelSynthesisListItemView>,
    },
    Groups {
        groups: Vec<SessionGroup>,
    },
    Runs {
        runs: Vec<RunRecord>,
    },
    Run {
        inspection: RunInspection,
    },
    RunExplanation {
        explanation: RunExplanationView,
    },
    RunLineage {
        #[serde(flatten)]
        view: RunLineageView,
    },
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteStatus {
    NotConfigured,
}
impl CliOutput {
    pub fn new(kind: OutputKind) -> Self {
        Self { v: 1, kind }
    }
}
pub fn write_output(
    output: &CliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)?;
        return Ok(());
    }
    write_human(&output.kind, writer)
}
fn write_human(kind: &OutputKind, writer: &mut impl Write) -> Result<(), io::Error> {
    match kind {
        OutputKind::HubStatus {
            schema_version,
            expected_schema_version,
            migration_pending,
            backups,
            healthy,
        } => write_hub_status(
            *schema_version,
            *expected_schema_version,
            *migration_pending,
            *backups,
            *healthy,
            writer,
        ),
        OutputKind::Hub { snapshot, remote } => write_hub(snapshot, *remote, writer),
        OutputKind::Sessions { scope, sessions } => write_sessions(scope, sessions, writer),
        OutputKind::SessionCreated { session } => {
            writeln!(writer, "created session {} — {}", session.id, session.title)
        }
        OutputKind::PromptAdded { prompt } => write_prompt_added(prompt, writer),
        OutputKind::Prompts { prompts } => write_prompts(prompts, writer),
        OutputKind::GroupCreated { group } => write_group_created(group, writer),
        OutputKind::GroupLinked { member } => write_group_linked(member, writer),
        OutputKind::GroupContext { .. }
        | OutputKind::GroupRunPrepared { .. }
        | OutputKind::GroupRun { .. }
        | OutputKind::GroupRuns { .. }
        | OutputKind::GroupExecutionStarted { .. }
        | OutputKind::GroupExecution { .. }
        | OutputKind::GroupExecutions { .. }
        | OutputKind::GroupModelAnalysisPrepared { .. }
        | OutputKind::GroupModelAnalysisSent { .. }
        | OutputKind::GroupModelAnalysis { .. }
        | OutputKind::GroupModelAnalyses { .. }
        | OutputKind::GroupAnalysisPanelPrepared { .. }
        | OutputKind::GroupAnalysisPanel { .. }
        | OutputKind::GroupAnalysisPanels { .. }
        | OutputKind::GroupPanelSynthesisPrepared { .. }
        | OutputKind::GroupPanelSynthesisSent { .. }
        | OutputKind::GroupPanelSynthesis { .. }
        | OutputKind::GroupPanelSyntheses { .. }
        | OutputKind::Groups { .. }
        | OutputKind::Runs { .. }
        | OutputKind::Run { .. }
        | OutputKind::RunExplanation { .. }
        | OutputKind::RunLineage { .. } => group_human::write(kind, writer),
    }
}
fn write_group_linked(
    member: &GroupProjectMember,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "linked project {} to group {} as {}",
        member.project_id, member.group_id, member.role
    )
}
fn write_group_created(group: &SessionGroup, writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "created local-private group {} — {}",
        group.id, group.name
    )
}
fn write_group_synthesis_kind(kind: &OutputKind, writer: &mut impl Write) -> Result<(), io::Error> {
    match kind {
        OutputKind::GroupPanelSynthesisPrepared {
            disposition,
            inspection,
        } => write_group_panel_synthesis_prepared(*disposition, inspection, writer),
        OutputKind::GroupPanelSynthesisSent {
            disposition,
            inspection,
        } => write_group_panel_synthesis_sent(*disposition, inspection, writer),
        OutputKind::GroupPanelSynthesis { inspection } => {
            write_group_panel_synthesis(inspection, writer)
        }
        OutputKind::GroupPanelSyntheses { syntheses, .. } => {
            write_group_panel_synthesis_list(syntheses, writer)
        }
        _ => unreachable!("caller routes only Group Panel Synthesis output"),
    }
}
fn write_prompt_added(prompt: &PromptReceipt, writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "remembered prompt {} in session {}",
        prompt.id, prompt.conversation_id
    )
}
fn write_group_panel_kind(kind: &OutputKind, writer: &mut impl Write) -> Result<(), io::Error> {
    match kind {
        OutputKind::GroupAnalysisPanelPrepared { disposition, panel } => {
            write_group_analysis_panel_prepared(*disposition, panel, writer)
        }
        OutputKind::GroupAnalysisPanel { panel } => write_group_analysis_panel(panel, writer),
        OutputKind::GroupAnalysisPanels { panels, .. } => {
            write_group_analysis_panel_list(panels, writer)
        }
        _ => unreachable!("caller routes only Group Analysis Panel output"),
    }
}
fn write_group_model_kind(kind: &OutputKind, writer: &mut impl Write) -> Result<(), io::Error> {
    match kind {
        OutputKind::GroupModelAnalysisPrepared {
            disposition,
            inspection,
        } => write_group_model_analysis_prepared(*disposition, inspection, writer),
        OutputKind::GroupModelAnalysisSent {
            disposition,
            inspection,
        } => write_group_model_analysis_sent(*disposition, inspection, writer),
        OutputKind::GroupModelAnalysis { inspection } => {
            write_group_model_analysis(inspection, writer)
        }
        OutputKind::GroupModelAnalyses { analyses, .. } => {
            write_group_model_analysis_list(analyses, writer)
        }
        _ => unreachable!("caller routes only Group Model Analysis output"),
    }
}
fn write_hub(
    snapshot: &HubSnapshot,
    remote: RemoteStatus,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(writer, "Forge Hub [{}]", scope_label(&snapshot.scope))?;
    writeln!(
        writer,
        "{} session(s) · {} project(s) · {} local-private group(s)",
        snapshot.conversations.len(),
        snapshot.projects.len(),
        snapshot.groups.len()
    )?;
    writeln!(writer, "remote: {}", remote_label(remote))?;
    for project in &snapshot.projects {
        writeln!(
            writer,
            "project {} — {} ({})",
            project.id,
            project.name,
            project.path.display()
        )?;
    }
    for session in &snapshot.conversations {
        writeln!(writer, "session {} — {}", session.id, session.title)?;
    }
    for member in &snapshot.group_project_members {
        writeln!(
            writer,
            "link {} → {} ({})",
            member.group_id, member.project_id, member.role
        )?;
    }
    Ok(())
}
fn write_sessions(
    scope: &ConversationScope,
    sessions: &[Conversation],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "sessions [{}]: {}",
        scope_label(scope),
        sessions.len()
    )?;
    for session in sessions {
        writeln!(
            writer,
            "{}\t{}\tupdated={}",
            session.id, session.title, session.updated_at_ms
        )?;
    }
    Ok(())
}
fn write_prompts(prompts: &[PromptRecord], writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(writer, "prompts: {}", prompts.len())?;
    for prompt in prompts {
        writeln!(
            writer,
            "{}\t{}\t{}\t{}",
            prompt.id, prompt.conversation_id, prompt.role, prompt.content
        )?;
    }
    Ok(())
}

fn write_groups(groups: &[SessionGroup], writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(writer, "local-private groups: {}", groups.len())?;
    for group in groups {
        writeln!(writer, "{}\t{}", group.id, group.name)?;
    }
    Ok(())
}

fn write_runs(runs: &[RunRecord], writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(writer, "runs: {}", runs.len())?;
    for run in runs {
        writeln!(
            writer,
            "{}\tconversation={}\tprompt={}\tcreated={}",
            run.run_id, run.conversation_id, run.prompt_id, run.created_at_ms
        )?;
    }
    Ok(())
}

fn write_run(inspection: &RunInspection, writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "run {} — {} — {} event(s)",
        inspection.run.run_id,
        recovery_label(&inspection.recovery.state),
        inspection.events.len()
    )?;
    for event in &inspection.events {
        writeln!(
            writer,
            "{}\t{}\t{}",
            event.seq,
            event.emitted_at_ms,
            event_label(&event.kind)
        )?;
    }
    Ok(())
}

fn recovery_label(state: &RunRecoveryState) -> String {
    match state {
        RunRecoveryState::Terminal { .. } => "terminal".into(),
        RunRecoveryState::Incomplete => "incomplete".into(),
        RunRecoveryState::PendingTool { calls } => {
            format!("blocked_pending_tool({})", calls.len())
        }
    }
}

fn event_label(kind: &RuntimeEventKind) -> &'static str {
    match kind {
        RuntimeEventKind::RunStarted { .. } => "run_started",
        RuntimeEventKind::TurnStarted { .. } => "turn_started",
        RuntimeEventKind::AssistantDelta { .. } => "assistant_delta",
        RuntimeEventKind::MessageCommitted { .. } => "message_committed",
        RuntimeEventKind::ToolStarted { .. } => "tool_started",
        RuntimeEventKind::ToolFinished { .. } => "tool_finished",
        RuntimeEventKind::ToolRejected { .. } => "tool_rejected",
        RuntimeEventKind::RuntimeError { .. } => "runtime_error",
        RuntimeEventKind::RunFinished { .. } => "run_finished",
    }
}

fn scope_label(scope: &ConversationScope) -> String {
    match scope {
        ConversationScope::Global => "global".into(),
        ConversationScope::Project(id) => format!("project:{id}"),
        ConversationScope::Group(id) => format!("group:{id}"),
    }
}

fn remote_label(status: RemoteStatus) -> &'static str {
    match status {
        RemoteStatus::NotConfigured => "not configured (phase 1 is local-only)",
    }
}

fn write_hub_status(
    schema_version: i64,
    expected: i64,
    migration_pending: bool,
    backups: usize,
    healthy: bool,
    writer: &mut impl io::Write,
) -> io::Result<()> {
    writeln!(
        writer,
        "schema_version: {schema_version} (expected {expected})"
    )?;
    writeln!(
        writer,
        "migration_pending: {migration_pending}\nbackups: {backups}\nhealthy: {healthy}"
    )
}

#[cfg(test)]
#[path = "hub_output_tests.rs"]
mod tests;
