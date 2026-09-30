use std::collections::VecDeque;

use super::{Command, DeviceCommand, next_value, usage};

pub(super) fn parse_runner_receipt_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-receipt-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-receipt-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunnerReceiptPreview {
        input,
    }))
}

pub(super) fn parse_runner_lease_fencing_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-lease-fencing-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-lease-fencing-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunnerLeaseFencingPreview {
        input,
    }))
}

pub(super) fn parse_execution_lease_checkpoint_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device execution-lease-checkpoint-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device execution-lease-checkpoint-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::ExecutionLeaseCheckpointPreview { input },
    ))
}

pub(super) fn parse_runner_dispatch_plan_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-dispatch-plan-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-dispatch-plan-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunnerDispatchPlanPreview {
        input,
    }))
}

pub(super) fn parse_runner_attempt_boundary_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-attempt-boundary-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-attempt-boundary-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunnerAttemptBoundaryPreview { input },
    ))
}

pub(super) fn parse_runner_execution_intent_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device runner-execution-intent-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device runner-execution-intent-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunnerExecutionIntentPreview { input },
    ))
}
