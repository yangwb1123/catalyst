use super::*;

#[test]
fn device_placement_dry_run_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "placement",
        "dry-run",
        "--input",
        "inventory.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Placement(DevicePlacementCommand::DryRun {
            input: "inventory.json".into(),
        },))
    );
    assert!(args.json);
}

#[test]
fn device_placement_dry_run_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "placement", "dry-run"],
        vec![
            "device",
            "placement",
            "dry-run",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device placement arguments must fail"
        );
    }
}

#[test]
fn device_run_intent_preview_requires_two_bounded_inputs() {
    let args = parse(&[
        "--json",
        "device",
        "placement",
        "run-intent-preview",
        "--input",
        "run.json",
        "--placement-input",
        "placement.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Placement(
            DevicePlacementCommand::RunIntentPreview {
                input: "run.json".into(),
                placement_input: "placement.json".into(),
            },
        ))
    );
    assert!(args.json);
}

#[test]
fn device_run_intent_preview_rejects_missing_duplicate_and_double_stdin_inputs() {
    for tokens in [
        vec![
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "run.json",
        ],
        vec![
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
            "--placement-input",
            "placement.json",
        ],
        vec![
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "-",
            "--placement-input",
            "-",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Run-intent preview arguments must fail"
        );
    }
}
