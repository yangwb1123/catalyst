//! Versioned, local-only execution-fabric ABI values.
//!
//! The current adapter deliberately describes only process execution on the
//! current Runtime host. It carries no remote transport or enrollment data.

use serde::{Deserialize, Serialize};

use crate::Capability;

pub const EXECUTION_FABRIC_ABI_VERSION: u16 = 1;
pub const LOCAL_EXECUTION_TARGET_ID: &str = "local";
pub const LOCAL_EXECUTION_ADAPTER_ID: &str = "forge.exec_command.local";
pub const LOCAL_EXECUTION_ADAPTER_VERSION: &str = "1";
pub const ENVIRONMENT_DIGEST_ALGORITHM: &str = "sha256";
pub const MAX_ENVIRONMENT_DIGEST_ENTRIES: u16 = 32;
pub const MAX_ENVIRONMENT_DIGEST_REASON_BYTES: usize = 256;

/// Identifies one call at the existing Runtime `ToolStarted` boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolInvocationRef {
    pub session_id: String,
    pub run_id: String,
    pub tool_call_id: String,
    pub tool_started_sequence: u64,
}

/// The only target type constructible while the Fabric remains OFF.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTargetKind {
    Local,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTargetStatus {
    Ready,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTargetScope {
    CurrentRuntimeOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionTargetRef {
    pub target_id: String,
    pub kind: ExecutionTargetKind,
    /// Implicit in-process namespace, not a globally addressable machine or
    /// Coordinator identity.
    pub scope: ExecutionTargetScope,
}

/// Local adapter declaration. It intentionally has no device identity,
/// capability inventory, or transport fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionTarget {
    pub v: u16,
    pub target_ref: ExecutionTargetRef,
    pub adapter_id: String,
    pub adapter_version: String,
    pub status: ExecutionTargetStatus,
}

impl ExecutionTarget {
    #[must_use]
    pub fn local() -> Self {
        Self {
            v: EXECUTION_FABRIC_ABI_VERSION,
            target_ref: ExecutionTargetRef {
                target_id: LOCAL_EXECUTION_TARGET_ID.into(),
                kind: ExecutionTargetKind::Local,
                scope: ExecutionTargetScope::CurrentRuntimeOnly,
            },
            adapter_id: LOCAL_EXECUTION_ADAPTER_ID.into(),
            adapter_version: LOCAL_EXECUTION_ADAPTER_VERSION.into(),
            status: ExecutionTargetStatus::Ready,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
/// Identifies the one local invocation represented here. It is not a remote
/// retry generation, lease epoch, fencing token, or physical-attempt ID.
pub struct AttemptRef {
    pub session_id: String,
    pub run_id: String,
    /// Sequence of the existing `ToolStarted` event; no second lifecycle is
    /// persisted by this ABI.
    pub tool_started_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementConstraint {
    /// The current ABI can require only the current Runtime's local target.
    pub target_required: ExecutionTargetRef,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EnvironmentDigest {
    Captured {
        algorithm: String,
        sha256: String,
        entry_count: u16,
    },
    NotCaptured {
        reason: String,
    },
}

impl EnvironmentDigest {
    /// Validates the shape of the local environment observation.
    ///
    /// This checks only the bounded ABI representation. It does not prove
    /// that a digest was calculated from a particular host environment.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Captured {
                algorithm,
                sha256,
                entry_count,
            } => {
                algorithm == ENVIRONMENT_DIGEST_ALGORITHM
                    && (1..=MAX_ENVIRONMENT_DIGEST_ENTRIES).contains(entry_count)
                    && sha256.len() == 64
                    && sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            }
            Self::NotCaptured { reason } => {
                !reason.is_empty()
                    && reason.len() <= MAX_ENVIRONMENT_DIGEST_REASON_BYTES
                    && !reason.contains('\0')
            }
        }
    }
}

/// Declared content-addressed metadata. This value does not prove that bytes
/// exist in a CAS or that the digest matches them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub artifact_id: String,
    pub sha256: String,
    pub media_type: String,
    pub size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClassification {
    PotentiallySideEffecting,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEffect {
    pub capability: Capability,
    pub classification: EffectClassification,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mobility {
    Pinned,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvidence {
    pub v: u16,
    pub attempt_ref: AttemptRef,
    pub target_ref: ExecutionTargetRef,
    pub source: ExecutionEvidenceSource,
    pub observation: LocalProcessObservation,
}

impl ExecutionEvidence {
    /// Validates evidence emitted by the current local process adapter.
    ///
    /// The caller supplies the durable Run envelope and rendered output so
    /// this check can bind the observation to the existing `ToolStarted` /
    /// `ToolFinished` lifecycle without introducing a second attempt journal.
    ///
    /// # Errors
    ///
    /// Returns an error when the evidence does not match the supplied Run
    /// envelope, local target, lifecycle sequence, or rendered output.
    pub fn validate_local(
        &self,
        session_id: &str,
        run_id: &str,
        finished_sequence: u64,
        output: &str,
        truncated: bool,
    ) -> Result<(), &'static str> {
        let expected_target = ExecutionTarget::local().target_ref;
        if self.v != EXECUTION_FABRIC_ABI_VERSION {
            return Err("evidence ABI version is unsupported");
        }
        if self.source != ExecutionEvidenceSource::LocalProcessObservation {
            return Err("evidence source is not local process observation");
        }
        if self.attempt_ref.session_id != session_id || self.attempt_ref.run_id != run_id {
            return Err("evidence attempt identity does not match the Run");
        }
        if self.attempt_ref.tool_started_sequence.checked_add(1) != Some(finished_sequence) {
            return Err("evidence does not bind to the adjacent ToolStarted event");
        }
        if self.target_ref != expected_target {
            return Err("evidence target is not the current local target");
        }
        if self.observation.output_truncated != truncated {
            return Err("evidence truncation does not match ToolFinished");
        }
        if u64::try_from(output.len()).ok() != Some(self.observation.rendered_output_bytes) {
            return Err("evidence output byte count does not match ToolFinished");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEvidenceSource {
    LocalProcessObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalProcessObservation {
    /// `None` means the OS did not report a normal exit code.
    pub exit_code: Option<i32>,
    pub rendered_output_bytes: u64,
    pub output_truncated: bool,
}

/// Immutable declaration used to route a process invocation through the local
/// target while preserving the existing event journal as lifecycle authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionAttempt {
    pub v: u16,
    pub attempt_ref: AttemptRef,
    pub tool_invocation: ToolInvocationRef,
    pub target_ref: ExecutionTargetRef,
    pub placement_constraint: PlacementConstraint,
    pub environment_digest: EnvironmentDigest,
    pub input_artifacts: Vec<ArtifactRef>,
    pub output_artifacts: Vec<ArtifactRef>,
    pub effect: ExecutionEffect,
    pub mobility: Mobility,
}

impl ExecutionAttempt {
    #[must_use]
    pub fn local_process(tool_invocation: ToolInvocationRef) -> Self {
        let target = ExecutionTarget::local().target_ref;
        Self {
            v: EXECUTION_FABRIC_ABI_VERSION,
            attempt_ref: AttemptRef {
                session_id: tool_invocation.session_id.clone(),
                run_id: tool_invocation.run_id.clone(),
                tool_started_sequence: tool_invocation.tool_started_sequence,
            },
            tool_invocation,
            target_ref: target.clone(),
            placement_constraint: PlacementConstraint {
                target_required: target,
            },
            environment_digest: EnvironmentDigest::NotCaptured {
                reason: "the current local process runner does not capture a complete environment digest".into(),
            },
            input_artifacts: Vec::new(),
            output_artifacts: Vec::new(),
            effect: ExecutionEffect {
                capability: Capability::Process,
                classification: EffectClassification::PotentiallySideEffecting,
            },
            mobility: Mobility::Pinned,
        }
    }

    #[must_use]
    pub fn local_process_with_environment(
        tool_invocation: ToolInvocationRef,
        environment_digest: EnvironmentDigest,
    ) -> Self {
        let mut attempt = Self::local_process(tool_invocation);
        attempt.environment_digest = environment_digest;
        attempt
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactRef, AttemptRef, EnvironmentDigest, ExecutionAttempt, ExecutionEffect,
        ExecutionEvidence, ExecutionEvidenceSource, ExecutionTarget, ExecutionTargetRef,
        ExecutionTargetScope, ExecutionTargetStatus, LocalProcessObservation, Mobility,
        PlacementConstraint, ToolInvocationRef,
    };
    use crate::Capability;

    #[test]
    fn local_target_and_attempt_match_the_frozen_fixture() {
        let target = ExecutionTarget::local();
        let attempt = ExecutionAttempt::local_process(ToolInvocationRef {
            session_id: "session-1".into(),
            run_id: "run-1".into(),
            tool_call_id: "call-1".into(),
            tool_started_sequence: 4,
        });
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/local_execution_attempt.v1.json"
        ))
        .expect("fixture parses");
        let evidence = ExecutionEvidence {
            v: super::EXECUTION_FABRIC_ABI_VERSION,
            attempt_ref: attempt.attempt_ref.clone(),
            target_ref: attempt.target_ref.clone(),
            source: ExecutionEvidenceSource::LocalProcessObservation,
            observation: LocalProcessObservation {
                exit_code: Some(0),
                rendered_output_bytes: 30,
                output_truncated: false,
            },
        };
        let artifact_ref = ArtifactRef {
            artifact_id: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            media_type: "application/octet-stream".into(),
            size_bytes: 16,
        };
        let actual = serde_json::json!({
            "target": target,
            "attempt": attempt,
            "evidence": evidence,
            "artifact_ref": artifact_ref,
        });
        assert_eq!(actual, fixture);
    }

    #[test]
    fn attempt_is_pinned_local_and_does_not_claim_missing_evidence() {
        let attempt = ExecutionAttempt::local_process(ToolInvocationRef {
            session_id: "session-1".into(),
            run_id: "run-1".into(),
            tool_call_id: "provider-call-7".into(),
            tool_started_sequence: 8,
        });

        assert_eq!(attempt.target_ref.target_id, "local");
        assert_eq!(
            attempt.placement_constraint,
            PlacementConstraint {
                target_required: ExecutionTargetRef {
                    target_id: "local".into(),
                    kind: super::ExecutionTargetKind::Local,
                    scope: ExecutionTargetScope::CurrentRuntimeOnly,
                },
            }
        );
        assert!(matches!(
            attempt.environment_digest,
            EnvironmentDigest::NotCaptured { .. }
        ));
        assert!(attempt.input_artifacts.is_empty());
        assert!(attempt.output_artifacts.is_empty());
        assert_eq!(
            attempt.effect,
            ExecutionEffect {
                capability: Capability::Process,
                classification: super::EffectClassification::PotentiallySideEffecting,
            }
        );
        assert_eq!(attempt.mobility, Mobility::Pinned);
    }

    #[test]
    fn target_has_no_remote_transport_or_identity_projection() {
        let target = ExecutionTarget::local();
        assert_eq!(target.status, ExecutionTargetStatus::Ready);
        assert_eq!(
            target.target_ref.scope,
            ExecutionTargetScope::CurrentRuntimeOnly
        );
        let value = serde_json::to_value(target).expect("target serializes");
        assert!(value.get("transport").is_none());
        assert!(value.get("identity_ref").is_none());
        assert!(value.get("capability_snapshot_ref").is_none());
    }

    #[test]
    fn attempt_reference_uses_existing_started_event_sequence() {
        let invocation = ToolInvocationRef {
            session_id: "s".into(),
            run_id: "r".into(),
            tool_call_id: "call".into(),
            tool_started_sequence: 19,
        };
        let attempt = ExecutionAttempt::local_process(invocation.clone());
        assert_eq!(
            attempt.attempt_ref,
            AttemptRef {
                session_id: "s".into(),
                run_id: "r".into(),
                tool_started_sequence: 19,
            }
        );
        assert_eq!(attempt.tool_invocation, invocation);
    }

    #[test]
    fn local_attempt_can_bind_a_non_secret_environment_digest() {
        let attempt = ExecutionAttempt::local_process_with_environment(
            ToolInvocationRef {
                session_id: "s".into(),
                run_id: "r".into(),
                tool_call_id: "call".into(),
                tool_started_sequence: 19,
            },
            EnvironmentDigest::Captured {
                algorithm: super::ENVIRONMENT_DIGEST_ALGORITHM.into(),
                sha256: "a".repeat(64),
                entry_count: 2,
            },
        );
        assert!(matches!(
            attempt.environment_digest,
            EnvironmentDigest::Captured { entry_count: 2, .. }
        ));
        assert!(attempt.input_artifacts.is_empty());
        assert!(attempt.output_artifacts.is_empty());
        assert_eq!(attempt.mobility, Mobility::Pinned);
    }

    #[test]
    fn local_evidence_validator_binds_the_existing_tool_lifecycle() {
        let mut evidence = ExecutionEvidence {
            v: super::EXECUTION_FABRIC_ABI_VERSION,
            attempt_ref: AttemptRef {
                session_id: "session-1".into(),
                run_id: "run-1".into(),
                tool_started_sequence: 4,
            },
            target_ref: ExecutionTarget::local().target_ref,
            source: ExecutionEvidenceSource::LocalProcessObservation,
            observation: LocalProcessObservation {
                exit_code: Some(0),
                rendered_output_bytes: 2,
                output_truncated: false,
            },
        };
        evidence
            .validate_local("session-1", "run-1", 5, "ok", false)
            .expect("matching local evidence is valid");

        evidence.observation.rendered_output_bytes = 3;
        assert!(
            evidence
                .validate_local("session-1", "run-1", 5, "ok", false)
                .is_err()
        );
    }
}
