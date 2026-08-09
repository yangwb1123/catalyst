use std::collections::BTreeMap;

use serde::Deserialize;

use crate::governance_contract::codec::canonical_record_set_json;
use crate::governance_contract::{
    ClaimObjectType, ClaimObjectValue, ClaimState, ClaimType, EvidenceType, GovernanceRecord,
    KnowledgeClaim, ValidationPlan, decode_canonical_record_set,
};

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/cognitive-atom-projection-v1.json");

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    api_version: String,
    canonicalization: String,
    digest_domains: BTreeMap<String, String>,
    expected: GoldenExpected,
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    atom_id: String,
    atom_set_sha256: String,
    canonical_atom_json: String,
    canonical_atom_payload_json: String,
    canonical_atom_set_json: String,
    canonical_atom_sha256: String,
    canonical_source_closure_json: String,
    source_closure_sha256: String,
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("CognitiveAtom golden fixture")
}

fn source_records(fixture: &GoldenFixture) -> Vec<GovernanceRecord> {
    decode_canonical_record_set(fixture.expected.canonical_source_closure_json.as_bytes())
        .expect("golden source record set")
}

fn claim_mut(record: &mut GovernanceRecord) -> &mut KnowledgeClaim {
    let GovernanceRecord::Claim(claim) = record else {
        panic!("expected KnowledgeClaim");
    };
    claim
}

fn reseal_record(record: &mut GovernanceRecord) {
    match record {
        GovernanceRecord::Evidence(evidence) => evidence.integrity.canonical_sha256.clear(),
        GovernanceRecord::Claim(claim) => claim.integrity.canonical_sha256.clear(),
    }
    let digest = record.expected_sha256().expect("record digest");
    match record {
        GovernanceRecord::Evidence(evidence) => evidence.integrity.canonical_sha256 = digest,
        GovernanceRecord::Claim(claim) => claim.integrity.canonical_sha256 = digest,
    }
}

fn source_variant(fixture: &GoldenFixture, claim_type: ClaimType, state: ClaimState) -> String {
    let mut records = source_records(fixture);
    let claim = claim_mut(&mut records[1]);
    claim.spec.claim_type = claim_type;
    claim.spec.confidence_micros = None;
    claim.spec.queue_ref = None;
    claim.spec.validation_plan = None;
    if matches!(
        claim_type,
        ClaimType::Assumption | ClaimType::Hypothesis | ClaimType::Inference
    ) {
        claim.spec.confidence_micros = Some(700_000);
    }
    if matches!(claim_type, ClaimType::Assumption | ClaimType::Hypothesis) {
        claim.spec.validation_plan = Some(ValidationPlan {
            due_at_unix_ms: 1_700_100_000_000,
            impact_if_false: "reassess task".into(),
            method: "rerun structural test".into(),
            owner_id: "governance-review".into(),
            required_evidence_types: vec![EvidenceType::TestRun],
        });
    }
    if claim_type == ClaimType::Unknown {
        claim.spec.queue_ref = Some("queue:unknown".into());
    }
    claim.status.state = state;
    reseal_record(&mut records[1]);
    canonical_record_set_json(&records).expect("canonical source variant")
}

fn reseal_atom(atom: &mut CognitiveAtom) {
    atom.integrity.canonical_sha256.clear();
    atom.integrity.canonical_sha256 = expected_atom_sha256(atom).expect("atom digest");
}

fn assert_fixture_contract(fixture: &GoldenFixture) {
    assert_eq!(fixture.api_version, "forgeos.aadm.cognitive-atom-golden/v1");
    assert_eq!(fixture.canonicalization, CANONICALIZATION);
    let expected = BTreeMap::from([
        ("atom".into(), "forgeos.aadm.cognitive-atom.v1".into()),
        ("atom_id".into(), "forgeos.aadm.cognitive-atom-id.v1".into()),
        (
            "atom_set".into(),
            "forgeos.aadm.cognitive-atom-set.v1".into(),
        ),
        (
            "source_closure".into(),
            "forgeos.governance.record-set.v1".into(),
        ),
    ]);
    assert_eq!(fixture.digest_domains, expected);
}

