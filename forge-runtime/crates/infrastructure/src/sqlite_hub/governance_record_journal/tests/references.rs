use rusqlite::params;

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    AppendGovernanceRecordBatch, GovernanceRecordJournalStore, GovernanceRecordKind, HubStoreError,
    MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH,
};

use super::fixtures::{
    StoreFixture, claim_successor, claim_variant, evidence_successor, golden_records, request,
    reseal, rewrite_record_batch, row_count,
};

#[test]
fn cross_batch_evidence_and_recursive_claim_derivations_resolve() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "evidence", 100);
    append(&fixture, &records[1..], "claim-one", 200);
    let second = claim_variant(
        &records[1],
        "kcr-0002",
        "claim-derived-two",
        vec!["kcr-0001".into()],
    );
    append(&fixture, &[second], "claim-two", 300);
    let third = claim_variant(
        &records[1],
        "kcr-0003",
        "claim-derived-three",
        vec!["kcr-0002".into()],
    );
    append(&fixture, &[third], "claim-three", 400);
    assert_eq!(row_count(&fixture.connection(), "governance_records"), 4);
}

#[test]
fn wrong_kind_wrong_subject_and_derivation_cycle_are_atomic_conflicts() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "base-evidence", 100);

    let mut wrong_kind = claim_variant(
        &records[1],
        "kcr-wrong-kind",
        "claim-wrong-kind",
        Vec::new(),
    );
    claim_mut(&mut wrong_kind)
        .spec
        .supporting_evidence_record_ids = vec!["kcr-wrong-kind".into()];
    reseal(&mut wrong_kind);
    assert_conflict(&fixture, &[wrong_kind], "wrong-kind");

    let other_evidence = other_subject_evidence(&records[0]);
    append(&fixture, &[other_evidence], "other-evidence", 200);
    let mut wrong_subject = claim_variant(
        &records[1],
        "kcr-wrong-subject",
        "claim-wrong-subject",
        Vec::new(),
    );
    claim_mut(&mut wrong_subject)
        .spec
        .supporting_evidence_record_ids = vec!["evr-other".into()];
    reseal(&mut wrong_subject);
    assert_conflict(&fixture, &[wrong_subject], "wrong-subject");

    let cycle = cyclic_claims(&records[1]);
    assert_conflict(&fixture, &cycle, "claim-cycle");
    assert_eq!(row_count(&fixture.connection(), "governance_records"), 2);
}

#[test]
fn sequence_gap_conflicts_and_stale_head_is_corruption() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "sequence-one", 100);
    let gap = evidence_successor(&records[0], "evr-0003", 3);
    assert_conflict(&fixture, &[gap], "sequence-gap");

    let second = evidence_successor(&records[0], "evr-0002", 2);
    append(&fixture, std::slice::from_ref(&second), "sequence-two", 200);
    make_head_stale(&fixture);
    let third = evidence_successor(&second, "evr-0003", 3);
    let error = fixture
        .store
        .append_governance_record_batch(&request(&[third], "stale-head", 300))
        .expect_err("stale head fails closed");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
}

#[test]
fn supersession_and_replay_validate_current_head_derivations_atomically() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    let (first, second) = current_chain_with_missing_ancestor(&fixture, &records);
    assert_corrupt_request_without_writes(&fixture, &request(&[first], "closure-first", 999));
    let third = claim_successor(&second, "kcr-chain-0003", Vec::new());
    assert_corrupt_request_without_writes(&fixture, &request(&[third], "closure-third", 500));
}

