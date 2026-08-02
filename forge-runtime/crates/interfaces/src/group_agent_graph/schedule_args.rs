use std::collections::VecDeque;

use super::{
    Command, GroupGraphRunCommand, GroupGraphRunScheduleCommand, duplicate, next_value,
    parse_limit, required_id, run_command, unknown, with_usage,
};

pub(super) fn parse(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("admit") => parse_admit(tokens, idempotency_key),
        Some("show") => parse_show(tokens),
        Some("list") => parse_list(tokens),
        Some(value) => Err(unknown("group graph run schedule", value)),
        None => Err(with_usage("group graph run schedule command is required")),
    }
}

fn parse_admit(
    tokens: &mut VecDeque<String>,
    idempotency_key: &mut Option<String>,
) -> Result<Command, String> {
    let graph_run_id = required_id(tokens, "group graph run schedule admit", "GRAPH_RUN_ID")?;
    let mut schedule_source = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--schedule" if schedule_source.is_none() => {
                schedule_source = Some(next_value(tokens, "--schedule")?);
            }
            "--schedule" => return Err(duplicate("--schedule")),
            "--idempotency-key" if idempotency_key.is_none() => {
                *idempotency_key = Some(next_value(tokens, "--idempotency-key")?);
            }
            "--idempotency-key" => return Err(duplicate("--idempotency-key")),
            _ => return Err(unknown("group graph run schedule admit", &option)),
        }
    }
    let schedule_source = schedule_source
        .ok_or_else(|| with_usage("group graph run schedule admit requires --schedule FILE|-"))?;
    Ok(run_command(GroupGraphRunCommand::Schedule(
        GroupGraphRunScheduleCommand::Admit {
            graph_run_id,
            schedule_source,
        },
    )))
}

fn parse_show(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let schedule_id = required_id(tokens, "group graph run schedule show", "SCHEDULE_ID")?;
    let include_schedule = match tokens.pop_front().as_deref() {
        Some("--include-schedule") => true,
        Some(option) => return Err(unknown("group graph run schedule show", option)),
        None => false,
    };
    super::require_empty(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Schedule(
        GroupGraphRunScheduleCommand::Show {
            schedule_id,
            include_schedule,
        },
    )))
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let graph_run_id = match tokens.front().map(String::as_str) {
        Some(value) if !value.starts_with('-') => tokens.pop_front(),
        _ => None,
    };
    let mut limit = 50;
    if tokens.front().is_some_and(|value| value == "--limit") {
        tokens.pop_front();
        limit = parse_limit(tokens)?;
    }
    super::require_empty(tokens)?;
    Ok(run_command(GroupGraphRunCommand::Schedule(
        GroupGraphRunScheduleCommand::List {
            graph_run_id,
            limit,
        },
    )))
}

#[cfg(test)]
mod tests {
    use crate::args::{
        Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand,
        GroupGraphRunScheduleCommand, parse_tokens,
    };

    fn parse(args: &[&str]) -> Result<crate::args::Args, String> {
        parse_tokens(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn parses_schedule_admit_with_local_key() {
        let args = parse(&[
            "group",
            "graph",
            "run",
            "schedule",
            "admit",
            "run-1",
            "--schedule",
            "schedule.json",
            "--idempotency-key",
            "schedule-key",
        ])
        .expect("schedule admission parses");
        assert_eq!(args.idempotency_key.as_deref(), Some("schedule-key"));
        assert!(matches!(
            args.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Schedule(GroupGraphRunScheduleCommand::Admit {
                    graph_run_id,
                    schedule_source,
                })
            ))) if graph_run_id == "run-1" && schedule_source == "schedule.json"
        ));
    }

    #[test]
    fn parses_redacted_show_and_filtered_list() {
        assert!(matches!(
            parse(&["group", "graph", "run", "schedule", "show", "schedule-1"])
                .expect("show parses")
                .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Schedule(GroupGraphRunScheduleCommand::Show {
                    schedule_id,
                    include_schedule: false,
                })
            ))) if schedule_id == "schedule-1"
        ));
        assert!(matches!(
            parse(&[
                "group", "graph", "run", "schedule", "list", "run-1", "--limit", "7"
            ])
            .expect("list parses")
            .command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Schedule(GroupGraphRunScheduleCommand::List {
                    graph_run_id: Some(graph_run_id),
                    limit: 7,
                })
            ))) if graph_run_id == "run-1"
        ));
    }

    #[test]
    fn rejects_missing_artifact_duplicate_flags_and_read_only_key() {
        assert!(
            parse(&["group", "graph", "run", "schedule", "admit", "run-1"])
                .expect_err("missing schedule rejects")
                .contains("requires --schedule")
        );
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "schedule",
                "show",
                "schedule-1",
                "--include-schedule",
                "--include-schedule"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "--idempotency-key",
                "key",
                "group",
                "graph",
                "run",
                "schedule",
                "show",
                "schedule-1"
            ])
            .expect_err("read-only command rejects mutation key")
            .contains("only valid for mutating commands")
        );
    }
}
