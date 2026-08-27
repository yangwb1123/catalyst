#![allow(clippy::missing_errors_doc, clippy::wildcard_imports)]

use super::*;

impl GroupAgentScheduledNodeDispatchClaim {
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_claim(self)
    }

    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        canonical_json(self)
    }

    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        claim_digest(self)
    }
}

impl GroupAgentScheduledNodeActiveLane {
    pub fn validate_against_claim(
        &self,
        claim: &GroupAgentScheduledNodeDispatchClaim,
    ) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_active_lane(self, claim)
    }

    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        canonical_json(self)
    }
}

impl GroupAgentScheduledNodeDispatchClaimEvent {
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_claim_event(self)
    }

    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        canonical_json(self)
    }

    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        claim_event_digest(self)
    }
}

impl ClaimGroupAgentScheduledNodeDispatch {
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_claim_request(self)
    }
}

impl AdjudicateGroupAgentScheduledNodeDispatch {
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_adjudication_request(self)
    }
}

impl GroupAgentScheduledNodeDispatchAuthority {
    pub fn new(
        request: &GroupAgentScheduledNodeProviderRequestRecord,
        claim: GroupAgentScheduledNodeDispatchClaim,
        body: Vec<u8>,
    ) -> Result<Self, GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_dispatch_authority(request, &claim, &body)?;
        Ok(Self {
            claim,
            request_body: body,
        })
    }

    #[must_use]
    pub const fn claim(&self) -> &GroupAgentScheduledNodeDispatchClaim {
        &self.claim
    }

    #[must_use]
    pub fn into_parts(self) -> (GroupAgentScheduledNodeDispatchClaim, Vec<u8>) {
        (self.claim, self.request_body)
    }
}

impl GroupAgentScheduledNodeTerminalArtifact {
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_artifact(self)
    }

    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        canonical_json(self)
    }

    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        artifact_digest(self)
    }
}

impl GroupAgentScheduledNodeTerminalControl {
    /// Strictly decodes one exact compact canonical terminal control.
    pub fn decode_exact(
        bytes: &[u8],
    ) -> Result<Self, GroupAgentScheduledNodeLifecycleValidationError> {
        validation::decode_exact_control(bytes)
    }

    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_control(self)
    }

    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        canonical_json(self)
    }

    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        control_digest(self)
    }
}

impl GroupAgentScheduledNodeTerminalReceipt {
    /// Strictly decodes one exact compact canonical Core receipt.
    pub fn decode_exact(
        bytes: &[u8],
    ) -> Result<Self, GroupAgentScheduledNodeLifecycleValidationError> {
        validation::decode_exact_receipt(bytes)
    }

    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_receipt(self)
    }

    pub fn validate_against_control(
        &self,
        control: &GroupAgentScheduledNodeTerminalControl,
    ) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_receipt_against_control(self, control)
    }

    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        canonical_json(self)
    }

    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
        receipt_digest(self)
    }
}

impl GroupAgentScheduledNodeLifecycleInspection {
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
        validation::validate_inspection(self)
    }
}

impl std::fmt::Display for GroupAgentScheduledNodeLifecycleValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentScheduledNodeLifecycleValidationError {}

#[must_use]
pub fn group_agent_scheduled_node_terminal_artifact_id(sha256: &str) -> String {
    format!("scheduled-node-terminal-artifact-{sha256}")
}

#[must_use]
pub fn group_agent_scheduled_node_terminal_receipt_id(sha256: &str) -> String {
    format!("scheduled-node-terminal-receipt-{sha256}")
}

#[must_use]
pub fn group_agent_scheduled_node_terminal_output_sha256(output: &str) -> String {
    digest_hex(
        GROUP_AGENT_SCHEDULED_NODE_OUTPUT_DIGEST_DOMAIN,
        output.as_bytes(),
    )
}
