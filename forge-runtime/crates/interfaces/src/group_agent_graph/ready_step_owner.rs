use std::{io, path::PathBuf, sync::Arc};

use crate::runtime_application::GroupAgentScheduledExecutorOwnerFactory;

#[cfg(target_os = "linux")]
use crate::runtime_application::{
    GroupAgentScheduledExecutorOwner, GroupAgentScheduledExecutorOwnerError,
};

#[cfg(target_os = "linux")]
use super::scheduled_executor_sidecar::ScheduledExecutorSidecar;

pub(super) fn factory(
    directory: PathBuf,
) -> Result<Arc<dyn GroupAgentScheduledExecutorOwnerFactory>, io::Error> {
    platform_factory(directory)
}

#[cfg(target_os = "linux")]
fn platform_factory(
    directory: PathBuf,
) -> Result<Arc<dyn GroupAgentScheduledExecutorOwnerFactory>, io::Error> {
    if directory.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scheduled executor owner directory is invalid",
        ));
    }
    Ok(Arc::new(LinuxOwnerFactory { directory }))
}

#[cfg(not(target_os = "linux"))]
fn platform_factory(
    _: PathBuf,
) -> Result<Arc<dyn GroupAgentScheduledExecutorOwnerFactory>, io::Error> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "scheduled ready-node step execution is Linux-only",
    ))
}

#[cfg(target_os = "linux")]
struct LinuxOwnerFactory {
    directory: PathBuf,
}

#[cfg(target_os = "linux")]
struct LinuxOwner {
    sidecar: ScheduledExecutorSidecar,
}

#[cfg(target_os = "linux")]
impl GroupAgentScheduledExecutorOwnerFactory for LinuxOwnerFactory {
    fn create(
        &self,
        provider_request_id: &str,
        lane_ownership_id: &str,
    ) -> Result<Box<dyn GroupAgentScheduledExecutorOwner>, GroupAgentScheduledExecutorOwnerError>
    {
        let sidecar = ScheduledExecutorSidecar::create(
            &self.directory,
            provider_request_id,
            lane_ownership_id,
        )
        .map_err(|_| GroupAgentScheduledExecutorOwnerError)?;
        Ok(Box::new(LinuxOwner { sidecar }))
    }
}

#[cfg(target_os = "linux")]
impl GroupAgentScheduledExecutorOwner for LinuxOwner {
    fn preserve_on_drop(&mut self) {
        self.sidecar.preserve_on_drop();
    }

    fn cleanup(self: Box<Self>) -> Result<(), GroupAgentScheduledExecutorOwnerError> {
        let Self { sidecar } = *self;
        sidecar
            .cleanup()
            .map_err(|_| GroupAgentScheduledExecutorOwnerError)
    }
}
