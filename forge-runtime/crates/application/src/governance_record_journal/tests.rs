use std::sync::Arc;

use serde::Deserialize;

use crate::runtime_domain::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult,
    GOVERNANCE_RECORD_JOURNAL_VERSION, GovernanceRecordAppendDisposition,
    GovernanceRecordAppendReceipt, GovernanceRecordInspection, GovernanceRecordJournalStore,
    GovernanceRecordKind, GovernanceRecordListFilter, GovernanceRecordMetadata,
    GovernanceStructuralHead, HubStoreError,
};
use crate::{
    AppendGovernanceRecordBatchInput, GovernanceRecordJournalService,
    GovernanceRecordJournalServiceError,
};

use crate::runtime_domain::governance_contract::GovernanceRecord;

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

#[derive(Clone)]
struct TestStore {
    append: AppendGovernanceRecordBatchResult,
    inspection: GovernanceRecordInspection,
    head: GovernanceStructuralHead,
}

impl GovernanceRecordJournalStore for TestStore {
    fn append_governance_record_batch(
        &self,
        _request: &AppendGovernanceRecordBatch,
    ) -> Result<AppendGovernanceRecordBatchResult, HubStoreError> {
        Ok(self.append.clone())
    }

    fn inspect_governance_record(
        &self,
        _record_id: &str,
        _include_record: bool,
    ) -> Result<GovernanceRecordInspection, HubStoreError> {
        Ok(self.inspection.clone())
    }

    fn list_governance_records(
        &self,
        _filter: &GovernanceRecordListFilter,
    ) -> Result<Vec<GovernanceRecordInspection>, HubStoreError> {
        Ok(vec![self.inspection.clone()])
    }

    fn inspect_governance_structural_head(
        &self,
        _record_kind: GovernanceRecordKind,
        _aggregate_id: &str,
    ) -> Result<GovernanceStructuralHead, HubStoreError> {
        Ok(self.head.clone())
    }

    fn rebuild_governance_structural_heads(&self) -> Result<usize, HubStoreError> {
        Ok(1)
    }
}

