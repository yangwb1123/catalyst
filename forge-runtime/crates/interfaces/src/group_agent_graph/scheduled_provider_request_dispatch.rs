use std::{error::Error, path::PathBuf, sync::Arc};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledNodeDispatchInput, ExecuteGroupAgentScheduledNodeDispatchResult,
    GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSource,
    GroupAgentNodeDispatchMetadataSourceError, GroupAgentScheduledNodeDispatchExecutionService,
    GroupAgentScheduledNodeDispatchExecutionServiceError,
    GroupAgentScheduledNodeDispatchReadinessService,
};
use forge_runtime_infrastructure::{
    PinnedScheduledCoreTerminalBridge, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{Args, GroupGraphRunScheduledContractProviderRequestCommand},
    group_agent_graph::scheduled_dispatch_execution_output::ScheduledExecutorOwnerCleanup,
    group_agent_graph::scheduled_executor_sidecar::{
        ScheduledExecutorLiveness, ScheduledExecutorSidecar, ScheduledExecutorSidecarError,
    },
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::{
        Cancellation, GroupAgentScheduledNodeAnyLifecycleStore,
        GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledNodeLifecycleStore,
    },
    state_path::hub_database_path,
};

use super::{
    GroupAgentScheduledNodeProviderRequestCommandCliOutput, read_authorization, read_pricing,
    scheduled_provider_request_output::GroupAgentScheduledNodeDispatchExecutionCliOutput,
};

#[path = "scheduled_provider_request_dispatch/reconcile.rs"]
mod reconcile;
use reconcile::{DispatchErrorReconciliation, reconcile_owner_after_error};

pub(super) struct DispatchInputs {
    pub(super) authorization_json: Option<String>,
    pub(super) pricing_json: Option<String>,
}

struct FixedDispatchMetadataSource {
    claim: GroupAgentNodeDispatchClaimMetadata,
}

struct DispatchInvocation<'a> {
    args: &'a Args,
    provider_request_id: &'a str,
    inputs: &'a DispatchInputs,
    metadata: GroupAgentNodeDispatchClaimMetadata,
    owner: ScheduledExecutorSidecar,
    confirm_off_machine: bool,
    confirm_predecessor_content: bool,
    include_result: bool,
}

impl GroupAgentNodeDispatchMetadataSource for FixedDispatchMetadataSource {
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>
    {
        Ok(self.claim.clone())
    }

    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
        crate::group_agent_graph::dispatch_execution_adapters::SystemDispatchMetadataSource
            .terminal_time_ms()
    }
}

fn create_claim_metadata() -> Result<GroupAgentNodeDispatchClaimMetadata, Box<dyn Error>> {
    crate::group_agent_graph::dispatch_execution_adapters::SystemDispatchMetadataSource
        .claim_metadata()
        .map_err(Into::into)
}

fn executor_owner_directory(args: &Args) -> Result<PathBuf, Box<dyn Error>> {
    Ok(crate::state_path::state_dir(args.state_dir.as_deref())?.join("scheduled-executor-owners"))
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
    ensure_scheduled_executor_supported()?;
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
    let metadata = create_claim_metadata()?;
    let owner = ScheduledExecutorSidecar::create(
        &executor_owner_directory(args)?,
        provider_request_id,
        &metadata.lane_ownership_id,
    )?;
    let service = execution_service(
        args,
        bridge,
        Arc::new(FixedDispatchMetadataSource {
            claim: metadata.clone(),
        }),
    )?;
    execute_with_signal_cancellation(
        &service,
        DispatchInvocation {
            args,
            provider_request_id,
            inputs,
            metadata,
            owner,
            confirm_off_machine,
            confirm_predecessor_content,
            include_result,
        },
    )
    .await
}

/// Runs one effectful dispatch with exact executor-owner evidence for
/// hard-crash adjudication and folds OS signals into clean uncertainty.
async fn execute_with_signal_cancellation(
    service: &GroupAgentScheduledNodeDispatchExecutionService,
    mut invocation: DispatchInvocation<'_>,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    let cancellation = Cancellation::default();
    let cancel_on_signal = tokio::spawn(cancel_on_os_signal(cancellation.clone()));
    invocation.owner.preserve_on_drop();
    let input = dispatch_input(&invocation, cancellation);
    let result = Box::pin(service.execute(&input)).await;
    cancel_on_signal.abort();
    let _ = cancel_on_signal.await;
    let (result, cleanup) = match result {
        Ok(value) => {
            let cleanup = owner_cleanup(invocation.owner.cleanup());
            (value, cleanup)
        }
        Err(error) => {
            let reconciliation = reconcile_owner_after_error(
                invocation.args,
                invocation.provider_request_id,
                &invocation.metadata,
                invocation.owner,
            );
            return recover_dispatch_error(reconciliation, error, invocation.include_result);
        }
    };
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(
            GroupAgentScheduledNodeDispatchExecutionCliOutput::from_result_with_owner_cleanup(
                result,
                invocation.include_result,
                cleanup,
            ),
        )),
    )
}

