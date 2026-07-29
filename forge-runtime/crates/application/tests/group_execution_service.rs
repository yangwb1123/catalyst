use std::sync::{Arc, Mutex};

use forge_runtime_application::{
    GroupExecutionService, HubError, HubField, MAX_ENTITY_ID_BYTES, MAX_GROUP_EXECUTION_LIST_LIMIT,
    MAX_IDEMPOTENCY_KEY_BYTES,
};
use forge_runtime_domain::{
    BeginGroupExecution, BeginGroupExecutionDisposition, BeginGroupExecutionResult,
    GROUP_CONTEXT_VERSION, GROUP_EXECUTION_PROTOCOL_VERSION, GROUP_EXECUTION_VERSION,
    GROUP_RUN_VERSION, GroupContextPayload, GroupContextPolicy, GroupContextSlice,
    GroupContextStats, GroupExecutionEvent, GroupExecutionInspection, GroupExecutionMode,
    GroupExecutionRecord, GroupExecutionStatus, GroupExecutionStore, GroupRunRecord,
    GroupRunSnapshot, GroupRunStatus, HubStoreError, SessionGroup,
};

#[test]
fn start_appends_only_the_missing_deterministic_suffix() {
    for retained in 0..3 {
        let store = Arc::new(MemoryExecutionStore::with_prefix(retained));
        let service = GroupExecutionService::new(store.clone());

        let result = service.start(&request()).expect("offline start");

        assert_eq!(result.v, GROUP_EXECUTION_VERSION);
        assert_eq!(result.inspection.events.len(), 3);
        assert_eq!(
            store.appended_sequences(),
            ((retained + 1)..=3)
                .map(|value| value as u64)
                .collect::<Vec<_>>()
        );
        let receipt = result.inspection.receipt.expect("verified receipt");
        assert_eq!(receipt.stats, context_stats());
        assert_eq!(receipt.snapshot_sha256, "22".repeat(32));
    }
}

#[test]
fn terminal_replay_returns_without_appending() {
    let store = Arc::new(MemoryExecutionStore::with_prefix(3));
    store.set_disposition(BeginGroupExecutionDisposition::Replayed);
    let service = GroupExecutionService::new(store.clone());

    let result = service.start(&request()).expect("terminal replay");

    assert_eq!(result.disposition, BeginGroupExecutionDisposition::Replayed);
    assert!(store.appended_sequences().is_empty());
}

#[test]
fn durable_but_nondeterministic_prefix_is_rejected() {
    let store = Arc::new(MemoryExecutionStore::with_prefix(2));
    store.mutate_receipt(|receipt| receipt.stats.member_count += 1);
    let service = GroupExecutionService::new(store.clone());

    assert!(matches!(
        service.start(&request()),
        Err(HubError::Store(HubStoreError::Corrupt { message }))
            if message.contains("deterministic")
    ));
    assert!(store.appended_sequences().is_empty());
}

#[test]
fn created_begin_cannot_replace_the_candidate_identity_or_time() {
    for mutate in [
        |record: &mut GroupExecutionRecord| record.execution_id = "other".into(),
        |record: &mut GroupExecutionRecord| record.created_at_ms += 1,
        |record: &mut GroupExecutionRecord| record.status = GroupExecutionStatus::Completed,
    ] {
        let store = Arc::new(MemoryExecutionStore::with_prefix(0));
        store.mutate_record(mutate);
        let service = GroupExecutionService::new(store);

        assert!(matches!(
            service.start(&request()),
            Err(HubError::Store(HubStoreError::Corrupt { message }))
                if message.contains("source bindings")
        ));
    }
}

#[test]
fn replay_may_preserve_the_original_identity_and_time() {
    let store = Arc::new(MemoryExecutionStore::with_prefix(3));
    store.set_disposition(BeginGroupExecutionDisposition::Replayed);
    store.mutate_record(|record| {
        record.execution_id = "original-execution".into();
        record.created_at_ms = 4;
    });
    store.rebind_events("original-execution");
    let service = GroupExecutionService::new(store);

    let result = service.start(&request()).expect("candidate fields ignored");

    assert_eq!(
        result.inspection.execution.execution_id,
        "original-execution"
    );
    assert_eq!(result.inspection.execution.created_at_ms, 4);
}

#[test]
fn start_validates_every_request_field_before_storage() {
    let cases = invalid_requests();
    for (candidate, expected) in cases {
        let store = Arc::new(MemoryExecutionStore::with_prefix(0));
        let service = GroupExecutionService::new(store.clone());

        let error = service.start(&candidate).expect_err("invalid request");

        assert!(expected(&error), "unexpected error: {error}");
        assert_eq!(store.begin_calls(), 0);
    }
}

