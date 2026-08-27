#![allow(clippy::missing_errors_doc)]

use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScheduledExecutorLiveness {
    Live,
    Dead,
    PidReused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScheduledExecutorSidecarError;

pub(crate) struct ScheduledExecutorSidecar;

impl ScheduledExecutorSidecar {
    pub(crate) fn create(
        _: &Path,
        _: &str,
        _: &str,
    ) -> Result<Self, ScheduledExecutorSidecarError> {
        Err(ScheduledExecutorSidecarError)
    }

    pub(crate) fn open(_: &Path, _: &str, _: &str) -> Result<Self, ScheduledExecutorSidecarError> {
        Err(ScheduledExecutorSidecarError)
    }

    pub(crate) fn liveness(
        &self,
    ) -> Result<ScheduledExecutorLiveness, ScheduledExecutorSidecarError> {
        Err(ScheduledExecutorSidecarError)
    }

    pub(crate) const fn preserve_on_drop(&mut self) {}

    pub(crate) fn cleanup(self) -> Result<(), ScheduledExecutorSidecarError> {
        Err(ScheduledExecutorSidecarError)
    }
}

impl std::fmt::Display for ScheduledExecutorSidecarError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("scheduled executor sidecars are unsupported on this platform")
    }
}

impl std::error::Error for ScheduledExecutorSidecarError {}
