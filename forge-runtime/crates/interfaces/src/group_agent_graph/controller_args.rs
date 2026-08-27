use std::collections::VecDeque;

use crate::args::{
    Command, GroupGraphRunCommand, GroupGraphRunControllerCommand,
    GroupGraphRunControllerStartOptions, GroupGraphRunControllerStepOptions, next_value,
};

use super::{duplicate, required_id, run_command, with_usage};

const OPERATION: &str = "group graph run controller";

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("start") => parse_start(tokens),
        Some("advance") => parse_advance(tokens),
        Some("show") => parse_show(tokens),
        Some("step") => parse_step(tokens),
        Some(_) => Err(with_usage("unknown group graph run controller command")),
        None => Err(with_usage("group graph run controller command is required")),
    }
}

#[derive(Default)]
struct StartValues {
    expected_schedule_sha256: Option<String>,
    core_bin: Option<String>,
    core_bin_sha256: Option<String>,
    endpoint: Option<String>,
    model: Option<String>,
    max_output_tokens: Option<String>,
    max_model_output_bytes: Option<String>,
    max_model_events: Option<String>,
    timeout_ms: Option<String>,
    max_cost_usd_micros: Option<String>,
    pricing_snapshot_sha256: Option<String>,
    max_result_bytes: Option<String>,
    max_effectful_steps: Option<String>,
    max_total_cost_usd_micros: Option<String>,
}

fn parse_start(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, &format!("{OPERATION} start"), "GRAPH_RUN_ID")?;
    let mut values = StartValues::default();
    while let Some(option) = tokens.pop_front() {
        let target = start_target(&mut values, &option)
            .ok_or_else(|| with_usage("unknown group graph run controller start option"))?;
        set_value(target, &option, tokens)?;
    }
    Ok(run_command(GroupGraphRunCommand::Controller(
        GroupGraphRunControllerCommand::Start(Box::new(values.finish(graph_run_id)?)),
    )))
}

fn start_target<'a>(values: &'a mut StartValues, option: &str) -> Option<&'a mut Option<String>> {
    match option {
        "--expected-schedule-sha256" => Some(&mut values.expected_schedule_sha256),
        "--core-bin" => Some(&mut values.core_bin),
        "--core-bin-sha256" => Some(&mut values.core_bin_sha256),
        "--endpoint" => Some(&mut values.endpoint),
        "--model" => Some(&mut values.model),
        "--max-output-tokens" => Some(&mut values.max_output_tokens),
        "--max-model-output-bytes" => Some(&mut values.max_model_output_bytes),
        "--max-model-events" => Some(&mut values.max_model_events),
        "--timeout-ms" => Some(&mut values.timeout_ms),
        "--max-cost-usd-micros" => Some(&mut values.max_cost_usd_micros),
        "--pricing-snapshot-sha256" => Some(&mut values.pricing_snapshot_sha256),
        "--max-result-bytes" => Some(&mut values.max_result_bytes),
        "--max-effectful-steps" => Some(&mut values.max_effectful_steps),
        "--max-total-cost-usd-micros" => Some(&mut values.max_total_cost_usd_micros),
        _ => None,
    }
}

impl StartValues {
    fn finish(self, graph_run_id: String) -> Result<GroupGraphRunControllerStartOptions, String> {
        Ok(GroupGraphRunControllerStartOptions {
            graph_run_id,
            expected_schedule_sha256: required(
                self.expected_schedule_sha256,
                "--expected-schedule-sha256",
            )?,
            core_bin: required(self.core_bin, "--core-bin")?,
            core_bin_sha256: required(self.core_bin_sha256, "--core-bin-sha256")?,
            endpoint: required(self.endpoint, "--endpoint")?,
            model: required(self.model, "--model")?,
            max_output_tokens: number(self.max_output_tokens, "--max-output-tokens")?,
            max_model_output_bytes: number(
                self.max_model_output_bytes,
                "--max-model-output-bytes",
            )?,
            max_model_events: number(self.max_model_events, "--max-model-events")?,
            timeout_ms: number(self.timeout_ms, "--timeout-ms")?,
            max_cost_usd_micros: number(self.max_cost_usd_micros, "--max-cost-usd-micros")?,
            pricing_snapshot_sha256: required(
                self.pricing_snapshot_sha256,
                "--pricing-snapshot-sha256",
            )?,
            max_result_bytes: number(self.max_result_bytes, "--max-result-bytes")?,
            max_effectful_steps: number(self.max_effectful_steps, "--max-effectful-steps")?,
            max_total_cost_usd_micros: number(
                self.max_total_cost_usd_micros,
                "--max-total-cost-usd-micros",
            )?,
        })
    }
}

