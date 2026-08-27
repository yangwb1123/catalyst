use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub enum GroupCommand {
    Analysis(GroupAnalysisCommand),
    Graph(GroupGraphCommand),
    Create {
        name: String,
    },
    Add {
        group_id: String,
        project: PathBuf,
        role: String,
    },
    Context {
        group_id: String,
        include_content: bool,
        max_bytes: usize,
    },
    Execution(GroupExecutionCommand),
    Panel(GroupPanelCommand),
    Run(GroupRunCommand),
    Synthesis(GroupSynthesisCommand),
    List,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphCommand {
    Prepare {
        group_run_id: String,
        spec_source: String,
    },
    Run(GroupGraphRunCommand),
    Show {
        graph_id: String,
        include_spec: bool,
    },
    List {
        group_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunCommand {
    Prepare {
        graph_id: String,
        plan_source: String,
    },
    Control(GroupGraphRunControlCommand),
    Controller(GroupGraphRunControllerCommand),
    Contract(GroupGraphRunContractCommand),
    Dispatch(GroupGraphRunDispatchCommand),
    Schedule(GroupGraphRunScheduleCommand),
    ScheduledContract(GroupGraphRunScheduledContractCommand),
    Reconcile {
        graph_run_id: String,
        core_bin: String,
        core_bin_sha256: String,
    },
    ReadyRelease {
        graph_run_id: String,
        core_bin: String,
        core_bin_sha256: String,
    },
    Step(Box<GroupGraphRunReadyStepOptions>),
    Show {
        graph_run_id: String,
        include_plan: bool,
    },
    List {
        graph_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunControllerCommand {
    Start(Box<GroupGraphRunControllerStartOptions>),
    Advance {
        graph_run_id: String,
        core_bin: String,
        core_bin_sha256: String,
    },
    Show {
        graph_run_id: String,
    },
    Step(Box<GroupGraphRunControllerStepOptions>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupGraphRunControllerStartOptions {
    pub graph_run_id: String,
    pub expected_schedule_sha256: String,
    pub core_bin: String,
    pub core_bin_sha256: String,
    pub endpoint: String,
    pub model: String,
    pub max_output_tokens: u64,
    pub max_model_output_bytes: u64,
    pub max_model_events: u64,
    pub timeout_ms: u64,
    pub max_cost_usd_micros: u64,
    pub pricing_snapshot_sha256: String,
    pub max_result_bytes: u64,
    pub max_effectful_steps: u16,
    pub max_total_cost_usd_micros: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupGraphRunControllerStepOptions {
    pub graph_run_id: String,
    pub expected_awaiting_event_sha256: String,
    pub expected_provider_request_id: String,
    pub expected_authorization_sha256: String,
    pub pricing_source: String,
    pub core_bin: String,
    pub core_bin_sha256: String,
    pub confirm_off_machine: bool,
    pub confirm_predecessor_content: bool,
    pub include_result: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupGraphRunReadyStepOptions {
    pub graph_run_id: String,
    pub expected_provider_request_id: String,
    pub expected_ready_authorization_sha256: String,
    pub pricing_source: String,
    pub core_bin: String,
    pub core_bin_sha256: String,
    pub confirm_off_machine: bool,
    pub confirm_predecessor_content: bool,
    pub include_result: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunScheduledContractCommand {
    Admit {
        graph_run_id: String,
        contract_source: String,
    },
    Show {
        contract_id: String,
        include_contract: bool,
    },
    List {
        graph_run_id: Option<String>,
        limit: usize,
    },
    ProviderRequest(GroupGraphRunScheduledContractProviderRequestCommand),
    PredecessorReceiptExport {
        provider_request_id: String,
    },
    Successor(GroupGraphRunScheduledContractSuccessorCommand),
    WaveAdmit {
        graph_run_id: String,
        predecessor_receipt_sources: Vec<String>,
        schedule_sha256: String,
        go_core: Option<String>,
        idempotency_key: Option<String>,
        execution: Box<WaveAdmitExecutionOptions>,
    },
}

/// Execution options for wave-admit: mirrored from the Go core's
/// graph-scheduled-node-contract flags so every candidate carries the
/// operator's real endpoint/model/budget/pricing, never literals
/// (review Finding 2).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WaveAdmitExecutionOptions {
    pub endpoint: String,
    pub model: String,
    pub max_output_tokens: String,
    pub max_model_output_bytes: String,
    pub max_model_events: String,
    pub timeout_ms: String,
    pub max_cost_usd_micros: String,
    pub pricing_snapshot_sha256: String,
    pub max_result_bytes: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunScheduledContractSuccessorCommand {
    Admit {
        graph_run_id: String,
        contract_source: String,
        predecessor_receipt_sources: Vec<String>,
        predecessor_content_source: Option<String>,
    },
    Show {
        contract_id: String,
        include_contract: bool,
    },
    List {
        graph_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunScheduledContractProviderRequestCommand {
    Prepare {
        scheduled_contract_id: String,
    },
    Show {
        provider_request_id: String,
        include_request: bool,
    },
    List {
        graph_run_id: Option<String>,
        limit: usize,
    },
    ReleaseControlExport {
        provider_request_id: String,
    },
    AuthorizationVerify {
        provider_request_id: String,
        authorization_source: String,
    },
    ReadinessVerify {
        provider_request_id: String,
        authorization_source: String,
        pricing_source: String,
    },
    Execute {
        provider_request_id: String,
        authorization_source: String,
        pricing_source: String,
        core_bin: String,
        core_bin_sha256: String,
        confirm_off_machine: bool,
        confirm_predecessor_content: bool,
        include_result: bool,
    },
    Adjudicate {
        provider_request_id: String,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunScheduleCommand {
    Admit {
        graph_run_id: String,
        schedule_source: String,
    },
    Show {
        schedule_id: String,
        include_schedule: bool,
    },
    List {
        graph_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunDispatchCommand {
    Prepare {
        graph_run_id: String,
    },
    Show {
        dispatch_request_id: String,
        include_request: bool,
    },
    List {
        graph_run_id: Option<String>,
        limit: usize,
    },
    ReleaseControlExport {
        graph_run_id: String,
    },
    AuthorizationVerify {
        graph_run_id: String,
        authorization_source: String,
    },
    ReadinessVerify {
        graph_run_id: String,
        authorization_source: String,
        pricing_source: String,
    },
    Execute {
        graph_run_id: String,
        authorization_source: String,
        pricing_source: String,
        core_bin: String,
        core_bin_sha256: String,
        confirm_off_machine: bool,
        include_result: bool,
    },
    Adjudicate {
        graph_run_id: String,
        authorization_source: String,
        pricing_source: String,
        core_bin: String,
        core_bin_sha256: String,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunControlCommand {
    Export { graph_run_id: String },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupGraphRunContractCommand {
    Admit {
        graph_run_id: String,
        contract_source: String,
    },
    Show {
        contract_id: String,
        include_contract: bool,
    },
    List {
        graph_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupSynthesisCommand {
    Prepare {
        panel_id: String,
        model: Option<String>,
        max_output_tokens: u32,
    },
    Send {
        synthesis_id: String,
        confirm_off_machine: bool,
        include_result: bool,
    },
    Show {
        synthesis_id: String,
        include_result: bool,
    },
    List {
        panel_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupPanelCommand {
    Prepare {
        group_run_id: String,
        analysis_ids: Vec<String>,
    },
    Show {
        panel_id: String,
        include_results: bool,
    },
    List {
        group_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupAnalysisCommand {
    Prepare {
        group_run_id: String,
        model: Option<String>,
        max_output_tokens: u32,
    },
    Send {
        analysis_id: String,
        confirm_off_machine: bool,
        include_result: bool,
    },
    Show {
        analysis_id: String,
        include_result: bool,
    },
    List {
        group_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupExecutionCommand {
    Start {
        group_run_id: String,
    },
    Show {
        execution_id: String,
    },
    List {
        group_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupRunCommand {
    Prepare {
        group_id: String,
        include_content: bool,
        max_bytes: usize,
    },
    Show {
        run_id: String,
        include_content: bool,
    },
    List {
        group_id: Option<String>,
        limit: usize,
    },
}
