use std::sync::{Arc, Mutex};

use forge_runtime_application::{
    MAX_ENTITY_ID_BYTES, MAX_IDEMPOTENCY_KEY_BYTES, RunError, RunField, RunService,
};
use forge_runtime_domain::{
    BeginRun, BeginRunBranch, BeginRunBranchResult, BeginRunDisposition, BeginRunResult,
    BoundRunPrompt, MAX_RUN_LIST_LIMIT, PROTOCOL_VERSION, PromptRecord, RUN_STORE_VERSION,
    RunExecution, RunInspection, RunLimits, RunLineageRecord, RunOutcome, RunProvider, RunRecord,
    RunRecovery, RunRecoveryState, RunStore, RunStoreError, RuntimeEvent,
};

#[test]
fn begin_validates_every_identifier_and_the_idempotency_key() {
    let store = Arc::new(SpyRunStore::default());
    let service = RunService::new(store.clone());
    for (request, field) in invalid_begin_requests() {
        assert!(matches!(
            service.begin_run(&request),
            Err(
                RunError::Empty { field: actual }
                    | RunError::TooLong {
                        field: actual,
                        ..
                    }
            )
                if actual == field
        ));
    }
    assert!(store.calls().is_empty());
}

#[test]
fn legacy_internal_prefix_is_a_valid_run_idempotency_key() {
    let store = Arc::new(SpyRunStore::default());
    let service = RunService::new(store.clone());
    let mut request = begin_request();
    request.idempotency_key = "internal:legacy-run".into();

    service
        .begin_run(&request)
        .expect("legacy retry key remains valid");

    assert_eq!(store.calls(), [Call::Begin(Box::new(request))]);
}

#[test]
fn list_and_inspection_validate_before_calling_storage() {
    let store = Arc::new(SpyRunStore::default());
    let service = RunService::new(store.clone());
    assert_empty_run_id(&service);
    assert_invalid_conversation_filter(&service);
    for limit in [0, MAX_RUN_LIST_LIMIT + 1] {
        assert!(matches!(
            service.list_runs(None, limit),
            Err(RunError::OutOfRange {
                field: RunField::RunLimit,
                min: 1,
                max: MAX_RUN_LIST_LIMIT
            })
        ));
    }
    assert!(store.calls().is_empty());
}

#[test]
fn valid_operations_delegate_without_rewriting_inputs() {
    let store = Arc::new(SpyRunStore::default());
    let service = RunService::new(store.clone());
    let request = begin_request();

    assert_eq!(service.begin_run(&request).expect("begin"), begin_result());
    assert_eq!(
        service.list_runs(Some("conversation-1"), 7).expect("list"),
        vec![record()]
    );
    assert_eq!(service.inspect_run("run-1").expect("inspect"), inspection());
    assert_eq!(
        service
            .find_run_by_idempotency_key("run-key")
            .expect("find"),
        Some(record())
    );
    assert_eq!(
        service
            .reconcile_completed_assistant("run-1")
            .expect("reconcile"),
        assistant_prompt()
    );
    assert_eq!(
        store.calls(),
        vec![
            Call::Begin(Box::new(request)),
            Call::List(Some("conversation-1".into()), 7),
            Call::Inspect("run-1".into()),
            Call::Find("run-key".into()),
            Call::Inspect("run-1".into()),
            Call::Reconcile("run-1".into()),
        ]
    );
}

#[test]
fn structured_storage_errors_are_preserved_for_every_operation() {
    let store = Arc::new(SpyRunStore::failing());
    let service = RunService::new(store.clone());

    assert_store_failure(&service.begin_run(&begin_request()));
    assert_store_failure(&service.list_runs(None, 1));
    assert_store_failure(&service.inspect_run("run-1"));
    assert_store_failure(&service.find_run_by_idempotency_key("run-key"));
    assert_store_failure(&service.reconcile_completed_assistant("run-1"));
    assert_eq!(store.calls().len(), 5);
}

fn assert_empty_run_id(service: &RunService) {
    assert!(matches!(
        service.inspect_run(" "),
        Err(RunError::Empty {
            field: RunField::RunId
        })
    ));
    let long_id = "r".repeat(MAX_ENTITY_ID_BYTES + 1);
    assert!(matches!(
        service.inspect_run(&long_id),
        Err(RunError::TooLong {
            field: RunField::RunId,
            ..
        })
    ));
}

fn assert_invalid_conversation_filter(service: &RunService) {
    assert!(matches!(
        service.list_runs(Some(" "), 1),
        Err(RunError::Empty {
            field: RunField::ConversationId
        })
    ));
    let long_id = "c".repeat(MAX_ENTITY_ID_BYTES + 1);
    assert!(matches!(
        service.list_runs(Some(&long_id), 1),
        Err(RunError::TooLong {
            field: RunField::ConversationId,
            ..
        })
    ));
}

fn assert_store_failure<T>(result: &Result<T, RunError>) {
    assert!(matches!(
        result,
        Err(RunError::Store(RunStoreError::Unavailable { message }))
            if message == "sentinel"
    ));
}

