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
