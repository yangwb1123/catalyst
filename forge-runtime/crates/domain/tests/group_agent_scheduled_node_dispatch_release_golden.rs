use forge_runtime_domain::{
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchReleaseControl,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct SharedScheduledReleaseFixture {
    v: u16,
    canonical_release_control_json: String,
    canonical_authorization_json: String,
    expected: SharedScheduledReleaseExpected,
}

#[derive(Deserialize)]
struct SharedScheduledReleaseExpected {
    release_control_snapshot_sha256: String,
    authorization_id: String,
    authorization_sha256: String,
    expected_last_event_seq: u64,
    expected_last_event_sha256: String,
    request_body_bytes: usize,
    request_body_sha256: String,
}

#[test]
fn shared_scheduled_release_and_go_authorization_have_exact_current_bindings() {
    let fixture: SharedScheduledReleaseFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("shared scheduled release fixture");
    assert_eq!(fixture.v, 1);
    let control = GroupAgentScheduledNodeDispatchReleaseControl::decode_exact(
        &fixture.canonical_release_control_json,
    )
    .expect("exact shared scheduled release control");
    let authorization = GroupAgentScheduledNodeDispatchAuthorization::decode_exact(
        &fixture.canonical_authorization_json,
    )
    .expect("exact shared scheduled authorization");

    authorization
        .validate_against_release_control(&control)
        .expect("Go authorization matches Rust scheduled release control");
    assert_eq!(
        control.canonical_json().expect("canonical release control"),
        fixture.canonical_release_control_json
    );
    assert_eq!(
        authorization
            .canonical_json()
            .expect("canonical authorization"),
        fixture.canonical_authorization_json
    );
    assert!(!fixture.canonical_release_control_json.ends_with('\n'));
    assert!(!fixture.canonical_authorization_json.ends_with('\n'));
    assert_expected(&control, &authorization, &fixture.expected);
}

fn assert_expected(
    control: &GroupAgentScheduledNodeDispatchReleaseControl,
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
    expected: &SharedScheduledReleaseExpected,
) {
    assert_eq!(
        control.snapshot_sha256,
        expected.release_control_snapshot_sha256
    );
    assert_eq!(authorization.authorization_id, expected.authorization_id);
    assert_eq!(
        authorization.authorization_sha256,
        expected.authorization_sha256
    );
    assert_eq!(
        authorization.expected_last_event_seq,
        expected.expected_last_event_seq
    );
    assert_eq!(
        authorization.expected_last_event_sha256,
        expected.expected_last_event_sha256
    );
    assert_eq!(
        authorization.request_body_bytes,
        expected.request_body_bytes
    );
    assert_eq!(
        authorization.request_body_sha256,
        expected.request_body_sha256
    );
}
