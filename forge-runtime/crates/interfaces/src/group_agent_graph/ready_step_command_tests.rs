#[cfg(target_os = "linux")]
#[tokio::test]
async fn catchable_signal_watcher_arms_before_dispatch() {
    let cancellation = crate::runtime_domain::Cancellation::default();
    let watcher =
        super::spawn_signal_cancellation(cancellation.clone()).expect("signal watcher registers");
    assert!(!cancellation.is_cancelled());
    watcher.abort();
    let _ = watcher.await;
}

#[test]
fn public_anchors_reject_control_and_bidirectional_text() {
    for invalid in ["graph\nrun", "graph\u{202e}run"] {
        let mut options = fixture_options();
        options.graph_run_id = invalid.into();
        assert!(super::validate_public_anchors(&options).is_err());

        let mut options = fixture_options();
        options.expected_provider_request_id = invalid.into();
        assert!(super::validate_public_anchors(&options).is_err());
    }
}

fn fixture_options() -> crate::args::GroupGraphRunReadyStepOptions {
    crate::args::GroupGraphRunReadyStepOptions {
        graph_run_id: "graph-run-1".into(),
        expected_provider_request_id: "provider-request-1".into(),
        expected_ready_authorization_sha256: "a".repeat(64),
        pricing_source: "pricing.json".into(),
        core_bin: "forge-core".into(),
        core_bin_sha256: "b".repeat(64),
        confirm_off_machine: true,
        confirm_predecessor_content: false,
        include_result: false,
    }
}
