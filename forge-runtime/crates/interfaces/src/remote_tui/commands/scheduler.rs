use super::{
    RemoteClient, RemoteError, TuiState, ensure_conversation_visible_to_client_instance, io_error,
    new_idempotency_key,
};
use serde_json::Value;
use std::io::Write;

pub(super) async fn scheduler_selection_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_usage(writer);
    }
    let request = match super::super::super::scheduler_selection::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler selection preview input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_fresh_scheduler_projection(
        client,
        state,
        &request,
        "Scheduler selection preview",
        writer,
    )
    .await?
    {
        return Ok(());
    }
    let response = match client.preview_scheduler_selection(&request).await {
        Ok(response) => response,
        Err(error) => {
            report_request_failure(state, &error, "Scheduler selection preview", writer)?;
            return Ok(());
        }
    };
    render_selection(&response, &request, writer)
}

pub(super) async fn scheduler_selection_lease<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_lease_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_lease_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_lease_usage(writer);
    }
    let request = match super::super::super::scheduler_lease::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler lease input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_fresh_scheduler_projection(client, state, &request, "Scheduler lease", writer)
        .await?
    {
        return Ok(());
    }
    let key = new_idempotency_key()?;
    let response = match client.claim_scheduler_selection_lease(&request, &key).await {
        Ok(response) => response,
        Err(error) => {
            report_request_failure(state, &error, "Scheduler lease", writer)?;
            return Ok(());
        }
    };
    match super::super::super::scheduler_lease::validate_response_for_request(&response, &request) {
        Ok(result) => {
            super::super::super::scheduler_lease::render_human(&result, writer)
                .map_err(io_error)?;
        }
        Err(error) => writeln!(
            writer,
            "Scheduler lease response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

pub(super) async fn scheduler_selection_lease_renew<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_lease_renewal_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_lease_renewal_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_lease_renewal_usage(writer);
    }
    let request = match super::super::super::scheduler_lease_renew::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler lease renewal input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_fresh_scheduler_projection(
        client,
        state,
        &request,
        "Scheduler lease renewal",
        writer,
    )
    .await?
    {
        return Ok(());
    }
    let key = new_idempotency_key()?;
    let response = match client.renew_scheduler_selection_lease(&request, &key).await {
        Ok(response) => response,
        Err(error) => {
            report_request_failure(state, &error, "Scheduler lease renewal", writer)?;
            return Ok(());
        }
    };
    render_lease_renewal(&response, &request, writer)
}

pub(super) async fn scheduler_selection_lease_release<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_lease_release_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_lease_release_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_lease_release_usage(writer);
    }
    let request = match super::super::super::scheduler_lease_release::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler lease release input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler lease release",
        writer,
    )? {
        return Ok(());
    }
    let key = new_idempotency_key()?;
    let response = match client
        .release_scheduler_selection_lease(&request, &key)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            report_request_failure(state, &error, "Scheduler lease release", writer)?;
            return Ok(());
        }
    };
    render_lease_release(&response, &request, writer)
}

pub(super) fn write_scheduler_selection_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-preview --input FILE. An active client-instance filter must declare the request Conversation; the request binds Conversation, Run, Attempt, and requirements; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

/// Keeps scheduler previews and lease lifecycle requests inside an active
/// client-instance display projection. Without an explicit instance filter,
/// the existing caller-supplied request behavior remains unchanged; once a
/// filter is active, a stale or foreign placement file cannot cross it.
pub(super) fn ensure_scheduler_request_visible_to_selected_instance<W: Write>(
    state: &TuiState,
    request: &Value,
    operation: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(conversation_id) = request.get("conversation_id").and_then(Value::as_str) else {
        writeln!(
            writer,
            "{operation} input is missing conversation_id; no request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    };
    if state.client_instance_filter.is_none() {
        return Ok(true);
    }
    ensure_conversation_visible_to_client_instance(state, conversation_id, writer)
}

pub(super) fn write_scheduler_selection_lease_usage<W: Write>(
    writer: &mut W,
) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-lease --input FILE. This creates one fenced lease and uses a fresh idempotency key; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

pub(super) fn write_scheduler_selection_lease_renewal_usage<W: Write>(
    writer: &mut W,
) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-lease-renew --input FILE. This renews one current fenced lease with a fresh idempotency key; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

pub(super) fn write_scheduler_selection_lease_release_usage<W: Write>(
    writer: &mut W,
) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-lease-release --input FILE. This releases one current fenced lease with a fresh idempotency key; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

async fn ensure_fresh_scheduler_projection<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    request: &Value,
    operation: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if !ensure_scheduler_request_visible_to_selected_instance(state, request, operation, writer)? {
        return Ok(false);
    }
    // A refresh can revoke the selected Conversation. Recheck the local
    // display projection before the convergence guard and candidate request.
    if !super::super::writes::refresh_explicit_inventory_resource_observations(
        client, state, operation, writer,
    )
    .await?
    {
        return Ok(false);
    }
    if !ensure_scheduler_request_visible_to_selected_instance(state, request, operation, writer)? {
        return Ok(false);
    }
    super::super::writes::ensure_inventory_resource_converged(state, operation, writer)
}

fn report_request_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(writer, "{operation} request failed: {error}").map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn render_selection<W: Write>(
    response: &Value,
    request: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match super::super::super::scheduler_selection::validate_response_for_request(response, request)
    {
        Ok(result) => {
            super::super::super::scheduler_selection::render_human(&result, writer)
                .map_err(io_error)?;
        }
        Err(error) => {
            writeln!(
                writer,
                "Scheduler selection preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

fn render_lease_renewal<W: Write>(
    response: &Value,
    request: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match super::super::super::scheduler_lease_renew::validate_response_for_request(
        response, request,
    ) {
        Ok(()) => super::super::super::scheduler_lease_renew::render_human(response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Scheduler lease renewal response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn render_lease_release<W: Write>(
    response: &Value,
    request: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match super::super::super::scheduler_lease_release::validate_response_for_request(
        response, request,
    ) {
        Ok(()) => super::super::super::scheduler_lease_release::render_human(response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Scheduler lease release response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}
