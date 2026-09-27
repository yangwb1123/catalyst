use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    execution::session_runner_receipt::SessionRunnerReceiptObservation,
    run_execution_evidence::{
        RunExecutionEvidence, RunExecutionEvidenceInput, observe_run_execution_evidence,
    },
    run_observed::RunObserved,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value, to_value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    run_observed: RunObserved,
    session_receipt_observed: SessionRunnerReceiptObservation,
}

/// Reads the bounded pair of already projected observations. No service or
/// device state is consulted here.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Run execution evidence input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote Run execution evidence input is invalid JSON".into()))?;
    let request = parse_request(&value)?;
    request
        .session_receipt_observed
        .validate()
        .map_err(|_| RemoteError("remote session Runner receipt observation is invalid".into()))?;
    request
        .run_observed
        .validate()
        .map_err(|_| RemoteError("remote Run observation is invalid".into()))?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI Run execution evidence preview requires a file path; '-' belongs to the standalone CLI".into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(value: &Value) -> Result<(String, String), RemoteError> {
    let request = parse_request(value)?;
    request
        .session_receipt_observed
        .validate()
        .map_err(|_| RemoteError("remote session Runner receipt observation is invalid".into()))?;
    request
        .run_observed
        .validate()
        .map_err(|_| RemoteError("remote Run observation is invalid".into()))?;
    if request.run_observed.conversation_id != request.session_receipt_observed.conversation_id
        || request.run_observed.run_id != request.session_receipt_observed.run_id
    {
        return Err(RemoteError(
            "remote Run execution evidence observations have different bindings".into(),
        ));
    }
    super::validation::validate_conversation_id(&request.run_observed.conversation_id)?;
    super::validation::validate_entity_id(&request.run_observed.run_id, "Run")?;
    Ok((
        request.run_observed.conversation_id,
        request.run_observed.run_id,
    ))
}

pub(super) fn validate_response(
    response: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    let request = parse_request(request)?;
    let expected = observe_run_execution_evidence(RunExecutionEvidenceInput {
        run: request.run_observed,
        receipt: request.session_receipt_observed,
    })
    .map_err(|_| RemoteError("Run execution evidence input is not bindable".into()))?;
    let actual: RunExecutionEvidence = from_value(response.clone())
        .map_err(|_| RemoteError("Forge API returned invalid Run execution evidence".into()))?;
    actual
        .validate()
        .map_err(|_| RemoteError("Forge API returned non-display Run execution evidence".into()))?;
    if actual.conversation_id != conversation_id
        || actual.run_id != run_id
        || to_value(&actual)
            .map_err(|_| RemoteError("Run execution evidence encoding failed".into()))?
            != to_value(&expected)
                .map_err(|_| RemoteError("Run execution evidence encoding failed".into()))?
    {
        return Err(RemoteError(
            "Forge API returned Run execution evidence with binding or value drift".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let evidence: RunExecutionEvidence = from_value(value.clone()).expect("validated evidence");
    writeln!(
        writer,
        "authenticated Run execution evidence [{}]",
        evidence.api_version
    )?;
    writeln!(
        writer,
        "owner_ref={} conversation={} prompt={} run={} status={}",
        evidence.owner_ref,
        evidence.conversation_id,
        evidence.prompt_id,
        evidence.run_id,
        evidence.run_status
    )?;
    writeln!(
        writer,
        "receipt: attempt={} target={} command={} digest={} disposition={} observed_at_ms={}",
        evidence.attempt_id,
        evidence.target_id,
        evidence.command_id,
        evidence.command_sha256,
        evidence.disposition_kind,
        evidence.receipt_observed_at_ms
    )?;
    writeln!(
        writer,
        "metadata_only: metadata_observed={} content_included={} uncertain={} reconciliation_required={}",
        evidence.metadata_observed,
        evidence.content_included,
        evidence.uncertain,
        evidence.reconciliation_required
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false owner_authorized=false run_authoritative=false receipt_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn parse_request(value: &Value) -> Result<Request, RemoteError> {
    from_value(value.clone()).map_err(|_| {
        RemoteError("remote Run execution evidence request has an invalid shape".into())
    })
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("remote Run execution evidence input could not be read".into())
            })?;
    } else {
        File::open(Path::new(input))
            .map_err(|_| {
                RemoteError("remote Run execution evidence input could not be opened".into())
            })?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("remote Run execution evidence input could not be read".into())
            })?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote Run execution evidence input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
