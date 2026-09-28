use super::{RemoteError, TuiState, io_error, scope_filter_label};
use crate::{args::parse_scope, client_instance_session_scope};
use std::io::Write;

pub(super) fn filter_sessions<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let selector = argument.trim();
    if selector == "clear" {
        let client_instance_filter_changed = state.client_instance_filter.is_some();
        state.scope_filter = None;
        state.client_instance_filter = None;
        if client_instance_filter_changed {
            state.clear_client_instance_private_projection();
        }
        writeln!(writer, "Scope filter cleared.").map_err(io_error)?;
        return Ok(());
    }
    if let Some(instance_id) = selector
        .strip_prefix("instance:")
        .or_else(|| selector.strip_prefix("client-instance:"))
    {
        return set_instance_filter(state, instance_id, writer);
    }
    let Ok(scope_filter) = parse_scope(selector) else {
        writeln!(
            writer,
            "Invalid scope filter. Use filter global|project:ID|group:ID or filter clear."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let label = scope_filter_label(&scope_filter);
    let client_instance_filter_changed = state.client_instance_filter.is_some();
    state.scope_filter = Some(scope_filter);
    state.client_instance_filter = None;
    if client_instance_filter_changed {
        state.clear_client_instance_private_projection();
    }
    writeln!(
        writer,
        "Scope filter set to {label}; this only organizes the displayed session list, not authorization or device identity."
    )
    .map_err(io_error)
}

pub(super) fn instance_filter_command<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let argument = argument.trim();
    if argument == "list" {
        return list_instances(state, writer);
    }
    if argument == "clear" {
        return filter_sessions(state, "clear", writer);
    }
    if argument.is_empty() {
        writeln!(
            writer,
            "Use instance INSTANCE_ID, instance list, or instance clear after opening a client-instance view."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let selector = if argument.starts_with("instance:") || argument.starts_with("client-instance:")
    {
        argument.to_owned()
    } else {
        format!("instance:{argument}")
    };
    filter_sessions(state, &selector, writer)
}

fn set_instance_filter<W: Write>(
    state: &mut TuiState,
    instance_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let instance_id = instance_id.trim();
    if client_instance_session_scope::validate_instance_id(instance_id).is_err() {
        writeln!(
                writer,
                "Invalid client-instance filter. Use filter instance:INSTANCE_ID after opening client-instances session-view or resource-view."
            )
            .map_err(io_error)?;
        return Ok(());
    }
    let Some(view) = state.active_client_instance_view() else {
        writeln!(
            writer,
            "Open client-instances session-view or resource-view before setting an instance filter."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if client_instance_session_scope::scope_from_view(view, instance_id).is_err() {
        writeln!(
                writer,
                "Unknown client-instance filter {instance_id:?}; use an instance declared by the observed view."
            )
            .map_err(io_error)?;
        return Ok(());
    }
    let client_instance_filter_changed =
        state.client_instance_filter.as_deref() != Some(instance_id);
    state.scope_filter = None;
    state.client_instance_filter = Some(instance_id.to_owned());
    if client_instance_filter_changed {
        state.clear_client_instance_private_projection();
    }
    state.reconcile_client_instance_selection();
    writeln!(
            writer,
            "Client-instance filter set to {instance_id:?}; this is a local display projection over unverified session_ids, not authorization or device identity."
        )
        .map_err(io_error)?;
    Ok(())
}

fn list_instances<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    let Some(view) = state.active_client_instance_view() else {
        writeln!(
            writer,
            "No client-instance view is open. Use client-instances session-view or resource-view first."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let active = state.client_instance_filter.as_deref();
    let mut count = 0usize;
    for instance_id in client_instance_session_scope::declared_instance_ids(view) {
        count += 1;
        let marker = if active == Some(instance_id) {
            "*"
        } else {
            " "
        };
        writeln!(writer, " {marker} {instance_id}").map_err(io_error)?;
    }
    if count == 0 {
        writeln!(
            writer,
            "The open client-instance view declares no instances."
        )
        .map_err(io_error)?;
    }
    Ok(())
}
