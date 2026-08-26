use std::io::{self, Write};

use crate::group_context_output::terminal_text;
use forge_runtime_domain::RunInspection;
use serde::Serialize;

#[path = "run_explain_projection.rs"]
mod projection;

/// A content-free, journal-derived explanation of one Project Run.
///
/// This view intentionally reports evidence and boundaries rather than
/// claiming semantic truth. In particular, the Project Run v1 record does not
/// snapshot the preceding Conversation history or an authenticated
/// Grant/Approval/PDP decision.
#[derive(Debug, Serialize)]
pub(crate) struct RunExplanationView {
    pub run_id: String,
    pub project_id: String,
    pub conversation_id: String,
    pub prompt_id: String,
    pub provider: &'static str,
    pub recovery: RecoveryView,
    pub continuation: ContinuationView,
    pub evidence: Vec<EvidenceView>,
    pub context: ContextView,
    pub authorization: AuthorizationView,
    pub open_assumptions: Vec<AssumptionView>,
    pub interpretation_boundary: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecoveryView {
    pub status: &'static str,
    pub outcome: Option<&'static str>,
    pub pending_tool_calls: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ContinuationView {
    pub command: Option<String>,
    pub safe: bool,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct EvidenceView {
    pub seq: u64,
    pub kind: &'static str,
    pub supports: &'static str,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ContextView {
    pub current_prompt: Option<ContentFingerprint>,
    pub system_prompt: ContentFingerprint,
    pub committed_messages: Vec<MessageView>,
    pub observed_tool_calls: Vec<ToolObservationView>,
    pub prior_conversation_history: BoundaryView,
    pub workspace_outside_configured_read_scope: BoundaryView,
}

#[derive(Debug, Serialize)]
pub(crate) struct ContentFingerprint {
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MessageView {
    pub seq: u64,
    pub role: &'static str,
    pub content: Option<ContentFingerprint>,
    pub tool_calls: usize,
    pub provider: Option<ContentFingerprint>,
    pub provider_items: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ToolObservationView {
    pub seq: u64,
    pub call_id_fingerprint: ContentFingerprint,
    pub name_label: &'static str,
    pub name_fingerprint: ContentFingerprint,
    pub outcome: &'static str,
    pub output: Option<ContentFingerprint>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BoundaryView {
    pub status: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuthorizationView {
    pub source: &'static str,
    pub workspace_read: CapabilityView,
    pub workspace_write: CapabilityView,
    pub process: CapabilityView,
    pub network: CapabilityView,
}

#[derive(Debug, Serialize)]
pub(crate) struct CapabilityView {
    pub status: &'static str,
    pub scope: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AssumptionView {
    pub id: &'static str,
    pub status: &'static str,
    pub reason: &'static str,
}

impl RunExplanationView {
    pub(crate) fn from_inspection(inspection: &RunInspection) -> Result<Self, String> {
        projection::from_inspection(inspection)
    }
}

pub(crate) fn write_run_explanation(
    explanation: &RunExplanationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let continuation = explanation
        .continuation
        .command
        .as_deref()
        .map_or_else(|| "none".into(), terminal_text);
    let outcome = explanation.recovery.outcome.unwrap_or("none");
    writeln!(
        writer,
        "run explanation {}",
        terminal_text(&explanation.run_id)
    )?;
    writeln!(
        writer,
        "status={} outcome={} provider={} evidence={} continuation={}",
        explanation.recovery.status,
        outcome,
        explanation.provider,
        explanation.evidence.len(),
        continuation
    )?;
    writeln!(
        writer,
        "continuation safe={}: {}",
        explanation.continuation.safe, explanation.continuation.reason
    )?;
    write_evidence(explanation, writer)?;
    write_messages(explanation, writer)?;
    write_authorization(explanation, writer)?;
    write_tool_observations(explanation, writer)?;
    write_assumptions(explanation, writer)
}

fn write_evidence(
    explanation: &RunExplanationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let streamed = explanation
        .evidence
        .iter()
        .filter(|item| item.kind == "assistant_delta")
        .count();
    writeln!(
        writer,
        "evidence (assistant_delta events summarized={streamed}):"
    )?;
    for item in explanation
        .evidence
        .iter()
        .filter(|item| item.kind != "assistant_delta")
    {
        writeln!(
            writer,
            "{}\t{}\t{}\t{}",
            item.seq,
            item.kind,
            item.supports,
            terminal_text(&item.detail)
        )?;
    }
    Ok(())
}

fn write_authorization(
    explanation: &RunExplanationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "authorization: {}",
        explanation.authorization.source
    )?;
    writeln!(
        writer,
        "workspace read scope: status={} paths={}",
        explanation.authorization.workspace_read.status,
        explanation.authorization.workspace_read.scope.len()
    )?;
    for path in &explanation.authorization.workspace_read.scope {
        writeln!(writer, "scope\t{}", terminal_text(path))?;
    }
    Ok(())
}

fn write_messages(
    explanation: &RunExplanationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "committed messages: {}",
        explanation.context.committed_messages.len()
    )?;
    for message in &explanation.context.committed_messages {
        let content = fingerprint_summary("content", message.content.as_ref());
        let provider = fingerprint_summary("provider", message.provider.as_ref());
        writeln!(
            writer,
            "{}\t{}\t{}\t{}\ttool_calls={} provider_items={}",
            message.seq,
            message.role,
            content,
            provider,
            message.tool_calls,
            message.provider_items
        )?;
    }
    Ok(())
}

fn fingerprint_summary(label: &str, value: Option<&ContentFingerprint>) -> String {
    value.map_or_else(
        || format!("{label}=none"),
        |value| {
            format!(
                "{label}_bytes={} {label}_sha256={}",
                value.bytes, value.sha256
            )
        },
    )
}

fn write_tool_observations(
    explanation: &RunExplanationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "tool observations: {}",
        explanation.context.observed_tool_calls.len()
    )?;
    for observation in &explanation.context.observed_tool_calls {
        let output = fingerprint_summary("output", observation.output.as_ref());
        writeln!(
            writer,
            "{}\t{}\tname_bytes={} name_sha256={}\t{}\tcall_id_bytes={} call_id_sha256={}\t{}",
            observation.seq,
            observation.name_label,
            observation.name_fingerprint.bytes,
            observation.name_fingerprint.sha256,
            observation.outcome,
            observation.call_id_fingerprint.bytes,
            observation.call_id_fingerprint.sha256,
            output
        )?;
    }
    Ok(())
}

fn write_assumptions(
    explanation: &RunExplanationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "open assumptions: {}",
        explanation.open_assumptions.len()
    )?;
    for assumption in &explanation.open_assumptions {
        writeln!(
            writer,
            "{}\t{}\t{}",
            assumption.id, assumption.status, assumption.reason
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "run_explain_output_tests.rs"]
mod tests;
