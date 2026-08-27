use std::{error::Error, fs, io, path::PathBuf, sync::Arc};

use crate::runtime_application::{
    AdvanceScheduledGraphControllerInput, GroupAgentScheduledExecutorOwnerFactory,
    GroupAgentScheduledReadyNodeDispatchExecutionService, ScheduledGraphControllerClock,
    ScheduledGraphControllerPricingSource, ScheduledGraphControllerPricingSourceError,
    ScheduledGraphControllerQueryService, ScheduledGraphControllerService,
    ScheduledGraphControllerServiceError, StartScheduledGraphControllerInput,
    StepScheduledGraphControllerInput,
};
use crate::runtime_domain::{
    Cancellation, HubStoreError, ScheduledGraphControllerExecutionProfile,
    ScheduledGraphControllerStore,
};
use forge_runtime_infrastructure::{
    CURRENT_SCHEMA_VERSION, PinnedScheduledCoreTerminalBridge,
    PinnedScheduledNodeMaterializationBridge, PinnedScheduledReadyNodeReleaseBridge,
    RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{
        Args, GroupGraphRunControllerCommand, GroupGraphRunControllerStartOptions,
        GroupGraphRunControllerStepOptions,
    },
    openai_prepared_dispatch::OpenAiRequestCodec,
    state_path::{hub_database_path, unix_time_millis},
};

use super::{
    controller_output::ScheduledGraphControllerCliOutput,
    dispatch_execution_adapters::{
        EnvironmentOpenAiCredentialSource, SystemDispatchMetadataSource,
    },
};

pub async fn execute(
    args: &Args,
    command: &GroupGraphRunControllerCommand,
) -> Result<ScheduledGraphControllerCliOutput, Box<dyn Error>> {
    match command {
        GroupGraphRunControllerCommand::Start(options) => execute_start(args, options),
        GroupGraphRunControllerCommand::Advance {
            graph_run_id,
            core_bin,
            core_bin_sha256,
        } => execute_advance(args, graph_run_id, core_bin, core_bin_sha256),
        GroupGraphRunControllerCommand::Step(options) => {
            Box::pin(execute_step_command(args, options)).await
        }
        GroupGraphRunControllerCommand::Show { graph_run_id } => show(args, graph_run_id),
    }
}

fn execute_start(
    args: &Args,
    options: &GroupGraphRunControllerStartOptions,
) -> Result<ScheduledGraphControllerCliOutput, Box<dyn Error>> {
    let input = start_input(options)?;
    ScheduledGraphControllerService::preflight_start(&input)?;
    let service = controller_service_for(
        args,
        &options.graph_run_id,
        &options.core_bin,
        &options.core_bin_sha256,
        true,
        ControllerMode::Passive,
    )?;
    let output = service.start(&input)?;
    Ok(ScheduledGraphControllerCliOutput::new(
        output, false, false, true,
    ))
}

fn execute_advance(
    args: &Args,
    graph_run_id: &str,
    core_bin: &str,
    core_bin_sha256: &str,
) -> Result<ScheduledGraphControllerCliOutput, Box<dyn Error>> {
    let input = AdvanceScheduledGraphControllerInput {
        graph_run_id: graph_run_id.into(),
        core_bin_sha256: core_bin_sha256.into(),
        observed_at_ms: unix_time_millis(),
    };
    ScheduledGraphControllerService::preflight_advance(&input)?;
    let service = controller_service_for(
        args,
        graph_run_id,
        core_bin,
        core_bin_sha256,
        false,
        ControllerMode::Passive,
    )?;
    let output = service.advance(&input)?;
    Ok(ScheduledGraphControllerCliOutput::new(
        output, false, false, true,
    ))
}

async fn execute_step_command(
    args: &Args,
    options: &GroupGraphRunControllerStepOptions,
) -> Result<ScheduledGraphControllerCliOutput, Box<dyn Error>> {
    let input = step_input(options);
    ScheduledGraphControllerService::preflight_step(&input)?;
    let service = controller_service_for(
        args,
        &options.graph_run_id,
        &options.core_bin,
        &options.core_bin_sha256,
        false,
        ControllerMode::Effectful,
    )?;
    Box::pin(execute_step(service, input, options.include_result)).await
}

fn show(
    args: &Args,
    graph_run_id: &str,
) -> Result<ScheduledGraphControllerCliOutput, Box<dyn Error>> {
    ScheduledGraphControllerQueryService::preflight_inspect(graph_run_id)?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_live_read_only(
        database,
    )?);
    let output = ScheduledGraphControllerQueryService::new(store).inspect(graph_run_id)?;
    Ok(ScheduledGraphControllerCliOutput::from_show(output))
}

