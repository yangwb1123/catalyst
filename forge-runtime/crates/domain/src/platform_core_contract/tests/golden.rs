use super::super::{
    artifact_ref_sha256, canonical_artifact_ref_json, canonical_command_envelope_json,
    canonical_event_envelope_json, command_envelope_sha256, decode_canonical_artifact_ref,
    decode_canonical_command_envelope, decode_canonical_event_envelope, event_envelope_sha256,
};
use super::fixture;

#[test]
fn golden_digests_and_round_trips_match_all_three_families() {
    let fixture = fixture();
    assert_eq!(
        fixture.api_version,
        "forge.platform-core-envelope-fixture/v1"
    );

    let artifact = canonical_artifact_ref_json(&fixture.artifact_ref).expect("artifact canonical");
    assert_eq!(
        artifact_ref_sha256(&fixture.artifact_ref).expect("artifact digest"),
        fixture.expected.artifact
    );
    assert_eq!(
        decode_canonical_artifact_ref(artifact.as_bytes()).expect("artifact decode"),
        fixture.artifact_ref
    );

    let command =
        canonical_command_envelope_json(&fixture.command_envelope).expect("command canonical");
    assert_eq!(
        command_envelope_sha256(&fixture.command_envelope).expect("command digest"),
        fixture.expected.command
    );
    assert_eq!(
        decode_canonical_command_envelope(command.as_bytes()).expect("command decode"),
        fixture.command_envelope
    );

    let event = canonical_event_envelope_json(&fixture.event_envelope).expect("event canonical");
    assert_eq!(
        event_envelope_sha256(&fixture.event_envelope).expect("event digest"),
        fixture.expected.event
    );
    assert_eq!(
        decode_canonical_event_envelope(event.as_bytes()).expect("event decode"),
        fixture.event_envelope
    );
}
