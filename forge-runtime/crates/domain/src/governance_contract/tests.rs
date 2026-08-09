use serde::Deserialize;

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    records: Vec<GoldenEntry>,
}

#[derive(Debug, Deserialize)]
struct GoldenEntry {
    digest_domain: String,
    expected: GoldenExpected,
    record: GovernanceRecord,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    #[serde(rename = "canonical_payload_json")]
    payload_json: String,
    #[serde(rename = "canonical_record_json")]
    record_json: String,
    #[serde(rename = "canonical_sha256")]
    sha256: String,
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("governance golden fixture")
}

fn fixture_records() -> Vec<GovernanceRecord> {
    fixture()
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

fn reseal(record: &mut GovernanceRecord) {
    record.integrity_mut().canonical_sha256.clear();
    let digest = record.expected_sha256().expect("canonical digest");
    record.integrity_mut().canonical_sha256 = digest;
}

fn evidence_mut(record: &mut GovernanceRecord) -> &mut EvidenceRecord {
    let GovernanceRecord::Evidence(evidence) = record else {
        panic!("expected evidence record");
    };
    evidence
}

fn claim_mut(record: &mut GovernanceRecord) -> &mut KnowledgeClaim {
    let GovernanceRecord::Claim(claim) = record else {
        panic!("expected claim record");
    };
    claim
}

#[test]
fn golden_records_match_exact_bytes_and_digests() {
    let fixture = fixture();
    for entry in &fixture.records {
        let domain = std::str::from_utf8(entry.record.digest_domain())
            .expect("ASCII domain")
            .trim_end_matches('\0');
        assert_eq!(domain, entry.digest_domain);
        assert_eq!(
            entry.record.canonical_payload_json().expect("payload"),
            entry.expected.payload_json
        );
        assert_eq!(
            entry.record.canonical_record_json().expect("record"),
            entry.expected.record_json
        );
        assert_eq!(
            entry.record.expected_sha256().expect("digest"),
            entry.expected.sha256
        );
        entry.record.validate().expect("valid golden record");
    }
}

#[test]
fn golden_canonical_inputs_decode_and_validate_as_a_set() {
    let fixture = fixture();
    let mut canonical_records = Vec::new();
    for entry in &fixture.records {
        let decoded = decode_canonical_record(entry.expected.record_json.as_bytes())
            .expect("decode golden record");
        assert_eq!(decoded, entry.record);
        canonical_records.push(entry.expected.record_json.as_str());
    }
    let canonical_set = format!("[{}]", canonical_records.join(","));
    let decoded = decode_canonical_record_set(canonical_set.as_bytes()).expect("golden set");
    assert_eq!(decoded, fixture_records());
}

#[test]
fn duplicate_unknown_float_and_noncanonical_inputs_are_rejected() {
    let canonical = fixture().records[0].expected.record_json.clone();
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.governance/v1\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_record(duplicate.as_bytes()).is_err());
    let unknown = canonical.replacen("{\"api_version\":", "{\"added\":1,\"api_version\":", 1);
    assert!(decode_canonical_record(unknown.as_bytes()).is_err());
    assert!(decode_canonical_record(format!(" {canonical}").as_bytes()).is_err());
    let claim = fixture().records[1].expected.record_json.clone();
    let float = claim.replacen(
        "\"object_value\":\"构建通过 <>& 😀\"",
        "\"object_value\":1.5",
        1,
    );
    assert!(decode_canonical_record(float.as_bytes()).is_err());
}

#[test]
fn digest_and_forbidden_unicode_tampering_are_rejected() {
    let canonical = fixture().records[0].expected.record_json.clone();
    let tampered = canonical.replacen("dc696353", "ac696353", 1);
    assert!(decode_canonical_record(tampered.as_bytes()).is_err());
    let claim = fixture().records[1].expected.record_json.clone();
    let forbidden = claim.replacen("候选事实", "候\u{202e}选事实", 1);
    assert!(decode_canonical_record(forbidden.as_bytes()).is_err());
}

#[test]
fn shadow_contract_rejects_authority_and_trusted_control() {
    let mut records = fixture_records();
    let claim = claim_mut(&mut records[1]);
    claim.status.state = ClaimState::Confirmed;
    reseal(&mut records[1]);
    assert!(records[1].validate().is_err());

    let evidence = evidence_mut(&mut records[0]);
    evidence.spec.content_role = ContentRole::TrustedControl;
    reseal(&mut records[0]);
    assert!(records[0].validate().is_err());
}

#[test]
fn evidence_locator_state_and_directness_rules_fail_closed() {
    let mut records = fixture_records();
    let evidence = evidence_mut(&mut records[0]);
    evidence.spec.evidence_type = EvidenceType::RepoLocator;
    evidence.spec.locator.locator_type = LocatorType::Repo;
    evidence.spec.locator.locator_ref = "../secret".into();
    evidence.spec.locator.exit_code = None;
    evidence.spec.locator.line_start = Some(1);
    evidence.spec.locator.line_end = Some(2);
    reseal(&mut records[0]);
    assert!(records[0].validate().is_err());

    let mut records = fixture_records();
    let evidence = evidence_mut(&mut records[0]);
    evidence.status.state = EvidenceState::Unavailable;
    evidence.status.reason_codes = vec!["missing-artifact".into()];
    reseal(&mut records[0]);
    assert!(records[0].validate().is_err());
}

