#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
SNAPLINK_CONSOLE_ROOT="${SNAPLINK_CONSOLE_ROOT:-$(dirname -- "$REPO_ROOT")/workspace/demo/snaplink-console}"
SNAPLINK_ROOT="${SNAPLINK_ROOT:-$(dirname -- "$REPO_ROOT")/workspace/demo/snaplink}"
AERO_ID_ROOT="${AERO_ID_ROOT:-$(dirname -- "$REPO_ROOT")/aero-id}"
AERO_IM_ROOT="${AERO_IM_ROOT:-$(dirname -- "$REPO_ROOT")/aero-im}"
AERO_VAULT_ROOT="${AERO_VAULT_ROOT:-$(dirname -- "$REPO_ROOT")/aero-vault}"
AUDIT_GOVERNANCE_ROOT="${AUDIT_GOVERNANCE_ROOT:-$(dirname -- "$REPO_ROOT")/workspace/demo/snaplink-audit-governance}"
FORGE_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-owned-conversation-page-v1.json"
FORGE_SESSION_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-shared-session-v1.json"
FORGE_RUN_RESUME_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-observer-resume-v1.json"
FORGE_INVENTORY_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
FORGE_HEARTBEAT_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-heartbeat-contract-v1.json"
FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-identity-proof-contract-v1.json"
FORGE_DEVICE_IDENTITY_ED25519_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-identity-proof-ed25519-v1.json"
FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-heartbeat-persistence-contract-v1.json"
FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-persistence-v1.json"
FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-persisted-observation-v1.json"
FORGE_INVENTORY_V2_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
FORGE_INVENTORY_PLACEMENT_V2_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v2.json"
FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v1.json"
FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-placement-batch-evaluation-v1.json"
FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-status-contract-v1.json"
FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-snapshot-canonical-v1.json"
FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-command-terminal-receipt-v1.json"
FORGE_RUNNER_TERMINAL_RECEIPT_CONTRACT_FIXTURE="$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE"
FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-execution-intent-v1.json"
FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json"
FORGE_PLACEMENT_PARITY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-placement-policy-parity-v1.json"
FORGE_PLACEMENT_GPU_PARITY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-placement-gpu-policy-parity-v1.json"
FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-session-placement-observation-v1.json"
FORGE_RUN_INTENT_OBSERVATION_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-intent-observation-v1.json"
FORGE_PENDING_RUN_INTENT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-pending-run-intent-v1.json"
FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-resource-summary-v1.json"
FORGE_SESSION_DEVICE_OBSERVATION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-session-device-observation-v1.json"
FORGE_PENDING_WRITE_RECOVERY_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-pending-write-recovery-v1.json"
FORGE_SNAPLINK_PROFILE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-snaplink-profile-v1.json"
FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-aero-id-profile-projection-v1.json"
FORGE_LEASE_FENCING_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-lease-fencing-v1.json"
FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-execution-lease-checkpoint-v1.json"
FORGE_ATTEMPT_LIFECYCLE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-attempt-lifecycle-v1.json"
FORGE_ATTEMPT_REQUEST_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-attempt-request-v1.json"
FORGE_RUN_OBSERVED_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-observed-v1.json"
FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-prompt-accepted-audit-v1.json"
FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-execution-evidence-v1.json"
FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-registry-placement-preview-v1.json"
FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-execution-consent-preview-v1.json"
FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-v1.json"
FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json"
FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json"
FORGE_RUNNER_DISPATCH_PLAN_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-local-execution-preview-v1.json"
FORGE_DEVICE_FABRIC_REVIEW_PROPOSED_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-fabric-activation-review-proposed-v1.json"
FORGE_DEVICE_FABRIC_REVIEW_SYNTHETIC_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-fabric-activation-review-observe-synthetic-v1.json"
FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-approval-rotation-v1.json"
FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-credential-lifecycle-v1.json"
FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-credential-candidate-v1.json"
FORGE_EXECUTION_RECONCILIATION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-execution-reconciliation-observation-v1.json"

