use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GROUP_AGENT_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION, GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION,
    GroupAgentGraphRunEvent, GroupAgentGraphRunInspection, GroupAgentNodeActiveLane,
    GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleInspection, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalClassification,
    GroupAgentNodeTerminalControl, GroupAgentNodeTerminalOutcome, GroupAgentNodeTerminalReceipt,
    MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_output_sha256,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct SharedLifecycleFixture {
    v: u16,
    canonical_claim_event_json: String,
    claim_event_sha256: String,
    canonical_terminal_artifact_json: String,
    artifact_sha256: String,
    canonical_terminal_control_json: String,
    terminal_control_sha256: String,
    canonical_terminal_receipt_json: String,
    terminal_receipt_sha256: String,
    canonical_terminal_event_json: String,
    terminal_event_sha256: String,
}

#[test]
fn shared_go_and_rust_terminal_lifecycle_bytes_and_digests_are_exact() {
    let fixture = fixture();
    let claim_event = GroupAgentGraphRunEvent::decode_exact(&fixture.canonical_claim_event_json)
        .expect("shared seq-4 event");
    let artifact = GroupAgentNodeTerminalArtifact::decode_exact(
        fixture.canonical_terminal_artifact_json.as_bytes(),
    )
    .expect("shared terminal artifact");
    let control = GroupAgentNodeTerminalControl::decode_exact(
        fixture.canonical_terminal_control_json.as_bytes(),
    )
    .expect("shared terminal control");
    let receipt = GroupAgentNodeTerminalReceipt::decode_exact(
        fixture.canonical_terminal_receipt_json.as_bytes(),
    )
    .expect("shared Core receipt");

    assert_eq!(fixture.v, 1);
    assert_eq!(artifact.v, GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION);
    assert_eq!(
        claim_event.expected_sha256().unwrap(),
        fixture.claim_event_sha256
    );
    assert_eq!(artifact.artifact_sha256, fixture.artifact_sha256);
    assert_eq!(artifact.expected_sha256().unwrap(), fixture.artifact_sha256);
    assert_eq!(control.snapshot_sha256, fixture.terminal_control_sha256);
    assert_eq!(
        control.expected_sha256().unwrap(),
        fixture.terminal_control_sha256
    );
    assert_eq!(receipt.receipt_sha256, fixture.terminal_receipt_sha256);
    assert_eq!(
        receipt.expected_sha256().unwrap(),
        fixture.terminal_receipt_sha256
    );
    receipt
        .validate_against_control(&control)
        .expect("shared receipt binds exact control");
    assert_no_trailing_lf(&fixture);
    validate_seq5(&fixture);
}

#[test]
fn v4_quarantine_and_v5_terminal_inspections_are_exact_and_final() {
    let fixture = fixture();
    let control = shared_control();
    let claimed = claimed_inspection(&control);
    claimed.validate().expect("durable v4 quarantine");
    let mut terminal = terminal_inspection(&fixture, &control, claimed.graph_run);
    terminal
        .validate()
        .expect("terminal lifecycle is final with no active lane");
    terminal.graph_run.run.status = forge_runtime_domain::GroupAgentGraphRunStatus::Failed;
    assert!(terminal.graph_run.validate().is_err());
}

fn claimed_inspection(
    control: &GroupAgentNodeTerminalControl,
) -> GroupAgentNodeLifecycleInspection {
    let v4 = GroupAgentGraphRunInspection {
        v: control.graph_run.v,
        run: control.graph_run.clone(),
        plan_json: control.plan.canonical_json().unwrap(),
        plan: control.plan.clone(),
        event_jsons: canonical_events(&control.journal_events),
        events: control.journal_events.clone(),
    };
    GroupAgentNodeLifecycleInspection {
        v: 1,
        graph_run: v4,
        claim: control.claim.clone(),
        claim_json: control.claim.canonical_json().unwrap(),
        active_lane: Some(control.active_lane.clone()),
        active_lane_json: Some(control.active_lane.canonical_json().unwrap()),
        artifact: None,
        artifact_json: None,
        terminal_receipt: None,
        terminal_receipt_json: None,
    }
}