fn recover_dispatch_error(
    reconciliation: DispatchErrorReconciliation,
    error: GroupAgentScheduledNodeDispatchExecutionServiceError,
    include_result: bool,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    match reconciliation {
        DispatchErrorReconciliation::Released {
            inspection,
            cleanup,
        } => Ok(
            GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(
                GroupAgentScheduledNodeDispatchExecutionCliOutput::from_any_inspection(
                    &inspection,
                    true,
                    true,
                    include_result,
                    cleanup,
                ),
            )),
        ),
        DispatchErrorReconciliation::NotClaimed | DispatchErrorReconciliation::Uncertain => {
            Err(error.into())
        }
    }
}

fn dispatch_input(
    invocation: &DispatchInvocation<'_>,
    cancellation: Cancellation,
) -> ExecuteGroupAgentScheduledNodeDispatchInput {
    ExecuteGroupAgentScheduledNodeDispatchInput {
        provider_request_id: invocation.provider_request_id.into(),
        authorization_json: invocation
            .inputs
            .authorization_json
            .clone()
            .expect("scheduled authorization was read before execution"),
        pricing_json: invocation
            .inputs
            .pricing_json
            .clone()
            .expect("scheduled pricing was read before execution"),
        confirm_off_machine: invocation.confirm_off_machine,
        confirm_predecessor_content: invocation.confirm_predecessor_content,
        cancellation,
    }
}

/// Uses durable exact-owner evidence to prove the old local executor stopped.
pub(super) fn adjudicate_dispatch(
    args: &Args,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestCommandCliOutput, Box<dyn Error>> {
    ensure_scheduled_executor_supported()?;
    let database = crate::state_path::hub_database_path(args.state_dir.as_deref())?;
    let store = std::sync::Arc::new(forge_runtime_infrastructure::SqliteHubStore::open(
        database,
    )?);
    let existing = store.inspect_group_agent_scheduled_node_any_lifecycle(provider_request_id)?;
    let lane_ownership_id = existing.claim().lane_ownership_id.clone();
    if existing.status() != GroupAgentScheduledNodeLifecycleStatus::Claimed {
        return Err(invalid_evidence("scheduled dispatch is not actively claimed").into());
    }
    let evidence = ScheduledExecutorSidecar::open(
        &executor_owner_directory(args)?,
        provider_request_id,
        &lane_ownership_id,
    )?;
    if evidence.liveness()? == ScheduledExecutorLiveness::Live {
        return Err(invalid_evidence("recorded scheduled executor is still alive").into());
    }
    let inspection = store.adjudicate_group_agent_scheduled_node_any_dispatch(
        &forge_runtime_domain::AdjudicateGroupAgentScheduledNodeDispatch {
            v: 1,
            provider_request_id: provider_request_id.into(),
            expected_lane_ownership_id: lane_ownership_id,
            adjudicated_at_ms: crate::state_path::unix_time_millis(),
        },
    )?;
    let cleanup = owner_cleanup(evidence.cleanup());
    Ok(
        GroupAgentScheduledNodeProviderRequestCommandCliOutput::Execution(Box::new(
            GroupAgentScheduledNodeDispatchExecutionCliOutput::from_any_inspection(
                &inspection,
                false,
                true,
                false,
                cleanup,
            ),
        )),
    )
}

fn owner_cleanup(
    result: Result<(), ScheduledExecutorSidecarError>,
) -> ScheduledExecutorOwnerCleanup {
    if result.is_ok() {
        ScheduledExecutorOwnerCleanup::Succeeded
    } else {
        ScheduledExecutorOwnerCleanup::Failed
    }
}

fn invalid_evidence(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// Watches SIGINT/SIGTERM and cancels the in-flight dispatch so the provider
/// stream is folded into a `Cancelled` uncertainty terminal and the Project
/// lane is released, instead of leaving a stranded v4 `dispatch_unknown`.
/// Hard crashes (SIGKILL/OOM) still leave quarantine; this only closes the
/// catchable-signal gap.
#[cfg(target_os = "linux")]
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

#[cfg(not(target_os = "linux"))]
async fn cancel_on_os_signal(_: Cancellation) {
    std::future::pending::<()>().await;
}

pub(super) fn ensure_scheduled_executor_supported() -> Result<(), std::io::Error> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "scheduled provider-request execution and adjudication are Linux-only",
        ))
    }
}

fn execution_service(
    args: &Args,
    bridge: Arc<PinnedScheduledCoreTerminalBridge>,
    metadata: Arc<dyn GroupAgentNodeDispatchMetadataSource>,
) -> Result<GroupAgentScheduledNodeDispatchExecutionService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let providers = Arc::new(RegisteredGroupAgentNodeProviderFactory::new());
    Ok(GroupAgentScheduledNodeDispatchExecutionService::new_with_successors(
        store.clone(),
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
        metadata,
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
    GroupAgentScheduledNodeDispatchReadinessService::new_with_successors(
        store.clone(),
        store.clone(),
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
