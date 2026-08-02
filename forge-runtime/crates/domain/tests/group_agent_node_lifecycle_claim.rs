use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind,
    GroupAgentGraphRunInspection, GroupAgentNodeActiveLane, GroupAgentNodeCoreTerminalReceiptPort,
    GroupAgentNodeDispatchClaim, GroupAgentNodeDispatchProviderFactory,
    GroupAgentNodeLifecycleInspection, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalClassification,
    GroupAgentNodeTerminalControl, GroupAgentNodeTerminalReceipt,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_output_sha256, group_agent_node_terminal_receipt_id,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Fixture {
    canonical_terminal_control_json: String,
    canonical_terminal_artifact_json: String,
    canonical_terminal_receipt_json: String,
    canonical_terminal_event_json: String,
    canonical_uncertainty_artifact_json: String,
    uncertainty_artifact_sha256: String,
}

#[test]
fn claim_and_lane_decoders_reject_noncanonical_and_ambiguous_json() {
    let control = control();
    let claim = control.claim.canonical_json().expect("claim JSON");
    let lane = control.active_lane.canonical_json().expect("lane JSON");
    for candidate in adversarial(&claim, "dispatch_id") {
        assert!(GroupAgentNodeDispatchClaim::decode_exact(candidate.as_bytes()).is_err());
    }
    for candidate in adversarial(&lane, "lane_ownership_id") {
        assert!(GroupAgentNodeActiveLane::decode_exact(candidate.as_bytes()).is_err());
    }
}

#[test]
fn opaque_claim_ids_are_bounded_safe_and_exactly_lane_bound() {
    let control = control();
    let baseline = control.claim;
    for bad in [
        String::new(),
        "unsafe\nid".into(),
        "x".repeat(MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES + 1),
    ] {
        let mut dispatch = baseline.clone();
        dispatch.dispatch_id = bad.clone();
        assert!(dispatch.validate().is_err());
        let mut ownership = baseline.clone();
        ownership.lane_ownership_id = bad;
        assert!(ownership.validate().is_err());
    }

    let mut wrong_lane = control.active_lane;
    wrong_lane.dispatch_id = "another-opaque-dispatch".into();
    assert!(wrong_lane.validate_against_claim(&baseline).is_err());
}

#[test]
fn lifecycle_effect_ports_are_object_safe() {
    fn factory(_: Option<&dyn GroupAgentNodeDispatchProviderFactory>) {}
    fn core(_: Option<&dyn GroupAgentNodeCoreTerminalReceiptPort>) {}
    factory(None);
    core(None);
}

#[test]
fn every_new_byte_decoder_rejects_non_utf8_input() {
    let invalid = [0xff];
    assert!(GroupAgentGraphRunEvent::decode_exact_bytes(&invalid).is_err());
    assert!(GroupAgentNodeDispatchClaim::decode_exact(&invalid).is_err());
    assert!(GroupAgentNodeActiveLane::decode_exact(&invalid).is_err());
    assert!(GroupAgentNodeTerminalArtifact::decode_exact(&invalid).is_err());
    assert!(GroupAgentNodeTerminalControl::decode_exact(&invalid).is_err());
    assert!(GroupAgentNodeTerminalReceipt::decode_exact(&invalid).is_err());
}

#[test]
fn terminal_output_uses_go_compatible_line_separator_escapes() {
    let mut artifact = control().artifact;
    artifact.output_text = "before\u{2028}middle\u{2029}after".into();
    artifact.output_bytes = artifact.output_text.len();
    artifact.output_sha256 = group_agent_node_terminal_output_sha256(&artifact.output_text);
    sign_artifact(&mut artifact);

    let canonical = artifact.canonical_json().expect("canonical artifact");
    assert!(canonical.contains("\\u2028"));
    assert!(canonical.contains("\\u2029"));
    assert!(!canonical.contains('\u{2028}'));
    assert!(!canonical.contains('\u{2029}'));
    assert_eq!(
        GroupAgentNodeTerminalArtifact::decode_exact(canonical.as_bytes()).unwrap(),
        artifact
    );
}

#[test]
fn shared_go_uncertainty_artifact_has_exact_rust_bytes_and_bindings() {
    let fixture = fixture();
    let control = GroupAgentNodeTerminalControl::decode_exact(
        fixture.canonical_terminal_control_json.as_bytes(),
    )
    .expect("shared terminal control");
    let artifact = GroupAgentNodeTerminalArtifact::decode_exact(
        fixture.canonical_uncertainty_artifact_json.as_bytes(),
    )
    .expect("shared uncertainty artifact");
    assert_eq!(
        artifact.artifact_kind,
        GroupAgentNodeTerminalArtifactKind::Uncertainty
    );
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::MissingUsage
    );
    assert_eq!(
        artifact.artifact_sha256,
        fixture.uncertainty_artifact_sha256
    );
    assert_eq!(
        artifact.expected_sha256().unwrap(),
        artifact.artifact_sha256
    );
    artifact
        .validate_against_claim(
            &control.claim,
            &control.active_lane,
            &control.authorization,
            &control.pricing,
            &control.contract,
        )
        .expect("uncertainty binds exact claimed request");
    assert!(!fixture.canonical_uncertainty_artifact_json.ends_with('\n'));
}

