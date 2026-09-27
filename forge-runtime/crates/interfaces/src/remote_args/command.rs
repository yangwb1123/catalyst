#[derive(Debug, Eq, PartialEq)]
pub enum RemoteCommand {
    Login,
    Tui,
    CredentialStorageStatus,
    PlacementPreview {
        input: String,
    },
    PlacementRegistryPreview {
        input: String,
    },
    SchedulerSelectionPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    SchedulerSelectionLease {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    SchedulerSelectionLeaseRenew {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    SchedulerSelectionLeaseRelease {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunnerDispatchAdmissionPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunnerTransportAdmissionPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunnerExecutionBoundaryPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunnerAttemptBoundaryPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunnerExecutionIntentPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    CredentialCandidatePreview {
        input: String,
    },
    SessionObservationPreview {
        input: String,
    },
    SessionRunnerReceiptPreview {
        input: String,
    },
    SessionRunnerReceiptHistoryPreview {
        input: String,
    },
    SessionRunnerReconciliationPreview {
        input: String,
    },
    SessionRunnerReconciliationRemotePreview {
        input: String,
    },
    RunExecutionEvidencePreview {
        input: String,
    },
    RunAttemptLeaseDispatchPreflightPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunnerDispatchPlanPreview {
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    LocalRunnerPreview {
        input: String,
    },
    ExecutionConsentPreview {
        conversation_id: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    ExecutionReconciliationPreview {
        input: String,
    },
    InventoryShow,
    InventoryShowV2,
    InventoryShowConverged,
    LifecycleRegistryShow,
    ClientInstanceSessionView,
    ClientInstanceResourceView,
    ClientInstancesShowConverged,
    SessionsList {
        after_id: Option<String>,
        scope: Option<RemoteConversationScope>,
        instance_id: Option<String>,
        instance_view: Option<String>,
        all_pages: bool,
    },
    SessionsShow {
        conversation_id: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    SessionsCreate {
        title: String,
        scope: RemoteConversationScope,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    SessionsImport {
        conversation_id: String,
        confirm: Option<String>,
    },
    RunsList {
        conversation_id: String,
        limit: usize,
        before_created_at_ms: Option<u64>,
        before_run_id: Option<String>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunObserved {
        conversation_id: String,
        run_id: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    RunTimeline {
        conversation_id: String,
        run_id: String,
        after_sequence: u64,
        limit: usize,
        resume: bool,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PendingRunIntentsList {
        conversation_id: String,
        limit: usize,
        before_submitted_at_ms: Option<u64>,
        before_intent_id: Option<String>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PendingRunIntentSubmit {
        conversation_id: String,
        expected_version: u64,
        content: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PendingRunIntentTimeline {
        conversation_id: String,
        intent_id: String,
        after_sequence: u64,
        limit: usize,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PromptsList {
        conversation_id: String,
        before_created_at_ms: Option<u64>,
        before_prompt_id: Option<String>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PromptsAdd {
        conversation_id: String,
        expected_version: u64,
        content: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    PromptsReceipt {
        conversation_id: String,
        expected_version: u64,
        content: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    ChangesList {
        after_cursor: Option<u64>,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    ChangesWatch {
        after_cursor: Option<u64>,
        polls: usize,
        min_delay_ms: u64,
        max_delay_ms: u64,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
    ChangesStream {
        after_cursor: Option<u64>,
        wait_ms: u64,
        instance_id: Option<String>,
        instance_view: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteConversationScope {
    Global,
    Project(String),
    Group(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptPageCursor {
    pub created_at_ms: u64,
    pub prompt_id: String,
}