fn invalid_begin_requests() -> Vec<(BeginRun, RunField)> {
    let mut cases = Vec::new();
    push_invalid_id(
        &mut cases,
        |request| request.run_id = " ".into(),
        RunField::RunId,
    );
    push_invalid_id(
        &mut cases,
        |request| request.conversation_id = " ".into(),
        RunField::ConversationId,
    );
    push_invalid_id(
        &mut cases,
        |request| request.prompt_id = " ".into(),
        RunField::PromptId,
    );
    push_invalid_id(
        &mut cases,
        |request| request.project_id = " ".into(),
        RunField::ProjectId,
    );
    push_invalid_id(
        &mut cases,
        |request| request.idempotency_key = " ".into(),
        RunField::IdempotencyKey,
    );
    push_too_long_requests(&mut cases);
    cases
}

fn push_invalid_id(
    cases: &mut Vec<(BeginRun, RunField)>,
    mutate: impl FnOnce(&mut BeginRun),
    field: RunField,
) {
    let mut request = begin_request();
    mutate(&mut request);
    cases.push((request, field));
}

fn push_too_long_requests(cases: &mut Vec<(BeginRun, RunField)>) {
    push_invalid_id(
        cases,
        |request| request.run_id = "r".repeat(MAX_ENTITY_ID_BYTES + 1),
        RunField::RunId,
    );
    push_invalid_id(
        cases,
        |request| {
            request.idempotency_key = "k".repeat(MAX_IDEMPOTENCY_KEY_BYTES + 1);
        },
        RunField::IdempotencyKey,
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Begin(Box<BeginRun>),
    Find(String),
    List(Option<String>, usize),
    Inspect(String),
    Reconcile(String),
}

#[derive(Default)]
struct SpyRunStore {
    calls: Mutex<Vec<Call>>,
    failure: Option<RunStoreError>,
}

impl SpyRunStore {
    fn failing() -> Self {
        Self {
            calls: Mutex::default(),
            failure: Some(RunStoreError::Unavailable {
                message: "sentinel".into(),
            }),
        }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("spy lock").clone()
    }

    fn record(&self, call: Call) -> Result<(), RunStoreError> {
        self.calls.lock().expect("spy lock").push(call);
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl RunStore for SpyRunStore {
    fn begin_run(&self, request: &BeginRun) -> Result<BeginRunResult, RunStoreError> {
        self.record(Call::Begin(Box::new(request.clone())))?;
        Ok(begin_result())
    }

    fn begin_run_branch(
        &self,
        _request: &BeginRunBranch,
    ) -> Result<BeginRunBranchResult, RunStoreError> {
        unreachable!("branch service has dedicated integration coverage")
    }

    fn find_run_lineage(&self, _run_id: &str) -> Result<Option<RunLineageRecord>, RunStoreError> {
        unreachable!("branch service has dedicated integration coverage")
    }

    fn append_event(&self, _event: &RuntimeEvent) -> Result<(), RunStoreError> {
        unreachable!("RunService does not append events")
    }

    fn find_run_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<RunRecord>, RunStoreError> {
        self.record(Call::Find(idempotency_key.into()))?;
        Ok(Some(record()))
    }

    fn inspect_run(&self, run_id: &str) -> Result<RunInspection, RunStoreError> {
        self.record(Call::Inspect(run_id.into()))?;
        Ok(inspection())
    }

    fn list_runs(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>, RunStoreError> {
        self.record(Call::List(conversation_id.map(str::to_owned), limit))?;
        Ok(vec![record()])
    }

    fn reconcile_completed_assistant(&self, run_id: &str) -> Result<PromptRecord, RunStoreError> {
        self.record(Call::Reconcile(run_id.into()))?;
        Ok(assistant_prompt())
    }
}

fn begin_request() -> BeginRun {
    BeginRun {
        v: RUN_STORE_VERSION,
        run_id: "run-1".into(),
        conversation_id: "conversation-1".into(),
        prompt_id: "prompt-1".into(),
        project_id: "project-1".into(),
        execution: execution(),
        idempotency_key: "run-key".into(),
        created_at_ms: 10,
    }
}

fn record() -> RunRecord {
    RunRecord {
        v: RUN_STORE_VERSION,
        run_id: "run-1".into(),
        conversation_id: "conversation-1".into(),
        prompt_id: "prompt-1".into(),
        project_id: "project-1".into(),
        execution: execution(),
        protocol_version: PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn begin_result() -> BeginRunResult {
    BeginRunResult {
        v: RUN_STORE_VERSION,
        disposition: BeginRunDisposition::Created,
        run: record(),
        prompt: BoundRunPrompt {
            v: RUN_STORE_VERSION,
            prompt_id: "prompt-1".into(),
            conversation_id: "conversation-1".into(),
            content: "hello".into(),
            created_at_ms: 5,
        },
    }
}

fn inspection() -> RunInspection {
    RunInspection {
        v: RUN_STORE_VERSION,
        run: record(),
        events: Vec::new(),
        recovery: RunRecovery {
            v: RUN_STORE_VERSION,
            state: RunRecoveryState::Terminal {
                outcome: RunOutcome::Completed {
                    answer: "answer".into(),
                },
            },
        },
    }
}

fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::DeterministicRead {
            path: "README.md".into(),
        },
        system_prompt: "Read only what is authorized.".into(),
        allowed_read_paths: vec!["README.md".into()],
        limits: RunLimits::default(),
    }
}

fn assistant_prompt() -> PromptRecord {
    PromptRecord {
        id: "prompt-assistant".into(),
        conversation_id: "conversation-1".into(),
        role: "assistant".into(),
        content: "answer".into(),
        idempotency_key: "opaque-writeback-key".into(),
        created_at_ms: 10,
    }
}