fn terminal_inspection(
    fixture: &SharedLifecycleFixture,
    control: &GroupAgentNodeTerminalControl,
    mut v5: GroupAgentGraphRunInspection,
) -> GroupAgentNodeLifecycleInspection {
    let receipt = shared_receipt();
    let terminal_event =
        GroupAgentGraphRunEvent::decode_exact(&fixture.canonical_terminal_event_json)
            .expect("shared seq-5 event");
    v5.v = GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION;
    v5.run.v = GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION;
    v5.run.status = receipt.graph_status;
    v5.run.last_event_seq = 5;
    v5.events.push(terminal_event);
    v5.event_jsons = canonical_events(&v5.events);
    v5.run.journal_bytes = v5.event_jsons.iter().map(String::len).sum();
    v5.validate().expect("terminal v5 Graph Run");
    GroupAgentNodeLifecycleInspection {
        v: 1,
        graph_run: v5,
        claim: control.claim.clone(),
        claim_json: control.claim.canonical_json().unwrap(),
        active_lane: None,
        active_lane_json: None,
        artifact: Some(shared_artifact()),
        artifact_json: Some(fixture.canonical_terminal_artifact_json.clone()),
        terminal_receipt: Some(receipt),
        terminal_receipt_json: Some(fixture.canonical_terminal_receipt_json.clone()),
    }
}

#[test]
fn claim_lane_and_nonclone_authority_bind_exact_persisted_body() {
    let control = shared_control();
    let claim_json = control.claim.canonical_json().expect("claim JSON");
    let lane_json = control.active_lane.canonical_json().expect("lane JSON");
    let claim = GroupAgentNodeDispatchClaim::decode_exact(claim_json.as_bytes()).unwrap();
    let lane = GroupAgentNodeActiveLane::decode_exact(lane_json.as_bytes()).unwrap();
    lane.validate_against_claim(&claim)
        .expect("exact active lane");
    assert_eq!(claim.v, GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION);
    assert_eq!(lane.v, GROUP_AGENT_NODE_ACTIVE_LANE_VERSION);

    let event = control.journal_events[3].clone();
    let body = control.provider_request_json.as_bytes().to_vec();
    let authority = forge_runtime_domain::GroupAgentNodeDispatchAuthority::new(
        &control.dispatch_request,
        claim.clone(),
        &event,
        body.clone(),
    )
    .expect("winner authority");
    let (owned_claim, owned_body) = authority.into_parts();
    assert_eq!(owned_claim, claim);
    assert_eq!(owned_body, body);

    let mut wrong = control.provider_request_json.into_bytes();
    wrong.push(b' ');
    assert!(
        forge_runtime_domain::GroupAgentNodeDispatchAuthority::new(
            &control.dispatch_request,
            claim,
            &event,
            wrong,
        )
        .is_err()
    );
}

#[test]
fn every_fixed_uncertainty_class_is_closed_world_and_nonretryable() {
    let baseline = shared_artifact();
    for class in [
        GroupAgentNodeTerminalClassification::ProviderError,
        GroupAgentNodeTerminalClassification::HttpError,
        GroupAgentNodeTerminalClassification::TransportError,
        GroupAgentNodeTerminalClassification::Timeout,
        GroupAgentNodeTerminalClassification::Cancelled,
        GroupAgentNodeTerminalClassification::EofBeforeTerminal,
        GroupAgentNodeTerminalClassification::MissingUsage,
        GroupAgentNodeTerminalClassification::ToolCall,
        GroupAgentNodeTerminalClassification::ProtocolError,
        GroupAgentNodeTerminalClassification::TrailingData,
        GroupAgentNodeTerminalClassification::LocalLimit,
    ] {
        let artifact = uncertainty(&baseline, class);
        artifact
            .validate()
            .unwrap_or_else(|error| panic!("{class:?}: {error}"));
        assert!(!artifact.retry_authorized);
    }
}

#[test]
fn completed_length_and_uncertainty_outcomes_are_not_interchangeable() {
    let completed = shared_artifact();
    completed.validate().expect("completed result");

    let mut length = completed.clone();
    length.classification = GroupAgentNodeTerminalClassification::Length;
    length.output_text.clear();
    sign_artifact(&mut length);
    length.validate().expect("known length failure");

    let mut bad = completed;
    bad.artifact_kind = GroupAgentNodeTerminalArtifactKind::Uncertainty;
    sign_artifact(&mut bad);
    assert!(bad.validate().is_err());
}

#[test]
fn terminal_decoders_reject_duplicate_unknown_missing_null_reordered_and_trailing() {
    let fixture = fixture();
    reject_artifact_json(&fixture.canonical_terminal_artifact_json);
    reject_control_json(&fixture.canonical_terminal_control_json);
    reject_receipt_json(&fixture.canonical_terminal_receipt_json);
    for event in [
        fixture.canonical_claim_event_json,
        fixture.canonical_terminal_event_json,
    ] {
        for (index, candidate) in adversarial_json(&event, "dispatch_id")
            .into_iter()
            .enumerate()
        {
            assert!(
                GroupAgentGraphRunEvent::decode_exact(&candidate).is_err(),
                "accepted adversarial event {index}: {candidate}"
            );
        }
    }
}

