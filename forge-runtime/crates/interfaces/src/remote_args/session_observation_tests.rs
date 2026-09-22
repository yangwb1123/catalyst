use super::*;
use crate::args::parse_tokens;

#[test]
fn remote_session_observation_preview_parses_a_bounded_input_path() {
    let parsed = parse_tokens(
        [
            "remote",
            "session-observation",
            "preview",
            "--input",
            "observation.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::SessionObservationPreview {
            input: "observation.json".into(),
        })
    );
    assert!(parse_tokens(["remote", "session-observation", "preview"].map(str::to_owned)).is_err());
    assert!(
        parse_tokens(
            [
                "remote",
                "session-observation",
                "preview",
                "--input",
                "observation.json",
                "--input",
                "other.json",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
}
