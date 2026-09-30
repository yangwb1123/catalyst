use std::collections::VecDeque;

use super::{Command, DeviceCommand, next_value, usage};

pub(super) fn parse_pending_run_intent_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device pending-run-intent-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device pending-run-intent-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::PendingRunIntentPreview {
        input,
    }))
}

pub(super) fn parse_attempt_request_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device attempt-request-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device attempt-request-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::AttemptRequestPreview {
        input,
    }))
}

pub(super) fn parse_session_runner_receipt_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device session-runner-receipt-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device session-runner-receipt-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::SessionRunnerReceiptPreview { input },
    ))
}

pub(super) fn parse_session_runner_receipt_history_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device session-runner-receipt-history-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device session-runner-receipt-history-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::SessionRunnerReceiptHistoryPreview { input },
    ))
}

pub(super) fn parse_run_execution_evidence_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device run-execution-evidence-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device run-execution-evidence-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunExecutionEvidencePreview { input },
    ))
}

pub(super) fn parse_run_attempt_lease_dispatch_preflight_preview(
    tokens: &mut VecDeque<String>,
) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device run-attempt-lease-dispatch-preflight-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device run-attempt-lease-dispatch-preflight-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(
        DeviceCommand::RunAttemptLeaseDispatchPreflightPreview { input },
    ))
}

pub(super) fn parse_run_observed_preview(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut input = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--input" if input.is_none() => input = Some(next_value(tokens, "--input")?),
            "--input" => return Err("--input was specified more than once".into()),
            value => {
                return Err(format!(
                    "unknown device run-observed-preview option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    let input = input.ok_or_else(|| {
        format!(
            "device run-observed-preview requires --input FILE|-\n\n{}",
            usage()
        )
    })?;
    if input.trim().is_empty() {
        return Err("--input requires a non-empty FILE|- value".into());
    }
    Ok(Command::Device(DeviceCommand::RunObservedPreview { input }))
}