fn assert_golden_atom(fixture: &GoldenFixture, projected: &CognitiveAtomProjection) {
    assert_eq!(projected.atoms.len(), 1);
    let atom = &projected.atoms[0];
    assert_eq!(atom.metadata.atom_id, fixture.expected.atom_id);
    assert_eq!(
        atom.integrity.canonical_sha256,
        fixture.expected.canonical_atom_sha256
    );
    assert_eq!(
        canonical_atom_payload_json(atom).expect("payload"),
        fixture.expected.canonical_atom_payload_json
    );
    assert_eq!(
        canonical_atom_json(atom).expect("atom"),
        fixture.expected.canonical_atom_json
    );
    assert_eq!(
        projected.canonical_atom_set_json,
        fixture.expected.canonical_atom_set_json
    );
    assert_eq!(projected.atom_set_sha256, fixture.expected.atom_set_sha256);
}

fn assert_golden_closure(fixture: &GoldenFixture, projected: &CognitiveAtomProjection) {
    let atom = &projected.atoms[0];
    assert_eq!(
        atom.source.closure_byte_count,
        i64::try_from(fixture.expected.canonical_source_closure_json.len()).unwrap()
    );
    assert_eq!(atom.source.closure_record_count, 2);
    assert_eq!(
        atom.source.closure_sha256,
        fixture.expected.source_closure_sha256
    );
    assert_eq!(
        cognitive_atom_set_sha256(&projected.atoms).expect("set digest"),
        fixture.expected.atom_set_sha256
    );
}

#[test]
fn golden_projection_matches_exact_cross_language_bytes() {
    let fixture = fixture();
    assert_fixture_contract(&fixture);
    let projected = project_canonical_record_set(
        fixture.expected.canonical_source_closure_json.as_bytes(),
        &fixture.task_id,
    )
    .expect("golden projection");
    assert_golden_atom(&fixture, &projected);
    assert_golden_closure(&fixture, &projected);
    assert_eq!(
        decode_canonical_atom_set(fixture.expected.canonical_atom_set_json.as_bytes())
            .expect("golden atom set"),
        projected.atoms
    );
    validate_projection(
        fixture.expected.canonical_source_closure_json.as_bytes(),
        &fixture.task_id,
        fixture.expected.canonical_atom_set_json.as_bytes(),
    )
    .expect("exact source projection");
}

fn admissible_cases() -> [(ClaimType, ClaimState, AtomType); 11] {
    [
        (ClaimType::Fact, ClaimState::Candidate, AtomType::Fact),
        (ClaimType::Fact, ClaimState::Contested, AtomType::Fact),
        (
            ClaimType::Constraint,
            ClaimState::Candidate,
            AtomType::Constraint,
        ),
        (
            ClaimType::Decision,
            ClaimState::Proposed,
            AtomType::Decision,
        ),
        (
            ClaimType::Inference,
            ClaimState::Candidate,
            AtomType::Inference,
        ),
        (
            ClaimType::Assumption,
            ClaimState::Open,
            AtomType::Assumption,
        ),
        (
            ClaimType::Assumption,
            ClaimState::Testing,
            AtomType::Assumption,
        ),
        (
            ClaimType::Hypothesis,
            ClaimState::Open,
            AtomType::Hypothesis,
        ),
        (
            ClaimType::Hypothesis,
            ClaimState::Testing,
            AtomType::Hypothesis,
        ),
        (ClaimType::Unknown, ClaimState::Open, AtomType::Unknown),
        (
            ClaimType::Unknown,
            ClaimState::Investigating,
            AtomType::Unknown,
        ),
    ]
}

#[test]
fn every_admissible_shadow_type_and_state_projects() {
    let fixture = fixture();
    for (claim_type, state, expected_type) in admissible_cases() {
        let source = source_variant(&fixture, claim_type, state);
        let projected = project_canonical_record_set(source.as_bytes(), &fixture.task_id)
            .expect("admissible projection");
        let spec = &projected.atoms[0].spec;
        assert_eq!(spec.atom_type, expected_type);
        assert_eq!(spec.epistemic_state, state);
        assert_eq!(
            spec.projection_confidence_micros.is_some(),
            matches!(
                expected_type,
                AtomType::Assumption | AtomType::Hypothesis | AtomType::Inference
            )
        );
    }
}

#[test]
fn lesson_and_proposal_are_closure_only_not_atoms() {
    let fixture = fixture();
    for (claim_type, state) in [
        (ClaimType::Lesson, ClaimState::Candidate),
        (ClaimType::Proposal, ClaimState::Draft),
    ] {
        let source = source_variant(&fixture, claim_type, state);
        let error = project_canonical_record_set(source.as_bytes(), &fixture.task_id)
            .expect_err("non-projectable claim type");
        assert!(error.message.contains("no projectable KnowledgeClaim"));
    }
}

