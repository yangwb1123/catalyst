use std::collections::VecDeque;

use crate::args::{Command, GroupGraphRunCommand, GroupGraphRunReadyStepOptions, next_value};

use super::{duplicate, required_id, run_command, with_usage};

const OPERATION: &str = "group graph run step";

#[derive(Default)]
struct Values {
    expected_provider_request_id: Option<String>,
    expected_ready_authorization_sha256: Option<String>,
    pricing_source: Option<String>,
    core_bin: Option<String>,
    core_bin_sha256: Option<String>,
    confirm_off_machine: bool,
    confirm_predecessor_content: bool,
    include_result: bool,
}

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, OPERATION, "GRAPH_RUN_ID")?;
    let mut values = Values::default();
    while let Some(option) = tokens.pop_front() {
        parse_option(&option, tokens, &mut values)?;
    }
    Ok(run_command(GroupGraphRunCommand::Step(Box::new(
        values.finish(graph_run_id)?,
    ))))
}

fn parse_option(
    option: &str,
    tokens: &mut VecDeque<String>,
    values: &mut Values,
) -> Result<(), String> {
    match option {
        "--expected-provider-request-id" => {
            set_value(&mut values.expected_provider_request_id, option, tokens)
        }
        "--expected-ready-authorization-sha256" => set_value(
            &mut values.expected_ready_authorization_sha256,
            option,
            tokens,
        ),
        "--pricing" => set_value(&mut values.pricing_source, option, tokens),
        "--core-bin" => set_value(&mut values.core_bin, option, tokens),
        "--core-bin-sha256" => set_value(&mut values.core_bin_sha256, option, tokens),
        "--confirm-off-machine" => set_flag(&mut values.confirm_off_machine, option),
        "--confirm-predecessor-content" => {
            set_flag(&mut values.confirm_predecessor_content, option)
        }
        "--include-result" => set_flag(&mut values.include_result, option),
        _ => Err(with_usage("unknown group graph run step option")),
    }
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

impl Values {
    fn finish(self, graph_run_id: String) -> Result<GroupGraphRunReadyStepOptions, String> {
        Ok(GroupGraphRunReadyStepOptions {
            graph_run_id,
            expected_provider_request_id: required(
                self.expected_provider_request_id,
                "--expected-provider-request-id",
            )?,
            expected_ready_authorization_sha256: required(
                self.expected_ready_authorization_sha256,
                "--expected-ready-authorization-sha256",
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

fn required(value: Option<String>, option: &str) -> Result<String, String> {
    value.ok_or_else(|| with_usage(&format!("group graph run step requires {option}")))
}

#[cfg(test)]
#[path = "ready_step_args_tests.rs"]
mod tests;
