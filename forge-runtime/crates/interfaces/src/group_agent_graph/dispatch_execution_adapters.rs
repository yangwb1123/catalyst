use std::{
    error::Error,
    fmt::Write as _,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use std::{fs::File, io::Read, path::PathBuf};

use forge_runtime_application::{
    ExecuteGroupAgentNodeDispatchInput, GroupAgentNodeCredentialSource,
    GroupAgentNodeCredentialSourceError, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchExecutionService, GroupAgentNodeDispatchExecutionServiceError,
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchMetadataSourceError,
    GroupAgentNodeDispatchReadinessService, GroupAgentNodeDispatchReleaseControlService,
    MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES, validate_group_agent_node_dispatch_topology,
};
use forge_runtime_infrastructure::{PinnedCoreTerminalBridge, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore};
use rand::TryRngCore;

use crate::{
    args::Args,
    openai_prepared_dispatch::OpenAiRequestCodec,
    runtime_domain::Cancellation,
    state_path::hub_database_path,
};

use super::dispatch_command::{DispatchInputs, GroupAgentGraphRunDispatchCommandCliOutput};

use crate::runtime_domain::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchProviderFactory,
    GroupAgentNodeDispatchProviderFactoryError, GroupAgentNodePricingSnapshot,
    GroupAgentNodeResolvedDispatch, PreparedModelProvider,
};

pub(super) struct EnvironmentOpenAiCredentialSource;

pub(super) struct SystemDispatchMetadataSource;

pub(super) struct PreparedDispatchDependencies {
    pub(super) providers: Arc<dyn GroupAgentNodeDispatchProviderFactory>,
    pub(super) credentials: Arc<dyn GroupAgentNodeCredentialSource>,
}

struct CachedCredentialSource {
    credential: Arc<str>,
}

struct PrebuiltProviderFactory {
    resolved: GroupAgentNodeResolvedDispatch,
    credential: Arc<str>,
    provider: Mutex<Option<Box<dyn PreparedModelProvider>>>,
}

impl PreparedDispatchDependencies {
    pub(super) fn prepare(
        authorization_json: &str,
        pricing_json: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let authorization =
            GroupAgentNodeDispatchAuthorization::decode_exact(authorization_json)
                .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidInput)?;
        let pricing = GroupAgentNodePricingSnapshot::decode_exact(pricing_json)
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::InvalidInput)?;
        let credential: Arc<str> = EnvironmentOpenAiCredentialSource
            .read_credential()
            .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::CredentialUnavailable)?
            .into();
        let registered = RegisteredGroupAgentNodeProviderFactory::new();
        let resolved =
            GroupAgentNodeDispatchProviderFactory::resolve(&registered, &authorization, &pricing)
                .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable)?;
        let provider = GroupAgentNodeDispatchProviderFactory::build(
            &registered,
            resolved.clone(),
            credential.to_string(),
        )
        .map_err(|_| GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable)?;
        Ok(Self {
            providers: Arc::new(PrebuiltProviderFactory {
                resolved,
                credential: credential.clone(),
                provider: Mutex::new(Some(provider)),
            }),
            credentials: Arc::new(CachedCredentialSource { credential }),
        })
    }
}

impl GroupAgentNodeCredentialSource for EnvironmentOpenAiCredentialSource {
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError> {
        std::env::var_os("OPENAI_API_KEY")
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty())
            .ok_or(GroupAgentNodeCredentialSourceError)
    }
}

impl GroupAgentNodeCredentialSource for CachedCredentialSource {
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError> {
        Ok(self.credential.to_string())
    }
}

impl GroupAgentNodeDispatchProviderFactory for PrebuiltProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodeResolvedDispatch, GroupAgentNodeDispatchProviderFactoryError> {
        let resolved = GroupAgentNodeDispatchProviderFactory::resolve(
            &RegisteredGroupAgentNodeProviderFactory::new(),
            authorization,
            pricing,
        )?;
        (resolved == self.resolved)
            .then_some(resolved)
            .ok_or_else(provider_unavailable)
    }

    fn build(
        &self,
        resolved: GroupAgentNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentNodeDispatchProviderFactoryError> {
        if resolved != self.resolved || credential.as_str() != self.credential.as_ref() {
            return Err(provider_unavailable());
        }
        self.provider
            .lock()
            .map_err(|_| provider_unavailable())?
            .take()
            .ok_or_else(provider_unavailable)
    }
}

impl GroupAgentNodeDispatchMetadataSource for SystemDispatchMetadataSource {
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>
    {
        let (dispatch_random, lane_random) = claim_randomness()?;
        Ok(GroupAgentNodeDispatchClaimMetadata {
            dispatch_id: random_id("graph-node-dispatch", dispatch_random),
            lane_ownership_id: random_id("graph-node-lane", lane_random),
            released_at_ms: now_ms()?,
        })
    }

    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
        now_ms()
    }
}