fn current_chain_with_missing_ancestor(
    fixture: &StoreFixture,
    records: &[GovernanceRecord],
) -> (GovernanceRecord, GovernanceRecord) {
    append(fixture, &records[..1], "closure-evidence", 100);
    let ancestor = claim_variant(&records[1], "kcr-ancestor", "claim-ancestor", Vec::new());
    append(
        fixture,
        std::slice::from_ref(&ancestor),
        "closure-ancestor",
        150,
    );
    let dependency = claim_variant(
        &records[1],
        "kcr-dependency",
        "claim-dependency",
        vec![ancestor.metadata().record_id.clone()],
    );
    append(
        fixture,
        std::slice::from_ref(&dependency),
        "closure-dependency",
        200,
    );
    let first = claim_variant(&records[1], "kcr-chain-0001", "claim-chain", Vec::new());
    append(fixture, std::slice::from_ref(&first), "closure-first", 300);
    let second = claim_successor(
        &first,
        "kcr-chain-0002",
        vec![dependency.metadata().record_id.clone()],
    );
    append(
        fixture,
        std::slice::from_ref(&second),
        "closure-second",
        400,
    );
    fixture
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DELETE FROM governance_claim_semantic_views WHERE aggregate_id='claim-ancestor';
             DELETE FROM governance_semantic_heads WHERE aggregate_id='claim-ancestor';
             DELETE FROM governance_structural_heads WHERE aggregate_id='claim-ancestor';
             DELETE FROM governance_records WHERE record_id='kcr-ancestor';",
        )
        .expect("remove referenced ancestor from every identity projection");
    (first, second)
}

#[test]
fn derived_reference_upgrades_a_prior_supersession_load() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "upgrade-evidence", 100);
    let root = claim_variant(
        &records[1],
        "kcr-upgrade-root",
        "claim-upgrade-root",
        Vec::new(),
    );
    append(&fixture, std::slice::from_ref(&root), "upgrade-root", 200);
    let prior = claim_variant(
        &records[1],
        "kcr-upgrade-prior",
        "claim-upgrade-chain",
        vec![root.metadata().record_id.clone()],
    );
    append(&fixture, std::slice::from_ref(&prior), "upgrade-prior", 300);
    let successor = claim_successor(
        &prior,
        "kcr-upgrade-successor",
        vec![prior.metadata().record_id.clone()],
    );
    append(&fixture, &[successor], "upgrade-successor", 400);
}

#[test]
fn stored_shared_subgraph_accepts_256_edges_and_rejects_257() {
    let accepted = StoreFixture::new();
    let records = golden_records();
    append(&accepted, &records[..1], "depth-evidence", 100);
    let chain = claim_chain(&records[1]);
    append(&accepted, &chain, "depth-chain", 200);
    let valid = shared_depth_candidates(&records[1], &chain[0], &chain[0]);
    append(&accepted, &valid, "depth-valid", 300);

    let rejected = StoreFixture::new();
    append(&rejected, &records[..1], "depth-evidence", 100);
    append(&rejected, &chain, "depth-chain", 200);
    let bridge = claim_variant(
        &records[1],
        "kcr-depth-bridge",
        "claim-depth-bridge",
        vec![chain[0].metadata().record_id.clone()],
    );
    append(
        &rejected,
        std::slice::from_ref(&bridge),
        "depth-bridge",
        250,
    );
    let invalid = shared_depth_candidates(&records[1], &chain[0], &bridge);
    assert_conflict(&rejected, &invalid, "depth-invalid");
    assert_eq!(row_count(&rejected.connection(), "governance_records"), 258);
}

#[test]
fn replay_and_rebuild_classify_stored_257_edge_graph_as_corrupt() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "depth-evidence", 100);
    let extra = claim_variant(
        &records[1],
        "kcr-depth-extra",
        "claim-depth-extra",
        Vec::new(),
    );
    append(&fixture, std::slice::from_ref(&extra), "depth-extra", 150);
    let chain = claim_chain(&records[1]);
    append(&fixture, &chain, "depth-chain", 200);
    let candidates = shared_depth_candidates(&records[1], &chain[0], &chain[0]);
    append(&fixture, &candidates, "depth-replay", 300);
    extend_stored_chain_batch(&fixture, &chain, &extra);

    let replay = fixture
        .store
        .append_governance_record_batch(&request(&candidates, "depth-replay", 999))
        .expect_err("stored over-depth replay is corruption");
    assert!(
        matches!(replay, HubStoreError::Corrupt { .. }),
        "{replay:?}"
    );
    let rebuild = fixture
        .store
        .rebuild_governance_structural_heads()
        .expect_err("stored over-depth rebuild is corruption");
    assert!(
        matches!(rebuild, HubStoreError::Corrupt { .. }),
        "{rebuild:?}"
    );
}

