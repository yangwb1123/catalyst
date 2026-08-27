use std::{error::Error, io, path::PathBuf, sync::Arc};

use crate::runtime_application::{
    ExecuteGroupAgentScheduledReadyNodeDispatchInput, GroupAgentScheduledExecutorOwnerFactory,
    GroupAgentScheduledReadyNodeDispatchExecutionService,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
};
use forge_runtime_infrastructure::{
    PinnedScheduledCoreTerminalBridge, PinnedScheduledReadyNodeReleaseBridge,
    RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};

use crate::{
    args::{Args, GroupGraphRunReadyStepOptions},
    runtime_domain::{Cancellation, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES},
    state_path::hub_database_path,
};

use super::{
    dispatch_execution_adapters::{
        EnvironmentOpenAiCredentialSource, SystemDispatchMetadataSource,
    },
    ready_step_output::ScheduledReadyNodeStepCliOutput,
};

struct PinnedStepCore {
    release: Arc<PinnedScheduledReadyNodeReleaseBridge>,
    terminal: Arc<PinnedScheduledCoreTerminalBridge>,
}

pub async fn execute(
    args: &Args,
    options: &GroupGraphRunReadyStepOptions,
) -> Result<ScheduledReadyNodeStepCliOutput, Box<dyn Error>> {
    require_off_machine_consent(options)?;
    validate_public_anchors(options)?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let owners = super::ready_step_owner::factory(owner_directory(&database)?)?;
    let core = pin_core(options)?;
    let pricing_json = super::dispatch_execution_adapters::read_pricing(&options.pricing_source)?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = service(store, core, owners);
    let cancellation = Cancellation::default();
    let cancel_on_signal = spawn_signal_cancellation(cancellation.clone())?;
    let input = application_input(options, pricing_json, cancellation);
    let result = Box::pin(service.execute(&input)).await;
    cancel_on_signal.abort();
    let _ = cancel_on_signal.await;
    let result = result?;
    Ok(ScheduledReadyNodeStepCliOutput::from_result(
        result,
        options.confirm_predecessor_content,
        options.include_result,
    ))
}

fn validate_public_anchors(
    options: &GroupGraphRunReadyStepOptions,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchExecutionServiceError> {
    let valid_ids = [&options.graph_run_id, &options.expected_provider_request_id]
        .into_iter()
        .all(|value| valid_identifier(value));
    let digest = &options.expected_ready_authorization_sha256;
    let valid_digest = digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    (valid_ids && valid_digest)
        .then_some(())
        .ok_or(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::InvalidInput)
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}

fn pin_core(options: &GroupGraphRunReadyStepOptions) -> Result<PinnedStepCore, Box<dyn Error>> {
    let path = PathBuf::from(&options.core_bin);
    let digest = options.core_bin_sha256.clone();
    let release = Arc::new(PinnedScheduledReadyNodeReleaseBridge::new(
        path.clone(),
        digest.clone(),
    )?);
    let terminal = Arc::new(PinnedScheduledCoreTerminalBridge::new(path, digest)?);
    Ok(PinnedStepCore { release, terminal })
}

fn require_off_machine_consent(
    options: &GroupGraphRunReadyStepOptions,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchExecutionServiceError> {
    options
        .confirm_off_machine
        .then_some(())
        .ok_or(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ConsentRequired)
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

fn application_input(
    options: &GroupGraphRunReadyStepOptions,
    pricing_json: String,
    cancellation: Cancellation,
) -> ExecuteGroupAgentScheduledReadyNodeDispatchInput {
    ExecuteGroupAgentScheduledReadyNodeDispatchInput {
        graph_run_id: options.graph_run_id.clone(),
        expected_provider_request_id: options.expected_provider_request_id.clone(),
        expected_authorization_sha256: options.expected_ready_authorization_sha256.clone(),
        pricing_json,
        confirm_off_machine: options.confirm_off_machine,
        confirm_predecessor_content: options.confirm_predecessor_content,
        cancellation,
    }
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
        "scheduled ready-node step signal handling is Linux-only",
    ))
}

fn service(
    store: Arc<SqliteHubStore>,
    core: PinnedStepCore,
    owners: Arc<dyn GroupAgentScheduledExecutorOwnerFactory>,
) -> GroupAgentScheduledReadyNodeDispatchExecutionService {
    GroupAgentScheduledReadyNodeDispatchExecutionService::new(
        store.clone(),
        store.clone(),
        core.release.clone(),
        core.release,
        store,
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
        Arc::new(EnvironmentOpenAiCredentialSource),
        core.terminal,
        Arc::new(SystemDispatchMetadataSource),
        owners,
    )
}

#[cfg(test)]
#[path = "ready_step_command_tests.rs"]
mod tests;