#[test]
fn authorization_exact_decoder_is_bounded_canonical_and_redacted() {
    let authorization = shared_control().authorization;
    let canonical = authorization.canonical_json().expect("authorization JSON");
    assert_eq!(
        forge_runtime_domain::GroupAgentNodeDispatchAuthorization::decode_exact(&canonical)
            .expect("exact authorization"),
        authorization
    );
    for candidate in adversarial_json(&canonical, "authorization_sha256") {
        assert!(
            forge_runtime_domain::GroupAgentNodeDispatchAuthorization::decode_exact(&candidate)
                .is_err()
        );
    }
    let credential_fixture = "credential-material-must-not-leak";
    let error = forge_runtime_domain::GroupAgentNodeDispatchAuthorization::decode_exact(&format!(
        "{{{credential_fixture}"
    ))
    .expect_err("invalid authorization");
    assert!(!error.message.contains(credential_fixture));
}

#[test]
fn artifact_rejects_digest_version_size_usage_and_redaction_drift() {
    let baseline = shared_artifact();
    let mut version = baseline.clone();
    version.v += 1;
    sign_artifact(&mut version);
    assert!(version.validate().is_err());

    let mut digest = baseline.clone();
    digest.output_sha256 = "0".repeat(64);
    assert!(digest.validate().is_err());

    let mut usage = baseline.clone();
    usage.usage_observed = false;
    sign_artifact(&mut usage);
    assert!(usage.validate().is_err());

    let oversized = vec![b'x'; MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES + 1];
    assert!(GroupAgentNodeTerminalArtifact::decode_exact(&oversized).is_err());

    let credential_fixture = "credential-must-never-echo";
    let error =
        GroupAgentNodeTerminalArtifact::decode_exact(format!("{{{credential_fixture}").as_bytes())
            .expect_err("invalid artifact JSON");
    assert!(!error.message.contains(credential_fixture));
}

#[test]
fn terminal_control_rejects_claim_lane_artifact_and_single_node_binding_drift() {
    let baseline = shared_control();
    let mut lane = baseline.clone();
    lane.active_lane.lane_ownership_id = "another-lane-owner".into();
    sign_control(&mut lane);
    assert!(lane.validate().is_err());

    let mut claim = baseline.clone();
    claim.claim.max_cost_usd_micros -= 1;
    sign_control(&mut claim);
    assert!(claim.validate().is_err());

    let mut artifact = baseline.clone();
    artifact.artifact.dispatch_id = "another-dispatch".into();
    sign_artifact(&mut artifact.artifact);
    sign_control(&mut artifact);
    assert!(artifact.validate().is_err());

    let mut topology = baseline;
    topology
        .plan
        .waves
        .push(vec![topology.claim.node_id.clone()]);
    sign_control(&mut topology);
    assert!(topology.validate().is_err());
}

#[test]
fn receipt_mapping_and_seq5_terminal_status_fail_closed() {
    let control = shared_control();
    let baseline = shared_receipt();
    assert_eq!(
        baseline.node_outcome,
        GroupAgentNodeTerminalOutcome::Completed
    );
    assert_eq!(
        baseline.graph_status,
        forge_runtime_domain::GroupAgentGraphRunStatus::Completed
    );

    let mut wrong = baseline;
    wrong.node_outcome = GroupAgentNodeTerminalOutcome::Failed;
    wrong.wave_outcome = GroupAgentNodeTerminalOutcome::Failed;
    wrong.graph_status = forge_runtime_domain::GroupAgentGraphRunStatus::Failed;
    sign_receipt(&mut wrong);
    assert!(wrong.validate_against_control(&control).is_err());

    let json = fixture().canonical_terminal_event_json;
    let event = GroupAgentGraphRunEvent::decode_exact(&json).expect("seq-5 event");
    assert_eq!(event.v, GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION);
    event.validate().expect("valid seq-5 event");
}

fn uncertainty(
    baseline: &GroupAgentNodeTerminalArtifact,
    class: GroupAgentNodeTerminalClassification,
) -> GroupAgentNodeTerminalArtifact {
    let mut value = baseline.clone();
    value.artifact_kind = GroupAgentNodeTerminalArtifactKind::Uncertainty;
    value.classification = class;
    value.terminal_seen = class == GroupAgentNodeTerminalClassification::MissingUsage;
    value.stream_eof_seen = class == GroupAgentNodeTerminalClassification::MissingUsage;
    value.output_text.clear();
    value.usage_observed = false;
    value.input_tokens = 0;
    value.output_tokens = 0;
    value.actual_cost_calculated = false;
    value.actual_cost_usd_micros = 0;
    sign_artifact(&mut value);
    value
}

