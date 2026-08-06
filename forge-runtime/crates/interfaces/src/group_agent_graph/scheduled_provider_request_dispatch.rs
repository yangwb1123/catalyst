use std::{error::Error, path::PathBuf, sync::Arc};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledNodeDispatchInput, ExecuteGroupAgentScheduledNodeDispatchResult,
    GroupAgentScheduledNodeDispatchExecutionService,
    GroupAgentScheduledNodeDispatchReadinessService,
};
use forge_runtime_infrastructure::{
    PinnedScheduledCoreTerminalBridge, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{Args, GroupGraphRunScheduledContractProviderRequestCommand},
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::{Cancellation, GroupAgentScheduledNodeLifecycleStore},
    state_path::hub_database_path,
};

use super::{
    GroupAgentScheduledNodeProviderRequestCommandCliOutput, read_authorization, read_pricing,
    scheduled_provider_request_output::GroupAgentScheduledNodeDispatchExecutionCliOutput,
};

pub(super) struct DispatchInputs {
    pub(super) authorization_json: Option<String>,
    pub(super) pricing_json: Option<String>,
}

impl DispatchInputs {
    pub(super) fn authorization(&self) -> &str {
        self.authorization_json
            .as_deref()
            .expect("scheduled authorization was read before execution")
    }

    pub(super) fn pricing(&self) -> &str {
        self.pricing_json
            .as_deref()
            .expect("scheduled pricing was read before execution")
    }
}

pub(super) fn inspect_existing_execution(
    args: &Args,
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<Option<GroupAgentScheduledNodeProviderRequestCommandCliOutput>, Box<dyn Error>> {
    let GroupGraphRunScheduledContractProviderRequestCommand::Execute {
        provider_request_id,
        include_result,
        ..
    } = command
    else {
        return Ok(None);
    };
    let database = hub_database_path(args.state_dir.as_deref())?;
    if !database.try_exists()? {
        return Ok(None);
    }
    let store = SqliteHubStore::open_existing_current_read_only(database)?;
    let inspection = match store.inspect_group_agent_scheduled_node_lifecycle(provider_request_id) {
        Ok(value) => value,
        Err(crate::runtime_domain::HubStoreError::NotFound { .. }) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let output = GroupAgentScheduledNodeDispatchExecutionCliOutput::from_result(
        ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(inspection),
        *include_result,
    );
    Ok(Some(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(output)),
    ))
}

pub(super) fn read_inputs(
    command: &GroupGraphRunScheduledContractProviderRequestCommand,
) -> Result<DispatchInputs, Box<dyn Error>> {
    match command {
        GroupGraphRunScheduledContractProviderRequestCommand::Execute {
            authorization_source,
            pricing_source,
            ..
        } => Ok(DispatchInputs {
            authorization_json: Some(read_authorization(authorization_source)?),
            pricing_json: Some(read_pricing(pricing_source)?),
        }),
        _ => Ok(DispatchInputs {
            authorization_json: None,
            pricing_json: None,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_dispatch(
    args: &Args,
    provider_request_id: &str,
    inputs: &DispatchInputs,
    core_bin: &str,
    core_bin_sha256: &str,
    confirm_off_machine: bool,
    confirm_predecessor_content: bool,
    include_result: bool,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    validate_execute_preflight(
        args,
        provider_request_id,
        inputs.authorization(),
        inputs.pricing(),
        confirm_off_machine,
    )?;
    let bridge = Arc::new(PinnedScheduledCoreTerminalBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let service = execution_service(args, bridge)?;
    let sidecar = ExecutorPidSidecar::write(args, provider_request_id)?;
    let cancellation = Cancellation::default();
    let cancel_on_signal = tokio::spawn(cancel_on_os_signal(cancellation.clone()));
    let result = Box::pin(
        service.execute(&ExecuteGroupAgentScheduledNodeDispatchInput {
            provider_request_id: provider_request_id.into(),
            authorization_json: inputs
                .authorization_json
                .clone()
                .expect("scheduled authorization was read before execution"),
            pricing_json: inputs
                .pricing_json
                .clone()
                .expect("scheduled pricing was read before execution"),
            confirm_off_machine,
            confirm_predecessor_content,
            cancellation,
        }),
    )
    .await?;
    cancel_on_signal.abort();
    let _ = cancel_on_signal.await;
    sidecar.remove();
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(
            GroupAgentScheduledNodeDispatchExecutionCliOutput::from_result(result, include_result),
        )),
    )
}

/// Local hard-crash adjudication evidence: records the executor pid + hostname
/// before the claim so a later operator can prove the old executor stopped.
pub(super) fn adjudicate_dispatch(
    args: &Args,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    let database = crate::state_path::hub_database_path(args.state_dir.as_deref())?;
    let store = std::sync::Arc::new(forge_runtime_infrastructure::SqliteHubStore::open(
        database,
    )?);
    let evidence = ExecutorPidSidecar::prove_stopped(args, provider_request_id)?;
    let inspection = store.adjudicate_group_agent_scheduled_node_dispatch(
        &forge_runtime_domain::AdjudicateGroupAgentScheduledNodeDispatch {
            v: 1,
            provider_request_id: provider_request_id.into(),
            adjudicated_at_ms: crate::state_path::unix_time_millis(),
        },
    )?;
    evidence.remove();
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(
            GroupAgentScheduledNodeDispatchExecutionCliOutput::from_inspection(
                &inspection,
                false,
                false,
            ),
        )),
    )
}

/// The `.forge/executor-pids/<request_id>.pid` sidecar: one line
/// "PID HOSTNAME", written before the claim and removed after the terminalize
/// transaction commits. Its absence after a hard crash is impossible for a
/// normally completed dispatch; adjudication requires the record to exist AND
/// the recorded pid to be provably dead on this host.
pub(super) struct ExecutorPidSidecar {
    path: std::path::PathBuf,
}

impl ExecutorPidSidecar {
    pub(super) fn write(args: &Args, provider_request_id: &str) -> Result<Self, Box<dyn Error>> {
        let state = crate::state_path::state_dir(args.state_dir.as_deref())?;
        let dir = state.join("executor-pids");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{provider_request_id}.pid"));
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".into());
        std::fs::write(&path, format!("{} {hostname}
", std::process::id()))?;
        Ok(Self { path })
    }

    fn remove(&self) {
        let _ = std::fs::remove_file(&self.path);
    }

    /// Proves the recorded executor stopped: sidecar exists, same hostname,
    /// and the recorded pid is not alive. Any other outcome rejects.
    fn prove_stopped(args: &Args, provider_request_id: &str) -> Result<Self, Box<dyn Error>> {
        let state = crate::state_path::state_dir(args.state_dir.as_deref())?;
        let path = state
            .join("executor-pids")
            .join(format!("{provider_request_id}.pid"));
        let raw = std::fs::read_to_string(&path).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no executor pid sidecar: hard-crash adjudication has no evidence to prove",
            )
        })?;
        let mut parts = raw.split_whitespace();
        let pid: u32 = parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| invalid_evidence("malformed executor pid sidecar"))?;
        let hostname = parts
            .next()
            .ok_or_else(|| invalid_evidence("malformed executor pid sidecar"))?;
        let current_host = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".into());
        if hostname != current_host {
            return Err(Box::new(invalid_evidence(
                "executor pid sidecar belongs to another host; cannot prove it stopped",
            )));
        }
        if pid_alive(pid) {
            return Err(Box::new(invalid_evidence(
                "recorded executor pid is still alive; adjudication refused",
            )));
        }
        Ok(Self { path })
    }
}

