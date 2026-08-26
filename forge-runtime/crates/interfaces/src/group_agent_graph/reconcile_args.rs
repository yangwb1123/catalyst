use std::collections::VecDeque;

use crate::args::{Command, GroupGraphRunCommand, next_value};

use super::{duplicate, required_id, run_command, unknown, with_usage};

pub(super) fn parse(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let operation = "group graph run reconcile";
    let graph_run_id = required_id(tokens, operation, "GRAPH_RUN_ID")?;
    let mut core_bin = None;
    let mut core_bin_sha256 = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--core-bin" if core_bin.is_none() => {
                core_bin = Some(next_value(tokens, "--core-bin")?);
            }
            "--core-bin-sha256" if core_bin_sha256.is_none() => {
                core_bin_sha256 = Some(next_value(tokens, "--core-bin-sha256")?);
            }
            "--core-bin" => return Err(duplicate("--core-bin")),
            "--core-bin-sha256" => return Err(duplicate("--core-bin-sha256")),
            _ => return Err(unknown(operation, &option)),
        }
    }
    let core_bin = core_bin.ok_or_else(|| with_usage("reconcile requires --core-bin"))?;
    let core_bin_sha256 =
        core_bin_sha256.ok_or_else(|| with_usage("reconcile requires --core-bin-sha256"))?;
    Ok(run_command(GroupGraphRunCommand::Reconcile {
        graph_run_id,
        core_bin,
        core_bin_sha256,
    }))
}

#[cfg(test)]
mod tests {
    use crate::args::{
        Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, parse_tokens,
    };

    fn parse(values: &[&str]) -> Result<crate::args::Args, String> {
        parse_tokens(values.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn parses_exact_pinned_reconcile_command() {
        let args = parse(&[
            "group",
            "graph",
            "run",
            "reconcile",
            "graph-run-1",
            "--core-bin",
            "/opt/forge",
            "--core-bin-sha256",
            &"a".repeat(64),
        ])
        .expect("reconcile parses");
        assert!(matches!(
            args.command,
            Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
                GroupGraphRunCommand::Reconcile { graph_run_id, core_bin, .. }
            ))) if graph_run_id == "graph-run-1" && core_bin == "/opt/forge"
        ));
    }

    #[test]
    fn rejects_missing_duplicate_unknown_and_idempotency_options() {
        let base = ["group", "graph", "run", "reconcile", "graph-run-1"];
        assert!(parse(&base).is_err());
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "reconcile",
                "graph-run-1",
                "--core-bin",
                "/one",
                "--core-bin",
                "/two",
                "--core-bin-sha256",
                &"a".repeat(64),
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "group",
                "graph",
                "run",
                "reconcile",
                "graph-run-1",
                "--unknown",
            ])
            .is_err()
        );
        let mut keyed = vec!["--idempotency-key", "key"];
        keyed.extend(base);
        assert!(parse(&keyed).is_err());
    }
}