#[test]
fn claim_shape_confidence_and_support_rules_fail_closed() {
    let mut records = fixture_records();
    let claim = claim_mut(&mut records[1]);
    claim.spec.object_type = ClaimObjectType::Boolean;
    reseal(&mut records[1]);
    assert!(records[1].validate().is_err());

    let mut records = fixture_records();
    let claim = claim_mut(&mut records[1]);
    claim.spec.claim_type = ClaimType::Inference;
    reseal(&mut records[1]);
    assert!(records[1].validate().is_err());
}

#[test]
fn record_set_rejects_order_missing_references_and_subject_drift() {
    let mut records = fixture_records();
    records.swap(0, 1);
    assert!(validate_record_set(&records).is_err());

    let mut records = fixture_records();
    claim_mut(&mut records[1])
        .spec
        .supporting_evidence_record_ids = vec!["evr-missing".into()];
    reseal(&mut records[1]);
    assert!(validate_record_set(&records).is_err());

    let mut records = fixture_records();
    evidence_mut(&mut records[0]).spec.subjects = vec!["module:other".into()];
    reseal(&mut records[0]);
    assert!(validate_record_set(&records).is_err());
}

#[test]
fn supersession_requires_the_immediate_prior_sequence() {
    let mut records = fixture_records();
    let mut next = records[0].clone();
    let metadata = match &mut next {
        GovernanceRecord::Evidence(evidence) => &mut evidence.metadata,
        GovernanceRecord::Claim(_) => unreachable!(),
    };
    metadata.record_id = "evr-0002".into();
    metadata.sequence = 2;
    metadata.supersedes_record_ids = vec!["evr-0001".into()];
    reseal(&mut next);
    records.insert(1, next);
    validate_record_set(&records).expect("valid sequence two record");

    let metadata = evidence_mut(&mut records[1]).metadata.sequence;
    assert_eq!(metadata, 2);
    evidence_mut(&mut records[1])
        .metadata
        .supersedes_record_ids
        .clear();
    reseal(&mut records[1]);
    assert!(validate_record_set(&records).is_err());
}

#[test]
fn claim_derivation_rejects_self_and_multi_record_cycles() {
    let mut records = fixture_records();
    claim_mut(&mut records[1])
        .spec
        .derived_from_claim_record_ids = vec!["kcr-0001".into()];
    reseal(&mut records[1]);
    assert!(validate_record_set(&records).is_err());

    let mut records = fixture_records();
    let mut other = records[1].clone();
    let other_claim = claim_mut(&mut other);
    other_claim.metadata.aggregate_id = "claim-cycle-b".into();
    other_claim.metadata.record_id = "kcr-0002".into();
    other_claim.spec.derived_from_claim_record_ids = vec!["kcr-0001".into()];
    claim_mut(&mut records[1])
        .spec
        .derived_from_claim_record_ids = vec!["kcr-0002".into()];
    reseal(&mut records[1]);
    reseal(&mut other);
    records.push(other);
    assert!(validate_record_set(&records).is_err());
}

#[test]
fn canonical_limits_reject_one_over_array_string_and_record_set_bounds() {
    let mut records = fixture_records();
    evidence_mut(&mut records[0]).spec.subjects = (0..=256)
        .map(|index| format!("subject:{index:03}"))
        .collect();
    assert!(records[0].canonical_payload_json().is_err());

    let mut records = fixture_records();
    let oversized = format!("{}ab", "界".repeat(5_461));
    assert_eq!(oversized.len(), MAX_STRING_BYTES + 1);
    claim_mut(&mut records[1]).spec.object_value = ClaimObjectValue::String(oversized);
    assert!(records[1].canonical_payload_json().is_err());

    let records = vec![fixture_records()[0].clone(); MAX_ARRAY_ITEMS + 1];
    assert!(validate_record_set(&records).is_err());
}

#[test]
fn canonical_payload_reserves_the_sealed_digest_bytes() {
    let mut record = fixture_records().remove(1);
    record.integrity_mut().canonical_sha256.clear();
    let claim = claim_mut(&mut record);
    claim.spec.supporting_evidence_record_ids = boundary_record_ids('a');
    claim.spec.contradicting_evidence_record_ids = boundary_record_ids('b');
    claim.spec.derived_from_claim_record_ids = boundary_record_ids('c');
    claim.spec.object_value = ClaimObjectValue::String("x".repeat(MAX_STRING_BYTES));
    claim.spec.reasoning.clear();
    let base = record.canonical_record_json().expect("boundary base");
    let target = MAX_RECORD_BYTES - 64 + 1;
    let padding = target.checked_sub(base.len()).expect("base below boundary");
    assert!((1..=MAX_STRING_BYTES).contains(&padding));
    claim_mut(&mut record).spec.reasoning = "r".repeat(padding);
    assert_eq!(
        record.canonical_record_json().expect("blank record").len(),
        target
    );
    assert!(record.canonical_payload_json().is_err());
    assert!(record.expected_sha256().is_err());
}

fn boundary_record_ids(prefix: char) -> Vec<String> {
    (0..MAX_ARRAY_ITEMS)
        .map(|index| format!("{prefix}:{index:03}:{}", "x".repeat(134)))
        .collect()
}

#[test]
fn repository_locator_rejects_windows_drive_paths() {
    for locator in ["C:/Windows/system.ini", "C:system.ini"] {
        let mut record = fixture_records().remove(0);
        let evidence = evidence_mut(&mut record);
        evidence.spec.evidence_type = EvidenceType::RepoLocator;
        evidence.spec.locator.locator_type = LocatorType::Repo;
        evidence.spec.locator.locator_ref = locator.into();
        evidence.spec.locator.exit_code = None;
        reseal(&mut record);
        assert!(record.validate().is_err(), "accepted {locator}");
    }
}