fn pid_alive(pid: u32) -> bool {
    // /proc/<pid> exists while the process (or a zombie) is present; a
    // completed reaped process leaves no entry. Best-effort local liveness,
    // not a cross-host or adversarial guarantee.
    let proc_path = format!("/proc/{pid}");
    let proc_entry = std::path::Path::new(&proc_path);
    if !proc_entry.exists() {
        return false;
    }
    // A zombie still occupies the pid; treat it as alive (conservative).
    std::fs::read_to_string(proc_entry.join("stat"))
        .map(|stat| !stat.contains(" Z "))
        .unwrap_or(true)
}

fn invalid_evidence(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// Watches SIGINT/SIGTERM and cancels the in-flight dispatch so the provider
/// stream is folded into a `Cancelled` uncertainty terminal and the Project
/// lane is released, instead of leaving a stranded v4 `dispatch_unknown`.
/// Hard crashes (SIGKILL/OOM) still leave quarantine; this only closes the
/// catchable-signal gap.
async fn cancel_on_os_signal(cancellation: Cancellation) {
    let Ok(mut sigint) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
    else {
        return;
    };
    let Ok(mut sigterm) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    else {
        return;
    };
    tokio::select! {
        _ = sigint.recv() => {}
        _ = sigterm.recv() => {}
    }
    cancellation.cancel();
}

fn execution_service(
    args: &Args,
    bridge: Arc<PinnedScheduledCoreTerminalBridge>,
) -> Result<GroupAgentScheduledNodeDispatchExecutionService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let providers = Arc::new(RegisteredGroupAgentNodeProviderFactory::new());
    Ok(GroupAgentScheduledNodeDispatchExecutionService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
        store,
        providers,
        Arc::new(crate::group_agent_graph::dispatch_execution_adapters::EnvironmentOpenAiCredentialSource),
        bridge,
        Arc::new(crate::group_agent_graph::dispatch_execution_adapters::SystemDispatchMetadataSource),
    ))
}

fn validate_execute_preflight(
    args: &Args,
    provider_request_id: &str,
    authorization_json: &str,
    pricing_json: &str,
    confirm_off_machine: bool,
) -> Result<(), Box<dyn Error>> {
    if !confirm_off_machine {
        return Err(forge_runtime_application::GroupAgentScheduledNodeDispatchExecutionServiceError::ConsentRequired.into());
    }
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    GroupAgentScheduledNodeDispatchReadinessService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    )
    .verify(provider_request_id, authorization_json, pricing_json)?;
    Ok(())
}
