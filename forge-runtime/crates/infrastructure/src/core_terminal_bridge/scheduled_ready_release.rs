use std::path::PathBuf;

use crate::runtime_domain::{
    GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseControl,
    MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
    MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES, MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
    ScheduledGraphProgressSnapshot, ScheduledGraphReconcileDecision, ScheduledGraphReconcilePort,
    ScheduledGraphReconcilePortError, ScheduledReadyNodeReleasePort,
    ScheduledReadyNodeReleasePortError,
};

use super::{
    CORE_DECISION_TIMEOUT, CORE_PREFLIGHT_TIMEOUT, CoreTerminalBridgeError,
    PinnedCoreTerminalBridge, invalid, validate_digest,
};

#[derive(Clone, Debug)]
pub struct PinnedScheduledReadyNodeReleaseBridge {
    inner: PinnedCoreTerminalBridge,
}

impl PinnedScheduledReadyNodeReleaseBridge {
    /// Handshakes both required protocols before any private input is sent.
    pub fn new(path: PathBuf, sha256: String) -> Result<Self, CoreTerminalBridgeError> {
        validate_digest(&sha256)?;
        let inner = PinnedCoreTerminalBridge { path, sha256 };
        handshake(
            &inner,
            &["graph-scheduled-reconcile", "--protocol-version"],
            b"1",
        )?;
        handshake(
            &inner,
            &[
                "graph-scheduled-ready-node-dispatch-authorize",
                "--protocol-version",
            ],
            b"2",
        )?;
        Ok(Self { inner })
    }

    fn reconcile_json(&self, snapshot: &[u8]) -> Result<Vec<u8>, CoreTerminalBridgeError> {
        if !(1..=MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES).contains(&snapshot.len()) {
            return Err(invalid(
                "scheduled progress snapshot is outside its byte bound",
            ));
        }
        let output = self.inner.invoke(
            &["graph-scheduled-reconcile", "--snapshot", "-"],
            snapshot,
            CORE_DECISION_TIMEOUT,
        )?;
        bounded_output(
            output,
            MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
            "scheduled reconcile decision exceeds its byte bound",
        )
    }

    fn authorize_json(&self, control: &[u8]) -> Result<Vec<u8>, CoreTerminalBridgeError> {
        if !(1..=MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES)
            .contains(&control.len())
        {
            return Err(invalid(
                "scheduled ready release control is outside its byte bound",
            ));
        }
        let output = self.inner.invoke_with_stdout_limit(
            &[
                "graph-scheduled-ready-node-dispatch-authorize",
                "--control",
                "-",
            ],
            control,
            CORE_DECISION_TIMEOUT,
            MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES,
        )?;
        bounded_output(
            output,
            MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES,
            "scheduled ready authorization exceeds its byte bound",
        )
    }
}

fn handshake(
    inner: &PinnedCoreTerminalBridge,
    args: &[&str],
    expected: &[u8],
) -> Result<(), CoreTerminalBridgeError> {
    let output = inner.invoke(args, b"", CORE_PREFLIGHT_TIMEOUT)?;
    (output == expected)
        .then_some(())
        .ok_or_else(|| invalid("Core scheduled ready release handshake failed"))
}

fn bounded_output(
    output: Vec<u8>,
    maximum: usize,
    message: &str,
) -> Result<Vec<u8>, CoreTerminalBridgeError> {
    (output.len() <= maximum)
        .then_some(output)
        .ok_or_else(|| invalid(message))
}

impl ScheduledGraphReconcilePort for PinnedScheduledReadyNodeReleaseBridge {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        snapshot
            .validate()
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        let snapshot_json = snapshot
            .canonical_json()
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        let output = self
            .reconcile_json(snapshot_json.as_bytes())
            .map_err(|_| ScheduledGraphReconcilePortError::Unavailable)?;
        let decision = ScheduledGraphReconcileDecision::decode_exact_bytes(&output)
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        decision
            .validate_against_snapshot(snapshot)
            .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)?;
        Ok(decision)
    }
}

impl ScheduledReadyNodeReleasePort for PinnedScheduledReadyNodeReleaseBridge {
    fn authorize(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<GroupAgentScheduledReadyNodeDispatchAuthorization, ScheduledReadyNodeReleasePortError>
    {
        control
            .validate()
            .map_err(|_| ScheduledReadyNodeReleasePortError::InvalidAuthorization)?;
        let control_json = control
            .canonical_json()
            .map_err(|_| ScheduledReadyNodeReleasePortError::InvalidAuthorization)?;
        let output = self
            .authorize_json(control_json.as_bytes())
            .map_err(|_| ScheduledReadyNodeReleasePortError::Unavailable)?;
        let json = std::str::from_utf8(&output)
            .map_err(|_| ScheduledReadyNodeReleasePortError::InvalidAuthorization)?;
        let authorization =
            GroupAgentScheduledReadyNodeDispatchAuthorization::decode_exact(json)
                .map_err(|_| ScheduledReadyNodeReleasePortError::InvalidAuthorization)?;
        authorization
            .validate_against_release_control(control)
            .map_err(|_| ScheduledReadyNodeReleasePortError::InvalidAuthorization)?;
        Ok(authorization)
    }
}