fn parse_advance(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, &format!("{OPERATION} advance"), "GRAPH_RUN_ID")?;
    let (core_bin, core_bin_sha256) = parse_core(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Controller(
        GroupGraphRunControllerCommand::Advance {
            graph_run_id,
            core_bin,
            core_bin_sha256,
        },
    )))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, &format!("{OPERATION} show"), "GRAPH_RUN_ID")?;
    if !tokens.is_empty() {
        return Err(with_usage("unknown group graph run controller show option"));
    }
    Ok(run_command(GroupGraphRunCommand::Controller(
        GroupGraphRunControllerCommand::Show { graph_run_id },
    )))
}

#[derive(Default)]
struct StepValues {
    expected_awaiting_event_sha256: Option<String>,
    expected_provider_request_id: Option<String>,
    expected_authorization_sha256: Option<String>,
    pricing_source: Option<String>,
    core_bin: Option<String>,
    core_bin_sha256: Option<String>,
    confirm_off_machine: bool,
    confirm_predecessor_content: bool,
    include_result: bool,
}

fn parse_step(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, &format!("{OPERATION} step"), "GRAPH_RUN_ID")?;
    let mut values = StepValues::default();
    while let Some(option) = tokens.pop_front() {
        parse_step_option(&mut values, &option, tokens)?;
    }
    Ok(run_command(GroupGraphRunCommand::Controller(
        GroupGraphRunControllerCommand::Step(Box::new(values.finish(graph_run_id)?)),
    )))
}

fn parse_step_option(
    values: &mut StepValues,
    option: &str,
    tokens: &mut VecDeque<String>,
) -> Result<(), String> {
    match option {
        "--expected-awaiting-event-sha256" => {
            set_value(&mut values.expected_awaiting_event_sha256, option, tokens)
        }
        "--expected-provider-request-id" => {
            set_value(&mut values.expected_provider_request_id, option, tokens)
        }
        "--expected-authorization-sha256" => {
            set_value(&mut values.expected_authorization_sha256, option, tokens)
        }
        "--pricing" => set_value(&mut values.pricing_source, option, tokens),
        "--core-bin" => set_value(&mut values.core_bin, option, tokens),
        "--core-bin-sha256" => set_value(&mut values.core_bin_sha256, option, tokens),
        "--confirm-off-machine" => set_flag(&mut values.confirm_off_machine, option),
        "--confirm-predecessor-content" => {
            set_flag(&mut values.confirm_predecessor_content, option)
        }
        "--include-result" => set_flag(&mut values.include_result, option),
        _ => Err(with_usage("unknown group graph run controller step option")),
    }
}

impl StepValues {
    fn finish(self, graph_run_id: String) -> Result<GroupGraphRunControllerStepOptions, String> {
        Ok(GroupGraphRunControllerStepOptions {
            graph_run_id,
            expected_awaiting_event_sha256: required(
                self.expected_awaiting_event_sha256,
                "--expected-awaiting-event-sha256",
            )?,
            expected_provider_request_id: required(
                self.expected_provider_request_id,
                "--expected-provider-request-id",
            )?,
            expected_authorization_sha256: required(
                self.expected_authorization_sha256,
                "--expected-authorization-sha256",
            )?,
            pricing_source: required(self.pricing_source, "--pricing")?,
            core_bin: required(self.core_bin, "--core-bin")?,
            core_bin_sha256: required(self.core_bin_sha256, "--core-bin-sha256")?,
            confirm_off_machine: self.confirm_off_machine,
            confirm_predecessor_content: self.confirm_predecessor_content,
            include_result: self.include_result,
        })
    }
}

fn parse_core(tokens: &mut VecDeque<String>) -> Result<(String, String), String> {
    let mut core_bin = None;
    let mut core_bin_sha256 = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--core-bin" => set_value(&mut core_bin, &option, tokens)?,
            "--core-bin-sha256" => set_value(&mut core_bin_sha256, &option, tokens)?,
            _ => {
                return Err(with_usage(
                    "unknown group graph run controller advance option",
                ));
            }
        }
    }
    Ok((
        required(core_bin, "--core-bin")?,
        required(core_bin_sha256, "--core-bin-sha256")?,
    ))
}

fn set_value(
    target: &mut Option<String>,
    option: &str,
    tokens: &mut VecDeque<String>,
) -> Result<(), String> {
    if target.is_some() {
        return Err(duplicate(option));
    }
    *target = Some(next_value(tokens, option)?);
    Ok(())
}

fn set_flag(target: &mut bool, option: &str) -> Result<(), String> {
    if *target {
        return Err(duplicate(option));
    }
    *target = true;
    Ok(())
}

fn required(value: Option<String>, option: &str) -> Result<String, String> {
    value.ok_or_else(|| with_usage(&format!("{OPERATION} requires {option}")))
}

fn number<T: std::str::FromStr>(value: Option<String>, option: &str) -> Result<T, String> {
    required(value, option)?
        .parse()
        .map_err(|_| with_usage(&format!("{option} must be an unsigned integer")))
}

#[cfg(test)]
#[path = "controller_args_tests.rs"]
mod tests;