#[test]
fn dependency_owning_batch_corruption_blocks_append_without_writes() {
    assert_dependency_batch_column_corruption("request_sha256=zeroblob(32)");
    assert_dependency_batch_column_corruption("record_set_sha256=zeroblob(32)");

    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records, "dependency-sibling", 100);
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET canonical_record_blob=zeroblob(canonical_record_bytes)
             WHERE record_id='kcr-0001'",
            [],
        )
        .expect("corrupt dependency batch sibling");
    assert_corrupt_append_without_writes(&fixture, &records[1], "dependency-sibling-new");
}

#[test]
fn record_collision_preserves_stored_batch_corruption_precedence() {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "collision-evidence", 100);
    append(&fixture, &records[1..], "collision-claim", 200);
    corrupt_claim_derivation(&fixture, &records[1], "kcr-missing-collision");
    let before_records = row_count(&fixture.connection(), "governance_records");
    let before_batches = row_count(&fixture.connection(), "governance_record_append_batches");
    let error = fixture
        .store
        .append_governance_record_batch(&request(&records[1..], "collision-new-key", 300))
        .expect_err("collision revalidates the stored owning batch first");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(
        row_count(&fixture.connection(), "governance_records"),
        before_records
    );
    assert_eq!(
        row_count(&fixture.connection(), "governance_record_append_batches"),
        before_batches
    );
}