fn exact_set(records: &[GovernanceRecord]) -> String {
    let mut canonical: Vec<_> = records
        .iter()
        .map(|record| {
            (
                record.metadata().record_id.as_str(),
                record.canonical_record_json().expect("canonical record"),
            )
        })
        .collect();
    canonical.sort_by_key(|(record_id, _)| *record_id);
    format!(
        "[{}]",
        canonical
            .into_iter()
            .map(|(_, record)| record)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn fixture_records() -> Vec<GovernanceRecord> {
    serde_json::from_str::<GoldenFixture>(FIXTURE)
        .expect("fixture")
        .records
        .into_iter()
        .map(|entry| entry.record)
        .collect()
}

fn fixture_request(records: &[GovernanceRecord]) -> AppendGovernanceRecordBatch {
    GovernanceRecordJournalService::prepare_append_request(AppendGovernanceRecordBatchInput {
        canonical_record_set_json: exact_set(records),
        idempotency_key: "application-test".into(),
        appended_at_ms: 200,
    })
    .expect("request")
}

fn fixture_receipt(
    request: &AppendGovernanceRecordBatch,
    records: &[GovernanceRecord],
) -> GovernanceRecordAppendReceipt {
    let mut record_ids: Vec<_> = records
        .iter()
        .map(|record| record.metadata().record_id.clone())
        .collect();
    record_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    GovernanceRecordAppendReceipt {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: request.batch_id.clone(),
        request_sha256: request.request_sha256.clone(),
        record_set_sha256: request.record_set_sha256.clone(),
        record_count: record_ids.len(),
        record_ids,
        appended_at_ms: request.appended_at_ms,
    }
}

fn fixture_metadata(
    request: &AppendGovernanceRecordBatch,
    record: &GovernanceRecord,
) -> GovernanceRecordMetadata {
    let canonical = record.canonical_record_json().expect("canonical record");
    GovernanceRecordMetadata {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: request.batch_id.clone(),
        batch_ordinal: 0,
        record_id: record.metadata().record_id.clone(),
        record_kind: GovernanceRecordKind::from(record),
        aggregate_id: record.metadata().aggregate_id.clone(),
        sequence: record.metadata().sequence,
        canonical_sha256: record.integrity().canonical_sha256.clone(),
        canonical_record_bytes: canonical.len(),
        created_at_unix_ms: record.metadata().created_at_unix_ms,
        appended_at_ms: request.appended_at_ms,
    }
}

fn fixture_store(request: &AppendGovernanceRecordBatch, records: &[GovernanceRecord]) -> TestStore {
    let receipt = fixture_receipt(request, records);
    let metadata = fixture_metadata(request, &records[0]);
    let head = GovernanceStructuralHead {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        record_kind: metadata.record_kind,
        aggregate_id: metadata.aggregate_id.clone(),
        record_id: metadata.record_id.clone(),
        sequence: metadata.sequence,
        canonical_sha256: metadata.canonical_sha256.clone(),
        updated_at_ms: request.appended_at_ms,
    };
    TestStore {
        append: AppendGovernanceRecordBatchResult {
            v: GOVERNANCE_RECORD_JOURNAL_VERSION,
            disposition: GovernanceRecordAppendDisposition::Stored,
            receipt,
        },
        inspection: GovernanceRecordInspection {
            v: GOVERNANCE_RECORD_JOURNAL_VERSION,
            metadata,
            canonical_record_json: None,
        },
        head,
    }
}

fn fixture() -> (AppendGovernanceRecordBatch, TestStore) {
    let records = fixture_records();
    let request = fixture_request(&records);
    let store = fixture_store(&request, &records);
    (request, store)
}

#[test]
fn preflight_and_service_validate_every_store_boundary() {
    let (request, store) = fixture();
    GovernanceRecordJournalService::preflight_inspect(&store.inspection.metadata.record_id)
        .expect("inspection preflight");
    GovernanceRecordJournalService::preflight_head(&store.head.aggregate_id)
        .expect("head preflight");
    let service = GovernanceRecordJournalService::new(Arc::new(store.clone()));
    service.append_prepared(&request).expect("append");
    service
        .inspect(&store.inspection.metadata.record_id, false)
        .expect("metadata-only inspection");
    service
        .inspect_structural_head(store.head.record_kind, &store.head.aggregate_id)
        .expect("structural head");
    assert_eq!(service.rebuild_structural_heads().expect("rebuild"), 1);

    let filter = GovernanceRecordListFilter {
        record_kind: Some(store.inspection.metadata.record_kind),
        aggregate_id: Some(store.inspection.metadata.aggregate_id.clone()),
        limit: 1,
        include_record: false,
    };
    GovernanceRecordJournalService::preflight_list(&filter).expect("list preflight");
    assert_eq!(service.list(&filter).expect("list").len(), 1);
}

#[test]
fn read_preflights_reject_invalid_inputs_without_a_store() {
    assert!(matches!(
        GovernanceRecordJournalService::preflight_append_key("unsafe\u{202e}key"),
        Err(GovernanceRecordJournalServiceError::InvalidInput { .. })
    ));
    assert!(matches!(
        GovernanceRecordJournalService::preflight_inspect("invalid id"),
        Err(GovernanceRecordJournalServiceError::InvalidInput { .. })
    ));
    assert!(matches!(
        GovernanceRecordJournalService::preflight_head("C:\\unsafe"),
        Err(GovernanceRecordJournalServiceError::InvalidInput { .. })
    ));
    let filter = GovernanceRecordListFilter {
        record_kind: None,
        aggregate_id: None,
        limit: 0,
        include_record: false,
    };
    assert!(matches!(
        GovernanceRecordJournalService::preflight_list(&filter),
        Err(GovernanceRecordJournalServiceError::InvalidInput { .. })
    ));
}

#[test]
fn inconsistent_receipt_and_implicit_content_are_rejected() {
    let (request, mut store) = fixture();
    store.append.receipt.record_count += 1;
    let service = GovernanceRecordJournalService::new(Arc::new(store.clone()));
    assert!(matches!(
        service.append_prepared(&request),
        Err(GovernanceRecordJournalServiceError::InconsistentStoreResult)
    ));

    store.append.receipt.record_count -= 1;
    let service = GovernanceRecordJournalService::new(Arc::new(store));
    assert!(matches!(
        service.inspect(
            &request.records().expect("records")[0].metadata().record_id,
            true
        ),
        Err(GovernanceRecordJournalServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn exact_replay_keeps_original_time_across_clock_skew() {
    let (mut request, mut store) = fixture();
    request.appended_at_ms = 100;
    request
        .validate()
        .expect("local retry time is not request identity");
    store.append.disposition = GovernanceRecordAppendDisposition::ExactReplay;
    store.append.receipt.appended_at_ms = 200;
    let service = GovernanceRecordJournalService::new(Arc::new(store));
    service
        .append_prepared(&request)
        .expect("replay returns the durable original receipt");
}

#[test]
fn list_validation_requires_the_stable_record_id_tiebreak() {
    let (_, store) = fixture();
    let mut lower = store.inspection.clone();
    lower.metadata.record_id = "record:a".into();
    let mut higher = store.inspection;
    higher.metadata.record_id = "record:z".into();
    let filter = GovernanceRecordListFilter {
        record_kind: None,
        aggregate_id: None,
        limit: 2,
        include_record: false,
    };
    assert!(super::validation::validate_list(&[higher.clone(), lower.clone()], &filter).is_ok());
    assert!(matches!(
        super::validation::validate_list(&[lower, higher], &filter),
        Err(GovernanceRecordJournalServiceError::InconsistentStoreResult)
    ));
}
