use serde::Deserialize;

use crate::governance_contract::{GovernanceRecord, decode_canonical_record_set};

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/governance-evidence-claim-v1.json");

#[derive(Deserialize)]
struct GoldenFixture {
    records: Vec<GoldenEntry>,
}

#[derive(Deserialize)]
struct GoldenEntry {
    record: GovernanceRecord,
}

fn fixture_records() -> Vec<GovernanceRecord> {
    serde_json::from_str::<GoldenFixture>(FIXTURE)
        .expect("governance fixture")
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

fn exact_set(records: &[GovernanceRecord]) -> String {
    let mut ordered: Vec<_> = records
        .iter()
        .map(|record| (record.metadata().record_id.as_str(), record))
        .collect();
    ordered.sort_by_key(|(record_id, _)| *record_id);
    let canonical: Vec<_> = ordered
        .into_iter()
        .map(|(_, record)| record.canonical_record_json().expect("canonical record"))
        .collect();
    format!("[{}]", canonical.join(","))
}

fn reseal(record: &mut GovernanceRecord) {
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

fn request(records: &[GovernanceRecord], key: &str, time: u64) -> AppendGovernanceRecordBatch {
    AppendGovernanceRecordBatch::from_canonical_record_set(exact_set(records), key.to_owned(), time)
        .expect("append request")
}

fn head(record: &GovernanceRecord, time: u64) -> GovernanceStructuralHead {
    GovernanceStructuralHead {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        record_kind: GovernanceRecordKind::from(record),
        aggregate_id: record.metadata().aggregate_id.clone(),
        record_id: record.metadata().record_id.clone(),
        sequence: record.metadata().sequence,
        canonical_sha256: record.integrity().canonical_sha256.clone(),
        updated_at_ms: time,
    }
}

#[test]
fn request_identity_is_exact_and_excludes_local_append_time() {
    let records = fixture_records();
    let first = request(&records, "journal-replay", 100);
    let retry = request(&records, "journal-replay", 200);
    assert_eq!(first.batch_id, retry.batch_id);
    assert_eq!(first.request_sha256, retry.request_sha256);
    assert_eq!(first.record_set_sha256, retry.record_set_sha256);
    assert_ne!(first.appended_at_ms, retry.appended_at_ms);
    assert_eq!(
        first.record_set_sha256,
        "d895441903bf26d4f68402e7f85a377f1d59127941e20ab2f411756d6d8c9650"
    );
    assert_eq!(
        first.request_sha256,
        "0fe2b4e4d6e58a4f9322256094365716016673a4710d86a5c0c6572bb9f7e00e"
    );
    assert_eq!(
        first.batch_id,
        "governance-record-batch-0fe2b4e4d6e58a4f9322256094365716016673a4710d86a5c0c6572bb9f7e00e"
    );

    let other_key = request(&records, "journal-other-key", 100);
    assert_ne!(first.batch_id, other_key.batch_id);
    assert_ne!(first.request_sha256, other_key.request_sha256);
    assert_eq!(first.record_set_sha256, other_key.record_set_sha256);

    request(&records, "unix-epoch", 0)
        .validate()
        .expect("Unix epoch is a valid append timestamp");
}

#[test]
fn append_batch_decoder_preserves_closed_record_set_semantics() {
    let records = fixture_records();
    let claim_only = exact_set(&records[1..]);
    AppendGovernanceRecordBatch::from_canonical_record_set(
        claim_only.clone(),
        "cross-batch-reference".into(),
        100,
    )
    .expect("journal batch may resolve the evidence later");
    assert!(decode_canonical_record_set(claim_only.as_bytes()).is_err());
}

#[test]
fn append_resolves_existing_evidence_without_claiming_authority() {
    let records = fixture_records();
    let append = request(&records[1..], "claim-batch", 200);
    let next = validate_governance_record_append(&append, &records[..1], &[])
        .expect("resolved claim append");
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].record_id, records[1].metadata().record_id);
    assert_eq!(next[0].sequence, 1);
    assert_eq!(next[0].updated_at_ms, 200);
}

#[test]
fn structural_head_requires_exact_contiguous_predecessor() {
    let records = fixture_records();
    let prior = records[0].clone();
    let mut successor = prior.clone();
    let GovernanceRecord::Evidence(evidence) = &mut successor else {
        panic!("expected evidence");
    };
    evidence.metadata.record_id = "evr-0002".into();
    evidence.metadata.sequence = 2;
    evidence.metadata.supersedes_record_ids = vec![prior.metadata().record_id.clone()];
    reseal(&mut successor);

    let append = request(&[successor.clone()], "evidence-successor", 300);
    let next = validate_governance_record_append(
        &append,
        std::slice::from_ref(&prior),
        &[head(&prior, 100)],
    )
    .expect("contiguous structural append");
    assert_eq!(next[0].record_id, successor.metadata().record_id);
    assert_eq!(next[0].sequence, 2);

    assert!(validate_governance_record_append(&append, &[], &[]).is_err());
    let mut wrong = head(&prior, 100);
    wrong.sequence = 2;
    assert!(validate_governance_record_append(&append, &[prior], &[wrong]).is_err());
}