#[test]
fn canonical_decoder_rejects_duplicate_unknown_float_bidi_and_noncanonical_input() {
    let fixture = fixture();
    let canonical = &fixture.expected.canonical_atom_set_json;
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.aadm.cognitive-atom/v1\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_atom_set(duplicate.as_bytes()).is_err());
    let unknown = canonical.replacen("{\"api_version\":", "{\"alien\":null,\"api_version\":", 1);
    assert!(decode_canonical_atom_set(unknown.as_bytes()).is_err());
    assert!(decode_canonical_atom_set(format!(" {canonical}").as_bytes()).is_err());
    let float = canonical.replacen("\"claim_sequence\":1", "\"claim_sequence\":1.0", 1);
    assert!(decode_canonical_atom_set(float.as_bytes()).is_err());
    let bidi = canonical.replacen("构建通过 <>& 😀", "构建\u{202e}通过 <>& 😀", 1);
    assert!(decode_canonical_atom_set(bidi.as_bytes()).is_err());
    let oversized = vec![b' '; MAX_ATOM_SET_BYTES + 1];
    assert!(decode_canonical_atom_set(&oversized).is_err());
}

#[test]
fn structural_validation_rejects_authority_object_interval_evidence_and_identity_drift() {
    let fixture = fixture();
    let base = project_canonical_record_set(
        fixture.expected.canonical_source_closure_json.as_bytes(),
        &fixture.task_id,
    )
    .expect("projection")
    .atoms
    .remove(0);

    let mut authority = base.clone();
    authority.spec.authority_ref = Some("grant-001".into());
    reseal_atom(&mut authority);
    assert!(authority.validate().is_err());

    let mut object = base.clone();
    object.spec.proposition.object_type = ClaimObjectType::Boolean;
    reseal_atom(&mut object);
    assert!(object.validate().is_err());

    let mut artifact = base.clone();
    artifact.spec.proposition.object_type = ClaimObjectType::ArtifactRef;
    artifact.spec.proposition.object_value = ClaimObjectValue::String("not an id".into());
    reseal_atom(&mut artifact);
    assert!(artifact.validate().is_err());

    let mut interval = base.clone();
    interval.spec.validity.valid_until_unix_ms = Some(interval.spec.validity.valid_from_unix_ms);
    reseal_atom(&mut interval);
    assert!(interval.validate().is_err());

    let mut overlap = base.clone();
    overlap.spec.contradicting_evidence_record_ids =
        overlap.spec.supporting_evidence_record_ids.clone();
    reseal_atom(&mut overlap);
    assert!(overlap.validate().is_err());

    let mut identity = base;
    identity.metadata.task_id = "fixture-task-drift".into();
    reseal_atom(&mut identity);
    assert!(identity.validate().is_err());
}

#[test]
fn exact_reprojection_rejects_structurally_valid_field_and_task_drift() {
    let fixture = fixture();
    let source = fixture.expected.canonical_source_closure_json.as_bytes();
    let mut atom = project_canonical_record_set(source, &fixture.task_id)
        .expect("projection")
        .atoms
        .remove(0);
    atom.source.closure_byte_count += 1;
    reseal_atom(&mut atom);
    atom.validate().expect("structurally valid drift");
    let drifted_set = canonical_atom_set_json(&[atom]).expect("drifted set");
    assert!(decode_canonical_atom_set(drifted_set.as_bytes()).is_ok());
    assert!(validate_projection(source, &fixture.task_id, drifted_set.as_bytes()).is_err());
    assert!(
        validate_projection(
            source,
            "fixture-task-drift",
            fixture.expected.canonical_atom_set_json.as_bytes(),
        )
        .is_err()
    );
}

#[test]
fn atom_set_rejects_empty_duplicate_and_wrong_digest() {
    let fixture = fixture();
    assert!(decode_canonical_atom_set(b"[]").is_err());
    let atom = &fixture.expected.canonical_atom_json;
    let duplicate = format!("[{atom},{atom}]");
    assert!(decode_canonical_atom_set(duplicate.as_bytes()).is_err());
    let wrong_digest = fixture.expected.canonical_atom_set_json.replacen(
        &fixture.expected.canonical_atom_sha256,
        &"a".repeat(64),
        1,
    );
    assert!(decode_canonical_atom_set(wrong_digest.as_bytes()).is_err());
}

#[test]
fn result_wording_is_explicitly_non_authoritative() {
    assert!(PROJECTED_SHADOW.starts_with("PROJECTED_SHADOW"));
    for boundary in [
        "no truth",
        "authority",
        "instruction",
        "hard-guard",
        "transition",
        "completion",
        "effect attestation",
    ] {
        assert!(PROJECTED_SHADOW.contains(boundary));
    }
}