fn claim_randomness() -> Result<([u8; 16], [u8; 16]), GroupAgentNodeDispatchMetadataSourceError> {
    let mut dispatch = [0_u8; 16];
    let mut lane = [0_u8; 16];
    let mut source = rand::rngs::OsRng;
    source
        .try_fill_bytes(&mut dispatch)
        .and_then(|()| source.try_fill_bytes(&mut lane))
        .map_err(|_| GroupAgentNodeDispatchMetadataSourceError)?;
    Ok((dispatch, lane))
}

fn random_id(prefix: &str, bytes: [u8; 16]) -> String {
    let mut value = String::with_capacity(prefix.len() + 1 + bytes.len() * 2);
    value.push_str(prefix);
    value.push('-');
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn now_ms() -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
    let milliseconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| GroupAgentNodeDispatchMetadataSourceError)?
        .as_millis();
    u64::try_from(milliseconds)
        .ok()
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or(GroupAgentNodeDispatchMetadataSourceError)
}

fn provider_unavailable() -> GroupAgentNodeDispatchProviderFactoryError {
    GroupAgentNodeDispatchProviderFactoryError {
        message: "registered Group Agent Node provider is unavailable".into(),
    }
}

use std::io::{self, Write};

use crate::runtime_domain::{
    GroupAgentGraphRunStatus, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeTerminalClassification,
};
use forge_runtime_application::ExecuteGroupAgentNodeDispatchResult;
use serde::Serialize;

use crate::group_context_output::terminal_text;

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeDispatchExecutionCliOutput {
    pub v: u16,
    pub r#type: &'static str,
    pub graph_run_id: String,
    pub dispatch_id: String,
    pub node_id: String,
    pub graph_status: GroupAgentGraphRunStatus,
    pub classification: Option<GroupAgentNodeTerminalClassification>,
    pub provider_poll_started: bool,
    pub terminal_seen: bool,
    pub stream_eof_seen: bool,
    pub lane_active: bool,
    pub retry_authorized: bool,
    pub dispatch_performed_this_invocation: bool,
    pub database_written_this_invocation: bool,
    pub metadata_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
}

impl GroupAgentNodeDispatchExecutionCliOutput {
    pub fn from_result(result: ExecuteGroupAgentNodeDispatchResult, include_result: bool) -> Self {
        let (inspection, performed) = match result {
            ExecuteGroupAgentNodeDispatchResult::Terminalized(inspection) => (inspection, true),
            ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection) => (inspection, false),
        };
        Self::from_inspection(inspection, performed, performed, include_result)
    }

    /// Adjudication output: the same inspection JSON as a terminalized
    /// execution. `dispatch_performed_this_invocation` is false (no provider
    /// request was ever sent) and `database_written_this_invocation` is true
    /// (the terminalize CAS committed); no result text is ever produced.
    pub fn from_adjudicated(inspection: GroupAgentNodeLifecycleInspection) -> Self {
        Self::from_inspection(inspection, false, true, false)
    }

    fn from_inspection(
        inspection: GroupAgentNodeLifecycleInspection,
        dispatch_performed: bool,
        database_written: bool,
        include_result: bool,
    ) -> Self {
        let artifact = inspection.artifact.as_ref();
        let result_text = include_result
            .then(|| artifact.map(|value| value.output_text.clone()))
            .flatten();
        Self {
            v: 1,
            r#type: "group_agent_node_dispatch_execution",
            graph_run_id: inspection.graph_run.run.graph_run_id,
            dispatch_id: inspection.claim.dispatch_id,
            node_id: inspection.claim.node_id,
            graph_status: inspection.graph_run.run.status,
            classification: artifact.map(|value| value.classification),
            provider_poll_started: artifact.is_some_and(|value| value.provider_poll_started),
            terminal_seen: artifact.is_some_and(|value| value.terminal_seen),
            stream_eof_seen: artifact.is_some_and(|value| value.stream_eof_seen),
            lane_active: inspection.active_lane.is_some(),
            retry_authorized: false,
            dispatch_performed_this_invocation: dispatch_performed,
            database_written_this_invocation: database_written,
            metadata_only: result_text.is_none(),
            result_text,
        }
    }
}

pub fn write_output(
    output: &GroupAgentNodeDispatchExecutionCliOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        serde_json::to_writer(&mut *writer, output)?;
        writeln!(writer)
    } else {
        writeln!(
            writer,
            "graph dispatch {} · graph_run={} · node={} · dispatch={} · lane_active={} · retry=false",
            status_text(output.graph_status),
            terminal_text(&output.graph_run_id),
            terminal_text(&output.node_id),
            terminal_text(&output.dispatch_id),
            output.lane_active,
        )?;
        if let Some(result) = &output.result_text {
            writeln!(writer, "result: {}", terminal_text(result))?;
        }
        Ok(())
    }
}

