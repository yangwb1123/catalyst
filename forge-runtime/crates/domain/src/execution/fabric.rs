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
    NotCaptured { reason: String },
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
}
