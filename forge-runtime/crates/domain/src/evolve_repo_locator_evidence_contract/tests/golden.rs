use crate::governance_contract::{
    CollectorType, EvidenceType, GovernanceRecord, LocatorType, PrincipalType, SnapshotType,
    decode_canonical_record,
};

use super::*;

#[test]
fn golden_mapping_matches_exact_cross_language_bytes_and_digests() {
    let fixture = fixture();
    assert_eq!(
        fixture.api_version,
        "forgeos.governance.evolve-repo-locator-evidence-adapter.fixture/v1"
    );
    assert_eq!(
        canonical_locator_json(&fixture.request.observation.locator).expect("locator"),
        fixture.expected.canonical_locator_json
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
    assert_eq!(adapted.locator_sha256, fixture.expected.locator_sha256);
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
fn golden_output_has_exact_shadow_identity_and_repo_mapping() {
    let adapted = adapt_fixture();
    decode_canonical_record(adapted.canonical_evidence_json.as_bytes())
        .expect("strict Governance decoder");
    let evidence = &adapted.evidence;
    assert_eq!(
        evidence.metadata.created_by.run_id,
        format!("evolve-locator-adaptation-{}", adapted.request_sha256)
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
    assert_eq!(evidence.spec.evidence_type, EvidenceType::RepoLocator);
    assert_eq!(evidence.spec.locator.locator_type, LocatorType::Repo);
    assert_eq!(evidence.spec.locator.exit_code, None);
    assert_eq!(evidence.spec.locator.line_start, Some(114));
    assert_eq!(evidence.spec.locator.line_end, Some(114));
    assert_eq!(
        evidence.spec.source_snapshot.snapshot_type,
        SnapshotType::Repository
    );
    assert_eq!(
        evidence.spec.artifact_sha256.as_deref(),
        Some(evidence.spec.locator.content_sha256.as_str())
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

#[test]
fn zero_line_projects_to_null_pair_without_reading_the_path() {
    let mut request = fixture().request;
    request.observation.locator.line = 0;
    request.observation.locator.path = "missing/not-read.txt".into();
    let canonical = canonical_request(&request);
    let adapted =
        adapt_canonical_request(canonical.as_bytes()).expect("pure file-level locator adaptation");
    assert_eq!(adapted.evidence.spec.locator.line_start, None);
    assert_eq!(adapted.evidence.spec.locator.line_end, None);
    assert_eq!(
        adapted.evidence.spec.locator.locator_ref,
        "missing/not-read.txt"
    );
}
