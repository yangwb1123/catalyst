use std::sync::{Arc, Mutex};

use forge_runtime_application::{
    GROUP_RUN_VERSION, GroupRunService, HubError, HubField, MAX_ENTITY_ID_BYTES,
    MAX_GROUP_RUN_LIST_LIMIT, MAX_IDEMPOTENCY_KEY_BYTES,
};
use forge_runtime_domain::{
    GROUP_CONTEXT_VERSION, GroupContextPayload, GroupContextPolicy, GroupContextSlice,
    GroupContextStats, GroupRunRecord, GroupRunSnapshot, GroupRunStatus, GroupRunStore, HubEntity,
    HubStoreError, MAX_GROUP_CONTEXT_CONTENT_BYTES, MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS,
    MAX_GROUP_CONTEXT_MEMBERS, MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS,
    MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES, MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION,
    PrepareGroupRun, PrepareGroupRunDisposition, PrepareGroupRunResult, SessionGroup,
};

#[test]
fn supported_operations_delegate_without_rewriting_inputs() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());
    let request = prepare_request();

    assert_eq!(
        service.prepare(&request).expect("prepare"),
        prepare_result()
    );
    assert_eq!(service.inspect("group-run-1").expect("inspect"), snapshot());
    assert_eq!(
        service.list(Some("group-1"), 7).expect("list"),
        vec![record()]
    );
    assert_eq!(
        store.calls(),
        vec![
            Call::Prepare(Box::new(request)),
            Call::Inspect("group-run-1".into()),
            Call::List(Some("group-1".into()), 7),
        ]
    );
}

#[test]
fn prepare_rejects_unsupported_version_without_calling_storage() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());
    let mut request = prepare_request();
    request.v = GROUP_RUN_VERSION + 1;

    assert!(matches!(
        service.prepare(&request),
        Err(HubError::UnsupportedGroupRunVersion {
            actual,
            expected: GROUP_RUN_VERSION
        }) if actual == GROUP_RUN_VERSION + 1
    ));
    assert!(store.calls().is_empty());
}

#[test]
fn prepare_rejects_invalid_identifiers_without_calling_storage() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());

    for (request, field, too_long) in invalid_identifier_requests() {
        let result = service.prepare(&request);
        assert_invalid_text(&result, field, too_long);
    }
    assert!(store.calls().is_empty());
}

#[test]
fn operations_reject_control_characters_before_storage() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());
    let mut request = prepare_request();
    request.run_id = "run\ninjected".into();

    assert!(matches!(
        service.prepare(&request),
        Err(HubError::InvalidCharacters {
            field: HubField::GroupRunId
        })
    ));
    assert!(matches!(
        service.inspect("run\u{1b}[2J\u{202e}"),
        Err(HubError::InvalidCharacters {
            field: HubField::GroupRunId
        })
    ));
    assert!(store.calls().is_empty());
}

#[test]
fn prepare_rejects_creation_time_outside_the_durable_range() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());
    let mut request = prepare_request();
    request.created_at_ms = u64::MAX;

    assert!(matches!(
        service.prepare(&request),
        Err(HubError::GroupRunCreationTimeOutOfRange)
    ));
    assert!(store.calls().is_empty());
}

#[test]
fn prepare_rejects_every_invalid_policy_field_without_calling_storage() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());

    for (mutate, field, max) in invalid_policy_cases() {
        for value in [0, max + 1] {
            let mut request = prepare_request();
            mutate(&mut request.policy, value);
            assert!(matches!(
                service.prepare(&request),
                Err(HubError::OutOfRange {
                    field: actual,
                    min: 1,
                    max: actual_max
                }) if actual == field && actual_max == max
            ));
        }
    }
    assert!(store.calls().is_empty());
}

type PolicyMutation = fn(&mut GroupContextPolicy, usize);

