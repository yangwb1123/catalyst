//! Local, bounded consumer for the Runner lease/fencing contract.
//!
//! This command exercises the shared value state machine against its canonical
//! fixture. It deliberately omits fencing tokens, terminal reasons, and
//! receipt digests from output so a preview cannot turn a caller-supplied
//! declaration into an authority or secret transport.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::lease::{
    LeaseError, LeaseGrant, LeaseProof, LeaseState, TerminalDisposition,
};
use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const SCHEMA_VERSION: &str = "forge.runner-lease-fencing/v1";
const EVALUATION_MODE: &str = "pure_lease_fencing_only";
const EXPECTED_CASE_COUNT: usize = 16;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    device_identity_verified: bool,
    command_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

impl Authority {
    fn is_offline(self) -> bool {
        !self.device_identity_verified
            && !self.command_persisted
            && !self.reservation_created
            && !self.execution_authorized
            && !self.dispatch_performed
            && !self.audit_published
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    grant: LeaseGrant,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    operation: String,
    #[serde(default)]
    observed_at_ms: u64,
    #[serde(default)]
    fencing_token: Option<String>,
    #[serde(default)]
    ttl_ms: Option<u64>,
    #[serde(default)]
    proof: Option<LeaseProof>,
    #[serde(default)]
    disposition: Option<TerminalDisposition>,
    #[serde(default)]
    seed_disposition: Option<TerminalDisposition>,
    #[serde(default)]
    seed_observed_at_ms: u64,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    epoch: Option<u64>,
    #[serde(default)]
    issued_at_ms: Option<u64>,
    #[serde(default)]
    expires_at_ms: Option<u64>,
    #[serde(default)]
    replayed: bool,
    #[serde(default)]
    uncertain: bool,
    #[serde(default)]
    automatic_retry: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LeaseFencingPreviewOutput {
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
    operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issued_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replayed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uncertain: Option<bool>,
}

impl CaseOutput {
    fn new(case: &Case) -> Self {
        Self {
            name: case.name.clone(),
            operation: case.operation.clone(),
            active: None,
            accepted: None,
            error: None,
            epoch: None,
            issued_at_ms: None,
            expires_at_ms: None,
            replayed: None,
            uncertain: None,
        }
    }
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<LeaseFencingPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::RunnerLeaseFencingPreview { input: input_path } = command else {
        return Err("device Runner lease fencing preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("Runner lease fencing input contains duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Runner lease fencing input is invalid JSON: {error}"))?;
    validate_fixture(&fixture)?;
    let cases = fixture
        .cases
        .iter()
        .map(|case| evaluate_case(&fixture.grant, case))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LeaseFencingPreviewOutput {
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        grant: GrantMetadata::from(&fixture.grant),
        cases,
        authority: fixture.authority,
    })
}

pub(crate) fn write_output(
    output: &LeaseFencingPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Runner lease fencing preview [{}]",
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
        match (case.active, case.accepted, case.error.as_deref()) {
            (Some(active), _, _) => writeln!(writer, "{}: active={active}", case.name)?,
            (_, Some(true), _) => writeln!(
                writer,
                "{}: accepted=true replayed={} uncertain={}",
                case.name,
                case.replayed.unwrap_or(false),
                case.uncertain.unwrap_or(false)
            )?,
            (_, Some(false), Some(error)) => {
                writeln!(writer, "{}: accepted=false error={error}", case.name)?
            }
            _ => writeln!(writer, "{}: no-result", case.name)?,
        }
    }
    writeln!(
        writer,
        "authority: device_identity_verified=false command_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn validate_fixture(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || !fixture.authority.is_offline()
    {
        return Err("Runner lease fencing input is not a pure read-only observation".into());
    }
    if fixture.cases.len() != EXPECTED_CASE_COUNT {
        return Err(format!(
            "Runner lease fencing fixture must contain {EXPECTED_CASE_COUNT} cases"
        )
        .into());
    }
    fixture
        .grant
        .validate()
        .map_err(|error| format!("Runner lease fencing grant rejected: {error}"))?;
    Ok(())
}

fn evaluate_case(grant: &LeaseGrant, case: &Case) -> Result<CaseOutput, Box<dyn Error>> {
    let mut output = CaseOutput::new(case);
    match case.operation.as_str() {
        "active" => {
            let active = grant.is_active(case.observed_at_ms);
            if case.expected.active != Some(active) {
                return Err(case_mismatch(case, "active expectation mismatch"));
            }
            output.active = Some(active);
        }
        "renew" => {
            let result = grant.renew(
                case.observed_at_ms,
                case.fencing_token
                    .clone()
                    .ok_or_else(|| case_mismatch(case, "renew token missing"))?,
                case.ttl_ms
                    .ok_or_else(|| case_mismatch(case, "renew ttl missing"))?,
            );
            check_result(case, &result, &mut output)?;
            if let Ok(next) = result {
                check_renew_expectation(case, &next)?;
                output.epoch = Some(next.epoch);
                output.issued_at_ms = Some(next.issued_at_ms);
                output.expires_at_ms = Some(next.expires_at_ms);
            }
        }
        "proof" => {
            let proof = case
                .proof
                .as_ref()
                .ok_or_else(|| case_mismatch(case, "proof missing"))?;
            let result = grant.validate_proof(proof, case.observed_at_ms);
            check_result(case, &result, &mut output)?;
        }
        "terminal" | "terminal_replay" | "terminal_conflict" => {
            let mut state = seeded_state(grant, case)?;
            let result = state.submit_terminal(
                grant.proof(),
                case.disposition
                    .clone()
                    .ok_or_else(|| case_mismatch(case, "terminal disposition missing"))?,
                case.observed_at_ms,
            );
            check_result(case, &result, &mut output)?;
            if let Ok(submission) = result {
                if submission.replayed != case.expected.replayed {
                    return Err(case_mismatch(case, "replay expectation mismatch"));
                }
                let uncertain = submission.receipt.disposition.is_uncertain();
                if uncertain != case.expected.uncertain {
                    return Err(case_mismatch(case, "uncertain expectation mismatch"));
                }
                if let Some(automatic_retry) = case.expected.automatic_retry
                    && automatic_retry
                {
                    return Err(case_mismatch(case, "automatic retry must remain disabled"));
                }
                output.replayed = Some(submission.replayed);
                output.uncertain = Some(uncertain);
            }
        }
        "renew_after_terminal" => {
            let mut state = seeded_state(grant, case)?;
            let result = state.renew(
                case.observed_at_ms,
                case.fencing_token
                    .clone()
                    .ok_or_else(|| case_mismatch(case, "renew token missing"))?,
                case.ttl_ms
                    .ok_or_else(|| case_mismatch(case, "renew ttl missing"))?,
            );
            check_result(case, &result, &mut output)?;
        }
        operation => {
            return Err(case_mismatch(
                case,
                &format!("unsupported operation {operation}"),
            ));
        }
    }
    Ok(output)
}

fn seeded_state(grant: &LeaseGrant, case: &Case) -> Result<LeaseState, Box<dyn Error>> {
    let mut state = LeaseState::new(grant.clone())?;
    if let Some(disposition) = case.seed_disposition.clone() {
        state.submit_terminal(grant.proof(), disposition, case.seed_observed_at_ms)?;
    }
    Ok(state)
}

fn check_result<T>(
    case: &Case,
    result: &Result<T, LeaseError>,
    output: &mut CaseOutput,
) -> Result<(), Box<dyn Error>> {
    let accepted = result.is_ok();
    output.accepted = Some(accepted);
    if let Err(error) = result {
        output.error = Some(error.code().to_owned());
    }
    if case.expected.accepted != accepted {
        return Err(case_mismatch(case, "accepted expectation mismatch"));
    }
    match result {
        Ok(_) => Ok(()),
        Err(error) => {
            if case.expected.error.as_deref() != Some(error.code()) {
                return Err(case_mismatch(case, "error expectation mismatch"));
            }
            Ok(())
        }
    }
}

fn check_renew_expectation(case: &Case, grant: &LeaseGrant) -> Result<(), Box<dyn Error>> {
    if case.expected.epoch != Some(grant.epoch)
        || case.expected.issued_at_ms != Some(grant.issued_at_ms)
        || case.expected.expires_at_ms != Some(grant.expires_at_ms)
    {
        return Err(case_mismatch(case, "renewed grant expectation mismatch"));
    }
    Ok(())
}

fn case_mismatch(case: &Case, message: &str) -> Box<dyn Error> {
    format!("{}: {message}", case.name).into()
}

impl From<&LeaseGrant> for GrantMetadata {
    fn from(grant: &LeaseGrant) -> Self {
        Self {
            v: grant.v,
            attempt_id: grant.attempt_id.clone(),
            target_id: grant.target_id.clone(),
            epoch: grant.epoch,
            issued_at_ms: grant.issued_at_ms,
            expires_at_ms: grant.expires_at_ms,
        }
    }
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("Runner lease fencing input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_runner_lease_fencing_command_tests.rs"]
mod tests;
