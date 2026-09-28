use super::{Command, parse_tokens};

#[test]
fn version_flags_parse_without_a_project_or_state_directory() {
    for flag in ["--version", "-V"] {
        let args = parse_tokens([flag].map(str::to_owned)).unwrap();
        assert_eq!(args.command, Command::Version);
        assert!(!args.json);
        assert_eq!(args.project, None);
        assert_eq!(args.state_dir, None);
    }
}

#[test]
fn version_json_accepts_either_global_option_order() {
    for flags in [
        ["--json", "--version"],
        ["--version", "--json"],
        ["--json", "-V"],
        ["-V", "--json"],
    ] {
        let args = parse_tokens(flags.map(str::to_owned)).unwrap();
        assert_eq!(args.command, Command::Version);
        assert!(args.json);
    }
}

#[test]
fn version_rejects_duplicates_commands_and_execution_options() {
    for flags in [
        vec!["--version", "-V"],
        vec!["--version", "--version"],
        vec!["-V", "-V"],
        vec!["--version", "session", "new"],
        vec!["--version", "./project"],
        vec!["--version", "--read", "README.md"],
        vec!["--version", "--idempotency-key", "key"],
        vec!["--version", "--unknown"],
    ] {
        assert!(
            parse_tokens(flags.iter().copied().map(str::to_owned)).is_err(),
            "flags={flags:?}"
        );
    }
}

#[test]
fn version_remains_available_as_a_bare_project_path() {
    let args = parse_tokens(["version"].map(str::to_owned)).unwrap();
    assert_eq!(args.command, Command::Hub);
    assert_eq!(args.project, Some("version".into()));
}