fn start_input(
    options: &GroupGraphRunControllerStartOptions,
) -> Result<StartScheduledGraphControllerInput, Box<dyn Error>> {
    let profile = ScheduledGraphControllerExecutionProfile {
        endpoint: options.endpoint.clone(),
        model: options.model.clone(),
        max_output_tokens: options.max_output_tokens,
        max_model_output_bytes: options.max_model_output_bytes,
        max_model_events: options.max_model_events,
        timeout_ms: options.timeout_ms,
        max_cost_usd_micros: options.max_cost_usd_micros,
        pricing_snapshot_sha256: options.pricing_snapshot_sha256.clone(),
        max_result_bytes: options.max_result_bytes,
        profile_sha256: String::new(),
    }
    .seal()?;
    Ok(StartScheduledGraphControllerInput {
        graph_run_id: options.graph_run_id.clone(),
        expected_schedule_sha256: options.expected_schedule_sha256.clone(),
        core_bin_sha256: options.core_bin_sha256.clone(),
        execution_profile: profile,
        max_effectful_steps: options.max_effectful_steps,
        max_total_cost_usd_micros: options.max_total_cost_usd_micros,
        observed_at_ms: unix_time_millis(),
    })
}

fn step_input(options: &GroupGraphRunControllerStepOptions) -> StepScheduledGraphControllerInput {
    StepScheduledGraphControllerInput {
        graph_run_id: options.graph_run_id.clone(),
        core_bin_sha256: options.core_bin_sha256.clone(),
        expected_awaiting_event_sha256: options.expected_awaiting_event_sha256.clone(),
        expected_provider_request_id: options.expected_provider_request_id.clone(),
        expected_authorization_sha256: options.expected_authorization_sha256.clone(),
        pricing_source: Arc::new(CliPricingSource(options.pricing_source.clone())),
        confirm_off_machine: options.confirm_off_machine,
        confirm_predecessor_content: options.confirm_predecessor_content,
        cancellation: Cancellation::default(),
        observed_at_ms: unix_time_millis(),
    }
}

async fn execute_step(
    service: ScheduledGraphControllerService,
    input: StepScheduledGraphControllerInput,
    include_result: bool,
) -> Result<ScheduledGraphControllerCliOutput, Box<dyn Error>> {
    let cancel_on_signal = spawn_signal_cancellation(input.cancellation.clone())?;
    let predecessor_content = input.confirm_predecessor_content;
    let result = service.step(&input).await;
    cancel_on_signal.abort();
    let _ = cancel_on_signal.await;
    Ok(ScheduledGraphControllerCliOutput::new(
        result?,
        predecessor_content,
        include_result,
        true,
    ))
}

struct CliPricingSource(String);

impl ScheduledGraphControllerPricingSource for CliPricingSource {
    fn read_pricing_json(&self) -> Result<String, ScheduledGraphControllerPricingSourceError> {
        super::dispatch_execution_adapters::read_pricing(&self.0)
            .map_err(|_| ScheduledGraphControllerPricingSourceError)
    }
}

struct ControllerCoreBridges {
    release: Arc<PinnedScheduledReadyNodeReleaseBridge>,
    materializer: Arc<PinnedScheduledNodeMaterializationBridge>,
    terminal: Arc<PinnedScheduledCoreTerminalBridge>,
}

fn controller_core_bridges(
    core_bin: &str,
    core_bin_sha256: &str,
) -> Result<ControllerCoreBridges, Box<dyn Error>> {
    let path = PathBuf::from(core_bin);
    Ok(ControllerCoreBridges {
        release: Arc::new(PinnedScheduledReadyNodeReleaseBridge::new(
            path.clone(),
            core_bin_sha256.into(),
        )?),
        materializer: Arc::new(PinnedScheduledNodeMaterializationBridge::new(
            path.clone(),
            core_bin_sha256.into(),
        )?),
        terminal: Arc::new(PinnedScheduledCoreTerminalBridge::new(
            path,
            core_bin_sha256.into(),
        )?),
    })
}

fn controller_service(
    store: Arc<SqliteHubStore>,
    database: &std::path::Path,
    bridges: ControllerCoreBridges,
    mode: ControllerMode,
) -> Result<ScheduledGraphControllerService, Box<dyn Error>> {
    match mode {
        ControllerMode::Passive => Ok(passive_controller_service(store, bridges)),
        ControllerMode::Effectful => effectful_controller_service(store, database, bridges),
    }
}

fn passive_controller_service(
    store: Arc<SqliteHubStore>,
    bridges: ControllerCoreBridges,
) -> ScheduledGraphControllerService {
    ScheduledGraphControllerService::new_passive(
        store,
        bridges.release.clone(),
        bridges.release,
        bridges.materializer,
        Arc::new(OpenAiRequestCodec),
        Arc::new(SystemControllerClock),
    )
}

