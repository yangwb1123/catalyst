#![cfg(unix)]

#[allow(clippy::duplicate_mod, dead_code)]
mod core_terminal_bridge_support;

use std::fs;

use forge_runtime_domain::{
    ScheduledGraphProgressSnapshot, ScheduledGraphReconcileDisposition,
    ScheduledGraphReconcilePort, ScheduledGraphReconcilePortError,
};
use forge_runtime_infrastructure::PinnedScheduledGraphReconcileBridge;
use tempfile::tempdir;

use core_terminal_bridge_support::{build_go_forge, script_digest, write_script};

const SNAPSHOT_SHA256: &str = "a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20";
const DECISION_SHA256: &str = "0c5682601d192a19abb1d23d8bb1597c0eacde8fa098a49b4db548fd5bc56af0";

const GOLDEN_SNAPSHOT: &str = concat!(
    "{\"v\":1,\"progress_protocol_version\":1,",
    "\"graph_run_id\":\"graph-run-golden\",\"graph_id\":\"graph-golden\",",
    "\"schedule_id\":\"graph-execution-schedule-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
    "\"schedule_sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
    "\"node_count\":2,\"execution_mode\":\"serial\",\"max_in_flight_nodes\":1,",
    "\"progression_policy\":\"completed_contiguous_prefix\",\"attempt_policy\":\"exactly_one\",",
    "\"failure_policy\":\"fail_fast_no_retry\",\"nodes\":[",
    "{\"execution_ordinal\":0,\"node_id\":\"build\",\"attempt\":1,",
    "\"candidate_id\":\"scheduled-node-contract-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",",
    "\"candidate_sha256\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",",
    "\"provider_request_id\":\"scheduled-node-provider-request-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",",
    "\"prepared_request_sha256\":\"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",",
    "\"lifecycle_status\":\"terminalized\",\"terminal_outcome\":\"completed\",",
    "\"terminal_receipt_sha256\":\"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\"},",
    "{\"execution_ordinal\":1,\"node_id\":\"verify\",\"attempt\":1,",
    "\"candidate_id\":\"scheduled-node-contract-eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\",",
    "\"candidate_sha256\":\"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\",",
    "\"provider_request_id\":null,\"prepared_request_sha256\":null,",
    "\"lifecycle_status\":null,\"terminal_outcome\":null,\"terminal_receipt_sha256\":null}],",
    "\"snapshot_sha256\":\"a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20\"}",
);

const GOLDEN_DECISION: &str = concat!(
    "{\"v\":1,\"progress_protocol_version\":1,",
    "\"graph_run_id\":\"graph-run-golden\",",
    "\"schedule_id\":\"graph-execution-schedule-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
    "\"schedule_sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
    "\"snapshot_sha256\":\"a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20\",",
    "\"disposition\":\"ready\",\"next_execution_ordinal\":1,\"next_node_id\":\"verify\",",
    "\"decision_sha256\":\"0c5682601d192a19abb1d23d8bb1597c0eacde8fa098a49b4db548fd5bc56af0\"}",
);

#[test]
fn compiled_go_core_reconciles_the_exact_rust_golden_snapshot() {
    let snapshot = ScheduledGraphProgressSnapshot::decode_exact(GOLDEN_SNAPSHOT)
        .expect("strict Go golden snapshot");
    assert_eq!(snapshot.snapshot_sha256, SNAPSHOT_SHA256);
    assert_eq!(
        snapshot.canonical_json().expect("canonical snapshot"),
        GOLDEN_SNAPSHOT
    );

    let directory = tempdir().expect("temporary Go build directory");
    let path = build_go_forge(directory.path());
    let bridge = PinnedScheduledGraphReconcileBridge::new(path.clone(), script_digest(&path))
        .expect("pinned compiled Go Core");
    let decision = bridge.decide(&snapshot).expect("Go reconcile decision");

    assert_eq!(
        decision.disposition,
        ScheduledGraphReconcileDisposition::Ready
    );
    assert_eq!(decision.next_execution_ordinal, Some(1));
    assert_eq!(decision.next_node_id.as_deref(), Some("verify"));
    assert_eq!(decision.decision_sha256, DECISION_SHA256);
    assert_eq!(
        decision.canonical_json().expect("canonical decision"),
        GOLDEN_DECISION
    );
}

#[test]
fn port_distinguishes_unavailable_core_from_invalid_successful_output() {
    let snapshot = ScheduledGraphProgressSnapshot::decode_exact(GOLDEN_SNAPSHOT)
        .expect("strict Go golden snapshot");
    for (name, decision, expected) in [
        (
            "unavailable",
            "exit 7",
            ScheduledGraphReconcilePortError::Unavailable,
        ),
        (
            "invalid-decision",
            "printf '%s' '{}'",
            ScheduledGraphReconcilePortError::InvalidDecision,
        ),
    ] {
        let directory = tempdir().expect("temporary Core directory");
        let script = reconcile_script(decision);
        let path = write_script(directory.path(), name, &script, true);
        let bridge = PinnedScheduledGraphReconcileBridge::new(path.clone(), script_digest(&path))
            .expect("valid test handshake");
        assert_eq!(bridge.decide(&snapshot), Err(expected));
    }
}

#[test]
fn bridge_constructor_does_not_start_core_before_a_validated_snapshot() {
    let directory = tempdir().expect("temporary Core directory");
    let sentinel = directory.path().join("started");
    let script = format!(
        "#!/bin/sh\nprintf '%s' 'started' > '{}'\nexit 7\n",
        sentinel.display()
    );
    let path = write_script(directory.path(), "deferred-core", &script, true);

    PinnedScheduledGraphReconcileBridge::new(path.clone(), script_digest(&path))
        .expect("constructor accepts a well-formed explicit pin");

    assert!(
        !sentinel.exists(),
        "Core started before a snapshot decision"
    );
    assert!(fs::metadata(path).expect("pinned Core remains").is_file());
}

fn reconcile_script(decision: &str) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" != \"graph-scheduled-reconcile\" ]; then exit 90; fi\n\
         if [ \"$2\" = \"--protocol-version\" ]; then printf '%s' '1'; exit 0; fi\n\
         if [ \"$2\" != \"--snapshot\" ] || [ \"$3\" != \"-\" ]; then exit 91; fi\n\
         {decision}\n"
    )
}
