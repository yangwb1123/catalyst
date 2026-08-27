use super::*;

#[path = "scheduled_ready_dispatch_release_test_authorization.rs"]
mod authorization_support;
#[path = "scheduled_ready_dispatch_release_test_support.rs"]
mod support;
use authorization_support::authorization;
use support::{initial_control, successor_control};

#[test]
fn initial_ready_control_and_authorization_are_exact_and_bound() {
    let control = initial_control();
    control.validate().expect("valid initial ready control");
    let json = control.canonical_json().expect("ready control JSON");
    assert_eq!(
        GroupAgentScheduledReadyNodeDispatchReleaseControl::decode_exact(&json)
            .expect("exact ready control"),
        control
    );
    assert!(json.contains(r#""direct_predecessor_receipts":[]"#));
    assert!(json.contains(r#""predecessor_content_artifact":null"#));

    let authorization = authorization(&control);
    authorization.validate().expect("valid ready authorization");
    authorization
        .validate_against_release_control(&control)
        .expect("authorization bound to ready control");
    let authorization_json = authorization.canonical_json().expect("authorization JSON");
    assert_eq!(
        GroupAgentScheduledReadyNodeDispatchAuthorization::decode_exact(&authorization_json)
            .expect("exact ready authorization"),
        authorization
    );
}

#[test]
fn successor_direct_receipt_and_content_closures_validate() {
    let successor = successor_control(None);
    successor.validate().expect("valid successor ready control");
    assert_eq!(successor.reconcile_decision.next_execution_ordinal, Some(2));
    assert_eq!(successor.direct_predecessor_receipts.len(), 2);
    assert!(successor.predecessor_content_artifact.is_none());

    let content = successor_control(Some("verified predecessor output"));
    content.validate().expect("valid content successor control");
    let artifact = content
        .predecessor_content_artifact
        .as_ref()
        .expect("content artifact");
    assert_eq!(artifact.output_text, "verified predecessor output");
    authorization(&content)
        .validate_against_release_control(&content)
        .expect("content successor authorization");
}

#[test]
fn semantic_ready_source_substitutions_fail_closed() {
    let original = successor_control(Some("verified predecessor output"));

    let mut reordered = original.clone();
    reordered.direct_predecessor_receipts.swap(0, 1);
    reordered.snapshot_sha256.clear();
    assert!(reordered.seal().is_err());

    let mut wrong_target = original.clone();
    wrong_target.reconcile_decision.next_execution_ordinal = Some(1);
    wrong_target.reconcile_decision.next_node_id =
        Some(wrong_target.schedule.nodes[1].node_id.clone());
    wrong_target.reconcile_decision.decision_sha256.clear();
    wrong_target.reconcile_decision = wrong_target
        .reconcile_decision
        .seal()
        .expect("sealed mutation");
    wrong_target.snapshot_sha256.clear();
    assert!(wrong_target.seal().is_err());

    let mut stale = original;
    stale
        .progress_snapshot
        .snapshot_sha256
        .replace_range(..1, "f");
    stale.snapshot_sha256.clear();
    assert!(stale.seal().is_err());
}

#[test]
fn v2_domains_policy_and_ordinal_boundary_are_frozen() {
    assert_eq!(
        GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
        b"forge.group-agent-scheduled-ready-node-dispatch-release-control.v2\0"
    );
    assert_eq!(
        GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
        b"forge.group-agent-scheduled-ready-node-dispatch-authorization.v2\0"
    );
    assert_eq!(
        MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
        64 * 1024 * 1024
    );
    let mut boundary = authorization(&initial_control());
    boundary.execution_ordinal = 31;
    boundary.authorization_id.clear();
    boundary.authorization_sha256.clear();
    let boundary = boundary.seal().expect("ordinal 31 authorization");
    assert!(boundary.validate().is_ok());
    assert!(
        boundary
            .validate_against_release_control(&initial_control())
            .is_err()
    );

    let mut outside = boundary;
    outside.execution_ordinal = 32;
    outside.authorization_id.clear();
    outside.authorization_sha256.clear();
    assert!(outside.seal().is_err());
}

#[test]
fn exact_decoders_reject_v1_unknown_trailing_and_noncanonical_input() {
    let ready = initial_control().canonical_json().expect("ready JSON");
    let legacy_fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("legacy fixture");
    assert!(
        GroupAgentScheduledReadyNodeDispatchReleaseControl::decode_exact(
            legacy_fixture["canonical_release_control_json"]
                .as_str()
                .expect("legacy JSON")
        )
        .is_err()
    );
    assert!(
        GroupAgentScheduledReadyNodeDispatchReleaseControl::decode_exact(&(ready.clone() + "\n"))
            .is_err()
    );
    let unknown = ready.replacen("{\"v\":2", "{\"v\":2,\"unknown\":0", 1);
    assert!(GroupAgentScheduledReadyNodeDispatchReleaseControl::decode_exact(&unknown).is_err());
}

#[test]
fn v2_top_level_decoders_reject_max_plus_one_inputs() {
    let oversized_control =
        vec![b' '; MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES + 1];
    assert!(
        GroupAgentScheduledReadyNodeDispatchReleaseControl::decode_exact_bytes(&oversized_control)
            .is_err()
    );
    drop(oversized_control);

    let oversized_authorization =
        vec![b' '; MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES + 1];
    assert!(
        GroupAgentScheduledReadyNodeDispatchAuthorization::decode_exact_bytes(
            &oversized_authorization
        )
        .is_err()
    );
}
