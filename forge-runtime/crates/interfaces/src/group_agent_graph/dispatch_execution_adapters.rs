use std::{
    error::Error,
    fmt::Write as _,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use forge_runtime_application::{
    GroupAgentNodeCredentialSource, GroupAgentNodeCredentialSourceError,
    GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchExecutionServiceError,
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchMetadataSourceError,
};
use forge_runtime_infrastructure::RegisteredGroupAgentNodeProviderFactory;
use rand::TryRngCore;

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
        Self::from_inspection(inspection, performed, include_result)
    }

    fn from_inspection(
        inspection: GroupAgentNodeLifecycleInspection,
        performed: bool,
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
            dispatch_performed_this_invocation: performed,
            database_written_this_invocation: performed,
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

#[cfg(test)]
#[path = "dispatch_execution_output_tests.rs"]
mod tests;
