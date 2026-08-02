use std::collections::VecDeque;

use super::{
    Command, GroupGraphRunCommand, GroupGraphRunDispatchCommand, duplicate, next_value,
    required_id, run_command, unknown_dispatch, with_usage,
};

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
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