fn claim_chain(template: &GovernanceRecord) -> Vec<GovernanceRecord> {
    let ids: Vec<_> = (0..MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH)
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

fn shared_depth_candidates(
    template: &GovernanceRecord,
    first_target: &GovernanceRecord,
    later_target: &GovernanceRecord,
) -> Vec<GovernanceRecord> {
    vec![
        claim_variant(
            template,
            "kcr-depth-a-first",
            "claim-depth-a-first",
            vec![first_target.metadata().record_id.clone()],
        ),
        claim_variant(
            template,
            "kcr-depth-z-later",
            "claim-depth-z-later",
            vec![later_target.metadata().record_id.clone()],
        ),
    ]
}

fn extend_stored_chain_batch(
    fixture: &StoreFixture,
    chain: &[GovernanceRecord],
    extra: &GovernanceRecord,
) {
    let mut extended = chain.to_vec();
    let leaf = {
        let leaf = extended.last_mut().expect("depth chain leaf");
        claim_mut(leaf).spec.derived_from_claim_record_ids =
            vec![extra.metadata().record_id.clone()];
        reseal(leaf);
        leaf.clone()
    };
    let prior = request(chain, "depth-chain", 200);
    let replacement = request(&extended, "depth-chain", 200);
    rewrite_record_batch(fixture, &leaf, &prior, &replacement);
}

fn assert_dependency_batch_column_corruption(assignment: &str) {
    let fixture = StoreFixture::new();
    let records = golden_records();
    append(&fixture, &records[..1], "dependency-digest", 100);
    fixture
        .connection()
        .execute(
            &format!("UPDATE governance_record_append_batches SET {assignment}"),
            [],
        )
        .expect("corrupt dependency batch digest");
    assert_corrupt_append_without_writes(&fixture, &records[1], "dependency-digest-new");
}

fn assert_corrupt_append_without_writes(
    fixture: &StoreFixture,
    template: &GovernanceRecord,
    key: &str,
) {
    let candidate = claim_variant(
        template,
        "kcr-dependency-new",
        "claim-dependency-new",
        Vec::new(),
    );
    assert_corrupt_request_without_writes(fixture, &request(&[candidate], key, 500));
}

fn assert_corrupt_request_without_writes(
    fixture: &StoreFixture,
    append_request: &AppendGovernanceRecordBatch,
) {
    let before_records = row_count(&fixture.connection(), "governance_records");
    let before_batches = row_count(&fixture.connection(), "governance_record_append_batches");
    let error = fixture
        .store
        .append_governance_record_batch(append_request)
        .expect_err("corrupt dependency batch blocks append");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(
        row_count(&fixture.connection(), "governance_records"),
        before_records
    );
    assert_eq!(
        row_count(&fixture.connection(), "governance_record_append_batches"),
        before_batches
    );
}

fn append(fixture: &StoreFixture, records: &[GovernanceRecord], key: &str, time: u64) {
    fixture
        .store
        .append_governance_record_batch(&request(records, key, time))
        .expect("append reference fixture");
}

fn assert_conflict(fixture: &StoreFixture, records: &[GovernanceRecord], key: &str) {
    let error = fixture
        .store
        .append_governance_record_batch(&request(records, key, 500))
        .expect_err("invalid relation conflicts");
    assert!(matches!(error, HubStoreError::Conflict { .. }), "{error:?}");
}

fn other_subject_evidence(template: &GovernanceRecord) -> GovernanceRecord {
    let mut record = template.clone();
    let GovernanceRecord::Evidence(evidence) = &mut record else {
        panic!("expected evidence");
    };
    evidence.metadata.record_id = "evr-other".into();
    evidence.metadata.aggregate_id = "evidence-other".into();
    evidence.spec.subjects = vec!["module:other".into()];
    reseal(&mut record);
    record
}

fn cyclic_claims(template: &GovernanceRecord) -> Vec<GovernanceRecord> {
    vec![
        claim_variant(
            template,
            "kcr-cycle-a",
            "claim-cycle-a",
            vec!["kcr-cycle-b".into()],
        ),
        claim_variant(
            template,
            "kcr-cycle-b",
            "claim-cycle-b",
            vec!["kcr-cycle-a".into()],
        ),
    ]
}

fn claim_mut(
    record: &mut GovernanceRecord,
) -> &mut crate::runtime_domain::governance_contract::KnowledgeClaim {
    let GovernanceRecord::Claim(claim) = record else {
        panic!("expected claim");
    };
    claim
}

fn make_head_stale(fixture: &StoreFixture) {
    fixture
        .connection()
        .execute(
            "UPDATE governance_structural_heads SET
               record_id='evr-0001',sequence=1,
               canonical_sha256=(SELECT canonical_sha256 FROM governance_records WHERE record_id='evr-0001'),
               updated_at_ms=100
             WHERE record_kind=?1 AND aggregate_id='evidence-check-pass'",
            [GovernanceRecordKind::EvidenceRecord.as_str()],
        )
        .expect("make head stale");
}

fn corrupt_claim_derivation(fixture: &StoreFixture, template: &GovernanceRecord, missing_id: &str) {
    let mut corrupted = template.clone();
    claim_mut(&mut corrupted).spec.derived_from_claim_record_ids = vec![missing_id.into()];
    reseal(&mut corrupted);
    let canonical = corrupted
        .canonical_record_json()
        .expect("corrupt dependency canonical");
    let digest = super::super::super::group_run_codec::decode_hex_digest(
        &corrupted.integrity().canonical_sha256,
    )
    .expect("corrupt dependency digest");
    fixture
        .connection()
        .execute(
            "UPDATE governance_records SET canonical_record_blob=?1,canonical_record_bytes=?2,
             canonical_sha256=?3 WHERE record_id=?4",
            params![
                canonical.as_bytes(),
                i64::try_from(canonical.len()).expect("canonical length fits SQLite"),
                digest.as_slice(),
                template.metadata().record_id
            ],
        )
        .expect("install dangling stored derivation");
}