fn status_text(status: GroupAgentGraphRunStatus) -> &'static str {
    match status {
        GroupAgentGraphRunStatus::AwaitingExecutionContract => "awaiting_execution_contract",
        GroupAgentGraphRunStatus::AwaitingCoreDispatch => "awaiting_core_dispatch",
        GroupAgentGraphRunStatus::AwaitingDispatchAuthorization => {
            "awaiting_dispatch_authorization"
        }
        GroupAgentGraphRunStatus::DispatchUnknown => "dispatch_unknown",
        GroupAgentGraphRunStatus::Completed => "completed",
        GroupAgentGraphRunStatus::Failed => "failed",
        GroupAgentGraphRunStatus::FailedUncertain => "failed_uncertain",
    }
}


pub(super) async fn execute_dispatch(
    args: &Args,
    graph_run_id: &str,
    inputs: &DispatchInputs,
    core_bin: &str,
    core_bin_sha256: &str,
    confirm_off_machine: bool,
    include_result: bool,
) -> Result<GroupAgentGraphRunDispatchCommandCliOutput, Box<dyn Error>> {
    validate_execute_preflight(
        args,
        graph_run_id,
        inputs.authorization(),
        inputs.pricing(),
        confirm_off_machine,
    )?;
    let bridge = Arc::new(PinnedCoreTerminalBridge::new(
        PathBuf::from(core_bin),
        core_bin_sha256.into(),
    )?);
    let dependencies =
        PreparedDispatchDependencies::prepare(inputs.authorization(), inputs.pricing())?;
    let service = execution_service(args, bridge, dependencies)?;
    let result = service
        .execute(&ExecuteGroupAgentNodeDispatchInput {
            graph_run_id: graph_run_id.into(),
            authorization_json: inputs.authorization().to_owned(),
            pricing_json: inputs.pricing().to_owned(),
            confirm_off_machine,
            cancellation: Cancellation::default(),
        })
        .await?;
    Ok(GroupAgentGraphRunDispatchCommandCliOutput::Execution(
        Box::new(GroupAgentNodeDispatchExecutionCliOutput::from_result(
            result,
            include_result,
        )),
    ))
}

fn execution_service(
    args: &Args,
    bridge: Arc<PinnedCoreTerminalBridge>,
    dependencies: PreparedDispatchDependencies,
) -> Result<GroupAgentNodeDispatchExecutionService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    Ok(GroupAgentNodeDispatchExecutionService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        dependencies.providers,
        dependencies.credentials,
        bridge,
        Arc::new(SystemDispatchMetadataSource),
    ))
}

fn validate_execute_preflight(
    args: &Args,
    graph_run_id: &str,
    authorization_json: &str,
    pricing_json: &str,
    confirm_off_machine: bool,
) -> Result<(), Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_dispatch_preflight_read_only(
        database,
    )?);
    let service = GroupAgentNodeDispatchReleaseControlService::new(
        store.clone(),
        store.clone(),
        Arc::new(OpenAiRequestCodec),
    );
    let exported = service.export(graph_run_id)?;
    validate_group_agent_node_dispatch_topology(&exported.release_control)?;
    if !confirm_off_machine {
        return Err(GroupAgentNodeDispatchExecutionServiceError::ConsentRequired.into());
    }
    GroupAgentNodeDispatchReadinessService::new(
        store.clone(),
        store,
        Arc::new(OpenAiRequestCodec),
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    )
    .verify(graph_run_id, authorization_json, pricing_json)?;
    Ok(())
}


/// Reads one bounded exact UTF-8 dispatch authorization artifact.
pub(super) fn read_authorization(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_authorization_bounded(io::stdin().lock())?
    } else {
        read_authorization_bounded(File::open(source)?)?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node Dispatch Authorization must be UTF-8").into())
}

/// Reads one bounded exact UTF-8 pricing snapshot artifact.
pub(super) fn read_pricing(source: &str) -> Result<String, Box<dyn Error>> {
    let bytes = if source == "-" {
        read_pricing_bounded(io::stdin().lock())?
    } else {
        read_pricing_bounded(File::open(source)?)?
    };
    String::from_utf8(bytes)
        .map_err(|_| invalid_input("Node pricing snapshot must be UTF-8").into())
}

pub(super) fn read_authorization_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES
        .checked_add(1)
        .expect("Node Dispatch Authorization bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("Node Dispatch Authorization bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES {
        return Err(invalid_input(
            "Node Dispatch Authorization exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

pub(super) fn read_pricing_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    read_bounded_artifact(
        reader,
        crate::runtime_domain::MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
        "Node pricing snapshot exceeds its byte limit",
    )
}

fn read_bounded_artifact(
    reader: impl Read,
    maximum: usize,
    message: &str,
) -> Result<Vec<u8>, io::Error> {
    let limit = maximum.checked_add(1).expect("artifact bound fits usize");
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).expect("artifact bound fits u64"))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid_input(message));
    }
    Ok(bytes)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "dispatch_execution_output_tests.rs"]
mod tests;
