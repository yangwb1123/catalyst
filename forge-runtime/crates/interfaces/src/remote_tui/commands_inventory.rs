use serde_json::Value;
use std::io::Write;

use crate::args::{DeviceCommand, DeviceInventoryCommand};

use super::super::state::{TuiState, io_error};
use super::super::{RemoteClient, RemoteError};

/// Displays one caller-supplied inventory observation in the TUI.
///
/// The TUI deliberately accepts a filesystem path only. Reading `-` here
/// would consume the interactive command stream, so stdin remains available
/// to the TUI and callers can use the standalone CLI for stdin input.
pub(super) async fn show_inventory<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if argument.trim() == "read" {
        return show_remote_inventory(client, state, writer).await;
    }
    if argument.trim() == "read-v2" {
        return show_remote_inventory_v2(client, state, writer).await;
    }
    let (kind, suffix) = if let Some(suffix) = argument.strip_prefix("show --input") {
        ("show", suffix)
    } else if let Some(suffix) = argument.strip_prefix("persistence-preview --input") {
        ("persistence-preview", suffix)
    } else if let Some(suffix) = argument.strip_prefix("persisted-observation --input") {
        ("persisted-observation", suffix)
    } else if let Some(suffix) = argument.strip_prefix("persisted-observation-v2 --input") {
        ("persisted-observation-v2", suffix)
    } else if let Some(suffix) = argument.strip_prefix("placement-evaluation --input") {
        ("placement-evaluation", suffix)
    } else if let Some(suffix) = argument.strip_prefix("status --input") {
        ("status", suffix)
    } else if let Some(suffix) = argument.strip_prefix("snapshot-canonical --input") {
        ("snapshot-canonical", suffix)
    } else if let Some(suffix) = argument.strip_prefix("resource-summary --input") {
        ("resource-summary", suffix)
    } else if let Some(suffix) = argument.strip_prefix("session-observation --input") {
        ("session-observation", suffix)
    } else if let Some(suffix) = argument.strip_prefix("placement-batch-evaluation --input") {
        ("placement-batch-evaluation", suffix)
    } else if let Some(suffix) = argument.strip_prefix("placement-evaluation-v2 --input") {
        ("placement-evaluation-v2", suffix)
    } else {
        return write_inventory_usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return write_inventory_usage(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return write_inventory_usage(writer);
    }
    match kind {
        "show" => show_inventory_observation(input, writer)?,
        "persistence-preview" => show_inventory_persistence_preview(input, writer)?,
        "persisted-observation" => show_inventory_persisted_observation(input, writer)?,
        "persisted-observation-v2" => show_inventory_persisted_observation_v2(input, writer)?,
        "placement-evaluation" => show_inventory_placement_evaluation(input, writer)?,
        "status" => show_inventory_status(input, writer)?,
        "snapshot-canonical" => show_inventory_snapshot_canonical(input, writer)?,
        "resource-summary" => show_inventory_resource_summary(input, writer)?,
        "placement-batch-evaluation" => show_inventory_placement_batch_evaluation(input, writer)?,
        "placement-evaluation-v2" => show_inventory_placement_evaluation_v2(input, writer)?,
        _ => show_inventory_session_observation(input, writer)?,
    }
    Ok(())
}

async fn show_remote_inventory<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let value = match client.read_device_inventory().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Remote device inventory request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    write_remote_inventory(&value, writer)?;
    // An explicit read opts this TUI process into refreshing the same
    // owner-scoped observation during later `sync` commands. The value is
    // display state only; it is never used as placement or execution input.
    state.device_inventory_observed = Some(value);
    Ok(())
}

async fn show_remote_inventory_v2<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let value = match client.read_device_inventory_v2().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Remote v2 device inventory request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    write_remote_inventory_v2(&value, writer)?;
    // An explicit read opts this TUI process into refreshing the same
    // owner-scoped observation during later `sync` commands. The value is
    // display state only; it is never used as placement or execution input.
    state.device_inventory_v2_observed = Some(value);
    Ok(())
}