#[test]
fn claim_context_rejects_unchecked_usage_and_nonmonotonic_artifact_time() {
    let fixture = fixture();
    let control = GroupAgentNodeTerminalControl::decode_exact(
        fixture.canonical_terminal_control_json.as_bytes(),
    )
    .unwrap();
    let baseline = GroupAgentNodeTerminalArtifact::decode_exact(
        fixture.canonical_uncertainty_artifact_json.as_bytes(),
    )
    .unwrap();
    let mut usage = baseline.clone();
    usage.classification = GroupAgentNodeTerminalClassification::LocalLimit;
    usage.usage_observed = true;
    usage.input_tokens = control.pricing.max_input_tokens + 1;
    usage.output_tokens = 1;
    sign_artifact(&mut usage);
    assert!(usage.validate().is_ok());
    assert!(validate_for_control(&usage, &control).is_err());

    let mut time = baseline.clone();
    time.created_at_ms = control.claim.released_at_ms - 1;
    sign_artifact(&mut time);
    assert!(validate_for_control(&time, &control).is_err());

    let mut invalid_pricing = control.pricing.clone();
    invalid_pricing.max_input_tokens += 1;
    assert!(
        baseline
            .validate_against_claim(
                &control.claim,
                &control.active_lane,
                &control.authorization,
                &invalid_pricing,
                &control.contract,
            )
            .is_err()
    );
}

#[test]
fn terminal_control_rejects_a_self_consistent_claim_that_drifted_from_authorization() {
    let mut control = control();
    control.claim.max_cost_usd_micros -= 1;
    let event = control.journal_events.get_mut(3).expect("seq-4 event");
    let GroupAgentGraphRunEventKind::NodeDispatchReleased {
        max_cost_usd_micros,
        ..
    } = &mut event.kind
    else {
        panic!("seq-4 event kind");
    };
    *max_cost_usd_micros = control.claim.max_cost_usd_micros;
    control.claim.claim_event_sha256 = event.expected_sha256().expect("claim digest");
    control
        .active_lane
        .claim_event_sha256
        .clone_from(&control.claim.claim_event_sha256);
    control
        .artifact
        .claim_event_sha256
        .clone_from(&control.claim.claim_event_sha256);
    sign_artifact(&mut control.artifact);
    control.graph_run.journal_bytes = canonical_events(&control.journal_events)
        .iter()
        .map(String::len)
        .sum();
    sign_control(&mut control);

    assert!(control.validate().is_err());
}

#[test]
fn terminal_inspection_rejects_artifact_and_receipt_cross_binding_drift() {
    let baseline = terminal_inspection();
    baseline.validate().expect("baseline terminal inspection");

    let mut artifact_drift = baseline.clone();
    let artifact = artifact_drift.artifact.as_mut().expect("artifact");
    artifact.dispatch_id = "different-dispatch".into();
    sign_artifact(artifact);
    let artifact_id = artifact.artifact_id.clone();
    let artifact_sha256 = artifact.artifact_sha256.clone();
    artifact_drift.artifact_json = Some(artifact.canonical_json().unwrap());
    let receipt = artifact_drift.terminal_receipt.as_mut().expect("receipt");
    receipt.artifact_id.clone_from(&artifact_id);
    receipt.artifact_sha256.clone_from(&artifact_sha256);
    sign_receipt(receipt);
    let receipt_id = receipt.receipt_id.clone();
    let receipt_sha256 = receipt.receipt_sha256.clone();
    artifact_drift.terminal_receipt_json = Some(receipt.canonical_json().unwrap());
    update_terminal_event(
        &mut artifact_drift,
        &artifact_id,
        &artifact_sha256,
        &receipt_id,
        &receipt_sha256,
    );
    assert!(artifact_drift.validate().is_err());

    let mut receipt_drift = baseline;
    let receipt = receipt_drift.terminal_receipt.as_mut().expect("receipt");
    receipt.dispatch_id = "different-dispatch".into();
    sign_receipt(receipt);
    let receipt_id = receipt.receipt_id.clone();
    let receipt_sha256 = receipt.receipt_sha256.clone();
    receipt_drift.terminal_receipt_json = Some(receipt.canonical_json().unwrap());
    let artifact = receipt_drift.artifact.as_ref().expect("artifact");
    let artifact_id = artifact.artifact_id.clone();
    let artifact_sha256 = artifact.artifact_sha256.clone();
    update_terminal_event(
        &mut receipt_drift,
        &artifact_id,
        &artifact_sha256,
        &receipt_id,
        &receipt_sha256,
    );
    assert!(receipt_drift.validate().is_err());
}

