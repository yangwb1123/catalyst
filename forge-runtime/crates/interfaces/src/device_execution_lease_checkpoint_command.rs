//! Local, bounded consumer for the execution-lease checkpoint contract.
//!
//! This command validates a caller-supplied restart image with the pure domain
//! state machine and renders only bounded status metadata. It never restores a
//! live lease, persists a terminal, reserves capacity, or dispatches a Runner.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::lease::{LeaseCheckpoint, LeaseState};
use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const SCHEMA_VERSION: &str = "forge.execution-lease-checkpoint/v1";
const EVALUATION_MODE: &str = "pure_execution_lease_checkpoint_only";
const EXPECTED_CASE_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Authority {
    lease_issued: bool,
    terminal_persisted: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

impl Authority {
    fn is_offline(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    checkpoint: LeaseCheckpoint,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    terminal: bool,
    #[serde(default)]
    uncertain: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ExecutionLeaseCheckpointPreviewOutput {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    grant: GrantMetadata,
    cases: Vec<CaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
struct GrantMetadata {
    v: u16,
    attempt_id: String,
    target_id: String,
    epoch: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Serialize)]
struct CaseOutput {
    name: String,
    accepted: bool,
    terminal: bool,
    uncertain: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<ExecutionLeaseCheckpointPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::ExecutionLeaseCheckpointPreview { input: input_path } = command else {
        return Err("device execution lease checkpoint preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("execution lease checkpoint input contains duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("execution lease checkpoint input is invalid JSON: {error}"))?;
    validate_fixture(&fixture)?;

    let cases = fixture
        .cases
        .iter()
        .map(evaluate_case)
        .collect::<Result<Vec<_>, _>>()?;
    let grant = fixture
        .cases
        .first()
        .map(|case| GrantMetadata::from(&case.checkpoint))
        .ok_or("execution lease checkpoint fixture has no grant")?;
    Ok(ExecutionLeaseCheckpointPreviewOutput {
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        grant,
        cases,
        authority: fixture.authority,
    })
}

pub(crate) fn write_output(
    output: &ExecutionLeaseCheckpointPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline execution lease checkpoint preview [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "attempt={} target={} epoch={} issued_at_ms={} expires_at_ms={}",
        output.grant.attempt_id,
        output.grant.target_id,
        output.grant.epoch,
        output.grant.issued_at_ms,
        output.grant.expires_at_ms
    )?;
    for case in &output.cases {
        if case.accepted {
            writeln!(
                writer,
                "{}: accepted=true terminal={} uncertain={}",
                case.name, case.terminal, case.uncertain
            )?;
        } else {
            writeln!(
                writer,
                "{}: accepted=false error={}",
                case.name,
                case.error.as_deref().unwrap_or("invalid_checkpoint")
            )?;
        }
    }
    writeln!(
        writer,
        "authority: lease_issued=false terminal_persisted=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

impl GrantMetadata {
    fn from(checkpoint: &LeaseCheckpoint) -> Self {
        Self {
            v: checkpoint.grant.v,
            attempt_id: checkpoint.grant.attempt_id.clone(),
            target_id: checkpoint.grant.target_id.clone(),
            epoch: checkpoint.grant.epoch,
            issued_at_ms: checkpoint.grant.issued_at_ms,
            expires_at_ms: checkpoint.grant.expires_at_ms,
        }
    }
}

fn evaluate_case(case: &Case) -> Result<CaseOutput, Box<dyn Error>> {
    let restored = LeaseState::from_checkpoint(case.checkpoint.clone());
    let accepted = restored.is_ok();
    if accepted != case.expected.accepted {
        return Err(format!("checkpoint case '{}' acceptance drifted", case.name).into());
    }
    if accepted {
        let state = restored.expect("accepted checkpoint was restored");
        let terminal = state.terminal().is_some();
        let uncertain = state
            .terminal()
            .is_some_and(|receipt| receipt.disposition.is_uncertain());
        if terminal != case.expected.terminal || uncertain != case.expected.uncertain {
            return Err(format!("checkpoint case '{}' terminal drifted", case.name).into());
        }
        Ok(CaseOutput {
            name: case.name.clone(),
            accepted: true,
            terminal,
            uncertain,
            error: None,
        })
    } else {
        let error = case
            .expected
            .error
            .clone()
            .ok_or_else(|| format!("checkpoint case '{}' omitted its error", case.name))?;
        if error != "invalid_checkpoint" {
            return Err(format!("checkpoint case '{}' has unsupported error", case.name).into());
        }
        Ok(CaseOutput {
            name: case.name.clone(),
            accepted: false,
            terminal: false,
            uncertain: false,
            error: Some(error),
        })
    }
}

fn validate_fixture(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || !fixture.authority.is_offline()
        || fixture.cases.len() != EXPECTED_CASE_COUNT
    {
        return Err("execution lease checkpoint input is not a pure read-only observation".into());
    }
    let expected_names = [
        "empty_state",
        "completed_receipt_survives_restart",
        "uncertain_receipt_remains_terminal",
        "foreign_proof_rejected",
    ];
    for (case, expected_name) in fixture.cases.iter().zip(expected_names) {
        if case.name != expected_name {
            return Err("execution lease checkpoint cases are not canonical".into());
        }
        if case.checkpoint.schema_version != SCHEMA_VERSION
            || case.checkpoint.evaluation_mode != EVALUATION_MODE
        {
            return Err("execution lease checkpoint case has an invalid envelope".into());
        }
    }
    let first = &fixture.cases[0].checkpoint.grant;
    if fixture
        .cases
        .iter()
        .any(|case| case.checkpoint.grant != *first)
    {
        return Err("execution lease checkpoint grants drift across cases".into());
    }
    Ok(())
}

fn read_bounded_input(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let reader: Box<dyn Read> = if path == "-" {
        Box::new(io::stdin().lock())
    } else {
        Box::new(File::open(Path::new(path))?)
    };
    let mut bytes = Vec::new();
    reader
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(
            format!("execution lease checkpoint input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_execution_lease_checkpoint_command_tests.rs"]
mod tests;
