use crate::governance_contract::{
    EvidenceState, GovernanceRecord, SourceTrust, decode_canonical_record,
};

use super::*;

#[test]
fn exact_revalidation_rejects_resealed_structural_projection_drift() {
    let adapted = adapt_fixture();
    let mut cases = Vec::new();
    let mut trust = adapted.evidence.clone();
    trust.spec.source_trust = SourceTrust::Untrusted;
    cases.push(trust);
    let mut collector = adapted.evidence.clone();
    collector.spec.collector.collector_id = "other-scanner".into();
    cases.push(collector);
    let mut identity = adapted.evidence.clone();
    identity.metadata.record_id = "evolve-locator-evidence-drift".into();
    cases.push(identity);
    let mut line = adapted.evidence.clone();
    line.spec.locator.line_end = Some(115);
    cases.push(line);
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
fn unsealed_and_noncanonical_outputs_are_rejected() {
    let adapted = adapt_fixture();
    let unsealed = adapted.canonical_evidence_json.replacen(
        &adapted.evidence.integrity.canonical_sha256,
        &"0".repeat(64),
        1,
    );
    assert!(
        validate_adaptation(
            adapted.canonical_request_json.as_bytes(),
            unsealed.as_bytes()
        )
        .is_err()
    );
    assert!(
        validate_adaptation(
            adapted.canonical_request_json.as_bytes(),
            format!(" {}", adapted.canonical_evidence_json).as_bytes()
        )
        .is_err()
    );
}

#[test]
fn adaptation_is_deterministic_and_exposes_no_load_bearing_capability() {
    let first = adapt_fixture();
    let second = adapt_canonical_request(first.canonical_request_json.as_bytes())
        .expect("repeat pure adaptation");
    assert_eq!(first, second);
    assert_eq!(
        ADAPTED_SHADOW,
        "ADAPTED_SHADOW (locator mapping only; no file/report verification, scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)"
    );
    for forbidden in [
        "KnowledgeClaim",
        "CognitiveAtom",
        "authority_ref",
        "trusted_control",
        "effect_attestation",
        "stored",
        "exact_replay",
        "finding",
        "opportunity",
    ] {
        assert!(
            !first.canonical_evidence_json.contains(forbidden),
            "EvidenceRecord contains forbidden capability {forbidden}"
        );
    }
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
