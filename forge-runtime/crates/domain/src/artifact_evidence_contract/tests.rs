use serde::Deserialize;

use crate::governance_contract::{
    CollectorType, EvidenceState, GovernanceRecord, PrincipalType, Sensitivity, SourceTrust,
    decode_canonical_record,
};

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/artifact-evidence-adapter-v1.json");

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    api_version: String,
    expected: GoldenExpected,
    request: ArtifactEvidenceRequest,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    canonical_evidence_record_json: String,
    canonical_request_json: String,
    canonical_source_json: String,
    evidence_record_sha256: String,
    request_sha256: String,
    result: String,
    source_snapshot_sha256: String,
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("artifact evidence golden fixture")
}

fn adapt_fixture() -> ArtifactEvidenceAdaptation {
    let fixture = fixture();
    adapt_canonical_request(fixture.expected.canonical_request_json.as_bytes())
        .expect("golden adaptation")
}

#[test]
fn golden_mapping_matches_exact_cross_language_bytes() {
    let fixture = fixture();
    assert_eq!(
        fixture.api_version,
        "forgeos.governance.artifact-evidence-adapter.fixture/v1"
    );
    assert_eq!(
        canonical_request_json(&fixture.request).expect("canonical request"),
        fixture.expected.canonical_request_json
    );
    assert_eq!(
        canonical_artifact_json(&fixture.request.artifact).expect("canonical source"),
        fixture.expected.canonical_source_json
    );
    let adapted = adapt_fixture();
    assert_eq!(adapted.request_sha256, fixture.expected.request_sha256);
    assert_eq!(
        adapted.source_sha256,
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
fn golden_output_is_strictly_revalidated_and_has_fixed_shadow_mapping() {
    let adapted = adapt_fixture();
    validate_adaptation(
        adapted.canonical_request_json.as_bytes(),
        adapted.canonical_evidence_json.as_bytes(),
    )
    .expect("exact reprojection");
    decode_canonical_record(adapted.canonical_evidence_json.as_bytes())
        .expect("strict governance decoder");
    let evidence = &adapted.evidence;
    assert_eq!(evidence.metadata.created_at_unix_ms, 1_786_341_896_789);
    assert_eq!(evidence.spec.observed_at_unix_ms, 1_786_341_896_789);
    assert_eq!(evidence.status.valid_from_unix_ms, 1_786_341_896_789);
    assert_eq!(
        evidence.metadata.created_by.principal_type,
        PrincipalType::Tool
    );
    assert_eq!(evidence.spec.collector.collector_type, CollectorType::Tool);
    assert_eq!(evidence.spec.source_trust, SourceTrust::Observed);
    assert_eq!(evidence.status.state, EvidenceState::Valid);
}

#[test]
fn exact_request_decoder_rejects_unknown_duplicate_and_noncanonical_input() {
    let fixture = fixture();
    let canonical = fixture.expected.canonical_request_json;
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.governance.artifact-evidence-adapter/v1\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_request(duplicate.as_bytes()).is_err());
    let unknown = canonical.replacen("{\"api_version\":", "{\"alien\":null,\"api_version\":", 1);
    assert!(decode_canonical_request(unknown.as_bytes()).is_err());
    assert!(decode_canonical_request(format!(" {canonical}").as_bytes()).is_err());
    let reordered = canonical.replacen(
        "{\"api_version\":\"forgeos.governance.artifact-evidence-adapter/v1\",",
        "{\"canonicalization\":\"forgeos.canonical-json/v1\",\"api_version\":\"forgeos.governance.artifact-evidence-adapter/v1\",",
        1,
    );
    assert!(decode_canonical_request(reordered.as_bytes()).is_err());
}

#[test]
fn decoder_rejects_legacy_format_float_overflow_and_unicode_escapes() {
    let fixture = fixture();
    let canonical = fixture.expected.canonical_request_json;
    let legacy = canonical.replacen("forgeos.artifact.v1", "", 1);
    assert!(decode_canonical_request(legacy.as_bytes()).is_err());
    let float = canonical.replacen("\"sequence\":1", "\"sequence\":1.0", 1);
    assert!(decode_canonical_request(float.as_bytes()).is_err());
    let overflow = canonical.replacen("\"sequence\":1", "\"sequence\":9223372036854775808", 1);
    assert!(decode_canonical_request(overflow.as_bytes()).is_err());
    let escaped = canonical.replacen("artifact-report", "artifact\\u002dreport", 1);
    assert!(decode_canonical_request(escaped.as_bytes()).is_err());
    let bidi = canonical.replacen("artifact-report", "artifact\\u202ereport", 1);
    assert!(decode_canonical_request(bidi.as_bytes()).is_err());
}

#[test]
fn artifact_path_unicode_time_and_list_boundaries_fail_closed() {
    let mut unicode = fixture().request;
    unicode.artifact.path = "dist/报告.json".into();
    let canonical = canonical_request_json(&unicode).expect("safe Unicode path");
    adapt_canonical_request(canonical.as_bytes()).expect("safe Unicode adaptation");
    for path in [
        "/etc/passwd",
        "C:/secret",
        "dist\\report.json",
        "dist//report.json",
        "dist/./report.json",
        "dist/../report.json",
    ] {
        let mut request = fixture().request;
        request.artifact.path = path.into();
        assert!(canonical_request_json(&request).is_err(), "accepted {path}");
    }
    let mut request = fixture().request;
    request.artifact.path = "dist/report\u{202e}.json".into();
    assert!(canonical_request_json(&request).is_err());
    let mut request = fixture().request;
    request.artifact.model = "界".repeat(4_097);
    assert!(canonical_request_json(&request).is_err());
    let mut request = fixture().request;
    request.artifact.created_at = "1969-12-31T23:59:59.999Z".into();
    assert!(canonical_request_json(&request).is_err());
    let mut request = fixture().request;
    request.binding.subjects = (0..=256)
        .map(|index| format!("subject:{index:03}"))
        .collect();
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn artifact_text_fields_reject_whitespace_only_values() {
    let mutations: [fn(&mut ArtifactEvidenceRequest); 5] = [
        |r| r.artifact.agent = "   ".into(),
        |r| r.artifact.model = "\u{00a0}".into(),
        |r| r.artifact.path = "\u{2003}".into(),
        |r| r.artifact.phase = "\u{202f}".into(),
        |r| r.artifact.workflow = "\u{3000}".into(),
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        assert!(canonical_request_json(&request).is_err());
    }
}

#[test]
fn timestamp_is_floored_while_exact_spelling_remains_identity_bearing() {
    let base = adapt_fixture();
    let mut request = fixture().request;
    request.artifact.created_at = "2026-08-10T12:34:56.789999999+06:30".into();
    let canonical = canonical_request_json(&request).expect("offset request");
    let variant = adapt_canonical_request(canonical.as_bytes()).expect("offset adaptation");
    assert_eq!(
        variant.evidence.metadata.created_at_unix_ms,
        1_786_341_896_789
    );
    assert_ne!(variant.source_sha256, base.source_sha256);
    assert_ne!(variant.request_sha256, base.request_sha256);
    let mut request = fixture().request;
    request.artifact.created_at = "2026-02-29T00:00:00Z".into();
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn timestamp_rejects_non_rfc3339nano_spelling_and_offsets() {
    for created_at in [
        "2026-08-10T06:04:56,1Z",
        "2026-08-10T06:04:56.1234567890Z",
        "2026-08-10T06:04:56+24:00",
        "2026-08-10T06:04:56+00:60",
        "2026-08-10t06:04:56Z",
        "2026-08-10T06:04:56z",
    ] {
        let mut request = fixture().request;
        request.artifact.created_at = created_at.into();
        assert!(
            canonical_request_json(&request).is_err(),
            "accepted {created_at}"
        );
    }
}

#[test]
fn artifact_drift_changes_source_request_and_evidence_identity() {
    let base = adapt_fixture();
    let mutations: [fn(&mut ArtifactEvidenceRequest); 9] = [
        |r| r.artifact.run_id = "run-artifact-0049".into(),
        |r| r.artifact.workflow = "release".into(),
        |r| r.artifact.phase = "reviewer".into(),
        |r| r.artifact.agent = "reviewer".into(),
        |r| r.artifact.model = "gpt-5.7".into(),
        |r| r.artifact.path = "docs/release/other.json".into(),
        |r| r.artifact.sha256 = "a".repeat(64),
        |r| r.artifact.size = 17,
        |r| r.artifact.prompt_sha256 = "b".repeat(64),
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        let canonical = canonical_request_json(&request).expect("artifact variant");
        let variant = adapt_canonical_request(canonical.as_bytes()).expect("adapt variant");
        assert_ne!(variant.source_sha256, base.source_sha256);
        assert_ne!(variant.request_sha256, base.request_sha256);
        assert_ne!(
            variant.evidence.integrity.canonical_sha256,
            base.evidence.integrity.canonical_sha256
        );
    }
}

#[test]
fn binding_drift_preserves_source_but_changes_request_and_evidence_identity() {
    let base = adapt_fixture();
    let mutations: [fn(&mut ArtifactEvidenceRequest); 11] = [
        |r| r.binding.aggregate_id = "artifact-run-artifact-0049".into(),
        |r| r.binding.context_sha256 = "a".repeat(64),
        |r| r.binding.policy_sha256 = "b".repeat(64),
        |r| r.binding.project_id = "project-other".into(),
        |r| r.binding.scope = "organization".into(),
        |r| r.binding.source_tree_sha256 = "c".repeat(64),
        |r| r.binding.source_revision = "c17e671".into(),
        |r| r.binding.sequence = 2,
        |r| r.binding.subjects = vec!["artifact:other".into()],
        |r| r.binding.supersedes_record_ids = vec!["evidence:prior".into()],
        |r| r.binding.sensitivity = Sensitivity::Restricted,
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        let canonical = canonical_request_json(&request).expect("binding variant");
        let variant = adapt_canonical_request(canonical.as_bytes()).expect("adapt variant");
        assert_eq!(variant.source_sha256, base.source_sha256);
        assert_ne!(variant.request_sha256, base.request_sha256);
        assert_ne!(
            variant.evidence.integrity.canonical_sha256,
            base.evidence.integrity.canonical_sha256
        );
    }
}

#[test]
fn exact_revalidation_rejects_structurally_valid_output_drift() {
    let adapted = adapt_fixture();
    let mut cases = Vec::new();
    let mut trust = adapted.evidence.clone();
    trust.spec.source_trust = SourceTrust::Untrusted;
    cases.push(trust);
    let mut collector = adapted.evidence.clone();
    collector.spec.collector.collector_id = "other-adapter".into();
    cases.push(collector);
    let mut identity = adapted.evidence.clone();
    identity.metadata.record_id = "artifact-evidence-drift".into();
    cases.push(identity);
    let mut snapshot = adapted.evidence.clone();
    snapshot.spec.source_snapshot.snapshot_sha256 = "f".repeat(64);
    cases.push(snapshot);
    let mut state = adapted.evidence.clone();
    state.status.state = EvidenceState::Invalid;
    state.status.reason_codes = vec!["adapter-drift".into()];
    cases.push(state);
    for evidence in cases {
        let drifted = reseal(evidence);
        decode_canonical_record(drifted.as_bytes()).expect("valid drifted EvidenceRecord");
        assert!(
            validate_adaptation(
                adapted.canonical_request_json.as_bytes(),
                drifted.as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn strict_revalidation_rejects_unsealed_self_digest_drift() {
    let adapted = adapt_fixture();
    let drifted = adapted.canonical_evidence_json.replacen(
        &adapted.evidence.integrity.canonical_sha256,
        &"0".repeat(64),
        1,
    );
    assert!(decode_canonical_record(drifted.as_bytes()).is_err());
    assert!(
        validate_adaptation(
            adapted.canonical_request_json.as_bytes(),
            drifted.as_bytes()
        )
        .is_err()
    );
}

fn reseal(evidence: crate::governance_contract::EvidenceRecord) -> String {
    let mut evidence = evidence;
    evidence.integrity.canonical_sha256.clear();
    let mut record = GovernanceRecord::Evidence(evidence);
    let digest = record.expected_sha256().expect("drift digest");
    let GovernanceRecord::Evidence(evidence) = &mut record else {
        unreachable!()
    };
    evidence.integrity.canonical_sha256 = digest;
    record.canonical_record_json().expect("drifted record")
}

#[test]
fn result_and_evidence_preserve_authority_persistence_and_effect_boundaries() {
    assert_eq!(
        ADAPTED_SHADOW,
        "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)"
    );
    let adapted = adapt_fixture();
    for forbidden in [
        "KnowledgeClaim",
        "CognitiveAtom",
        "authority_ref",
        "trusted_control",
        "effect_attestation",
        "stored",
        "exact_replay",
    ] {
        assert!(
            !adapted.canonical_evidence_json.contains(forbidden),
            "EvidenceRecord contains forbidden capability {forbidden}"
        );
    }
}
