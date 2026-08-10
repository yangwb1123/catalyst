use crate::governance_contract::{
    CollectorType, EvidenceType, GovernanceRecord, PrincipalType, SnapshotType,
    decode_canonical_record,
};

use super::*;

#[test]
fn golden_mapping_matches_exact_cross_language_bytes_and_digests() {
    let fixture = fixture();
    assert_eq!(
        fixture.api_version,
        "forgeos.governance.command-observation-evidence-adapter.fixture/v1"
    );
    assert_eq!(
        canonical_command_json(&fixture.request.observation.command).expect("command"),
        fixture.expected.canonical_command_json
    );
    assert_eq!(
        canonical_observation_json(&fixture.request.observation).expect("observation"),
        fixture.expected.canonical_observation_json
    );
    assert_eq!(
        canonical_request_json(&fixture.request).expect("request"),
        fixture.expected.canonical_request_json
    );
    let adapted = adapt_fixture();
    assert_eq!(adapted.command_sha256, fixture.expected.command_sha256);
    assert_eq!(adapted.request_sha256, fixture.expected.request_sha256);
    assert_eq!(
        adapted.source_snapshot_sha256,
        fixture.expected.source_snapshot_sha256
    );
    assert_eq!(
        adapted.canonical_evidence_json,
        fixture.expected.canonical_evidence_record_json
    );
    assert_eq!(
        adapted.evidence.integrity.canonical_sha256,
        fixture.expected.evidence_record_sha256
    );
    assert_eq!(adapted.result, fixture.expected.result);
}

#[test]
fn golden_output_has_exact_shadow_identity_and_observed_collector() {
    let adapted = adapt_fixture();
    decode_canonical_record(adapted.canonical_evidence_json.as_bytes())
        .expect("strict governance decoder");
    let evidence = &adapted.evidence;
    assert_eq!(
        evidence.metadata.created_by.run_id,
        format!("command-adaptation-{}", adapted.request_sha256)
    );
    assert_ne!(
        evidence.metadata.created_by.run_id,
        evidence.spec.collector.run_id
    );
    assert_eq!(
        evidence.metadata.created_by.principal_type,
        PrincipalType::Tool
    );
    assert_eq!(evidence.spec.collector.collector_type, CollectorType::Tool);
    assert_eq!(
        evidence.spec.collector.parameters_sha256,
        adapted.command_sha256
    );
    assert_eq!(evidence.spec.evidence_type, EvidenceType::GateResult);
    assert_eq!(
        evidence.spec.source_snapshot.snapshot_type,
        SnapshotType::Runtime
    );
    assert_eq!(evidence.spec.locator.exit_code, Some(0));
}

#[test]
fn gate_and_test_observations_accept_zero_and_nonzero_real_exits() {
    let gate = adapt_fixture();
    assert_eq!(gate.evidence.spec.evidence_type, EvidenceType::GateResult);
    assert_eq!(gate.evidence.spec.locator.exit_code, Some(0));

    let mut request = fixture().request;
    request.observation.evidence_type = CommandEvidenceType::TestRun;
    request.observation.termination.exit_code = Some(17);
    let canonical = canonical_request(&request);
    let test_run = adapt_canonical_request(canonical.as_bytes()).expect("nonzero test run");
    assert_eq!(test_run.evidence.spec.evidence_type, EvidenceType::TestRun);
    assert_eq!(test_run.evidence.spec.locator.exit_code, Some(17));
    assert_ne!(test_run.request_sha256, gate.request_sha256);
    assert_ne!(
        test_run.evidence.integrity.canonical_sha256,
        gate.evidence.integrity.canonical_sha256
    );
}

#[test]
fn exact_golden_projection_revalidates() {
    let fixture = fixture();
    let validated = validate_adaptation(
        fixture.expected.canonical_request_json.as_bytes(),
        fixture.expected.canonical_evidence_record_json.as_bytes(),
    )
    .expect("exact deterministic projection");
    assert_eq!(validated.request_sha256, fixture.expected.request_sha256);
    assert!(matches!(
        decode_canonical_record(validated.canonical_evidence_json.as_bytes()),
        Ok(GovernanceRecord::Evidence(_))
    ));
}
