use std::collections::VecDeque;

use super::{Command, DeviceCommand, DevicePlacementCommand, next_value, usage};

pub(super) fn parse_placement(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("dry-run") => parse_dry_run(tokens),
        Some("run-intent-preview") => parse_run_intent_preview(tokens),
        Some(value) => Err(format!(
            "unknown device placement command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "device placement command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_run_intent_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    let mut placement_input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            "--placement-input" if placement_input.is_none() => {
                placement_input = Some(next_value(tokens, "--placement-input")?);
            }
            "--placement-input" => {
                return Err("--placement-input was specified more than once".into());
            }
            value => {
                return Err(format!(
                    "unknown device placement run-intent-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device placement run-intent-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    let placement_input = placement_input.ok_or_else(|| {
        format!(
            "device placement run-intent-preview requires --placement-input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() || placement_input.trim().is_empty() {
        return Err("input options require a non-empty FILE|- value".into());
    }
    if input == "-" && placement_input == "-" {
        return Err("--input and --placement-input cannot both read stdin".into());
    }
    Ok(Command::Device(DeviceCommand::Placement(
        DevicePlacementCommand::RunIntentPreview {
            input,
            placement_input,
        },
    )))
}

fn parse_dry_run(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device placement dry-run option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device placement dry-run requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::Placement(
        DevicePlacementCommand::DryRun { input },
    )))
}