if [[ ! -f "$FORGE_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge conversation contract fixture: %s\n' "$FORGE_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge shared-session contract fixture: %s\n' "$FORGE_SESSION_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUN_RESUME_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge Run observer resume contract fixture: %s\n' "$FORGE_RUN_RESUME_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge device inventory contract fixture: %s\n' "$FORGE_INVENTORY_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_HEARTBEAT_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge device heartbeat contract fixture: %s\n' "$FORGE_HEARTBEAT_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge device identity contract fixture: %s\n' "$FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_IDENTITY_ED25519_FIXTURE" ]]; then
  printf 'Missing Forge Ed25519 device identity fixture: %s\n' "$FORGE_DEVICE_IDENTITY_ED25519_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge heartbeat persistence contract fixture: %s\n' "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge inventory persistence contract fixture: %s\n' "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" ]]; then
  printf 'Missing Forge persisted inventory observation fixture: %s\n' "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_V2_FIXTURE" ]]; then
  printf 'Missing Forge v2 inventory observation fixture: %s\n' "$FORGE_INVENTORY_V2_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" ]]; then
  printf 'Missing Forge v2 inventory placement-evaluation fixture: %s\n' "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge inventory placement-input contract fixture: %s\n' "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" ]]; then
  printf 'Missing Forge inventory placement-evaluation fixture: %s\n' "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" ]]; then
  printf 'Missing Forge inventory placement-batch-evaluation fixture: %s\n' "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge inventory status contract fixture: %s\n' "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" ]]; then
  printf 'Missing Forge inventory snapshot canonical fixture: %s\n' "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge Runner command contract fixture: %s\n' "$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge Runner execution intent fixture: %s\n' "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge session Runner receipt observation fixture: %s\n' "$FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_PLACEMENT_PARITY_FIXTURE" ]]; then
  printf 'Missing Forge placement parity fixture: %s\n' "$FORGE_PLACEMENT_PARITY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_PLACEMENT_GPU_PARITY_FIXTURE" ]]; then
  printf 'Missing Forge GPU placement parity fixture: %s\n' "$FORGE_PLACEMENT_GPU_PARITY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge session placement observation fixture: %s\n' "$FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUN_INTENT_OBSERVATION_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge Run intent observation fixture: %s\n' "$FORGE_RUN_INTENT_OBSERVATION_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_PENDING_RUN_INTENT_FIXTURE" ]]; then
  printf 'Missing Forge pending Run-intent fixture: %s\n' "$FORGE_PENDING_RUN_INTENT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge device resource summary fixture: %s\n' "$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_DEVICE_OBSERVATION_FIXTURE" ]]; then
  printf 'Missing Forge session device observation fixture: %s\n' "$FORGE_SESSION_DEVICE_OBSERVATION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_PENDING_WRITE_RECOVERY_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge pending-write recovery fixture: %s\n' "$FORGE_PENDING_WRITE_RECOVERY_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SNAPLINK_PROFILE_FIXTURE" ]]; then
  printf 'Missing Forge Snaplink profile fixture: %s\n' "$FORGE_SNAPLINK_PROFILE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE" ]]; then
  printf 'Missing Forge Aero-ID profile projection fixture: %s\n' "$FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_LEASE_FENCING_FIXTURE" ]]; then
  printf 'Missing Forge Runner lease/fencing fixture: %s\n' "$FORGE_LEASE_FENCING_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" ]]; then
  printf 'Missing Forge execution lease checkpoint fixture: %s\n' "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" ]]; then
  printf 'Missing Forge Attempt lifecycle fixture: %s\n' "$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_ATTEMPT_REQUEST_FIXTURE" ]]; then
  printf 'Missing Forge Attempt request fixture: %s\n' "$FORGE_ATTEMPT_REQUEST_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUN_OBSERVED_FIXTURE" ]]; then
  printf 'Missing Forge Run observed fixture: %s\n' "$FORGE_RUN_OBSERVED_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" ]]; then
  printf 'Missing Forge accepted-Prompt audit fixture: %s\n' "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" ]]; then
  printf 'Missing Forge Run execution evidence fixture: %s\n' "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" ]]; then
  printf 'Missing Forge client-instance/session-view fixture: %s\n' "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" ]]; then
  printf 'Missing Forge client-instance/resource-view fixture: %s\n' "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" ]]; then
  printf 'Missing Forge registry placement-preview fixture: %s\n' "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE" ]]; then
  printf 'Missing Forge execution-consent preview fixture: %s\n' "$FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE" ]]; then
  printf 'Missing Forge enrollment-heartbeat lifecycle fixture: %s\n' "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" ]]; then
  printf 'Missing Forge enrollment-heartbeat lifecycle persistence fixture: %s\n' "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" ]]; then
  printf 'Missing Forge Run-attempt-lease-dispatch preflight fixture: %s\n' "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" ]]; then
  printf 'Missing Forge Run-attempt-lease-dispatch preflight request fixture: %s\n' "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" ]]; then
  printf 'Missing Forge Runner dispatch-plan preview fixture: %s\n' "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" ]]; then
  printf 'Missing Forge Runner local execution preview fixture: %s\n' "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_FABRIC_REVIEW_PROPOSED_FIXTURE" ]]; then
  printf 'Missing Forge device-fabric proposed review fixture: %s\n' "$FORGE_DEVICE_FABRIC_REVIEW_PROPOSED_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_FABRIC_REVIEW_SYNTHETIC_FIXTURE" ]]; then
  printf 'Missing Forge device-fabric synthetic review fixture: %s\n' "$FORGE_DEVICE_FABRIC_REVIEW_SYNTHETIC_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" ]]; then
  printf 'Missing Forge device approval/rotation fixture: %s\n' "$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" ]]; then
  printf 'Missing Forge device credential candidate fixture: %s\n' "$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_EXECUTION_RECONCILIATION_FIXTURE" ]]; then
  printf 'Missing Forge execution reconciliation fixture: %s\n' "$FORGE_EXECUTION_RECONCILIATION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$SNAPLINK_CONSOLE_ROOT/pubspec.yaml" ]]; then
  printf 'Flutter Console repo not found: %s\n' "$SNAPLINK_CONSOLE_ROOT" >&2
  exit 1
fi
if [[ ! -f "$SNAPLINK_ROOT/go.mod" ]]; then
  printf 'Snaplink repo not found: %s\n' "$SNAPLINK_ROOT" >&2
  exit 1
fi
if [[ ! -f "$AERO_ID_ROOT/go.mod" ]]; then
  printf 'Aero-ID repo not found: %s\n' "$AERO_ID_ROOT" >&2
  exit 1
fi
if [[ ! -f "$AERO_IM_ROOT/Cargo.toml" ]]; then
  printf 'Aero-IM repo not found: %s\n' "$AERO_IM_ROOT" >&2
  exit 1
fi
if [[ ! -f "$AERO_VAULT_ROOT/go.mod" ]]; then
  printf 'Aero-Vault repo not found: %s\n' "$AERO_VAULT_ROOT" >&2
  exit 1
fi
if [[ ! -f "$AUDIT_GOVERNANCE_ROOT/go.mod" ]]; then
  printf 'Audit Governance repo not found: %s\n' "$AUDIT_GOVERNANCE_ROOT" >&2
  exit 1
fi

export FORGE_CONTRACT_FIXTURE
export FORGE_SESSION_CONTRACT_FIXTURE
export FORGE_RUN_RESUME_CONTRACT_FIXTURE
export FORGE_INVENTORY_CONTRACT_FIXTURE
export FORGE_HEARTBEAT_CONTRACT_FIXTURE
export FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE
export FORGE_DEVICE_IDENTITY_ED25519_FIXTURE
export FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE
export FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE
export FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE
export FORGE_INVENTORY_V2_FIXTURE
export FORGE_INVENTORY_PLACEMENT_V2_FIXTURE
export FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE
export FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE
export FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE
export FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE
export FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE
export FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE
export FORGE_RUNNER_TERMINAL_RECEIPT_CONTRACT_FIXTURE
export FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE
export FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE
export FORGE_PLACEMENT_PARITY_FIXTURE
export FORGE_PLACEMENT_GPU_PARITY_FIXTURE
export FORGE_SESSION_PLACEMENT_CONTRACT_FIXTURE
export FORGE_RUN_INTENT_OBSERVATION_CONTRACT_FIXTURE
export FORGE_PENDING_RUN_INTENT_FIXTURE
export FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE
export FORGE_SESSION_DEVICE_OBSERVATION_FIXTURE
export FORGE_PENDING_WRITE_RECOVERY_CONTRACT_FIXTURE
export FORGE_SNAPLINK_PROFILE_FIXTURE
export FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE
export FORGE_LEASE_FENCING_FIXTURE
export FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE
export FORGE_ATTEMPT_LIFECYCLE_FIXTURE
export FORGE_ATTEMPT_REQUEST_FIXTURE
export FORGE_RUN_OBSERVED_FIXTURE
export FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE
export FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE
export FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE
export FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE
export FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE
export FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE
export FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE
export FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE
export FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE
export FORGE_RUNNER_DISPATCH_PLAN_FIXTURE
export FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE
export FORGE_DEVICE_FABRIC_REVIEW_PROPOSED_FIXTURE
export FORGE_DEVICE_FABRIC_REVIEW_SYNTHETIC_FIXTURE
export FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE
export FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE
export FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE
export FORGE_EXECUTION_RECONCILIATION_FIXTURE