fn invalid_policy_cases() -> Vec<(PolicyMutation, HubField, usize)> {
    vec![
        (
            |p, v| p.max_members = v,
            HubField::GroupContextMembers,
            MAX_GROUP_CONTEXT_MEMBERS,
        ),
        (
            |p, v| p.max_group_conversations = v,
            HubField::GroupContextGroupConversations,
            MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS,
        ),
        (
            |p, v| p.max_project_conversations_per_member = v,
            HubField::GroupContextProjectConversations,
            MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS,
        ),
        (
            |p, v| p.max_prompts_per_conversation = v,
            HubField::GroupContextPrompts,
            MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION,
        ),
        (
            |p, v| p.max_prompt_excerpt_bytes = v,
            HubField::GroupContextPromptExcerptBytes,
            MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES,
        ),
        (
            |p, v| p.max_total_content_bytes = v,
            HubField::GroupContextBytes,
            MAX_GROUP_CONTEXT_CONTENT_BYTES,
        ),
    ]
}

#[test]
fn inspect_validates_the_run_identifier_before_storage() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());
    let long_id = "r".repeat(MAX_ENTITY_ID_BYTES + 1);

    assert!(matches!(
        service.inspect(" "),
        Err(HubError::Empty {
            field: HubField::GroupRunId
        })
    ));
    assert!(matches!(
        service.inspect(&long_id),
        Err(HubError::TooLong {
            field: HubField::GroupRunId,
            max_bytes: MAX_ENTITY_ID_BYTES
        })
    ));
    assert!(store.calls().is_empty());
}

#[test]
fn list_accepts_optional_group_and_both_limit_boundaries() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());

    assert_eq!(service.list(None, 1).expect("minimum"), vec![record()]);
    assert_eq!(
        service
            .list(Some("group-1"), MAX_GROUP_RUN_LIST_LIMIT)
            .expect("maximum"),
        vec![record()]
    );
    assert_eq!(
        store.calls(),
        vec![
            Call::List(None, 1),
            Call::List(Some("group-1".into()), MAX_GROUP_RUN_LIST_LIMIT),
        ]
    );
}

#[test]
fn list_rejects_invalid_filter_and_limits_before_storage() {
    let store = Arc::new(SpyGroupRunStore::default());
    let service = GroupRunService::new(store.clone());
    let long_group_id = "g".repeat(MAX_ENTITY_ID_BYTES + 1);

    assert_invalid_group_filter(&service, &long_group_id);
    for limit in [0, MAX_GROUP_RUN_LIST_LIMIT + 1] {
        assert!(matches!(
            service.list(None, limit),
            Err(HubError::OutOfRange {
                field: HubField::GroupRunLimit,
                min: 1,
                max: MAX_GROUP_RUN_LIST_LIMIT
            })
        ));
    }
    assert!(store.calls().is_empty());
}

#[test]
fn structured_store_errors_are_preserved_for_every_operation() {
    let store = Arc::new(SpyGroupRunStore::failing());
    let service = GroupRunService::new(store.clone());

    assert_store_failure(&service.prepare(&prepare_request()));
    assert_store_failure(&service.inspect("group-run-1"));
    assert_store_failure(&service.list(Some("group-1"), 1));
    assert_eq!(store.calls().len(), 3);
}

fn assert_invalid_text<T>(result: &Result<T, HubError>, field: HubField, too_long: bool) {
    if too_long {
        assert!(matches!(
            result,
            Err(HubError::TooLong {
                field: actual,
                ..
            }) if *actual == field
        ));
    } else {
        assert!(matches!(
            result,
            Err(HubError::Empty { field: actual }) if *actual == field
        ));
    }
}

fn invalid_identifier_requests() -> Vec<(PrepareGroupRun, HubField, bool)> {
    let mut cases = Vec::new();
    push_text_case(&mut cases, HubField::GroupRunId, false, |request| {
        request.run_id = " ".into();
    });
    push_text_case(&mut cases, HubField::GroupId, false, |request| {
        request.group_id = " ".into();
    });
    push_text_case(&mut cases, HubField::IdempotencyKey, false, |request| {
        request.idempotency_key = " ".into();
    });
    push_too_long_cases(&mut cases);
    cases
}

