//! Strict local consumer for the manual session Runner reconciliation projection.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::{
    session_runner_receipt_history::SessionRunnerReceiptHistoryObservation,
    session_runner_reconciliation_projection::{
        SessionRunnerReconciliationProjection, project_session_runner_reconciliation,
    },
};
use serde_json::Value;

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 256 * 1024;

/// Reads one explicit canonical projection without opening a remote client.
pub(super) fn read_projection(
    input: &str,
) -> Result<SessionRunnerReconciliationProjection, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError(
            "remote session Runner reconciliation projection contains duplicate JSON keys".into(),
        )
    })?;
    let projection: SessionRunnerReconciliationProjection = serde_json::from_slice(&bytes)
        .map_err(|_| {
            RemoteError("remote session Runner reconciliation projection is invalid JSON".into())
        })?;
    projection.validate().map_err(|_| invalid_projection())?;
    Ok(projection)
}

pub(super) fn read_tui_projection(
    input: &str,
) -> Result<SessionRunnerReconciliationProjection, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI session Runner reconciliation projection requires a file path; '-' belongs to the standalone CLI".into(),
        ));
    }
    read_projection(input)
}

pub(super) fn read_value(input: &str) -> Result<Value, RemoteError> {
    serde_json::to_value(read_projection(input)?).map_err(|_| invalid_projection())
}

/// Reads the full receipt history used by the authenticated projection route.
/// The local `preview` command intentionally remains projection-only; this
/// helper is used only by the explicit `remote-preview` path.
pub(super) fn read_remote_request(input: &str) -> Result<Value, RemoteError> {
    super::session_runner_receipt_history::read_request(input)
}

pub(super) fn read_remote_tui_request(input: &str) -> Result<Value, RemoteError> {
    super::session_runner_receipt_history::read_tui_request(input)
}

pub(super) fn remote_conversation_and_run(request: &Value) -> Result<(&str, &str), RemoteError> {
    super::session_runner_receipt_history::conversation_and_run(request)
}

/// Recomputes the pure projection from the submitted history and rejects any
/// response whose binding, summary, uncertain state, or authority differs.
pub(super) fn validate_remote_response(
    response: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    super::session_runner_receipt_history::validate_value(request)?;
    let (request_conversation, request_run) = remote_conversation_and_run(request)?;
    if request_conversation != conversation_id || request_run != run_id {
        return Err(RemoteError(
            "remote session Runner reconciliation request does not match the URL path".into(),
        ));
    }
    let history: SessionRunnerReceiptHistoryObservation =
        serde_json::from_value(request.clone()).map_err(|_| invalid_history())?;
    let expected =
        project_session_runner_reconciliation(&history).map_err(|_| invalid_history())?;
    let actual: SessionRunnerReconciliationProjection =
        serde_json::from_value(response.clone()).map_err(|_| invalid_projection())?;
    actual.validate().map_err(|_| invalid_projection())?;
    if actual != expected {
        return Err(RemoteError(
            "Forge API returned a session Runner reconciliation projection with a different reduction".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_remote_human(response: &Value, writer: &mut impl Write) -> io::Result<()> {
    let projection: SessionRunnerReconciliationProjection =
        serde_json::from_value(response.clone()).expect("validated session Runner projection");
    render_human(&projection, writer)
}

pub(super) fn render_human(
    projection: &SessionRunnerReconciliationProjection,
    writer: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        writer,
        "session Runner reconciliation projection [{}]",
        projection.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={}",
        projection.owner.subject,
        projection.conversation_id,
        projection.prompt_id,
        projection.run_id
    )?;
    writeln!(
        writer,
        "source: schema={} attempts={} latest_attempt={} latest_command={} latest_target={} disposition={} observed_at_ms={}",
        projection.source.schema_version,
        projection.source.attempt_count,
        projection.latest_attempt_id,
        projection.latest_command_id,
        projection.latest_target_id,
        projection.latest_disposition_kind,
        projection.latest_observed_at_ms
    )?;
    writeln!(
        writer,
        "reconciliation: kind={} reason={} required={} manual_review_required={} automatic_retry={} follow_up={}",
        projection.reconciliation_kind,
        projection.reconciliation_reason,
        projection.reconciliation_required,
        projection.manual_review_required,
        projection.automatic_retry,
        projection.follow_up
    )?;
    writeln!(
        writer,
        "binding: preview_only={} selected_target=none",
        projection.preview_only
    )?;
    writeln!(
        writer,
        "authority: identity_verified={} receipt_persisted={} execution_authorized={} dispatch_performed={} audit_published={}",
        projection.authority.identity_verified,
        projection.authority.receipt_persisted,
        projection.authority.execution_authorized,
        projection.authority.dispatch_performed,
        projection.authority.audit_published
    )
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::new();
    let reader: Box<dyn Read> = if input == "-" {
        Box::new(io::stdin())
    } else {
        Box::new(File::open(Path::new(input)).map_err(|_| {
            RemoteError(
                "remote session Runner reconciliation projection could not be opened".into(),
            )
        })?)
    };
    reader
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            RemoteError("remote session Runner reconciliation projection could not be read".into())
        })?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote session Runner reconciliation projection exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

fn invalid_projection() -> RemoteError {
    RemoteError("remote session Runner reconciliation projection is invalid".into())
}

fn invalid_history() -> RemoteError {
    RemoteError("remote session Runner receipt history is invalid".into())
}
