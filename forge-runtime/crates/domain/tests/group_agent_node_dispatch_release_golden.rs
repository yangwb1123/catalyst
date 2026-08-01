use forge_runtime_domain::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchReleaseControl,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct SharedReleaseFixture {
    v: u16,
    canonical_release_control_json: String,
    canonical_authorization_json: String,
    expected: SharedReleaseExpected,
}

#[derive(Deserialize)]
struct SharedReleaseExpected {
    release_control_snapshot_sha256: String,
    authorization_id: String,
    authorization_sha256: String,
    expected_last_event_sha256: String,
    request_body_bytes: usize,
    request_body_sha256: String,
}

#[test]
fn shared_rust_control_and_go_authorization_have_exact_current_bindings() {
    let fixture: SharedReleaseFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-dispatch-authorization-v1.json"
    )))
    .expect("shared release fixture");
    assert_eq!(fixture.v, 1);
    let control: GroupAgentNodeDispatchReleaseControl =
        serde_json::from_str(&fixture.canonical_release_control_json).expect("release control");
    let authorization: GroupAgentNodeDispatchAuthorization =
        serde_json::from_str(&fixture.canonical_authorization_json).expect("authorization");

    control.validate().expect("valid shared release control");
    authorization
        .validate_against_release_control(&control)
        .expect("Go authorization matches Rust control");
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
    control: &GroupAgentNodeDispatchReleaseControl,
    authorization: &GroupAgentNodeDispatchAuthorization,
    expected: &SharedReleaseExpected,
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