fn push_too_long_cases(cases: &mut Vec<(PrepareGroupRun, HubField, bool)>) {
    push_text_case(cases, HubField::GroupRunId, true, |request| {
        request.run_id = "r".repeat(MAX_ENTITY_ID_BYTES + 1);
    });
    push_text_case(cases, HubField::GroupId, true, |request| {
        request.group_id = "g".repeat(MAX_ENTITY_ID_BYTES + 1);
    });
    push_text_case(cases, HubField::IdempotencyKey, true, |request| {
        request.idempotency_key = "k".repeat(MAX_IDEMPOTENCY_KEY_BYTES + 1);
    });
}

fn push_text_case(
    cases: &mut Vec<(PrepareGroupRun, HubField, bool)>,
    field: HubField,
    too_long: bool,
    mutate: impl FnOnce(&mut PrepareGroupRun),
) {
    let mut request = prepare_request();
    mutate(&mut request);
    cases.push((request, field, too_long));
}

fn assert_invalid_group_filter(service: &GroupRunService, long_group_id: &str) {
    assert!(matches!(
        service.list(Some(" "), 1),
        Err(HubError::Empty {
            field: HubField::GroupId
        })
    ));
    assert!(matches!(
        service.list(Some(long_group_id), 1),
        Err(HubError::TooLong {
            field: HubField::GroupId,
            max_bytes: MAX_ENTITY_ID_BYTES
        })
    ));
}

fn assert_store_failure<T>(result: &Result<T, HubError>) {
    assert!(matches!(
        result,
        Err(HubError::Store(HubStoreError::Conflict {
            entity: HubEntity::GroupRun,
            message
        })) if message == "sentinel"
    ));
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Prepare(Box<PrepareGroupRun>),
    Inspect(String),
    List(Option<String>, usize),
}

#[derive(Default)]
struct SpyGroupRunStore {
    calls: Mutex<Vec<Call>>,
    failure: Option<HubStoreError>,
}

impl SpyGroupRunStore {
    fn failing() -> Self {
        Self {
            calls: Mutex::default(),
            failure: Some(HubStoreError::Conflict {
                entity: HubEntity::GroupRun,
                message: "sentinel".into(),
            }),
        }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("spy lock").clone()
    }

    fn record_call(&self, call: Call) -> Result<(), HubStoreError> {
        self.calls.lock().expect("spy lock").push(call);
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl GroupRunStore for SpyGroupRunStore {
    fn prepare_group_run(
        &self,
        request: &PrepareGroupRun,
    ) -> Result<PrepareGroupRunResult, HubStoreError> {
        self.record_call(Call::Prepare(Box::new(request.clone())))?;
        Ok(prepare_result())
    }

    fn inspect_group_run(&self, run_id: &str) -> Result<GroupRunSnapshot, HubStoreError> {
        self.record_call(Call::Inspect(run_id.into()))?;
        Ok(snapshot())
    }

    fn list_group_runs(
        &self,
        group_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubStoreError> {
        self.record_call(Call::List(group_id.map(str::to_owned), limit))?;
        Ok(vec![record()])
    }
}

fn prepare_request() -> PrepareGroupRun {
    PrepareGroupRun {
        v: GROUP_RUN_VERSION,
        run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        policy: GroupContextPolicy::default(),
        idempotency_key: "prepare-key".into(),
        created_at_ms: 10,
    }
}

fn prepare_result() -> PrepareGroupRunResult {
    PrepareGroupRunResult {
        v: GROUP_RUN_VERSION,
        disposition: PrepareGroupRunDisposition::Created,
        snapshot: snapshot(),
    }
}

fn snapshot() -> GroupRunSnapshot {
    GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: record(),
        context: context(),
        context_json: r#"{"frozen":"context"}"#.into(),
    }
}

fn record() -> GroupRunRecord {
    GroupRunRecord {
        v: GROUP_RUN_VERSION,
        run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        status: GroupRunStatus::Prepared,
        context_version: GROUP_CONTEXT_VERSION,
        context_slice_sha256: "11".repeat(32),
        snapshot_sha256: "22".repeat(32),
        snapshot_bytes: 20,
        created_at_ms: 10,
    }
}

fn context() -> GroupContextSlice {
    GroupContextSlice {
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
            stats: GroupContextStats::default(),
        },
        slice_sha256: "11".repeat(32),
    }
}
