use std::collections::VecDeque;

use super::{Command, DeviceCommand, DeviceInventoryCommand, next_value, usage};

pub(super) fn parse_inventory(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("show") => parse_inventory_show(tokens),
        Some("persistence-preview") => parse_inventory_persistence_preview(tokens),
        Some("persisted-observation") => parse_inventory_persisted_observation_preview(tokens),
        Some("persisted-observation-v2") => {
            parse_inventory_persisted_observation_v2_preview(tokens)
        }
        Some("placement-evaluation") => parse_inventory_placement_evaluation(tokens),
        Some("status") => parse_inventory_status(tokens),
        Some("snapshot-canonical") => parse_inventory_snapshot_canonical(tokens),
        Some("resource-summary") => parse_inventory_resource_summary(tokens),
        Some("session-observation") => parse_inventory_session_observation(tokens),
        Some("placement-batch-evaluation") => parse_inventory_placement_batch_evaluation(tokens),
        Some("placement-evaluation-v2") => parse_inventory_placement_evaluation_v2(tokens),
        Some(value) => Err(format!(
            "unknown device inventory command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "device inventory command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_inventory_persistence_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory persistence-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory persistence-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PersistencePreview { input },
    )))
}

fn parse_inventory_persisted_observation_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory persisted-observation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory persisted-observation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PersistedObservation { input },
    )))
}

fn parse_inventory_persisted_observation_v2_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory persisted-observation-v2 option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory persisted-observation-v2 requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PersistedObservationV2 { input },
    )))
}

fn parse_inventory_placement_batch_evaluation(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory placement-batch-evaluation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory placement-batch-evaluation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PlacementBatchEvaluation { input },
    )))
}

fn parse_inventory_placement_evaluation_v2(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory placement-evaluation-v2 option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory placement-evaluation-v2 requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PlacementEvaluationV2 { input },
    )))
}

fn parse_inventory_placement_evaluation(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory placement-evaluation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory placement-evaluation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::PlacementEvaluation { input },
    )))
}

fn parse_inventory_session_observation(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory session-observation option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory session-observation requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::SessionObservation { input },
    )))
}

fn parse_inventory_snapshot_canonical(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory snapshot-canonical option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory snapshot-canonical requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::SnapshotCanonical { input },
    )))
}

fn parse_inventory_status(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory status option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory status requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::Status { input },
    )))
}

fn parse_inventory_resource_summary(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory resource-summary option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory resource-summary requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::ResourceSummary { input },
    )))
}

fn parse_inventory_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device inventory show option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device inventory show requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Inventory(
        DeviceInventoryCommand::Show { input },
    )))
}