(
  cd "$REPO_ROOT/forge-core"
  go test ./internal/runtimebridge/model -run '^TestOwnedConversationContractFixture$' -count=1
  go test ./internal/runtimebridge -run '^TestSharedSessionContractFixture$' -count=1
  go test ./internal/runtimebridge -run '^TestRunObserverResumeContractFixture$' -count=1
  FORGE_PENDING_RUN_INTENT_FIXTURE="$FORGE_PENDING_RUN_INTENT_FIXTURE" go test ./internal/runtimebridge -run '^TestPendingRunIntentContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestInventoryObservationContractFixture$' -count=1
  go test ./internal/deviceheartbeat -run '^TestHeartbeatContractFixture$' -count=1
	go test ./internal/deviceheartbeat -run '^TestApply(RejectsInvalidIdentityAndApprovalBoundariesBeforeCapabilityValidation|AcceptsKnownPendingApprovalAsPureHeartbeatInput|PreservesBoundaryErrorPrecedenceBeforeCapabilityValidation)$|^TestCommitInheritsHeartbeatBoundaryValidationWithoutReplacingCurrent$' -count=1
	go test ./internal/deviceidentity -run '^TestIdentityProofContractFixture$' -count=1
	go test ./internal/deviceidentity -run '^TestVerifySignedProof|^TestSignedProof' -count=1
  go test ./internal/deviceheartbeat -run '^TestHeartbeatPersistenceContractFixture$' -count=1
  FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE="$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" go test ./internal/deviceinventory -run '^TestPersistedInventoryContractFixture$' -count=1
  go test ./internal/deviceinventory -run '^TestPersistedInventoryFileReadAdapter' -count=1
  FORGE_DEVICE_INVENTORY_PLACEMENT_INPUT_FIXTURE="$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" go test ./internal/deviceplacement -run '^TestPersistedInventoryPlacementInputContractFixture$' -count=1
  FORGE_DEVICE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE="$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" go test ./internal/deviceplacement -run '^TestPersistedInventoryPlacementEvaluationContractFixture$' -count=1
  FORGE_DEVICE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE="$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" go test ./internal/deviceplacement -run '^TestPersistedInventoryPlacementBatchContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestBuildPersistedInventoryObservation' -count=1
  FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE="$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" go test ./internal/deviceplacement -run '^TestPersistedInventoryObservationContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^Test.*PersistedInventoryObservationV2' -count=1
  FORGE_DEVICE_INVENTORY_PLACEMENT_V2_FIXTURE="$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" go test ./internal/deviceplacement -run '^TestPersistedInventoryPlacementV2' -count=1
  go test ./internal/deviceplacement -run '^TestPersistedObservationV2Command' -count=1
  go test ./internal/deviceinventory -run '^TestInventoryStatusContractFixture$' -count=1
  go test ./internal/deviceinventory -run '^TestInventorySnapshotCanonicalContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^(TestPolicyParityFixture|TestGpuPolicyParityFixture)$' -count=1
  go test ./internal/deviceplacement -run '^TestSessionPlacementObservationContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestRunIntentObservationContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestDeviceResourceSummaryContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestSessionDeviceObservationSharedFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestDecodeSessionPlacementObservationRequestRequiresNestedShape$' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerExecutionIntent' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerTerminalReceipt' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerDispatchPlanPreview' -count=1
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE="$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" go test ./internal/deviceplacement -run '^TestRunAttemptLeaseDispatchPreflightCanonicalFixtureValidates$' -count=1
  go test ./internal/deviceplacement -run '^TestSessionRunnerReceiptObservation' -count=1
  go test ./internal/pendingwrite -run '^TestRecoveryContractFixture$' -count=1
  go test ./internal/authn -run '^TestSnaplinkProfileContractFixture$' -count=1
  FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE="$FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE" go test ./internal/aeroidprofile -run '^TestProjection' -count=1
  FORGE_LEASE_FENCING_FIXTURE="$FORGE_LEASE_FENCING_FIXTURE" go test ./internal/executionlease -run '^TestLeaseFencingContractFixture$' -count=1
  FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE="$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" go test ./internal/executionlease -run '^TestLeaseCheckpointContractFixture$' -count=1
  FORGE_ATTEMPT_LIFECYCLE_FIXTURE="$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" go test ./internal/executionattempt -run '^TestAttemptLifecycleContractFixture$' -count=1
  FORGE_ATTEMPT_REQUEST_FIXTURE="$FORGE_ATTEMPT_REQUEST_FIXTURE" go test ./internal/executionattempt -run '^TestAttemptRequestContractFixture$' -count=1
  go test ./internal/auditprojection -run '^TestRunObserved' -count=1
  go test ./internal/auditprojection -run '^TestPromptAccepted(Projection|Schema)' -count=1
  FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE="$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" go test ./internal/deviceplacement -run '^TestRunExecutionEvidence' -count=1
  go test ./internal/deviceplacement -run '^TestDecodeClientInstanceSessionViewFixtureAndRejectsMutations$' -count=1
  go test ./internal/deviceplacement -run '^TestDecodeClientInstanceResourceViewFixtureAndRejectsMutations$' -count=1
  go test ./internal/auditprojection -run '^Test(ProjectionPackageHasNoOutboundOrExecutionBoundary|ForgeDoesNotDependOnUnapprovedEcosystemServices)$' -count=1
  go test ./internal/appserver -run '^TestSnaplinkAuthenticatedConversationOwnerParity$' -count=1
  go test ./internal/appserver -run '^TestSnaplink(IntrospectionAuthenticatedConversationOwnerParity|IntrospectionFailsClosedForAudienceTenantSecretAndScope)$' -count=1
  go test ./internal/appserver -run '^TestSnaplinkAuthenticatedInertExecutionToRustHubWhenConfigured$' -count=1
  go test ./internal/appserver -run '^TestSessionDeviceObservationPreview' -count=1
  go test ./internal/appserver -run '^TestPersistedInventoryReadSource' -count=1
  go test ./internal/appserver -run '^TestPersistedInventoryFileReadSource' -count=1
  go test ./internal/appserver -run '^TestSnaplinkAuthenticatedPersistedInventoryFileRustCLIE2EWhenConfigured$' -count=1
  go test ./internal/appserver -run '^TestSessionRunnerReceiptObservationPreview' -count=1
  go test ./internal/appserver -run '^TestLocalRunnerPreviewCandidate' -count=1
  FORGE_LOCAL_RUNNER_PREVIEW_E2E=1 go test ./internal/appserver -run '^TestSnaplinkAuthenticatedLocalRunnerPreviewWhenConfigured$' -count=1
  go test ./internal/appserver -run '^TestRunAttemptLeaseDispatchPreflightCandidate' -count=1
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_E2E=1 go test ./internal/appserver -run '^TestSnaplinkAuthenticatedRunAttemptLeaseDispatchPreflightWhenConfigured$' -count=1
  go test ./internal/appserver -run '^TestClientInstanceSessionViewCandidate' -count=1
  FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE="$FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE" go test ./internal/appserver -run '^TestExecutionConsentPreviewCandidateIsStrictReadOnly$|^TestExecutionConsentPreviewContractFixture$' -count=1
  FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE="$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE" go test ./internal/deviceinventory -run '^TestEnrollmentHeartbeatLifecycleContractFixture$' -count=1
  go test ./internal/deviceinventory -run '^TestPersistedEnrollmentHeartbeatLifecycle' -count=1
  go test ./internal/deviceinventory -run '^TestPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter' -count=1
  go test ./internal/deviceinventory -run '^TestCommitPersistedEnrollmentHeartbeatLifecycleRegistry' -count=1
  go test ./internal/deviceinventory -run '^TestCanonicalizePersistedEnrollmentHeartbeatLifecycleRegistry' -count=1
  go test ./internal/deviceinventory -run '^TestPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter' -count=1
  go test ./internal/appserver -run '^TestPersistedLifecycleRegistryFileSetReadSource' -count=1
  go test ./internal/appserver -run '^TestLifecycleRegistryCandidate' -count=1
  go test ./internal/appserver -run '^TestLifecycleRegistryClientInstanceCandidates' -count=1
  go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSessionProjectionE2EWhenConfigured$' -count=1
	go test ./internal/appserver -run '^TestLifecycleHeartbeatCandidate' -count=1
	go test ./internal/appserver -run '^TestLifecycleApprovalCandidate' -count=1
  go test ./internal/appserver -run '^TestLifecycleCredentialCandidate' -count=1
  go test ./internal/appserver -run '^TestLifecycleRegistryInventoryCandidateComposition' -count=1
  go test ./internal/appserver -run '^TestPersistedLifecycleRegistryPlacementPreviewContract$' -count=1
  go test ./internal/appserver -run '^Test(DevicePlacementRegistryCandidate|LifecycleRegistryPlacementCandidate)' -count=1
  go test ./internal/devicefabricgate ./internal/appserver -run '^Test(ZeroValueRequestKeepsFabricOff|ProposedInventoryFailsClosedWithStableReasons|AcceptedInventoryRequiresEveryEvidenceBoundary|AcceptedObserveAndExecuteAreDistinctStages|MigrationAndFederationRemainBlockedBySeparateDecisions|UnknownModeFailsClosed|ConfigRejectsExplicitDeviceFabricActivationWhileADRIsProposed|ConfigZeroValueDeviceFabricRemainsDefaultOff|AcceptedDeviceFabricActivationReloadsObservationImagesPerRead)$' -count=1
  go test ./internal/devicefabricgate -run '^Test(ParseManifest|LoadManifestFile|Review)' -count=1
  go test ./cmd/forge-server -run '^TestRunRejectsBlockedDeviceFabricManifestBeforeStateMutation$' -count=1
  FORGE_DEVICE_FABRIC_REVIEW_PROPOSED_FIXTURE="$FORGE_DEVICE_FABRIC_REVIEW_PROPOSED_FIXTURE" FORGE_DEVICE_FABRIC_REVIEW_SYNTHETIC_FIXTURE="$FORGE_DEVICE_FABRIC_REVIEW_SYNTHETIC_FIXTURE" go test ./internal/devicefabricgate -run '^TestReview' -count=1
  FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE="$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" go test ./internal/deviceapproval -run '^TestDeviceApprovalRotationContractFixture$' -count=1
  FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE="$FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE" go test ./internal/devicecredential -run '^TestDeviceCredentialLifecycleContractFixture$' -count=1
  go test ./internal/executionreconcile -run '^TestContractFixture$|^TestObservationRejects|^TestObserveRejects' -count=1
  go test ./internal/appserver -run '^TestExecutionReconciliationPreviewCandidate' -count=1
  go test ./internal/appserver -run '^TestConversationRunRoutesRejectUnsafeBackendPages$|^TestRunObservedCandidate' -count=1
  process_runtime_bin="${FORGE_RUNTIME_BIN:-$REPO_ROOT/forge-runtime/target/debug/forge-runtime}"
  if [[ "$process_runtime_bin" != /* ]]; then
    process_runtime_bin="$REPO_ROOT/$process_runtime_bin"
  fi
  if [[ ! -x "$process_runtime_bin" ]]; then
    (cd "$REPO_ROOT/forge-runtime" && cargo build -p forge-runtime-cli --bin forge-runtime)
  fi
  FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./cmd/forge-server -run '^TestForgeServerProcess(ContentionReuseAndInterrupt|AuthenticatedSharedSessionBoundary)$' -count=1
)

(
  cd "$SNAPLINK_ROOT"
  go test ./interfaces/sso -run '^TestRcov2DE_ForgeClientOwnerParity$' -count=1
)

(
  cd "$AERO_ID_ROOT"
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" internal/connector/auditgovernance/testdata/forge-run-observed-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" internal/connector/auditgovernance/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" internal/connector/auditgovernance/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-identity-proof-contract-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_ED25519_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-identity-proof-ed25519-v1.json
  cmp "$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-resource-summary-v1.json
  cmp "$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-approval-rotation-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-credential-lifecycle-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-credential-candidate-v1.json
  cmp "$FORGE_EXECUTION_RECONCILIATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-execution-reconciliation-observation-v1.json
  cmp "$FORGE_INVENTORY_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-observation-v1.json
  cmp "$FORGE_INVENTORY_V2_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-observation-v2.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" internal/connector/auditgovernance/testdata/forge-run-attempt-lease-dispatch-preflight-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_LEASE_FENCING_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-lease-fencing-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" internal/connector/auditgovernance/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-local-execution-preview-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v2.json
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-input-v1.json
  go test ./internal/connector/auditgovernance -run '^TestForgeRunObserved|^TestForgeObserverCorrelationContract$|^TestPayloadStringOmitsUntrustedShapes$|^TestForgeRunExecutionEvidence|^TestForgePromptAcceptedAudit|^TestForgeClientInstance|^TestForgeRunAttemptLeaseDispatchPreflight|^TestForgeRunnerLeaseFencing' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeExecutionLeaseCheckpoint' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerDispatchPlanPreview' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerLocalExecutionPreview' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryPlacementBatchEvaluationReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryPlacementEvaluationReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryPlacementEvaluationV2Receiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryPlacementInputReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceEnrollmentHeartbeatLifecyclePersistenceFixture$|^TestForgeDeviceEnrollmentHeartbeatLifecyclePersistenceRejectsMutation$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceHeartbeatPersistenceReceiver' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryPersistenceReceiver' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryStatusReceiver' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventorySnapshotCanonicalReceiver' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryPersistedObservation' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceResourceSummary' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceApprovalRotation' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceCredentialLifecycle' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceCredentialCandidate' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeExecutionReconciliationObservation' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryObservationReceiver' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryObservationV2Receiver' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryRegistryPlacementPreviewReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceIdentityProof' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeSignedDeviceIdentityProof' -count=1
)

(
  cd "$AERO_IM_ROOT"
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-run-observed-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-identity-proof-contract-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_ED25519_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-identity-proof-ed25519-v1.json
  cmp "$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-resource-summary-v1.json
  cmp "$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-approval-rotation-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-credential-lifecycle-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-credential-candidate-v1.json
  cmp "$FORGE_EXECUTION_RECONCILIATION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-execution-reconciliation-observation-v1.json
  cmp "$FORGE_INVENTORY_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-observation-v1.json
  cmp "$FORGE_INVENTORY_V2_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-observation-v2.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-run-attempt-lease-dispatch-preflight-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-execution-lease-checkpoint-v1.json
  cargo test -p aero-audit-connector --test forge_observer_contract
  cargo test -p aero-audit-connector --test forge_prompt_accepted_contract
  cargo test -p aero-audit-connector --test forge_execution_evidence_contract
  cargo test -p aero-audit-connector --test forge_client_instance_view_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_registry_placement_contract
  cargo test -p aero-audit-connector --test forge_device_identity_proof_contract
  cargo test -p aero-audit-connector --test forge_device_identity_proof_ed25519_contract
  cargo test -p aero-audit-connector --test forge_run_attempt_lease_dispatch_preflight_contract
  cargo test -p aero-audit-connector --test forge_device_enrollment_heartbeat_lifecycle_persistence_contract
  cargo test -p aero-audit-connector --test forge_device_heartbeat_persistence_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_persistence_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_status_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_snapshot_canonical_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_persisted_observation_contract
  cargo test -p aero-audit-connector --test forge_device_resource_summary_contract
  cargo test -p aero-audit-connector --test forge_device_approval_rotation_contract
  cargo test -p aero-audit-connector --test forge_device_credential_lifecycle_contract
  cargo test -p aero-audit-connector --test forge_device_credential_candidate_contract
  cargo test -p aero-audit-connector --test forge_execution_reconciliation_observation_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_observation_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_observation_v2_contract
  cmp "$FORGE_LEASE_FENCING_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-lease-fencing-v1.json
  cargo test -p aero-audit-connector --test forge_runner_lease_fencing_contract
  cargo test -p aero-audit-connector --test forge_execution_lease_checkpoint_contract
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-dispatch-plan-preview-v1.json
  cargo test -p aero-audit-connector --test forge_runner_dispatch_plan_preview_contract
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-local-execution-preview-v1.json
  cargo test -p aero-audit-connector --test forge_runner_local_execution_preview_contract
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cargo test -p aero-audit-connector --test forge_device_inventory_placement_batch_evaluation_contract
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-placement-evaluation-v1.json
  cargo test -p aero-audit-connector --test forge_device_inventory_placement_evaluation_contract
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-placement-evaluation-v2.json
  cargo test -p aero-audit-connector --test forge_device_inventory_placement_evaluation_v2_contract
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-placement-input-v1.json
  cargo test -p aero-audit-connector --test forge_device_inventory_placement_input_contract
)

(
  cd "$AERO_VAULT_ROOT"
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" internal/auditgovernance/testdata/forge-run-observed-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" internal/auditgovernance/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" internal/auditgovernance/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" internal/auditgovernance/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" internal/auditgovernance/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-identity-proof-contract-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_ED25519_FIXTURE" internal/auditgovernance/testdata/forge-device-identity-proof-ed25519-v1.json
  cmp "$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-resource-summary-v1.json
  cmp "$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" internal/auditgovernance/testdata/forge-device-approval-rotation-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE" internal/auditgovernance/testdata/forge-device-credential-lifecycle-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" internal/auditgovernance/testdata/forge-device-credential-candidate-v1.json
  cmp "$FORGE_EXECUTION_RECONCILIATION_FIXTURE" internal/auditgovernance/testdata/forge-execution-reconciliation-observation-v1.json
  cmp "$FORGE_INVENTORY_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-observation-v1.json
  cmp "$FORGE_INVENTORY_V2_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-observation-v2.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" internal/auditgovernance/testdata/forge-run-attempt-lease-dispatch-preflight-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" internal/auditgovernance/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_LEASE_FENCING_FIXTURE" internal/auditgovernance/testdata/forge-runner-lease-fencing-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" internal/auditgovernance/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" internal/auditgovernance/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" internal/auditgovernance/testdata/forge-runner-local-execution-preview-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v2.json
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-input-v1.json
  go test ./internal/auditgovernance -run '^TestForgeRunObserved|^TestForgeRunExecutionEvidence|^TestForgePromptAcceptedAudit|^TestForgeClientInstance|^TestForgeRunAttemptLeaseDispatchPreflight|^TestForgeRunnerLeaseFencing' -count=1
  go test ./internal/auditgovernance -run '^TestForgeExecutionLeaseCheckpoint' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerDispatchPlanPreview' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerLocalExecutionPreview' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryPlacementBatchEvaluationReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryPlacementEvaluationReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryPlacementEvaluationV2Receiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryPlacementInputReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceEnrollmentHeartbeatLifecyclePersistenceFixture$|^TestForgeDeviceEnrollmentHeartbeatLifecyclePersistenceRejectsMutation$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceHeartbeatPersistenceReceiver' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryPersistenceReceiver' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryStatusReceiver' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventorySnapshotCanonicalReceiver' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryPersistedObservation' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceResourceSummary' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceApprovalRotation' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceCredentialLifecycle' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceCredentialCandidate' -count=1
  go test ./internal/auditgovernance -run '^TestForgeExecutionReconciliationObservation' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryObservationReceiver' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryObservationV2Receiver' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryRegistryPlacementPreviewReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceIdentityProof' -count=1
  go test ./internal/auditgovernance -run '^TestForgeSignedDeviceIdentityProof' -count=1
)

(
  cd "$AUDIT_GOVERNANCE_ROOT"
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" internal/domain/testdata/forge-run-observed-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" internal/domain/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" internal/domain/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" internal/domain/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" internal/domain/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" internal/domain/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-identity-proof-contract-v1.json
  cmp "$FORGE_DEVICE_IDENTITY_ED25519_FIXTURE" internal/domain/testdata/forge-device-identity-proof-ed25519-v1.json
  cmp "$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-resource-summary-v1.json
  cmp "$FORGE_DEVICE_APPROVAL_ROTATION_FIXTURE" internal/domain/testdata/forge-device-approval-rotation-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_LIFECYCLE_FIXTURE" internal/domain/testdata/forge-device-credential-lifecycle-v1.json
  cmp "$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" internal/domain/testdata/forge-device-credential-candidate-v1.json
  cmp "$FORGE_EXECUTION_RECONCILIATION_FIXTURE" internal/domain/testdata/forge-execution-reconciliation-observation-v1.json
  cmp "$FORGE_INVENTORY_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-observation-v1.json
  cmp "$FORGE_INVENTORY_V2_FIXTURE" internal/domain/testdata/forge-device-inventory-observation-v2.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" internal/domain/testdata/forge-run-attempt-lease-dispatch-preflight-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" internal/domain/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" internal/domain/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" internal/domain/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_LEASE_FENCING_FIXTURE" internal/domain/testdata/forge-runner-lease-fencing-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" internal/domain/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" internal/domain/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" internal/domain/testdata/forge-runner-local-execution-preview-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-evaluation-v2.json
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-input-v1.json
  go test ./internal/domain -run '^TestForgeRunObserver|^TestForgeRunExecutionEvidence|^TestForgePromptAccepted|^TestForgeClientInstance|^TestForgeRunAttemptLeaseDispatchPreflight|^TestForgeRunnerLeaseFencing' -count=1
  go test ./internal/domain -run '^TestForgeExecutionLeaseCheckpoint' -count=1
  go test ./internal/domain -run '^TestForgeRunnerDispatchPlanPreview' -count=1
  go test ./internal/domain -run '^TestForgeRunnerLocalExecutionPreview' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryPlacementBatchEvaluationReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryPlacementEvaluationReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryPlacementEvaluationV2Receiver$' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryPlacementInputReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeDeviceEnrollmentHeartbeatLifecyclePersistenceFixture$|^TestForgeDeviceEnrollmentHeartbeatLifecyclePersistenceRejectsMutation$' -count=1
  go test ./internal/domain -run '^TestForgeDeviceHeartbeatPersistenceReceiver' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryPersistenceReceiver' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryStatusReceiver' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventorySnapshotCanonicalReceiver' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryPersistedObservation' -count=1
  go test ./internal/domain -run '^TestForgeDeviceResourceSummary' -count=1
  go test ./internal/domain -run '^TestForgeDeviceApprovalRotation' -count=1
  go test ./internal/domain -run '^TestForgeDeviceCredentialLifecycle' -count=1
  go test ./internal/domain -run '^TestForgeDeviceCredentialCandidate' -count=1
  go test ./internal/domain -run '^TestForgeExecutionReconciliationObservation' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryObservationReceiver' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryObservationV2Receiver' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryRegistryPlacementPreviewReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeDeviceIdentityProof' -count=1
  go test ./internal/domain -run '^TestForgeSignedDeviceIdentityProof' -count=1
)

(
  cd "$REPO_ROOT/forge-runtime"
  cargo test -p forge-runtime-cli --bin forge-runtime conversation_contract_fixture
  cargo test -p forge-runtime-cli --bin forge-runtime shared_session_contract_fixture
  cargo test -p forge-runtime-cli --bin forge-runtime run_observer_resume_contract_fixture
  cargo test -p forge-runtime-cli --bin forge-runtime run_timeline_checkpoint
  cargo test -p forge-runtime-cli --bin forge-runtime resumed_timeline
  cargo test -p forge-runtime-cli --test cli_device_placement
  cargo test -p forge-runtime-cli --test cli_device_inventory
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_show_bounded_offline_inventory_without_a_device_request
  cargo test -p forge-runtime-cli --test cli_device_inventory_status
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_show_bounded_offline_inventory_status_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_inventory_persistence_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_persisted_inventory_observation_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_persisted_inventory_observation_v2_without_a_device_request
  cargo test -p forge-runtime-cli --test cli_device_inventory_snapshot
  cargo test -p forge-runtime-cli --test cli_device_inventory_placement_evaluation
  cargo test -p forge-runtime-cli --test cli_device_inventory_placement_batch_evaluation
  cargo test -p forge-runtime-cli --test cli_device_inventory_persistence
  cargo test -p forge-runtime-cli --test cli_device_inventory_persisted_observation
  cargo test -p forge-runtime-cli --test cli_device_inventory_observation_v2
  cargo test -p forge-runtime-cli --test cli_device_inventory_placement_evaluation_v2
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_persisted_placement_from_a_file_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_v2_placement_from_a_file_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime device_inventory_persisted_observation_requires_a_bounded_input_source
  cargo test -p forge-runtime-cli --test cli_device_heartbeat_persistence
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_heartbeat_persistence_without_a_device_request
  cargo test -p forge-runtime-cli --test cli_device_identity_proof
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_identity_proof_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_show_bounded_offline_inventory_snapshot_without_a_device_request
  cargo test -p forge-runtime-cli --test cli_device_resource_summary
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_show_bounded_offline_resource_summary_without_a_device_request
  cargo test -p forge-runtime-cli --test cli_device_session_observation
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_show_bounded_session_device_observation_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_session_observation_preview
  cargo test -p forge-runtime-cli --bin forge-runtime session_observation_preview_posts_the_bound_request_once
  cargo test -p forge-runtime-cli --bin forge-runtime remote_session_observation_requires_complete_nested_shape_before_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_session_observation_preview_uses_the_authenticated_session_route
  cargo test -p forge-runtime-cli --test cli_device_run_intent_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_run_intent_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime runner_receipt_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_runner_terminal_receipt_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime runner_lease_fencing_preview
  cargo test -p forge-runtime-cli --bin forge-runtime execution_lease_checkpoint_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_runner_lease_fencing_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_execution_lease_checkpoint_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime runner_execution_intent_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_runner_execution_intent_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime session_runner_receipt_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_session_runner_receipt_preview
  cargo test -p forge-runtime-cli --bin forge-runtime session_runner_receipt_preview_posts_the_bound_request_once
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_session_runner_receipt_preview_uses_the_authenticated_session_route
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_session_runner_receipt_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime local_runner_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_local_runner_preview
  cargo test -p forge-runtime-cli --bin forge-runtime args::remote_args::tests::remote_pending_run_intent
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::dispatch::tests::hidden_instance_pending_run_intent
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::dispatch::tests::visible_instance_pending_run_intent
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::tui::tests::scope_tests::remote_tui_instance_filter_blocks_hidden_pending_run_intent
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::tui::tests::pending_run_intents
  cargo test -p forge-runtime-cli --bin forge-runtime args::remote_args::tests::remote_run_reads
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::dispatch::tests::hidden_instance_run_reads
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::dispatch::tests::local_instance_run_list
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::tui::tests::scope_tests::remote_tui_instance_filter_blocks_hidden_run
  cargo test -p forge-runtime-cli --bin forge-runtime run_execution_evidence_preview
  cargo test -p forge-runtime-cli --bin forge-runtime device_run_attempt_lease_dispatch_preflight
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_run_attempt_lease_dispatch_preflight
  cargo test -p forge-runtime-cli --bin forge-runtime run_attempt_lease_dispatch_preflight_posts_exact_bound_request_once
  cargo test -p forge-runtime-cli --bin forge-runtime run_attempt_lease_dispatch_preflight_does_not_retry_server_failure
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_posts_authenticated_run_attempt_lease_dispatch_preflight_for_selected_session
  cargo test -p forge-runtime-cli --bin forge-runtime client_instance_session_view
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_client_instance_session_view
  cargo test -p forge-runtime-cli --bin forge-runtime client_instance_resource_view
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_client_instance_resource_view
  cargo test -p forge-runtime-cli --bin forge-runtime device_credential_candidate_command
  cargo test -p forge-runtime-cli --bin forge-runtime credential_candidate
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_credential_candidate_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_posts_credential_candidate_only_after_explicit_command
  cargo test -p forge-runtime-cli --bin forge-runtime client_instance_session_view_read
  cargo test -p forge-runtime-cli --bin forge-runtime client_instance_resource_view_read
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_read_authenticated_client_instance
  cargo test -p forge-runtime-cli --bin forge-runtime execution_consent
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_execution_consent_preview
  cargo test -p forge-runtime-cli --bin forge-runtime execution_reconciliation
  cargo test -p forge-runtime-cli --bin forge-runtime lifecycle_registry
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::placement_registry::tests
  cargo test -p forge-runtime-cli --bin forge-runtime remote_registry_placement_preview_parses_a_requirements_file
  cargo test -p forge-runtime-cli --bin forge-runtime registry_placement_preview_posts_requirements_once_and_validates_v2_response
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_registry_placement_preview_posts_requirements_and_renders_no_selection
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_run_execution_evidence_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime run_observed_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_run_observed_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime device_attempt_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_attempt_request_without_a_network_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_attempt_request_preview_requires_a_file_path
  cargo test -p forge-runtime-cli --bin forge-runtime tui_sync_refreshes_the_selected_run_timeline_incrementally
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui
  cargo test -p forge-runtime-domain --lib placement_parity
  cargo test -p forge-runtime-domain --lib shared_run_intent_fixture_binds_prompt_receipt_to_placement_preview
  cargo test -p forge-runtime-domain --lib resource_summary_fixture_aggregates_unverified_declarations
  cargo test -p forge-runtime-domain --lib inventory_observation_contract
  cargo test -p forge-runtime-domain --lib heartbeat_contract
  cargo test -p forge-runtime-domain --lib identity_proof_contract_fixture
  cargo test -p forge-runtime-domain --lib heartbeat_persistence_contract_fixture
  cargo test -p forge-runtime-domain --lib persisted_inventory_contract
  cargo test -p forge-runtime-domain --lib persisted_inventory_observation
  cargo test -p forge-runtime-domain --lib persisted_inventory_observation_v2
  cargo test -p forge-runtime-domain --lib persisted_placement_v2
  cargo test -p forge-runtime-domain --lib persisted_inventory_placement_input_contract_fixture
  cargo test -p forge-runtime-domain --lib persisted_inventory_offline_placement_evaluation_contract_fixture
  cargo test -p forge-runtime-domain --lib persisted_inventory_placement_batch_contract
  cargo test -p forge-runtime-domain --lib inventory_status_contract_fixture
  cargo test -p forge-runtime-domain --lib inventory_snapshot_canonical_contract_fixture
  cargo test -p forge-runtime-domain --lib runner_command_contract_fixture
  cargo test -p forge-runtime-domain --lib terminal_receipt_observation_is_preview_only
  cargo test -p forge-runtime-domain --lib runner_execution_intent
  cargo test -p forge-runtime-cli --bin forge-runtime runner_dispatch_plan_preview
  cargo test -p forge-runtime-domain --lib session_runner_receipt
  cargo test -p forge-runtime-domain --lib runner_execution_intent
  cargo test -p forge-runtime-domain --lib pending_write_recovery_contract_fixture
  cargo test -p forge-runtime-cli --bin forge-runtime snaplink_profile_fixture_matches_the_cli_device_client
  cargo test -p forge-runtime-domain --lib aero_id_profile_projection
  cargo test -p forge-runtime-domain --lib lease_fencing_contract_fixture
  cargo test -p forge-runtime-domain --lib lease_checkpoint_contract_fixture
  cargo test -p forge-runtime-domain --lib reconciliation_contract_fixture
  cargo test -p forge-runtime-domain --lib attempt_lifecycle_contract
  cargo test -p forge-runtime-domain --lib attempt_request_contract_fixture
  cargo test -p forge-runtime-domain --lib run_observed
  cargo test -p forge-runtime-domain --lib run_execution_evidence
  cargo test -p forge-runtime-domain --lib run_attempt_lease_dispatch_preflight
  cargo test -p forge-runtime-domain --lib enrollment_heartbeat_lifecycle_contract
  FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE="$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" cargo test -p forge-runtime-domain --lib enrollment_heartbeat_lifecycle_persistence_contract
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE="$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" cargo test -p forge-runtime-domain --lib run_attempt_lease_dispatch_preflight
)

(
  cd "$SNAPLINK_CONSOLE_ROOT"
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json
  python3 -m py_compile android/tests/run_forge_shared_session_instrumentation.py ios/tests/validate_forge_ios_shared_session_input.py
  bash -n ios/tests/run_forge_shared_session_acceptance.sh
  python3 android/tests/run_forge_shared_session_instrumentation.py
  ios/tests/run_forge_shared_session_acceptance.sh
  flutter test test/forge_preflight_fixture_test.dart
  FORGE_RUNNER_DISPATCH_PLAN_FIXTURE="$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" flutter test test/forge_runner_dispatch_plan_preview_contract_test.dart
  flutter test test/forge_run_attempt_lease_dispatch_preflight_api_test.dart
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE="$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" FORGE_RUNNER_DISPATCH_PLAN_FIXTURE="$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" flutter test test/forge_sessions_gate_runner_dispatch_plan_candidate_test.dart
  flutter test test/forge_run_attempt_lease_dispatch_preflight_api_e2e_test.dart
  flutter test test/forge_conversations_api_test.dart
  flutter test test/forge_conversations_models_test.dart
  flutter test test/forge_conversations_contract_test.dart
  flutter test test/forge_run_observer_resume_contract_test.dart
  flutter test test/forge_run_observed_contract_test.dart
  flutter test test/forge_run_observed_widget_test.dart
  flutter test test/forge_candidate_resource_status_transport_test.dart
  flutter test test/forge_run_execution_evidence_contract_test.dart
  flutter test test/forge_run_execution_evidence_widget_test.dart
  FORGE_EXECUTION_RECONCILIATION_FIXTURE="$FORGE_EXECUTION_RECONCILIATION_FIXTURE" flutter test test/forge_execution_reconciliation_contract_test.dart
  flutter test test/forge_execution_reconciliation_api_test.dart
  flutter test test/forge_execution_reconciliation_widget_test.dart
  flutter test test/forge_sessions_gate_execution_reconciliation_candidate_test.dart
  FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE="$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" flutter test test/forge_client_instance_session_view_contract_test.dart
  flutter test test/forge_client_instance_session_view_widget_test.dart
  FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE="$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" flutter test test/forge_sessions_client_instance_session_import_widget_test.dart
  FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE="$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" flutter test test/forge_client_instance_resource_view_contract_test.dart
  flutter test test/forge_client_instance_resource_view_widget_test.dart
  FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE="$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" flutter test test/forge_sessions_client_instance_resource_import_widget_test.dart
  flutter test test/forge_client_instance_resource_view_api_e2e_test.dart
  flutter test test/forge_client_instance_session_view_api_e2e_test.dart
  flutter test test/forge_client_instance_session_projection_e2e_test.dart
  flutter test test/forge_client_instance_resource_projection_e2e_test.dart
  flutter test test/forge_client_instance_dispatch_plan_preview_e2e_test.dart
  flutter test test/forge_client_instance_execution_readiness_projection_e2e_test.dart
  flutter test test/forge_client_instance_candidate_transport_test.dart
  flutter test test/forge_execution_consent_api_test.dart
  flutter test test/forge_sessions_gate_execution_consent_candidate_test.dart
  flutter test test/forge_execution_consent_api_e2e_test.dart
  FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE="$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE" flutter test test/forge_device_enrollment_heartbeat_lifecycle_contract_test.dart
  flutter test test/forge_lifecycle_registry_contract_test.dart
  flutter test test/forge_lifecycle_registry_api_test.dart
  flutter test test/forge_lifecycle_registry_widget_test.dart
  flutter test test/forge_sessions_gate_lifecycle_registry_candidate_test.dart
  FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE="$FORGE_DEVICE_CREDENTIAL_CANDIDATE_FIXTURE" flutter test test/forge_device_credential_candidate_contract_test.dart
  flutter test test/forge_device_credential_candidate_api_test.dart
  flutter test test/forge_sessions_gate_device_credential_candidate_test.dart
  flutter test test/forge_device_registry_placement_preview_api_test.dart
  flutter test test/forge_sessions_gate_registry_placement_candidate_test.dart
  flutter test test/forge_attempt_lifecycle_contract_test.dart
  flutter test test/forge_attempt_request_preview_contract_test.dart
  flutter test test/forge_attempt_request_preview_card_test.dart
  flutter test test/forge_run_timeline_cursor_store_test.dart
  flutter test test/forge_device_inventory_contract_test.dart
  flutter test test/forge_device_inventory_api_test.dart
  flutter test test/forge_device_inventory_persisted_observation_contract_test.dart
  FORGE_INVENTORY_V2_FIXTURE="$FORGE_INVENTORY_V2_FIXTURE" flutter test test/forge_device_inventory_v2_contract_test.dart
  FORGE_INVENTORY_V2_FIXTURE="$FORGE_INVENTORY_V2_FIXTURE" flutter test test/forge_device_inventory_v2_panel_test.dart
  flutter test test/forge_sessions_gate_device_inventory_candidate_test.dart
  FORGE_INVENTORY_PLACEMENT_V2_FIXTURE="$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" flutter test test/forge_device_inventory_placement_evaluation_v2_contract_test.dart
  FORGE_INVENTORY_PLACEMENT_V2_FIXTURE="$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" flutter test test/forge_sessions_placement_evaluation_v2_widget_test.dart
  FORGE_INVENTORY_PLACEMENT_V2_FIXTURE="$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" flutter test test/forge_sessions_gate_precondition_preview_test.dart
  flutter test test/forge_device_heartbeat_contract_test.dart
  flutter test test/forge_device_identity_contract_test.dart
  flutter test test/forge_device_heartbeat_persistence_contract_test.dart
  flutter test test/forge_device_inventory_persistence_contract_test.dart
  flutter test test/forge_device_inventory_status_contract_test.dart
  flutter test test/forge_device_inventory_snapshot_contract_test.dart
  flutter test test/forge_device_inventory_placement_input_contract_test.dart
  flutter test test/forge_device_inventory_placement_evaluation_contract_test.dart
  FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE="$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" flutter test test/forge_sessions_placement_evaluation_widget_test.dart
  flutter test test/forge_device_inventory_placement_batch_evaluation_contract_test.dart
  FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE="$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" flutter test test/forge_sessions_placement_batch_evaluation_widget_test.dart
  flutter test test/forge_device_placement_parity_test.dart
  flutter test test/forge_session_placement_observation_contract_test.dart
  flutter test test/forge_run_intent_observation_contract_test.dart
  flutter test test/forge_pending_run_intent_contract_test.dart
  flutter test test/forge_pending_run_intent_card_test.dart
  flutter test test/forge_pending_run_intent_api_test.dart
  flutter test test/forge_pending_run_intent_submit_widget_test.dart
  flutter test test/forge_device_resource_summary_contract_test.dart
  FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE="$FORGE_DEVICE_RESOURCE_SUMMARY_CONTRACT_FIXTURE" flutter test test/forge_sessions_resource_summary_widget_test.dart
  flutter test test/forge_session_device_observation_wire_test.dart
  flutter test test/forge_session_device_observation_api_test.dart
  flutter test test/forge_sessions_device_observation_test.dart
  flutter test test/forge_run_intent_observation_card_test.dart
  flutter test test/forge_sessions_run_intent_observation_test.dart
  flutter test test/forge_device_inventory_panel_test.dart
  flutter test test/forge_sessions_device_inventory_widget_test.dart
  flutter test test/forge_pending_write_recovery_contract_test.dart
  flutter test test/forge_auth_profile_contract_test.dart
  flutter test test/forge_aero_id_profile_projection_contract_test.dart
  flutter test test/forge_device_placement_api_test.dart
  flutter test test/forge_runner_execution_intent_contract_test.dart
  flutter test test/forge_runner_lease_fencing_contract_test.dart
  FORGE_LEASE_FENCING_FIXTURE="$FORGE_LEASE_FENCING_FIXTURE" flutter test test/forge_runner_lease_fencing_preview_widget_test.dart
  FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE="$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" flutter test test/forge_execution_lease_checkpoint_contract_test.dart
  flutter test test/forge_execution_lease_checkpoint_preview_widget_test.dart
  flutter test test/forge_runner_terminal_receipt_contract_test.dart
  flutter test test/forge_session_runner_receipt_observation_contract_test.dart
  flutter test test/forge_session_runner_receipt_observation_api_test.dart
  flutter test test/forge_local_runner_preview_api_test.dart
  flutter test test/forge_sessions_gate_local_runner_preview_candidate_test.dart
  flutter test test/forge_sessions_run_intent_observation_test.dart
  flutter test test/forge_sessions_deep_link_test.dart
  flutter test test/forge_runs_widget_test.dart
  flutter test test/forge_sessions_route_test.dart
  flutter test test/forge_sessions_widget_test.dart
)