#[test]
fn inspect_and_list_validate_ids_controls_and_bounds() {
    let store = Arc::new(MemoryExecutionStore::with_prefix(0));
    let service = GroupExecutionService::new(store.clone());
    let long = "x".repeat(MAX_ENTITY_ID_BYTES + 1);

    for id in [" ", "bad\nid", "bad\u{202e}id", long.as_str()] {
        assert!(service.inspect(id).is_err());
        assert!(service.list(Some(id), 1).is_err());
    }
    for limit in [0, MAX_GROUP_EXECUTION_LIST_LIMIT + 1] {
        assert!(matches!(
            service.list(None, limit),
            Err(HubError::OutOfRange {
                field: HubField::GroupExecutionLimit,
                min: 1,
                max: MAX_GROUP_EXECUTION_LIST_LIMIT
            })
        ));
    }
    assert_eq!(store.inspect_calls(), 0);
    assert_eq!(store.list_calls(), 0);
}

#[test]
fn inspect_and_list_delegate_after_validation() {
    let store = Arc::new(MemoryExecutionStore::with_prefix(0));
    let service = GroupExecutionService::new(store.clone());

    let inspection = service.inspect("group-execution-1").expect("valid inspect");
    let records = service
        .list(Some("group-run-1"), MAX_GROUP_EXECUTION_LIST_LIMIT)
        .expect("valid list");

    assert_eq!(inspection.events.len(), 0);
    assert_eq!(records, vec![record(GroupExecutionStatus::Incomplete)]);
    assert_eq!(store.inspect_calls(), 1);
    assert_eq!(store.list_calls(), 1);
}

type ErrorPredicate = fn(&HubError) -> bool;

fn invalid_requests() -> Vec<(BeginGroupExecution, ErrorPredicate)> {
    let mut cases = Vec::new();
    push_case(
        &mut cases,
        |request| request.v += 1,
        |error| matches!(error, HubError::UnsupportedGroupExecutionVersion { .. }),
    );
    push_case(
        &mut cases,
        |request| request.execution_id = " ".into(),
        |error| {
            matches!(
                error,
                HubError::Empty {
                    field: HubField::GroupExecutionId
                }
            )
        },
    );
    push_case(
        &mut cases,
        |request| request.group_run_id = "bad\n".into(),
        |error| {
            matches!(
                error,
                HubError::InvalidCharacters {
                    field: HubField::GroupRunId
                }
            )
        },
    );
    push_case(
        &mut cases,
        |request| request.idempotency_key = "k".repeat(MAX_IDEMPOTENCY_KEY_BYTES + 1),
        |error| {
            matches!(
                error,
                HubError::TooLong {
                    field: HubField::IdempotencyKey,
                    ..
                }
            )
        },
    );
    push_invalid_time_case(&mut cases);
    cases
}

fn push_invalid_time_case(cases: &mut Vec<(BeginGroupExecution, ErrorPredicate)>) {
    push_case(
        cases,
        |request| request.created_at_ms = u64::MAX,
        |error| matches!(error, HubError::GroupExecutionCreationTimeOutOfRange),
    );
}

fn push_case(
    cases: &mut Vec<(BeginGroupExecution, ErrorPredicate)>,
    mutate: impl FnOnce(&mut BeginGroupExecution),
    predicate: ErrorPredicate,
) {
    let mut candidate = request();
    mutate(&mut candidate);
    cases.push((candidate, predicate));
}

struct MemoryExecutionStore {
    record: Mutex<GroupExecutionRecord>,
    snapshot: GroupRunSnapshot,
    events: Mutex<Vec<GroupExecutionEvent>>,
    appended: Mutex<Vec<u64>>,
    disposition: Mutex<BeginGroupExecutionDisposition>,
    calls: Mutex<(usize, usize, usize)>,
}

impl MemoryExecutionStore {
    fn with_prefix(length: usize) -> Self {
        let store = Self {
            record: Mutex::new(record(GroupExecutionStatus::Incomplete)),
            snapshot: snapshot(),
            events: Mutex::new(Vec::new()),
            appended: Mutex::new(Vec::new()),
            disposition: Mutex::new(BeginGroupExecutionDisposition::Created),
            calls: Mutex::new((0, 0, 0)),
        };
        let expected = expected_events();
        store
            .events
            .lock()
            .expect("events")
            .extend_from_slice(&expected[..length]);
        if length == 3 {
            store.record.lock().expect("record").status = GroupExecutionStatus::Completed;
        }
        store
    }

    fn set_disposition(&self, value: BeginGroupExecutionDisposition) {
        *self.disposition.lock().expect("disposition") = value;
    }

    fn mutate_record(&self, mutate: impl FnOnce(&mut GroupExecutionRecord)) {
        mutate(&mut self.record.lock().expect("record"));
    }

    fn mutate_receipt(
        &self,
        mutate: impl FnOnce(&mut forge_runtime_domain::GroupExecutionReceipt),
    ) {
        let mut events = self.events.lock().expect("events");
        let forge_runtime_domain::GroupExecutionEventKind::SnapshotVerified { receipt } =
            &mut events[1].kind
        else {
            panic!("second event must be receipt")
        };
        mutate(receipt);
    }