#[test]
fn revealed_record_must_match_minimized_metadata() {
    let record = fixture_records().remove(0);
    let canonical = record.canonical_record_json().expect("canonical record");
    let metadata = GovernanceRecordMetadata {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: format!("{GOVERNANCE_RECORD_BATCH_ID_PREFIX}{}", "a".repeat(64)),
        batch_ordinal: 0,
        record_id: record.metadata().record_id.clone(),
        record_kind: GovernanceRecordKind::from(&record),
        aggregate_id: record.metadata().aggregate_id.clone(),
        sequence: record.metadata().sequence,
        canonical_sha256: record.integrity().canonical_sha256.clone(),
        canonical_record_bytes: canonical.len(),
        created_at_unix_ms: record.metadata().created_at_unix_ms,
        appended_at_ms: 100,
    };
    let inspection = GovernanceRecordInspection {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        metadata,
        canonical_record_json: Some(canonical),
    };
    inspection.validate().expect("exact reveal");

    let mut drifted = inspection;
    drifted.metadata.sequence += 1;
    assert!(drifted.validate().is_err());
}

#[test]
fn list_filter_and_receipt_bounds_fail_closed() {
    let filter = GovernanceRecordListFilter {
        record_kind: None,
        aggregate_id: None,
        limit: MAX_GOVERNANCE_RECORD_LIST_LIMIT,
        include_record: false,
    };
    filter.validate().expect("bounded filter");
    let mut invalid = filter;
    invalid.limit += 1;
    assert!(invalid.validate().is_err());
    assert!(!is_governance_record_identifier("Uppercase"));
    assert!(!is_governance_record_identifier(
        &"a".repeat(MAX_GOVERNANCE_RECORD_IDENTIFIER_BYTES + 1)
    ));

    let append = request(&fixture_records(), "receipt", 100);
    let records = append.records().expect("request records");
    let receipt = GovernanceRecordAppendReceipt {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: append.batch_id,
        request_sha256: append.request_sha256,
        record_set_sha256: append.record_set_sha256,
        record_count: records.len(),
        record_ids: records
            .iter()
            .map(|record| record.metadata().record_id.clone())
            .collect(),
        appended_at_ms: append.appended_at_ms,
    };
    receipt.validate().expect("valid receipt");

    let mut divergent = receipt;
    divergent.batch_id = format!("{GOVERNANCE_RECORD_BATCH_ID_PREFIX}{}", "f".repeat(64));
    assert!(divergent.validate().is_err());

    assert!(
        AppendGovernanceRecordBatch::from_canonical_record_set(
            exact_set(&fixture_records()),
            "unsafe\u{2028}key".into(),
            100,
        )
        .is_err()
    );
}

#[test]
fn shared_derivation_subgraph_accepts_every_candidate_at_256_edges() {
    let (candidates, dependencies) = shared_derivation_graph(false);
    validate_governance_record_relations(
        &request(&candidates, "shared-depth-256", 100),
        &dependencies,
    )
    .expect("every candidate path is within the inclusive depth limit");
}

#[test]
fn shared_derivation_subgraph_rejects_a_later_257_edge_candidate() {
    let (candidates, dependencies) = shared_derivation_graph(true);
    let error = validate_governance_record_relations(
        &request(&candidates, "shared-depth-257", 100),
        &dependencies,
    )
    .expect_err("the longer path through a memoized shared subgraph is rejected");
    assert!(error.message.contains("depth limit"), "{error:?}");
}

fn shared_derivation_graph(
    extend_later_candidate: bool,
) -> (Vec<GovernanceRecord>, Vec<GovernanceRecord>) {
    let fixtures = fixture_records();
    let mut dependencies = vec![fixtures[0].clone()];
    let chain = claim_chain(&fixtures[1], MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH);
    let shared_id = chain[0].metadata().record_id.clone();
    dependencies.extend(chain);
    let later_target = if extend_later_candidate {
        let bridge = claim_variant(
            &fixtures[1],
            "kcr-depth-bridge",
            "claim-depth-bridge",
            vec![shared_id.clone()],
        );
        let target = bridge.metadata().record_id.clone();
        dependencies.push(bridge);
        target
    } else {
        shared_id.clone()
    };
    let first = claim_variant(
        &fixtures[1],
        "kcr-depth-a-first",
        "claim-depth-a-first",
        vec![shared_id],
    );
    let later = claim_variant(
        &fixtures[1],
        "kcr-depth-z-later",
        "claim-depth-z-later",
        vec![later_target],
    );
    (vec![first, later], dependencies)
}

fn claim_chain(template: &GovernanceRecord, count: usize) -> Vec<GovernanceRecord> {
    let ids: Vec<_> = (0..count)
        .map(|index| format!("kcr-depth-chain-{index:04}"))
        .collect();
    ids.iter()
        .enumerate()
        .map(|(index, record_id)| {
            let derived = ids.get(index + 1).cloned().into_iter().collect();
            claim_variant(
                template,
                record_id,
                &format!("claim-depth-chain-{index:04}"),
                derived,
            )
        })
        .collect()
}

fn claim_variant(
    template: &GovernanceRecord,
    record_id: &str,
    aggregate_id: &str,
    derived_from: Vec<String>,
) -> GovernanceRecord {
    let mut record = template.clone();
    let GovernanceRecord::Claim(claim) = &mut record else {
        panic!("expected claim fixture");
    };
    claim.metadata.record_id = record_id.into();
    claim.metadata.aggregate_id = aggregate_id.into();
    claim.spec.derived_from_claim_record_ids = derived_from;
    reseal(&mut record);
    record
}
