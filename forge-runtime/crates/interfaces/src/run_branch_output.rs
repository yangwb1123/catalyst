use std::io::{self, Write};

use forge_runtime_application::PrepareRunBranchResult;
use serde::Serialize;

use crate::runtime_domain::{BeginRunDisposition, RunBranchMode, RunInspection, RunRecoveryState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BranchTargetState {
    ReadyToResume,
    Incomplete,
    PendingToolEffect,
    Terminal,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)] // Explicit booleans are stable CLI wire fields.
pub(crate) struct RunBranchOutput {
    v: u16,
    #[serde(rename = "type")]
    kind: &'static str,
    disposition: BeginRunDisposition,
    run_id: String,
    parent_run_id: String,
    branch_mode: RunBranchMode,
    source_event_seq: u64,
    source_event_sha256: String,
    state: BranchTargetState,
    journal_events: usize,
    resume_required: bool,
    external_effects: bool,
    context_snapshot_bound: bool,
    workspace_snapshot_bound: bool,
}

impl RunBranchOutput {
    pub(crate) fn new(result: PrepareRunBranchResult) -> Self {
        let state = BranchTargetState::from_inspection(&result.inspection);
        Self {
            v: 1,
            kind: "run_branch_prepared",
            disposition: result.disposition,
            run_id: result.inspection.run.run_id,
            parent_run_id: result.lineage.parent_run_id,
            branch_mode: result.lineage.branch_mode,
            source_event_seq: result.lineage.source_event_seq,
            source_event_sha256: result.lineage.source_event_sha256,
            state,
            journal_events: result.inspection.events.len(),
            resume_required: state.resume_required(),
            external_effects: false,
            context_snapshot_bound: false,
            workspace_snapshot_bound: false,
        }
    }
}

impl BranchTargetState {
    fn from_inspection(inspection: &RunInspection) -> Self {
        match &inspection.recovery.state {
            RunRecoveryState::Terminal { .. } => Self::Terminal,
            RunRecoveryState::PendingTool { .. } => Self::PendingToolEffect,
            RunRecoveryState::Incomplete if inspection.events.len() == 1 => Self::ReadyToResume,
            RunRecoveryState::Incomplete => Self::Incomplete,
        }
    }

    const fn resume_required(self) -> bool {
        matches!(self, Self::ReadyToResume | Self::Incomplete)
    }
}

pub(crate) fn write(
    output: &RunBranchOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    let run_id = crate::group_context_output::terminal_text(&output.run_id);
    let parent = crate::group_context_output::terminal_text(&output.parent_run_id);
    match output.state {
        BranchTargetState::ReadyToResume => writeln!(
            writer,
            "prepared branch Run {run_id} from {parent} ({:?}); resume explicitly with `run resume {run_id}`",
            output.disposition,
        ),
        BranchTargetState::Incomplete => writeln!(
            writer,
            "branch Run {run_id} from {parent} is incomplete ({:?}); continue explicitly with `run resume {run_id}`",
            output.disposition,
        ),
        BranchTargetState::PendingToolEffect => writeln!(
            writer,
            "branch Run {run_id} from {parent} has a pending tool effect ({:?}); inspect before operator action",
            output.disposition,
        ),
        BranchTargetState::Terminal => writeln!(
            writer,
            "branch Run {run_id} from {parent} is already terminal ({:?}); no resume is required",
            output.disposition,
        ),
    }
}