pub(super) fn write_remote_inventory_v2<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let evaluated_at_ms = value
        .get("evaluated_at_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let devices = value
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
    writeln!(
        writer,
        "remote device inventory [forge.device-inventory-observation/v2] at {evaluated_at_ms} devices={}",
        devices.len()
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={}",
        value
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        value
            .get("inventory_declarations_unverified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created={} execution_authorized={} dispatch_performed={}",
        value
            .get("reservation_created")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        value
            .get("execution_authorized")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        value
            .get("dispatch_performed")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    )
    .map_err(io_error)?;
    for candidate in devices {
        let object = candidate.as_object().ok_or_else(|| {
            RemoteError("Forge API returned an invalid v2 device inventory".into())
        })?;
        let device = object
            .get("device")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RemoteError("Forge API returned an invalid v2 device inventory".into())
            })?;
        let instance = object
            .get("instance_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let device_id = device
            .get("device_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let gpus = device
            .get("gpus")
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or_default();
        writeln!(
            writer,
            "{} / {}: revision={} generation={} heartbeat={} reservation={} cpu={} memory={} storage={} gpus={}",
            device_id,
            instance,
            object.get("revision").and_then(Value::as_u64).unwrap_or_default(),
            object.get("generation").and_then(Value::as_u64).unwrap_or_default(),
            object
                .get("heartbeat_sequence")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("reservation_state")
                .and_then(Value::as_str)
                .unwrap_or(""),
            device
                .get("available_cpu_cores")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("available_memory_bytes")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("available_storage_bytes")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            gpus,
        )
        .map_err(io_error)?;
    }
    Ok(())
}

pub(super) fn write_remote_inventory<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let schema = value
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("forge.device-inventory-observation/v1");
    let evaluated_at_ms = value
        .get("evaluated_at_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    writeln!(
        writer,
        "remote device inventory [{schema}] at {evaluated_at_ms}"
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={}",
        value
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        value
            .get("inventory_declarations_unverified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized={} dispatch_performed={}",
        value
            .get("execution_authorized")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        value
            .get("dispatch_performed")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    )
    .map_err(io_error)?;
    let Some(devices) = value.get("devices").and_then(Value::as_array) else {
        return Err(RemoteError(
            "Forge API returned an invalid device inventory".into(),
        ));
    };
    for candidate in devices {
        let Some(device) = candidate.get("device") else {
            return Err(RemoteError(
                "Forge API returned an invalid device inventory".into(),
            ));
        };
        writeln!(
            writer,
            "{} / {}: approval={} cordon={} liveness={} cpu={} memory={} storage={} gpu={} gpu_memory={} runtimes={}",
            device
                .get("device_id")
                .and_then(Value::as_str)
                .unwrap_or("invalid"),
            candidate
                .get("instance_id")
                .and_then(Value::as_str)
                .unwrap_or("invalid"),
            device
                .get("approval_state")
                .and_then(Value::as_str)
                .unwrap_or("invalid"),
            device
                .get("cordon_state")
                .and_then(Value::as_str)
                .unwrap_or("invalid"),
            device
                .get("liveness")
                .and_then(Value::as_str)
                .unwrap_or("invalid"),
            device
                .get("available_cpu_cores")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("available_memory_bytes")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("available_storage_bytes")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("gpu")
                .and_then(|gpu| gpu.get("present"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            device
                .get("gpu")
                .and_then(|gpu| gpu.get("memory_bytes"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            device
                .get("runtimes")
                .and_then(Value::as_array)
                .map(|runtimes| {
                    runtimes
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default(),
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn show_inventory_observation<W: Write>(input: &str, writer: &mut W) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::Show {
        input: input.to_owned(),
    });
    match crate::device_inventory_command::execute(&command) {
        Ok(output) => crate::device_inventory_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(writer, "Device inventory failed: {error}").map_err(io_error)?,
    }
    Ok(())
}

fn show_inventory_persistence_preview<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PersistencePreview {
        input: input.to_owned(),
    });
    match crate::device_inventory_persistence_command::execute(&command) {
        Ok(output) => {
            crate::device_inventory_persistence_command::write_output(&output, false, writer)
                .map_err(io_error)?;
        }
        Err(error) => writeln!(
            writer,
            "Device inventory persistence preview failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn show_inventory_persisted_observation<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PersistedObservation {
        input: input.to_owned(),
    });
    match crate::device_inventory_persisted_observation_command::execute(&command) {
        Ok(output) => crate::device_inventory_persisted_observation_command::write_output(
            &output, false, writer,
        )
        .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Device inventory persisted-observation failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn show_inventory_persisted_observation_v2<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PersistedObservationV2 {
        input: input.to_owned(),
    });
    match crate::device_inventory_observation_v2_command::execute(&command) {
        Ok(output) => {
            crate::device_inventory_observation_v2_command::write_output(&output, false, writer)
                .map_err(io_error)?
        }
        Err(error) => writeln!(
            writer,
            "Device inventory persisted-observation-v2 failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn show_inventory_status<W: Write>(input: &str, writer: &mut W) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::Status {
        input: input.to_owned(),
    });
    match crate::device_inventory_status_command::execute(&command) {
        Ok(output) => crate::device_inventory_status_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(writer, "Device inventory status failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

fn show_inventory_snapshot_canonical<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::SnapshotCanonical {
        input: input.to_owned(),
    });
    match crate::device_inventory_snapshot_command::execute(&command) {
        Ok(output) => {
            crate::device_inventory_snapshot_command::write_output(&output, false, writer)
                .map_err(io_error)?;
        }
        Err(error) => {
            writeln!(writer, "Device inventory snapshot failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

fn show_inventory_resource_summary<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::ResourceSummary {
        input: input.to_owned(),
    });
    match crate::device_resource_summary_command::execute(&command) {
        Ok(output) => crate::device_resource_summary_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(writer, "Device resource summary failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

fn show_inventory_session_observation<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::SessionObservation {
        input: input.to_owned(),
    });
    match crate::device_resource_summary_command::session_observation::execute(&command) {
        Ok(output) => crate::device_resource_summary_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(writer, "Session device observation failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

fn show_inventory_placement_batch_evaluation<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PlacementBatchEvaluation {
        input: input.to_owned(),
    });
    match crate::device_inventory_placement_batch_evaluation_command::execute(&command) {
        Ok(output) => crate::device_inventory_placement_batch_evaluation_command::write_output(
            &output, false, writer,
        )
        .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Device inventory placement batch evaluation failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn show_inventory_placement_evaluation<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PlacementEvaluation {
        input: input.to_owned(),
    });
    match crate::device_inventory_placement_evaluation_command::execute(&command) {
        Ok(output) => crate::device_inventory_placement_evaluation_command::write_output(
            &output, false, writer,
        )
        .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Device inventory placement evaluation failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn show_inventory_placement_evaluation_v2<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PlacementEvaluationV2 {
        input: input.to_owned(),
    });
    match crate::device_inventory_placement_evaluation_v2_command::execute(&command) {
        Ok(output) => crate::device_inventory_placement_evaluation_v2_command::write_output(
            &output, false, writer,
        )
        .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Device inventory placement-evaluation-v2 failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn write_inventory_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use inventory read, inventory read-v2, inventory show --input FILE, inventory persistence-preview --input FILE, inventory persisted-observation --input FILE, inventory persisted-observation-v2 --input FILE, inventory placement-evaluation --input FILE, inventory status --input FILE, inventory snapshot-canonical --input FILE, inventory resource-summary --input FILE, inventory session-observation --input FILE, inventory placement-batch-evaluation --input FILE, or inventory placement-evaluation-v2 --input FILE. All file previews are offline, unverified observations; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)?;
    Ok(())
}
