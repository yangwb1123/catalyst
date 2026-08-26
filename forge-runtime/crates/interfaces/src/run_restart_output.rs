use std::io::{self, Write};

use forge_runtime_application::PrepareRunRestartResult;
use forge_runtime_domain::{BeginRunDisposition, RunInspection, RunRecoveryState};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RestartTargetState {
    ReadyToResume,
    Incomplete,
    PendingToolEffect,
    Terminal,
}

#[derive(Debug, Serialize)]
pub(crate) struct RunRestartOutput {
    v: u16,
    #[serde(rename = "type")]
    kind: &'static str,
    disposition: BeginRunDisposition,
    run_id: String,
    state: RestartTargetState,
    journal_events: usize,
    resume_required: bool,
    external_effects: bool,
}

impl RunRestartOutput {
    pub(crate) fn new(result: PrepareRunRestartResult) -> Self {
        let state = RestartTargetState::from_inspection(&result.inspection);
        Self {
            v: 1,
            kind: "run_restart_prepared",
            disposition: result.disposition,
            run_id: result.inspection.run.run_id,
            state,
            journal_events: result.inspection.events.len(),
            resume_required: state.resume_required(),
            external_effects: false,
        }
    }
}

impl RestartTargetState {
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
    output: &RunRestartOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    let run_id = crate::group_context_output::terminal_text(&output.run_id);
    match output.state {
        RestartTargetState::ReadyToResume => writeln!(
            writer,
            "prepared restart Run {run_id} ({:?}); resume explicitly with `run resume {run_id}`",
            output.disposition,
        ),
        RestartTargetState::Incomplete => writeln!(
            writer,
            "restart Run {run_id} is incomplete ({:?}); continue explicitly with `run resume {run_id}`",
            output.disposition,
        ),
        RestartTargetState::PendingToolEffect => writeln!(
            writer,
            "restart Run {run_id} has a pending tool effect ({:?}); inspect before operator action",
            output.disposition,
        ),
        RestartTargetState::Terminal => writeln!(
            writer,
            "restart Run {run_id} is already terminal ({:?}); no resume is required",
            output.disposition,
        ),
    }
}
