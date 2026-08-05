use std::collections::VecDeque;

use super::{
    Command, GroupGraphRunCommand, GroupGraphRunDispatchCommand, duplicate, next_value,
    required_id, run_command, unknown_dispatch, with_usage,
};

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id =
        super::required_id(tokens, "group graph run dispatch execute", "GRAPH_RUN_ID")?;
    let mut values = ExecuteValues::default();
    while let Some(option) = tokens.pop_front() {
        parse_option(tokens, &option, &mut values)?;
    }
    values.finish(graph_run_id)
}

#[derive(Default)]
struct ExecuteValues {
    authorization: Option<String>,
    pricing: Option<String>,
    core_bin: Option<String>,
    core_sha256: Option<String>,
    confirm: bool,
    include_result: bool,
}

fn parse_option(
    tokens: &mut VecDeque<String>,
    option: &str,
    values: &mut ExecuteValues,
) -> Result<(), String> {
    match option {
        "--authorization" if values.authorization.is_none() => {
            values.authorization = Some(next_value(tokens, option)?);
        }
        "--pricing" if values.pricing.is_none() => {
            values.pricing = Some(next_value(tokens, option)?);
        }
        "--core-bin" if values.core_bin.is_none() => {
            values.core_bin = Some(next_value(tokens, option)?);
        }
        "--core-bin-sha256" if values.core_sha256.is_none() => {
            values.core_sha256 = Some(next_value(tokens, option)?);
        }
        "--confirm-off-machine" if !values.confirm => values.confirm = true,
        "--include-result" if !values.include_result => values.include_result = true,
        _ => return Err(unknown_dispatch("group graph run dispatch execute")),
    }
    Ok(())
}

impl ExecuteValues {
    fn finish(self, graph_run_id: String) -> Result<Command, String> {
        let authorization_source = required(self.authorization, "--authorization FILE|-")?;
        let pricing_source = required(self.pricing, "--pricing FILE|-")?;
        if authorization_source == "-" && pricing_source == "-" {
            return Err(with_usage(
                "dispatch execute accepts standard input for only one artifact",
            ));
        }
        Ok(run_command(GroupGraphRunCommand::Dispatch(
            GroupGraphRunDispatchCommand::Execute {
                graph_run_id,
                authorization_source,
                pricing_source,
                core_bin: required(self.core_bin, "--core-bin ABSOLUTE_FILE")?,
                core_bin_sha256: required(self.core_sha256, "--core-bin-sha256 SHA256")?,
                confirm_off_machine: self.confirm,
                include_result: self.include_result,
            },
        )))
    }
}

fn required(value: Option<String>, option: &str) -> Result<String, String> {
    value.ok_or_else(|| with_usage(&format!("dispatch execute requires {option}")))
}

pub(super) fn parse_readiness(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("verify") => parse_verify(tokens),
        Some(_) => Err(unknown_dispatch("group graph run dispatch readiness")),
        None => Err(with_usage(
            "group graph run dispatch readiness command is required",
        )),
    }
}

fn parse_verify(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = required_id(
        tokens,
        "group graph run dispatch readiness verify",
        "GRAPH_RUN_ID",
    )?;
    let (authorization_source, pricing_source) = parse_sources(tokens)?;
    if authorization_source == "-" && pricing_source == "-" {
        return Err(with_usage(
            "authorization and pricing cannot both read from stdin",
        ));
    }
    Ok(run_command(GroupGraphRunCommand::Dispatch(
        GroupGraphRunDispatchCommand::ReadinessVerify {
            graph_run_id,
            authorization_source,
            pricing_source,
        },
    )))
}

fn parse_sources(tokens: &mut VecDeque<String>) -> Result<(String, String), String> {
    let mut authorization = None;
    let mut pricing = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--authorization" if authorization.is_none() => {
                authorization = Some(next_value(tokens, "--authorization")?);
            }
            "--pricing" if pricing.is_none() => {
                pricing = Some(next_value(tokens, "--pricing")?);
            }
            "--authorization" => return Err(duplicate("--authorization")),
            "--pricing" => return Err(duplicate("--pricing")),
            _ => {
                return Err(unknown_dispatch(
                    "group graph run dispatch readiness verify",
                ));
            }
        }
    }
    let authorization = authorization
        .ok_or_else(|| with_usage("dispatch readiness verify requires --authorization FILE|-"))?;
    let pricing =
        pricing.ok_or_else(|| with_usage("dispatch readiness verify requires --pricing FILE|-"))?;
    Ok((authorization, pricing))
}
