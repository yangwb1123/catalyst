use super::{RemoteError, io_error};
use crate::args::{DeviceCommand, DeviceInventoryCommand};
use std::io::Write;

pub(super) fn show_inventory_observation<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
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

pub(super) fn show_inventory_persistence_preview<W: Write>(
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

pub(super) fn show_inventory_persisted_observation<W: Write>(
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

pub(super) fn show_inventory_persisted_observation_v2<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let command = DeviceCommand::Inventory(DeviceInventoryCommand::PersistedObservationV2 {
        input: input.to_owned(),
    });
    match crate::device_inventory_observation_v2_command::execute(&command) {
        Ok(output) => {
            crate::device_inventory_observation_v2_command::write_output(&output, false, writer)
                .map_err(io_error)?;
        }
        Err(error) => writeln!(
            writer,
            "Device inventory persisted-observation-v2 failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

pub(super) fn show_inventory_status<W: Write>(
    input: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
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

pub(super) fn show_inventory_snapshot_canonical<W: Write>(
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

pub(super) fn show_inventory_resource_summary<W: Write>(
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

pub(super) fn show_inventory_session_observation<W: Write>(
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

pub(super) fn show_inventory_placement_batch_evaluation<W: Write>(
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

pub(super) fn show_inventory_placement_evaluation<W: Write>(
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

pub(super) fn show_inventory_placement_evaluation_v2<W: Write>(
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

pub(super) fn write_inventory_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use inventory read, inventory read-v2, inventory show-converged, inventory show --input FILE, inventory persistence-preview --input FILE, inventory persisted-observation --input FILE, inventory persisted-observation-v2 --input FILE, inventory placement-evaluation --input FILE, inventory status --input FILE, inventory snapshot-canonical --input FILE, inventory resource-summary --input FILE, inventory session-observation --input FILE, inventory placement-batch-evaluation --input FILE, or inventory placement-evaluation-v2 --input FILE. All file previews are offline, unverified observations; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)?;
    Ok(())
}