    fn rebind_events(&self, execution_id: &str) {
        let mut events = self.events.lock().expect("events");
        for event in events.iter_mut() {
            event.execution_id = execution_id.into();
            if let forge_runtime_domain::GroupExecutionEventKind::SnapshotVerified { receipt } =
                &mut event.kind
            {
                receipt.execution_id = execution_id.into();
            }
        }
    }

    fn appended_sequences(&self) -> Vec<u64> {
        self.appended.lock().expect("appended").clone()
    }

    fn begin_calls(&self) -> usize {
        self.calls.lock().expect("calls").0
    }

    fn inspect_calls(&self) -> usize {
        self.calls.lock().expect("calls").1
    }

    fn list_calls(&self) -> usize {
        self.calls.lock().expect("calls").2
    }
}

impl GroupExecutionStore for MemoryExecutionStore {
    fn begin_group_execution(
        &self,
        _request: &BeginGroupExecution,
    ) -> Result<BeginGroupExecutionResult, HubStoreError> {
        self.calls.lock().expect("calls").0 += 1;
        Ok(BeginGroupExecutionResult {
            v: GROUP_EXECUTION_VERSION,
            disposition: *self.disposition.lock().expect("disposition"),
            execution: self.record.lock().expect("record").clone(),
            snapshot: self.snapshot.clone(),
        })
    }

    fn append_group_execution_event(
        &self,
        event: &GroupExecutionEvent,
    ) -> Result<(), HubStoreError> {
        let mut events = self.events.lock().expect("events");
        events.push(event.clone());
        self.appended.lock().expect("appended").push(event.seq);
        if events.len() == 3 {
            self.record.lock().expect("record").status = GroupExecutionStatus::Completed;
        }
        Ok(())
    }

    fn inspect_group_execution(
        &self,
        _execution_id: &str,
    ) -> Result<GroupExecutionInspection, HubStoreError> {
        self.calls.lock().expect("calls").1 += 1;
        GroupExecutionInspection::validate(
            self.record.lock().expect("record").clone(),
            self.events.lock().expect("events").clone(),
        )
        .map_err(|error| HubStoreError::Corrupt {
            message: error.message,
        })
    }

    fn list_group_executions(
        &self,
        _group_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupExecutionRecord>, HubStoreError> {
        self.calls.lock().expect("calls").2 += 1;
        Ok(vec![self.record.lock().expect("record").clone()])
    }
}

fn request() -> BeginGroupExecution {
    BeginGroupExecution {
        v: GROUP_EXECUTION_VERSION,
        execution_id: "group-execution-1".into(),
        group_run_id: "group-run-1".into(),
        mode: GroupExecutionMode::OfflineSnapshotValidation,
        idempotency_key: "execution-key".into(),
        created_at_ms: 10,
    }
}

fn record(status: GroupExecutionStatus) -> GroupExecutionRecord {
    GroupExecutionRecord {
        v: GROUP_EXECUTION_VERSION,
        execution_id: "group-execution-1".into(),
        group_run_id: "group-run-1".into(),
        mode: GroupExecutionMode::OfflineSnapshotValidation,
        status,
        source_snapshot_sha256: "22".repeat(32),
        protocol_version: GROUP_EXECUTION_PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn snapshot() -> GroupRunSnapshot {
    GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: GroupRunRecord {
            v: GROUP_RUN_VERSION,
            run_id: "group-run-1".into(),
            group_id: "group-1".into(),
            status: GroupRunStatus::Prepared,
            context_version: GROUP_CONTEXT_VERSION,
            context_slice_sha256: "11".repeat(32),
            snapshot_sha256: "22".repeat(32),
            snapshot_bytes: 100,
            created_at_ms: 1,
        },
        context: GroupContextSlice {
            v: GROUP_CONTEXT_VERSION,
            payload: GroupContextPayload {
                policy: GroupContextPolicy::default(),
                group: SessionGroup {
                    id: "group-1".into(),
                    name: "Delivery".into(),
                    created_at_ms: 1,
                },
                members: Vec::new(),
                conversations: Vec::new(),
                stats: context_stats(),
            },
            slice_sha256: "11".repeat(32),
        },
        context_json: r#"{"frozen":"context"}"#.into(),
    }
}

fn context_stats() -> GroupContextStats {
    GroupContextStats {
        content_bytes: 12,
        ..GroupContextStats::default()
    }
}

fn expected_events() -> Vec<GroupExecutionEvent> {
    let store = Arc::new(MemoryExecutionStore {
        record: Mutex::new(record(GroupExecutionStatus::Incomplete)),
        snapshot: snapshot(),
        events: Mutex::new(Vec::new()),
        appended: Mutex::new(Vec::new()),
        disposition: Mutex::new(BeginGroupExecutionDisposition::Created),
        calls: Mutex::new((0, 0, 0)),
    });
    let service = GroupExecutionService::new(store.clone());
    service.start(&request()).expect("derive events");
    store.events.lock().expect("events").clone()
}
