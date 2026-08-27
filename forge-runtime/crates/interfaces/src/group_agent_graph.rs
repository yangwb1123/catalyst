#[path = "group_agent_graph/command.rs"]
pub(crate) mod command;
#[path = "group_agent_graph/contract_command.rs"]
pub(crate) mod contract_command;
#[path = "group_agent_graph/contract_output.rs"]
pub(crate) mod contract_output;
#[path = "group_agent_graph/dispatch_authorization_output.rs"]
pub(crate) mod dispatch_authorization_output;
#[path = "group_agent_graph/dispatch_command.rs"]
pub(crate) mod dispatch_command;
#[path = "group_agent_graph/dispatch_execution_adapters.rs"]
pub(crate) mod dispatch_execution_adapters;
#[path = "group_agent_graph/dispatch_output.rs"]
pub(crate) mod dispatch_output;
#[path = "group_agent_graph/dispatch_readiness_output.rs"]
pub(crate) mod dispatch_readiness_output;
#[path = "group_agent_graph/output.rs"]
pub(crate) mod output;
#[path = "group_agent_graph/ready_release_command.rs"]
pub(crate) mod ready_release_command;
#[path = "group_agent_graph/ready_step_command.rs"]
pub(crate) mod ready_step_command;
#[path = "group_agent_graph/ready_step_output.rs"]
pub(crate) mod ready_step_output;
#[path = "group_agent_graph/ready_step_owner.rs"]
pub(crate) mod ready_step_owner;
#[path = "group_agent_graph/reconcile_command.rs"]
pub(crate) mod reconcile_command;
#[path = "group_agent_graph/run_command.rs"]
pub(crate) mod run_command;
#[path = "group_agent_graph/run_output.rs"]
pub(crate) mod run_output;
#[path = "group_agent_graph/schedule_args.rs"]
pub(crate) mod schedule_command;
#[path = "group_agent_graph/schedule_output.rs"]
pub(crate) mod schedule_output;
#[path = "group_agent_graph/scheduled_contract_command.rs"]
pub(crate) mod scheduled_contract_command;
#[path = "group_agent_graph/scheduled_contract_output.rs"]
pub(crate) mod scheduled_contract_output;
#[path = "group_agent_graph/scheduled_dispatch_execution_output.rs"]
pub(crate) mod scheduled_dispatch_execution_output;
#[cfg(target_os = "linux")]
#[allow(
    dead_code,
    reason = "shared owner/adjudication protocol includes observation APIs used by separate commands"
)]
#[path = "group_agent_graph/scheduled_executor_sidecar.rs"]
pub(crate) mod scheduled_executor_sidecar;
#[cfg(not(target_os = "linux"))]
#[allow(
    dead_code,
    reason = "portable callers share the Linux sidecar API but fail explicitly as unsupported"
)]
#[path = "group_agent_graph/scheduled_executor_sidecar_unsupported.rs"]
pub(crate) mod scheduled_executor_sidecar;
#[path = "group_agent_graph/scheduled_provider_request_command.rs"]
pub(crate) mod scheduled_provider_request_command;
#[path = "group_agent_graph/scheduled_provider_request_output.rs"]
pub(crate) mod scheduled_provider_request_output;
#[path = "group_agent_graph/scheduled_release/output.rs"]
pub(crate) mod scheduled_release_output;
#[path = "group_agent_graph/scheduled_release/readiness_output.rs"]
pub(crate) mod scheduled_release_readiness_output;
#[path = "group_agent_graph/wave_command.rs"]
pub(crate) mod wave_command;
