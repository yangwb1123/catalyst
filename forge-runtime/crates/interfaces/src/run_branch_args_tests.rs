use super::{Command, RunCommand, parse_tokens};

#[test]
fn branch_and_lineage_parse_as_distinct_operations() {
    let branch = parse_tokens(
        [
            "--idempotency-key",
            "branch-key",
            "-C",
            "/srv/api",
            "run",
            "branch",
            "run-1",
        ]
        .map(str::to_owned),
    )
    .expect("parse branch");
    assert_eq!(
        branch.command,
        Command::Run(RunCommand::Branch {
            run_id: "run-1".into(),
        })
    );
    let lineage =
        parse_tokens(["run", "lineage", "run-1"].map(str::to_owned)).expect("parse lineage");
    assert_eq!(
        lineage.command,
        Command::Run(RunCommand::Lineage {
            run_id: "run-1".into(),
        })
    );
}

#[test]
fn branch_requires_mutation_controls_and_lineage_rejects_them() {
    let missing_key = parse_tokens(["-C", "/srv/api", "run", "branch", "run-1"].map(str::to_owned))
        .expect_err("branch needs an explicit key");
    assert!(missing_key.contains("explicit --idempotency-key"));

    let missing_project = parse_tokens(
        ["--idempotency-key", "branch-key", "run", "branch", "run-1"].map(str::to_owned),
    )
    .expect_err("branch needs an explicit Project");
    assert!(missing_project.contains("run branch requires -C/--project"));

    let selected = parse_tokens(["-C", "/srv/api", "run", "lineage", "run-1"].map(str::to_owned))
        .expect_err("lineage cannot ignore a selector");
    assert!(selected.contains("selectors are not valid"));

    let keyed = parse_tokens(
        ["--idempotency-key", "query-key", "run", "lineage", "run-1"].map(str::to_owned),
    )
    .expect_err("lineage cannot accept a mutation key");
    assert!(keyed.contains("only valid for mutating commands"));
}