fn sign_artifact(value: &mut GroupAgentNodeTerminalArtifact) {
    value.output_bytes = value.output_text.len();
    value.output_sha256 = group_agent_node_terminal_output_sha256(&value.output_text);
    value.artifact_bytes = value
        .canonical_payload_json()
        .expect("artifact payload")
        .len();
    value.artifact_sha256 = value.expected_sha256().expect("artifact digest");
    value.artifact_id = group_agent_node_terminal_artifact_id(&value.artifact_sha256);
}

fn sign_control(value: &mut GroupAgentNodeTerminalControl) {
    value.snapshot_sha256 = value.expected_sha256().expect("control digest");
}

fn sign_receipt(value: &mut GroupAgentNodeTerminalReceipt) {
    value.receipt_sha256 = value.expected_sha256().expect("receipt digest");
    value.receipt_id =
        forge_runtime_domain::group_agent_node_terminal_receipt_id(&value.receipt_sha256);
}

fn reject_artifact_json(canonical: &str) {
    for candidate in adversarial_json(canonical, "artifact_kind") {
        assert!(GroupAgentNodeTerminalArtifact::decode_exact(candidate.as_bytes()).is_err());
    }
}

fn reject_control_json(canonical: &str) {
    for candidate in adversarial_json(canonical, "terminal_control_protocol_version") {
        assert!(GroupAgentNodeTerminalControl::decode_exact(candidate.as_bytes()).is_err());
    }
}

fn reject_receipt_json(canonical: &str) {
    for candidate in adversarial_json(canonical, "graph_status") {
        assert!(GroupAgentNodeTerminalReceipt::decode_exact(candidate.as_bytes()).is_err());
    }
}

fn adversarial_json(canonical: &str, removed_field: &str) -> Vec<String> {
    let first_end = canonical.find(',').expect("first JSON field");
    let first = &canonical[1..first_end];
    let duplicate = format!("{{{first},{first}{}", &canonical[first_end..]);
    let unknown = format!("{{\"unknown\":1,{}", &canonical[1..]);
    let mut missing = serde_json::from_str::<Value>(canonical).expect("JSON object");
    missing.as_object_mut().unwrap().remove(removed_field);
    let mut null = serde_json::from_str::<Value>(canonical).expect("JSON object");
    null[removed_field] = Value::Null;
    let reordered = reorder_first_two_fields(canonical);
    vec![
        duplicate,
        unknown,
        serde_json::to_string(&missing).unwrap(),
        serde_json::to_string(&null).unwrap(),
        reordered,
        format!("{canonical}\n"),
    ]
}

fn reorder_first_two_fields(canonical: &str) -> String {
    let first_end = canonical.find(',').expect("first JSON field");
    let second_relative = canonical[first_end + 1..]
        .find(',')
        .expect("second JSON field");
    let second_end = first_end + 1 + second_relative;
    format!(
        "{{{},{}{}",
        &canonical[first_end + 1..second_end],
        &canonical[1..first_end],
        &canonical[second_end..]
    )
}

fn validate_seq5(fixture: &SharedLifecycleFixture) {
    let event = GroupAgentGraphRunEvent::decode_exact(&fixture.canonical_terminal_event_json)
        .expect("shared seq-5 event");
    assert_eq!(
        event.expected_sha256().unwrap(),
        fixture.terminal_event_sha256
    );
    assert!(!fixture.canonical_terminal_event_json.ends_with('\n'));
}

fn assert_no_trailing_lf(fixture: &SharedLifecycleFixture) {
    for json in [
        &fixture.canonical_claim_event_json,
        &fixture.canonical_terminal_artifact_json,
        &fixture.canonical_terminal_control_json,
        &fixture.canonical_terminal_receipt_json,
    ] {
        assert!(!json.ends_with('\n'));
    }
}

fn fixture() -> SharedLifecycleFixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-terminal-lifecycle-v1.json"
    )))
    .expect("shared terminal lifecycle fixture")
}

fn shared_artifact() -> GroupAgentNodeTerminalArtifact {
    GroupAgentNodeTerminalArtifact::decode_exact(
        fixture().canonical_terminal_artifact_json.as_bytes(),
    )
    .expect("shared terminal artifact")
}

fn shared_control() -> GroupAgentNodeTerminalControl {
    GroupAgentNodeTerminalControl::decode_exact(
        fixture().canonical_terminal_control_json.as_bytes(),
    )
    .expect("shared terminal control")
}

fn shared_receipt() -> GroupAgentNodeTerminalReceipt {
    GroupAgentNodeTerminalReceipt::decode_exact(
        fixture().canonical_terminal_receipt_json.as_bytes(),
    )
    .expect("shared terminal receipt")
}

fn canonical_events(events: &[GroupAgentGraphRunEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| event.canonical_json().expect("canonical event"))
        .collect()
}