fn effectful_controller_service(
    store: Arc<SqliteHubStore>,
    database: &std::path::Path,
    bridges: ControllerCoreBridges,
) -> Result<ScheduledGraphControllerService, Box<dyn Error>> {
    let owners = super::ready_step_owner::factory(owner_directory(database)?)?;
    let executor = Arc::new(ready_executor(
        store.clone(),
        bridges.release.clone(),
        bridges.terminal,
        owners,
    ));
    Ok(ScheduledGraphControllerService::new(
        store,
        bridges.release.clone(),
        bridges.release,
        bridges.materializer,
        Arc::new(OpenAiRequestCodec),
        executor,
        Arc::new(SystemControllerClock),
    ))
}

struct SystemControllerClock;

impl ScheduledGraphControllerClock for SystemControllerClock {
    fn now_ms(&self) -> u64 {
        unix_time_millis()
    }
}

#[derive(Clone, Copy)]
enum ControllerMode {
    Passive,
    Effectful,
}

fn controller_service_for(
    args: &Args,
    graph_run_id: &str,
    core_bin: &str,
    core_bin_sha256: &str,
    controller_may_be_absent: bool,
    mode: ControllerMode,
) -> Result<ScheduledGraphControllerService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    preflight_controller_pin(
        &database,
        graph_run_id,
        core_bin_sha256,
        controller_may_be_absent,
    )?;
    let bridges = controller_core_bridges(core_bin, core_bin_sha256)?;
    let store = Arc::new(SqliteHubStore::open(database.clone())?);
    controller_service(store, &database, bridges, mode)
}

fn preflight_controller_pin(
    database: &std::path::Path,
    graph_run_id: &str,
    core_bin_sha256: &str,
    controller_may_be_absent: bool,
) -> Result<(), ScheduledGraphControllerServiceError> {
    let exists = match fs::symlink_metadata(database) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => return Err(ScheduledGraphControllerServiceError::StoreUnavailable),
    };
    if !exists {
        return controller_may_be_absent
            .then_some(())
            .ok_or(ScheduledGraphControllerServiceError::StoreUnavailable);
    }
    let store = SqliteHubStore::open_existing_dispatch_inspection_read_only(database)
        .map_err(|error| map_preflight_store_error(&error))?;
    if store
        .inspected_schema_version()
        .map_err(|error| map_preflight_store_error(&error))?
        != CURRENT_SCHEMA_VERSION
    {
        return controller_may_be_absent
            .then_some(())
            .ok_or(ScheduledGraphControllerServiceError::StoreUnavailable);
    }
    match store.inspect_scheduled_graph_controller(graph_run_id) {
        Ok(journal) if journal.header.core_bin_sha256 == core_bin_sha256 => Ok(()),
        Ok(_) => Err(ScheduledGraphControllerServiceError::CorePinMismatch),
        Err(HubStoreError::NotFound { .. }) if controller_may_be_absent => Ok(()),
        Err(error) => Err(map_preflight_store_error(&error)),
    }
}

fn map_preflight_store_error(error: &HubStoreError) -> ScheduledGraphControllerServiceError {
    match error {
        HubStoreError::Conflict { .. } | HubStoreError::Corrupt { .. } => {
            ScheduledGraphControllerServiceError::CorruptEvidence
        }
        HubStoreError::NotFound { .. } | HubStoreError::Unavailable { .. } => {
            ScheduledGraphControllerServiceError::StoreUnavailable
        }
    }
}

fn ready_executor(
    store: Arc<SqliteHubStore>,
    release: Arc<PinnedScheduledReadyNodeReleaseBridge>,
    terminal: Arc<PinnedScheduledCoreTerminalBridge>,
    owners: Arc<dyn GroupAgentScheduledExecutorOwnerFactory>,
) -> GroupAgentScheduledReadyNodeDispatchExecutionService {
    GroupAgentScheduledReadyNodeDispatchExecutionService::new(
        store.clone(),
        store.clone(),
        release.clone(),
        release,
        store,
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
        Arc::new(EnvironmentOpenAiCredentialSource),
        terminal,
        Arc::new(SystemDispatchMetadataSource),
        owners,
    )
}

fn owner_directory(database: &std::path::Path) -> Result<PathBuf, io::Error> {
    database
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.join("scheduled-executor-owners"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Hub path has no state directory",
            )
        })
}

#[cfg(target_os = "linux")]
fn spawn_signal_cancellation(
    cancellation: Cancellation,
) -> Result<tokio::task::JoinHandle<()>, io::Error> {
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    Ok(tokio::spawn(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
        cancellation.cancel();
    }))
}

#[cfg(not(target_os = "linux"))]
fn spawn_signal_cancellation(_: Cancellation) -> Result<tokio::task::JoinHandle<()>, io::Error> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "scheduled Graph controller signal handling is Linux-only",
    ))
}