fn terminal_inspection() -> GroupAgentNodeLifecycleInspection {
    let fixture = fixture();
    let control = control();
    let artifact = GroupAgentNodeTerminalArtifact::decode_exact(
        fixture.canonical_terminal_artifact_json.as_bytes(),
    )
    .expect("terminal artifact");
    let receipt = GroupAgentNodeTerminalReceipt::decode_exact(
        fixture.canonical_terminal_receipt_json.as_bytes(),
    )
    .expect("terminal receipt");
    let terminal_event =
        GroupAgentGraphRunEvent::decode_exact(&fixture.canonical_terminal_event_json)
            .expect("seq-5 event");
    let mut events = control.journal_events.clone();
    events.push(terminal_event);
    let event_jsons = canonical_events(&events);
    let mut run = control.graph_run.clone();
    run.v = GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION;
    run.status = receipt.graph_status;
    run.last_event_seq = 5;
    run.journal_bytes = event_jsons.iter().map(String::len).sum();
    GroupAgentNodeLifecycleInspection {
        v: 1,
        graph_run: GroupAgentGraphRunInspection {
            v: GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION,
            run,
            plan_json: control.plan.canonical_json().expect("plan JSON"),
            plan: control.plan,
            event_jsons,
            events,
        },
        claim_json: control.claim.canonical_json().expect("claim JSON"),
        claim: control.claim,
        active_lane: None,
        active_lane_json: None,
        artifact: Some(artifact),
        artifact_json: Some(fixture.canonical_terminal_artifact_json),
        terminal_receipt: Some(receipt),
        terminal_receipt_json: Some(fixture.canonical_terminal_receipt_json),
    }
}

fn update_terminal_event(
    inspection: &mut GroupAgentNodeLifecycleInspection,
    artifact_id_value: &str,
    artifact_sha256_value: &str,
    receipt_id_value: &str,
    receipt_sha256_value: &str,
) {
    let event = inspection.graph_run.events.get_mut(4).expect("seq-5 event");
    let GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        artifact_id,
        artifact_sha256,
        terminal_receipt_id,
        terminal_receipt_sha256,
        ..
    } = &mut event.kind
    else {
        panic!("seq-5 event kind");
    };
    *artifact_id = artifact_id_value.into();
    *artifact_sha256 = artifact_sha256_value.into();
    *terminal_receipt_id = receipt_id_value.into();
    *terminal_receipt_sha256 = receipt_sha256_value.into();
    inspection.graph_run.event_jsons = canonical_events(&inspection.graph_run.events);
    inspection.graph_run.run.journal_bytes = inspection
        .graph_run
        .event_jsons
        .iter()
        .map(String::len)
        .sum();
}

fn canonical_events(events: &[GroupAgentGraphRunEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| event.canonical_json().expect("event JSON"))
        .collect()
}

fn adversarial(canonical: &str, required: &str) -> Vec<String> {
    let first_end = canonical.find(',').expect("first field");
    let first = &canonical[1..first_end];
    let duplicate = format!("{{{first},{first}{}", &canonical[first_end..]);
    let unknown = format!("{{\"unknown\":true,{}", &canonical[1..]);
    let reordered = reorder_first_two(canonical, first_end);
    let mut missing = serde_json::from_str::<Value>(canonical).expect("JSON object");
    missing.as_object_mut().unwrap().remove(required);
    let mut null = serde_json::from_str::<Value>(canonical).expect("JSON object");
    null[required] = Value::Null;
    vec![
        duplicate,
        unknown,
        reordered,
        serde_json::to_string(&missing).unwrap(),
        serde_json::to_string(&null).unwrap(),
        format!("{canonical}\n"),
    ]
}

fn reorder_first_two(canonical: &str, first_end: usize) -> String {
    let relative = canonical[first_end + 1..].find(',').expect("second field");
    let second_end = first_end + 1 + relative;
    format!(
        "{{{},{}{}",
        &canonical[first_end + 1..second_end],
        &canonical[1..first_end],
        &canonical[second_end..]
    )
}

fn control() -> GroupAgentNodeTerminalControl {
    let fixture = fixture();
    GroupAgentNodeTerminalControl::decode_exact(fixture.canonical_terminal_control_json.as_bytes())
        .expect("shared terminal control")
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-terminal-lifecycle-v1.json"
    )))
    .expect("shared lifecycle fixture")
}

fn sign_artifact(artifact: &mut GroupAgentNodeTerminalArtifact) {
    artifact.artifact_bytes = artifact.canonical_payload_json().unwrap().len();
    artifact.artifact_sha256 = artifact.expected_sha256().unwrap();
    artifact.artifact_id = group_agent_node_terminal_artifact_id(&artifact.artifact_sha256);
}

fn sign_control(control: &mut GroupAgentNodeTerminalControl) {
    control.snapshot_sha256 = control.expected_sha256().expect("control digest");
}

fn sign_receipt(receipt: &mut GroupAgentNodeTerminalReceipt) {
    receipt.receipt_sha256 = receipt.expected_sha256().expect("receipt digest");
    receipt.receipt_id = group_agent_node_terminal_receipt_id(&receipt.receipt_sha256);
}

fn validate_for_control(
    artifact: &GroupAgentNodeTerminalArtifact,
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), forge_runtime_domain::GroupAgentNodeLifecycleValidationError> {
    artifact.validate_against_claim(
        &control.claim,
        &control.active_lane,
        &control.authorization,
        &control.pricing,
        &control.contract,
    )
}
