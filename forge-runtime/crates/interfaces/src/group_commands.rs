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
    Contract(GroupGraphRunContractCommand),
    Dispatch(GroupGraphRunDispatchCommand),
    Schedule(GroupGraphRunScheduleCommand),
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
