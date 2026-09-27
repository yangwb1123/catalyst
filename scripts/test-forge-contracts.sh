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
FORGE_PROMPT_APPEND_RECEIPT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-prompt-append-receipt-v1.json"
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
FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-attempt-boundary-v1.json"
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
FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-execution-lease-registry-v1.json"
FORGE_EXECUTION_LEASE_RELEASE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-execution-lease-release-v1.json"
FORGE_ATTEMPT_LIFECYCLE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-attempt-lifecycle-v1.json"
FORGE_ATTEMPT_REQUEST_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-attempt-request-v1.json"
FORGE_RUN_OBSERVED_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-observed-v1.json"
FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-prompt-accepted-audit-v1.json"
FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-execution-evidence-v1.json"
FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-client-instance-session-resource-convergence-v1.json"
FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-inventory-registry-placement-preview-v1.json"
FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-scheduler-selection-preview-v1.json"
FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-execution-consent-preview-v1.json"
FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-v1.json"
FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json"
FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json"
FORGE_RUNNER_DISPATCH_PLAN_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-dispatch-admission-v1.json"
FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-transport-admission-v1.json"
FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-execution-boundary-v1.json"
FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-local-execution-preview-v1.json"
FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
FORGE_RUNNER_COMMAND_DIGEST_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-command-digest-v1.json"
FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-runner-terminal-receipt-vectors-v1.json"
FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-session-runner-receipt-vectors-v1.json"
FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-session-runner-reconciliation-projection-v1.json"
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
if [[ ! -f "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" ]]; then
  printf 'Missing Forge Prompt append receipt fixture: %s\n' "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" >&2
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
if [[ ! -f "$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" ]]; then
  printf 'Missing Forge Runner Attempt boundary fixture: %s\n' "$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge Runner execution intent fixture: %s\n' "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" ]]; then
  printf 'Missing Forge Runner execution-intent request fixture: %s\n' "$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" ]]; then
  printf 'Missing Forge Runner command digest fixture: %s\n' "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" ]]; then
  printf 'Missing Forge Runner terminal receipt vectors fixture: %s\n' "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" ]]; then
  printf 'Missing Forge session Runner receipt vectors fixture: %s\n' "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" ]]; then
  printf 'Missing Forge session Runner receipt history fixture: %s\n' "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" ]]; then
  printf 'Missing Forge session Runner reconciliation projection fixture: %s\n' "$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" >&2
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
if [[ ! -f "$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" ]]; then
  printf 'Missing Forge execution lease registry fixture: %s\n' "$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_EXECUTION_LEASE_RELEASE_FIXTURE" ]]; then
  printf 'Missing Forge execution lease release fixture: %s\n' "$FORGE_EXECUTION_LEASE_RELEASE_FIXTURE" >&2
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
if [[ ! -f "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" ]]; then
  printf 'Missing Forge client-instance session/resource convergence fixture: %s\n' "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" ]]; then
  printf 'Missing Forge device inventory/resource convergence fixture: %s\n' "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" ]]; then
  printf 'Missing Forge registry placement-preview fixture: %s\n' "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE" ]]; then
  printf 'Missing Forge scheduler-selection preview fixture: %s\n' "$FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE" >&2
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
if [[ ! -f "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" ]]; then
  printf 'Missing Forge Runner dispatch-admission fixture: %s\n' "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" ]]; then
  printf 'Missing Forge Runner transport-admission fixture: %s\n' "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" ]]; then
  printf 'Missing Forge Runner execution-boundary fixture: %s\n' "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" >&2
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
export FORGE_PROMPT_APPEND_RECEIPT_FIXTURE
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
export FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE
export FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE
export FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE
export FORGE_RUNNER_COMMAND_DIGEST_FIXTURE
export FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE
export FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE
export FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE
export FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE
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
export FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE
export FORGE_EXECUTION_LEASE_RELEASE_FIXTURE
export FORGE_ATTEMPT_LIFECYCLE_FIXTURE
export FORGE_ATTEMPT_REQUEST_FIXTURE
export FORGE_RUN_OBSERVED_FIXTURE
export FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE
export FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE
export FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE
export FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE
export FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE
export FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE
export FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE
export FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_FIXTURE
export FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE
export FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE
export FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE
export FORGE_RUNNER_DISPATCH_PLAN_FIXTURE
export FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE
export FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE
export FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE
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
  FORGE_PROMPT_APPEND_RECEIPT_FIXTURE="$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" go test ./internal/promptappendreceipt -run '^TestPromptAppendReceipt' -count=1
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
  go test ./internal/deviceplacement -run '^TestPlacementPolicyRegistry|^TestEvaluatePolicyComplete' -count=1
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
  FORGE_RUNNER_COMMAND_DIGEST_FIXTURE="$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" go test ./internal/deviceplacement -run '^TestRunnerCommandDigestVectors' -count=1
  FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE="$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" go test ./internal/deviceplacement -run '^TestRunnerTerminalReceiptVectors' -count=1
  FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE="$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" go test ./internal/deviceplacement -run '^TestRunnerAttemptBoundaryContractFixture' -count=1
  FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" go test ./internal/deviceplacement -run '^TestSessionRunnerReceiptVectors' -count=1
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" go test ./internal/deviceplacement -run '^TestSessionRunnerReceiptHistory' -count=1
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE="$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" go test ./internal/deviceplacement -run '^TestSessionRunnerReconciliationProjection' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerTerminalReceipt' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerDispatchPlanPreview' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerDispatchAdmission' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerTransportAdmission' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerExecutionBoundary' -count=1
  go test ./internal/deviceplacement -run '^TestRunnerExecutionBoundary' -count=1
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE="$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" go test ./internal/deviceplacement -run '^TestRunAttemptLeaseDispatchPreflightCanonicalFixtureValidates$' -count=1
  go test ./internal/deviceplacement -run '^TestSessionRunnerReceiptObservation' -count=1
  go test ./internal/pendingwrite -run '^TestRecoveryContractFixture$' -count=1
  go test ./internal/authn -run '^TestSnaplinkProfileContractFixture$' -count=1
  FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE="$FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE" go test ./internal/aeroidprofile -run '^TestProjection' -count=1
  FORGE_LEASE_FENCING_FIXTURE="$FORGE_LEASE_FENCING_FIXTURE" go test ./internal/executionlease -run '^TestLeaseFencingContractFixture$' -count=1
  FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE="$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" go test ./internal/executionlease -run '^TestLeaseCheckpointContractFixture$' -count=1
  go test ./internal/executionlease -run '^TestClaim' -count=1
  go test ./internal/executionlease -run '^TestRelease' -count=1
  go test ./internal/deviceinventory -run '^TestPersistedExecutionLeaseRegistry' -count=1
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
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" go test ./internal/appserver -run '^TestSessionRunnerReceiptHistoryPreview' -count=1
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" go test ./internal/appserver -run '^TestSessionRunnerReconciliationProjectionPreview' -count=1
  go test ./internal/appserver -run '^TestLocalRunnerPreviewCandidate' -count=1
  FORGE_LOCAL_RUNNER_PREVIEW_E2E=1 go test ./internal/appserver -run '^TestSnaplinkAuthenticatedLocalRunnerPreviewWhenConfigured$' -count=1
  go test ./internal/appserver -run '^TestRunAttemptLeaseDispatchPreflightCandidate' -count=1
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_E2E=1 go test ./internal/appserver -run '^TestSnaplinkAuthenticatedRunAttemptLeaseDispatchPreflightWhenConfigured$' -count=1
  go test ./internal/appserver -run '^TestRunnerTransportAdmission' -count=1
  go test ./internal/appserver -run '^TestRunnerExecutionBoundary' -count=1
  go test ./internal/appserver -run '^TestRunnerExecutionIntentPreview' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteExecutionReconciliationAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteSessionRunnerReceiptAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteSessionRunnerReconciliationE2E$' -count=1
  go test ./internal/appserver -run '^TestRunExecutionEvidencePreview|^TestRunAcceptedExecuteRunExecutionEvidenceAcrossClients$' -count=1
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
  FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_E2E=1 go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSessionResourceConvergenceE2EWhenConfigured$' -count=1
  FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_CONVERGENCE_E2E=1 go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSchedulerPreviewConvergenceE2EWhenConfigured$' -count=1
	go test ./internal/appserver -run '^TestLifecycleHeartbeatCandidate' -count=1
	go test ./internal/appserver -run '^TestLifecycleApprovalCandidate' -count=1
  go test ./internal/appserver -run '^TestLifecycleCredentialCandidate' -count=1
  go test ./internal/appserver -run '^TestLifecycleRegistryInventoryCandidateComposition' -count=1
  go test ./internal/appserver -run '^TestPersistedLifecycleRegistryPlacementPreviewContract$' -count=1
  go test ./internal/appserver -run '^Test(DevicePlacementRegistryCandidate|LifecycleRegistryPlacementCandidate)' -count=1
  # §703 covers the opt-in mobile planning-only scheduler-preview handoff;
  # §704 separately keeps lifecycle candidates exact-404 on ordinary Coordinator.
  go test ./internal/appserver -run '^TestSchedulerSelectionPreview' -count=1
  go test ./internal/appserver -run '^TestLifecycleCandidatesRemainUnregisteredOnSessionCoordinator$' -count=1
  go test ./internal/appserver -run '^TestSchedulerSelectionLease' -count=1
  go test ./internal/appserver -run '^TestRunnerDispatchAdmission' -count=1
  go test ./internal/appserver -run '^TestRunnerAttemptBoundaryPreview' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerAttemptBoundaryAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerDispatchAdmissionAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerTransportAdmissionAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerExecutionBoundaryAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerExecutionIntentAcrossClients$' -count=1
  go test ./internal/appserver -run '^TestConfigRequiresExecuteActivationForLeaseRegistry$' -count=1
  go test ./internal/appserver -run '^TestConfig(ExplicitlyDisabledRunnerAuthority|RunnerExecutionAuthority)' -count=1
  go test ./internal/devicefabricgate ./internal/appserver -run '^Test(ZeroValueRequestKeepsFabricOff|ProposedInventoryFailsClosedWithStableReasons|AcceptedInventoryRequiresEveryEvidenceBoundary|AcceptedObserveAndExecuteAreDistinctStages|MigrationAndFederationRemainBlockedBySeparateDecisions|UnknownModeFailsClosed|ConfigRejectsExplicitDeviceFabricActivationWhileADRIsProposed|ConfigZeroValueDeviceFabricRemainsDefaultOff|AcceptedDeviceFabricActivationReloadsObservationImagesPerRead)$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteActivationMountsAdmissionRoutes$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteSchedulerLeaseClaimsAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerDispatchAdmissionAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerTransportAdmissionAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerExecutionBoundaryAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerAttemptBoundaryAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerExecutionIntentAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteExecutionReconciliationAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteSessionRunnerReceiptAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunExecutionEvidenceAcrossClients$' -count=1
  go test ./internal/devicefabricgate -run '^Test(ParseManifest|LoadManifestFile|Review)' -count=1
  go test ./internal/devicefabricgate -run '^TestRunnerExecutionGate' -count=1
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
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedInventoryActivationMountsOwnerScopedDeviceRoute$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteActivationMountsAdmissionRoutes$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteSchedulerLeaseClaimsAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerDispatchAdmissionAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerTransportAdmissionAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerExecutionBoundaryAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerAttemptBoundaryAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunnerExecutionIntentAcrossClients$' -count=1
  # §738 drives the real Runtime CLI through client-instance and inventory/resource convergence before the execution-intent candidate POST; the adjacent dispatch-plan CLI/TUI checks retain the same fresh pair boundary.
  FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_CONVERGENCE_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunnerExecutionIntentConvergenceE2EWhenConfigured$' -count=1
  FORGE_CLIENT_INSTANCE_RUNNER_DISPATCH_PLAN_CONVERGENCE_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunnerDispatchPlanConvergenceE2EWhenConfigured$' -count=1
  FORGE_CLIENT_INSTANCE_RUNNER_DISPATCH_PLAN_TUI_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunnerDispatchPlanTUIConvergenceE2EWhenConfigured$' -count=1
  # §751 drives Runtime CLI dispatch/transport admission through the real JWT
  # with session/resource plus inventory/resource freshness and zero POST on drift.
  FORGE_CLIENT_INSTANCE_RUNNER_ADMISSION_CLI_CONVERGENCE_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunnerAdmissionCLIConvergenceE2EWhenConfigured$' -count=1
  # §739 drives the shared Console Web/App/Mobile Gate through the same real JWT, with a fresh session/resource and inventory/resource image before the execution-intent candidate.
  FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_GATE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunnerExecutionIntentGateE2EWhenConfigured$' -count=1
  # §741 drives the real Runtime TUI through the same inventory/resource freshness boundary before its execution-intent candidate.
  FORGE_CLIENT_INSTANCE_RUNNER_EXECUTION_INTENT_TUI_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunnerExecutionIntentTUIConvergenceE2EWhenConfigured$' -count=1
  # §742 drives the real Console Web/App/Mobile Gate through client-instance, inventory/resource, and scheduler-preview target binding.
  FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_GATE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSchedulerSelectionPreviewGateE2EWhenConfigured$' -count=1
  # §671 proves the shared Console Sessions Gate accepts a real JWT pair before dispatch-plan POST; §743 also rejects targets absent from the owner resource image.
  FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_GATE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceDispatchPlanGateE2EWhenConfigured$' -count=1
  # §672/§705/§706/§709 prove scheduler-preview pair/inventory freshness; §707/§708 prove explicit CLI/TUI Prompt writes add inventory/resource convergence while hidden or drifting instances stop before POST.
  FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_CONVERGENCE_E2E=1 FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_TUI_E2E=1 FORGE_CLIENT_INSTANCE_SCHEDULER_PREVIEW_CLI_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSchedulerPreviewConvergenceE2EWhenConfigured$' -count=1
  # §667/§668/§673/§678 cover CLI/TUI/Web/App/Mobile lease ordering with hidden fail-before-POST.
  FORGE_CLIENT_INSTANCE_SCHEDULER_LEASE_PROJECTION_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSchedulerLeaseProjectionE2EWhenConfigured$' -count=1
  # §683 proves the Run/Attempt/lease preflight projection repeats pair and inventory/resource reads before POST and keeps hidden Web Runs request-free.
  FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_PROJECTION_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunAttemptLeaseDispatchPreflightProjectionE2EWhenConfigured$' -count=1
  # §686 proves the same selected client-instance boundary through a real Snaplink JWT Runtime CLI.
  FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_CLI_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunAttemptLeaseDispatchPreflightCLIProjectionE2EWhenConfigured$' -count=1
  # §687 proves the same selected client-instance boundary through a real Snaplink JWT Runtime TUI.
  FORGE_CLIENT_INSTANCE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_TUI_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceRunAttemptLeaseDispatchPreflightTUIConvergenceE2EWhenConfigured$' -count=1
  # §688 proves the authenticated owner-scoped Conversation change SSE/long-poll boundary.
  go test ./internal/appserver -run '^TestConversationChangesStreamRoute' -count=1
  # §689 proves the same change stream through a real Snaplink JWT while the ordinary constructor fails closed without a Runtime backend.
  go test ./internal/appserver -run '^TestSnaplinkAuthenticatedConversationChangesStreamE2E$' -count=1
  # §690 covers the explicit Runtime CLI/TUI one-page owner change SSE consumer.
  cargo test --manifest-path "$REPO_ROOT/forge-runtime/Cargo.toml" -p forge-runtime-cli conversation_changes_stream -- --nocapture
  cargo test --manifest-path "$REPO_ROOT/forge-runtime/Cargo.toml" -p forge-runtime-cli remote_tui_changes_stream -- --nocapture
  cargo test --manifest-path "$REPO_ROOT/forge-runtime/Cargo.toml" -p forge-runtime-cli remote_change_stream -- --nocapture
  # §692 keeps the Runtime TUI change feed behind a fresh selected
  # client-instance session/resource projection.
  cargo test --manifest-path "$REPO_ROOT/forge-runtime/Cargo.toml" -p forge-runtime-cli remote_tui_changes -- --nocapture
  # §693 drives Runtime CLI and TUI through the real Snaplink JWT SSE route.
  FORGE_RUNTIME_CONVERSATION_CHANGES_STREAM_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedRuntimeConversationChangesStreamE2EWhenConfigured$' -count=1
  # §694 drives the shared Flutter Console API through the same real Snaplink JWT SSE route.
  FORGE_CONVERSATION_CHANGES_STREAM_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedConsoleConversationChangesStreamE2EWhenConfigured$' -count=1
  # §696 drives the same owner-scoped SSE route from a real same-origin Chromium tab when Web E2E is enabled.
  if [[ "${FORGE_BROWSER_E2E:-0}" == "1" ]]; then
    if [[ -z "${FORGE_WEB_BUILD_DIR:-}" ]]; then
      printf 'FORGE_WEB_BUILD_DIR is required when FORGE_BROWSER_E2E=1 for Conversation SSE E2E\n' >&2
      exit 1
    fi
    FORGE_BROWSER_E2E=1 FORGE_CONVERSATION_CHANGES_STREAM_BROWSER_E2E=1 FORGE_WEB_BUILD_DIR="$FORGE_WEB_BUILD_DIR" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedConsoleConversationChangesStreamBrowserE2EWhenConfigured$' -count=1
  fi
  # §674/§707/§708/§710/§711/§729/§730/§731/§732/§733/§734/§735 cover real JWT Console Web/App/Mobile and Runtime CLI/TUI session create/Prompt writes after client-instance convergence; explicit Prompt writes converge inventory/resource before POST, missing resource scope blocks create before POST, TUI create requires and refreshes a converged session/resource pair before keeping a hidden create unselected, and TUI Prompt refreshes inventory/resource immediately before its visible POST.
  FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedClientInstanceSessionProjectionE2EWhenConfigured$' -count=1
  # §736 proves the Runtime TUI instance-filtered SSE feed refreshes the owner-bound pair, applies visible rows only, advances across hidden rows, and emits no write or device/execution request.
  FORGE_RUNTIME_CLIENT_INSTANCE_CHANGES_STREAM_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedRuntimeClientInstanceChangesStreamE2EWhenConfigured$' -count=1
  # §737 proves the Runtime CLI instance-filtered SSE feed refreshes the owner-bound pair, returns visible rows only, advances across hidden rows, and emits no write or device/execution request.
  FORGE_RUNTIME_CLIENT_INSTANCE_CHANGES_CLI_E2E=1 FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestSnaplinkAuthenticatedRuntimeClientInstanceChangesStreamCLIE2EWhenConfigured$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteExecutionReconciliationAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteSessionRunnerReceiptAcrossClients$' -count=1
  FORGE_CONSOLE_E2E=1 SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./internal/appserver -run '^TestRunAcceptedExecuteRunExecutionEvidenceAcrossClients$' -count=1
  FORGE_RUNTIME_BIN="$process_runtime_bin" go test ./cmd/forge-server -run '^TestForgeServerProcess(ContentionReuseAndInterrupt|AuthenticatedSharedSessionBoundary)$' -count=1
)

(
  cd "$SNAPLINK_ROOT"
  FORGE_SNAPLINK_PROFILE_FIXTURE="$FORGE_SNAPLINK_PROFILE_FIXTURE" go test ./config -run '^TestSnaplinkForgeProfileFixtureMatchesDistributedClients$' -count=1
  go test ./interfaces/sso -run '^TestRcov2DE_ForgeClientOwnerParity$' -count=1
)

(
  cd "$AERO_ID_ROOT"
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" internal/connector/auditgovernance/testdata/forge-run-observed-v1.json
  cmp "$FORGE_SESSION_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-shared-session-v1.json
  cmp "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" internal/connector/auditgovernance/testdata/forge-prompt-append-receipt-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" internal/connector/auditgovernance/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" internal/connector/auditgovernance/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" internal/connector/auditgovernance/testdata/forge-client-instance-session-resource-convergence-v1.json
  cmp "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-resource-convergence-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-scheduler-selection-preview-v1.json
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
  # §680 extends the archival preflight-request ABI to Aero-ID without granting execution authority.
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" internal/connector/auditgovernance/testdata/forge-run-attempt-lease-dispatch-preflight-request-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_LEASE_FENCING_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-lease-fencing-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" internal/connector/auditgovernance/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" internal/connector/auditgovernance/testdata/forge-execution-lease-registry-v1.json
  cmp "$FORGE_EXECUTION_LEASE_RELEASE_FIXTURE" internal/connector/auditgovernance/testdata/forge-execution-lease-release-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-dispatch-admission-v1.json
  cmp "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-transport-admission-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-execution-boundary-v1.json
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-local-execution-preview-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-execution-intent-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-execution-intent-request-v1.json
  cmp "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-command-digest-v1.json
  cmp "$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-command-terminal-receipt-v1.json
  cmp "$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-attempt-boundary-v1.json
  cmp "$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" internal/connector/auditgovernance/testdata/forge-attempt-lifecycle-v1.json
  cmp "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" internal/connector/auditgovernance/testdata/forge-runner-terminal-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" internal/connector/auditgovernance/testdata/forge-session-runner-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" internal/connector/auditgovernance/testdata/forge-session-runner-receipt-history-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" internal/connector/auditgovernance/testdata/forge-session-runner-reconciliation-projection-v1.json
  cmp "$FORGE_PLACEMENT_PARITY_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-placement-policy-parity-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v2.json
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" internal/connector/auditgovernance/testdata/forge-device-inventory-placement-input-v1.json
  go test ./internal/connector/auditgovernance -run '^TestForgeRunObserved|^TestForgeObserverCorrelationContract$|^TestPayloadStringOmitsUntrustedShapes$|^TestForgeRunExecutionEvidence|^TestForgePromptAcceptedAudit|^TestForgeClientInstance|^TestForgeRunAttemptLeaseDispatchPreflight|^TestForgeRunnerLeaseFencing' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgePreflightRequestArchive' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeClientInstanceSessionResourceConvergence' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceInventoryResourceConvergence' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeSharedSession' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgePromptAppendReceiptReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeExecutionLeaseCheckpoint' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeExecutionLeaseRegistryReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeExecutionLeaseReleaseReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerDispatchPlanPreview' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerDispatchAdmission' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerTransportAdmission' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerExecutionBoundary' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerLocalExecutionPreview' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerExecutionIntent' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerExecutionIntentRequest' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerCommandDigestVectors' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerCommandTerminalReceipt' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeRunnerAttemptBoundaryReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeAttemptLifecycleReceiver$' -count=1
  FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE="$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" go test ./internal/connector/auditgovernance -run '^TestForgeRunnerTerminalReceiptVectors' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeSessionRunnerReceiptVectors' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeSessionRunnerReceiptHistory' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeSessionRunnerReconciliationProjection' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDevicePlacementPolicyParity' -count=1
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
  go test ./internal/connector/auditgovernance -run '^TestForgeSchedulerSelectionPreviewReceiver$' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeDeviceIdentityProof' -count=1
  go test ./internal/connector/auditgovernance -run '^TestForgeSignedDeviceIdentityProof' -count=1
)

(
  cd "$AERO_IM_ROOT"
  cmp "$FORGE_SESSION_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-shared-session-v1.json
  cmp "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-prompt-append-receipt-v1.json
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-run-observed-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-client-instance-session-resource-convergence-v1.json
  cmp "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-resource-convergence-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-scheduler-selection-preview-v1.json
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
  # §681 keeps the bounded dispatch-preflight request ABI byte-identical in Aero-IM.
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-run-attempt-lease-dispatch-preflight-request-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-execution-lease-registry-v1.json
  cmp "$FORGE_EXECUTION_LEASE_RELEASE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-execution-lease-release-v1.json
  cargo test -p aero-audit-connector --test forge_observer_contract
  cargo test -p aero-audit-connector --test forge_shared_session_contract
  cargo test -p aero-audit-connector --test forge_prompt_append_receipt_contract
  cargo test -p aero-audit-connector --test forge_prompt_accepted_contract
  cargo test -p aero-audit-connector --test forge_execution_evidence_contract
  cargo test -p aero-audit-connector --test forge_client_instance_view_contract
  cargo test -p aero-audit-connector --test forge_client_instance_session_resource_convergence_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_resource_convergence_contract
  cargo test -p aero-audit-connector --test forge_device_inventory_registry_placement_contract
  cargo test -p aero-audit-connector --test forge_scheduler_selection_preview_contract
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
  cargo test -p aero-audit-connector --test forge_run_attempt_lease_dispatch_preflight_request_contract
  cmp "$FORGE_LEASE_FENCING_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-lease-fencing-v1.json
  cargo test -p aero-audit-connector --test forge_runner_lease_fencing_contract
  cargo test -p aero-audit-connector --test forge_execution_lease_checkpoint_contract
  cargo test -p aero-audit-connector --test forge_execution_lease_release_contract
  cargo test -p aero-audit-connector --test forge_execution_lease_registry_contract
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-dispatch-admission-v1.json
  cmp "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-transport-admission-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-execution-boundary-v1.json
  cargo test -p aero-audit-connector --test forge_runner_dispatch_plan_preview_contract
  cargo test -p aero-audit-connector --test forge_runner_dispatch_admission_contract
  cargo test -p aero-audit-connector --test forge_runner_transport_admission_contract
  cargo test -p aero-audit-connector --test forge_runner_execution_boundary_contract
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-local-execution-preview-v1.json
  cargo test -p aero-audit-connector --test forge_runner_local_execution_preview_contract
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-execution-intent-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-execution-intent-request-v1.json
  cmp "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-command-digest-v1.json
  cmp "$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-command-terminal-receipt-v1.json
  cmp "$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-attempt-boundary-v1.json
  cmp "$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-attempt-lifecycle-v1.json
  cmp "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-runner-terminal-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-session-runner-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-session-runner-receipt-history-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-session-runner-reconciliation-projection-v1.json
  cmp "$FORGE_PLACEMENT_PARITY_FIXTURE" crates/aero-audit-connector/tests/testdata/forge-device-placement-policy-parity-v1.json
  cargo test -p aero-audit-connector --test forge_runner_execution_intent_contract
  cargo test -p aero-audit-connector --test forge_runner_execution_intent_request_contract
  cargo test -p aero-audit-connector --test forge_runner_command_digest_vectors
  cargo test -p aero-audit-connector --test forge_runner_command_terminal_receipt_contract
  cargo test -p aero-audit-connector --test forge_runner_attempt_boundary_contract
  cargo test -p aero-audit-connector --test forge_attempt_lifecycle_contract
  cargo test -p aero-audit-connector --test forge_runner_terminal_receipt_vectors
  cargo test -p aero-audit-connector --test forge_session_runner_receipt_vectors
  cargo test -p aero-audit-connector --test forge_session_runner_receipt_history_contract
  cargo test -p aero-audit-connector --test forge_session_runner_reconciliation_projection_contract
  cargo test -p aero-audit-connector --test forge_device_placement_policy_parity_contract
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
  cmp "$FORGE_SESSION_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-shared-session-v1.json
  cmp "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" internal/auditgovernance/testdata/forge-prompt-append-receipt-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" internal/auditgovernance/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" internal/auditgovernance/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" internal/auditgovernance/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" internal/auditgovernance/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" internal/auditgovernance/testdata/forge-client-instance-session-resource-convergence-v1.json
  cmp "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-resource-convergence-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE" internal/auditgovernance/testdata/forge-scheduler-selection-preview-v1.json
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
  # §681 keeps the bounded dispatch-preflight request ABI byte-identical in Aero-Vault.
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" internal/auditgovernance/testdata/forge-run-attempt-lease-dispatch-preflight-request-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" internal/auditgovernance/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_LEASE_FENCING_FIXTURE" internal/auditgovernance/testdata/forge-runner-lease-fencing-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" internal/auditgovernance/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" internal/auditgovernance/testdata/forge-execution-lease-registry-v1.json
  cmp "$FORGE_EXECUTION_LEASE_RELEASE_FIXTURE" internal/auditgovernance/testdata/forge-execution-lease-release-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" internal/auditgovernance/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" internal/auditgovernance/testdata/forge-runner-dispatch-admission-v1.json
  cmp "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" internal/auditgovernance/testdata/forge-runner-transport-admission-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" internal/auditgovernance/testdata/forge-runner-execution-boundary-v1.json
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" internal/auditgovernance/testdata/forge-runner-local-execution-preview-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-runner-execution-intent-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" internal/auditgovernance/testdata/forge-runner-execution-intent-request-v1.json
  cmp "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" internal/auditgovernance/testdata/forge-runner-command-digest-v1.json
  cmp "$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-runner-command-terminal-receipt-v1.json
  cmp "$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" internal/auditgovernance/testdata/forge-runner-attempt-boundary-v1.json
  cmp "$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" internal/auditgovernance/testdata/forge-attempt-lifecycle-v1.json
  cmp "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" internal/auditgovernance/testdata/forge-runner-terminal-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" internal/auditgovernance/testdata/forge-session-runner-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" internal/auditgovernance/testdata/forge-session-runner-receipt-history-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" internal/auditgovernance/testdata/forge-session-runner-reconciliation-projection-v1.json
  cmp "$FORGE_PLACEMENT_PARITY_FIXTURE" internal/auditgovernance/testdata/forge-device-placement-policy-parity-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-evaluation-v2.json
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" internal/auditgovernance/testdata/forge-device-inventory-placement-input-v1.json
  go test ./internal/auditgovernance -run '^TestForgeRunObserved|^TestForgeRunExecutionEvidence|^TestForgePromptAcceptedAudit|^TestForgeClientInstance|^TestForgeRunAttemptLeaseDispatchPreflight|^TestForgeRunnerLeaseFencing' -count=1
  go test ./internal/auditgovernance -run '^TestForgePreflightRequestArchive' -count=1
  go test ./internal/auditgovernance -run '^TestForgeClientInstanceSessionResourceConvergence' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceInventoryResourceConvergence' -count=1
  go test ./internal/auditgovernance -run '^TestForgeSharedSession' -count=1
  go test ./internal/auditgovernance -run '^TestForgePromptAppendReceiptReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeExecutionLeaseCheckpoint' -count=1
  go test ./internal/auditgovernance -run '^TestForgeExecutionLeaseRegistryReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeExecutionLeaseReleaseReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerDispatchPlanPreview' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerDispatchAdmission' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerExecutionBoundary' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerTransportAdmission' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerLocalExecutionPreview' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerExecutionIntent' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerExecutionIntentRequest' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerCommandDigestVectors' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerCommandTerminalReceipt' -count=1
  go test ./internal/auditgovernance -run '^TestForgeRunnerAttemptBoundaryReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeAttemptLifecycleReceiver$' -count=1
  FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE="$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" go test ./internal/auditgovernance -run '^TestForgeRunnerTerminalReceiptVectors' -count=1
  go test ./internal/auditgovernance -run '^TestForgeSessionRunnerReceiptVectors' -count=1
  go test ./internal/auditgovernance -run '^TestForgeSessionRunnerReceiptHistory' -count=1
  go test ./internal/auditgovernance -run '^TestForgeSessionRunnerReconciliationProjection' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDevicePlacementPolicyParity' -count=1
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
  go test ./internal/auditgovernance -run '^TestForgeSchedulerSelectionPreviewReceiver$' -count=1
  go test ./internal/auditgovernance -run '^TestForgeDeviceIdentityProof' -count=1
  go test ./internal/auditgovernance -run '^TestForgeSignedDeviceIdentityProof' -count=1
)

(
  cd "$AUDIT_GOVERNANCE_ROOT"
  cmp "$FORGE_RUN_OBSERVED_FIXTURE" internal/domain/testdata/forge-run-observed-v1.json
  cmp "$FORGE_SESSION_CONTRACT_FIXTURE" internal/domain/testdata/forge-shared-session-v1.json
  cmp "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" internal/domain/testdata/forge-prompt-append-receipt-v1.json
  cmp "$FORGE_PROMPT_ACCEPTED_AUDIT_FIXTURE" internal/domain/testdata/forge-prompt-accepted-audit-v1.json
  cmp "$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" internal/domain/testdata/forge-run-execution-evidence-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_VIEW_FIXTURE" internal/domain/testdata/forge-client-instance-session-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_RESOURCE_VIEW_FIXTURE" internal/domain/testdata/forge-client-instance-resource-view-v1.json
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" internal/domain/testdata/forge-client-instance-session-resource-convergence-v1.json
  cmp "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" internal/domain/testdata/forge-device-inventory-resource-convergence-v1.json
  cmp "$FORGE_REGISTRY_PLACEMENT_PREVIEW_FIXTURE" internal/domain/testdata/forge-device-inventory-registry-placement-preview-v1.json
  cmp "$FORGE_SCHEDULER_SELECTION_PREVIEW_FIXTURE" internal/domain/testdata/forge-scheduler-selection-preview-v1.json
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
  # §677 keeps the scheduler-to-dispatch request itself auditable without granting Audit Governance execution authority.
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" internal/domain/testdata/forge-run-attempt-lease-dispatch-preflight-request-v1.json
  cmp "$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" internal/domain/testdata/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json
  cmp "$FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-heartbeat-persistence-contract-v1.json
  cmp "$FORGE_INVENTORY_PERSISTENCE_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-persistence-v1.json
  cmp "$FORGE_INVENTORY_STATUS_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-status-contract-v1.json
  cmp "$FORGE_INVENTORY_SNAPSHOT_CANONICAL_FIXTURE" internal/domain/testdata/forge-device-inventory-snapshot-canonical-v1.json
  cmp "$FORGE_INVENTORY_PERSISTED_OBSERVATION_FIXTURE" internal/domain/testdata/forge-device-inventory-persisted-observation-v1.json
  cmp "$FORGE_LEASE_FENCING_FIXTURE" internal/domain/testdata/forge-runner-lease-fencing-v1.json
  cmp "$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" internal/domain/testdata/forge-execution-lease-checkpoint-v1.json
  cmp "$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" internal/domain/testdata/forge-execution-lease-registry-v1.json
  cmp "$FORGE_EXECUTION_LEASE_RELEASE_FIXTURE" internal/domain/testdata/forge-execution-lease-release-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" internal/domain/testdata/forge-runner-dispatch-plan-preview-v1.json
  cmp "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" internal/domain/testdata/forge-runner-dispatch-admission-v1.json
  cmp "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" internal/domain/testdata/forge-runner-transport-admission-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" internal/domain/testdata/forge-runner-execution-boundary-v1.json
  cmp "$FORGE_RUNNER_LOCAL_EXECUTION_PREVIEW_FIXTURE" internal/domain/testdata/forge-runner-local-execution-preview-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_CONTRACT_FIXTURE" internal/domain/testdata/forge-runner-execution-intent-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" internal/domain/testdata/forge-runner-execution-intent-request-v1.json
  cmp "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" internal/domain/testdata/forge-runner-command-digest-v1.json
  cmp "$FORGE_RUNNER_COMMAND_CONTRACT_FIXTURE" internal/domain/testdata/forge-runner-command-terminal-receipt-v1.json
  cmp "$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" internal/domain/testdata/forge-runner-attempt-boundary-v1.json
  cmp "$FORGE_ATTEMPT_LIFECYCLE_FIXTURE" internal/domain/testdata/forge-attempt-lifecycle-v1.json
  cmp "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" internal/domain/testdata/forge-runner-terminal-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" internal/domain/testdata/forge-session-runner-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" internal/domain/testdata/forge-session-runner-receipt-history-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" internal/domain/testdata/forge-session-runner-reconciliation-projection-v1.json
  cmp "$FORGE_PLACEMENT_PARITY_FIXTURE" internal/domain/testdata/forge-device-placement-policy-parity-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_BATCH_EVALUATION_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-batch-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_EVALUATION_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-evaluation-v1.json
  cmp "$FORGE_INVENTORY_PLACEMENT_V2_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-evaluation-v2.json
  cmp "$FORGE_INVENTORY_PLACEMENT_INPUT_CONTRACT_FIXTURE" internal/domain/testdata/forge-device-inventory-placement-input-v1.json
  go test ./internal/domain -run '^TestForgeRunObserver|^TestForgeRunExecutionEvidence|^TestForgePromptAccepted|^TestForgeClientInstance|^TestForgeRunAttemptLeaseDispatchPreflight|^TestForgeRunnerLeaseFencing' -count=1
  go test ./internal/domain -run '^TestForgePreflightRequestArchive' -count=1
  go test ./internal/domain -run '^TestForgeClientInstanceSessionResourceConvergence' -count=1
  go test ./internal/domain -run '^TestForgeDeviceInventoryResourceConvergence' -count=1
  go test ./internal/domain -run '^TestForgeSharedSession' -count=1
  go test ./internal/domain -run '^TestForgePromptAppendReceiptReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeExecutionLeaseCheckpoint' -count=1
  go test ./internal/domain -run '^TestForgeExecutionLeaseRegistryReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeExecutionLeaseReleaseReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeRunnerDispatchPlanPreview' -count=1
  go test ./internal/domain -run '^TestForgeRunnerDispatchAdmission' -count=1
  go test ./internal/domain -run '^TestForgeRunnerExecutionBoundary' -count=1
  go test ./internal/domain -run '^TestForgeRunnerTransportAdmission' -count=1
  go test ./internal/domain -run '^TestForgeRunnerLocalExecutionPreview' -count=1
  go test ./internal/domain -run '^TestForgeRunnerExecutionIntent' -count=1
  go test ./internal/domain -run '^TestForgeRunnerExecutionIntentRequest' -count=1
  go test ./internal/domain -run '^TestForgeRunnerCommandDigestVectors' -count=1
  go test ./internal/domain -run '^TestForgeRunnerCommandTerminalReceipt' -count=1
  go test ./internal/domain -run '^TestForgeRunnerAttemptBoundaryReceiver$' -count=1
  go test ./internal/domain -run '^TestForgeAttemptLifecycleReceiver$' -count=1
  FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE="$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" go test ./internal/domain -run '^TestForgeRunnerTerminalReceiptVectors' -count=1
  go test ./internal/domain -run '^TestForgeSessionRunnerReceiptVectors' -count=1
  go test ./internal/domain -run '^TestForgeSessionRunnerReceiptHistory' -count=1
  go test ./internal/domain -run '^TestForgeSessionRunnerReconciliationProjection' -count=1
  go test ./internal/domain -run '^TestForgeDevicePlacementPolicyParity' -count=1
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
  go test ./internal/domain -run '^TestForgeSchedulerSelectionPreviewReceiver$' -count=1
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
  cargo test -p forge-runtime-cli --bin forge-runtime prompt_append_receipt
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::tests::prompt_writes::prompt_receipt_binds_backend_prompt_before_projection
  cargo test -p forge-runtime-cli --bin forge-runtime remote_prompt_receipt
  cargo test -p forge-runtime-cli --bin forge-runtime runner_receipt_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_runner_terminal_receipt_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime runner_lease_fencing_preview
  cargo test -p forge-runtime-cli --bin forge-runtime execution_lease_checkpoint_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_runner_lease_fencing_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_execution_lease_checkpoint_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime runner_execution_intent_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_runner_execution_intent_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime session_runner_receipt_preview
  cargo test -p forge-runtime-cli --bin forge-runtime device_session_runner_receipt_history_command
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_reduce_session_runner_receipt_history_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_session_runner_receipt_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_session_runner_receipt_history
  cargo test -p forge-runtime-cli --bin forge-runtime remote_session_runner_reconciliation
  cargo test -p forge-runtime-cli --bin forge-runtime session_runner_receipt_preview_posts_the_bound_request_once
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_session_runner_receipt_preview_uses_the_authenticated_session_route
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_a_session_runner_receipt_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime run_execution_evidence
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_run_execution_evidence_preview_uses_the_authenticated_session_route
  # §744 covers the selected Runtime TUI local Runner preview's converged
  # inventory/resource and device/Runner target guard.
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
  cargo test -p forge-runtime-cli --bin forge-runtime converged_inventory_resource_read
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_consumes_the_canonical_inventory_resource_pair_during_sync
  # §707 keeps online CLI Prompt add/receipt writes behind the strict
  # inventory/resource convergence read while preserving local and unfiltered paths.
  cargo test -p forge-runtime-cli --bin forge-runtime unfiltered_prompt_add_keeps_the_single_prompt_post
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_prompt_add_posts_after_projection_check
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_prompt_receipt_reads_inventory_after_projection_check
  cargo test -p forge-runtime-cli --bin forge-runtime inventory_drifted_visible_instance_prompt_add_is_rejected_before_post
  # §714 keeps an online instance-scoped pending Run-intent submit behind the
  # strict inventory/resource reread and rejects drift before the candidate POST.
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_pending_run_intent_submit_reads_inventory_after_pair_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime inventory_resource_drift_blocks_visible_pending_run_intent_submit
  # §715 keeps online instance-scoped scheduler lease claim/renew behind the
  # strict inventory/resource reread while preserving local/unfiltered paths.
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_scheduler_lease_claim_reads_converged_pair_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_scheduler_lease_renewal_reads_inventory_after_pair_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime inventory_drifted_visible_instance_scheduler_lease_claim_is_rejected_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime inventory_drifted_visible_instance_scheduler_lease_renewal_is_rejected_before_post
  # §716 refreshes an explicitly opened TUI inventory/resource pair before a
  # pending Run-intent submit or retry and retains the pending intent on drift.
  cargo test -p forge-runtime-cli --bin forge-runtime explicit_instance_pending_run_intent_retry_refreshes_inventory_resource_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime explicit_instance_pending_run_intent_retry_keeps_pending_on_inventory_resource_drift
  # §717 refreshes an explicitly opened TUI inventory/resource pair before a
  # candidate scheduler lease claim/renew and rechecks instance visibility.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_scheduler_selection_lease_refreshes_explicit_inventory_resource_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_scheduler_selection_lease_renewal_drift_blocks_post
  # §718 keeps Runner metadata previews behind the same fresh device-resource
  # boundary across CLI/TUI and the opt-in shared Console candidate Gate.
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_runner_execution_intent_reads_converged_pair_before_candidate_post
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_runner_execution_intent_blocks_inventory_resource_drift_before_candidate_post
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_runner_dispatch_plan_reads_converged_pair_before_candidate_post
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_run_attempt_preflight_reads_converged_pair_before_candidate_post
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_runner_execution_intent_refreshes_explicit_pair_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_runner_execution_intent_blocks_pair_drift_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_run_attempt_lease_preflight_refreshes_pair_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_run_attempt_lease_preflight_blocks_refresh_drift_before_post
  # §745 keeps a selected TUI dispatch-plan behind a converged resource pair
  # and rejects lease/intent/placement targets absent from that image.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_blocks_selected_instance_dispatch_target_absent_from_resource
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_requires_converged_resource_pair_for_selected_dispatch_plan
  # §746 keeps selected-instance Runner dispatch admission behind the same
  # converged resource pair and lease-proof target boundary.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_dispatch_admission_requires_selected_resource_pair
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_dispatch_admission_blocks_target_absent_from_resource
  # §747 keeps selected-instance Runner transport admission behind the same
  # converged resource pair and lease-proof target boundary.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_transport_admission_requires_selected_resource_pair
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_transport_admission_blocks_target_absent_from_resource
  # §750 keeps the non-interactive Runtime CLI admission previews behind an
  # explicit client-instance resource target gate while preserving unfiltered
  # input and validating local resource/converged fixtures.
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_runner_admission
  cargo test -p forge-runtime-cli --bin forge-runtime local_runner_admission
  cargo test -p forge-runtime-cli --bin forge-runtime remote_runner_admission_preview_accepts_instance_projection
  # §752 keeps a selected Runtime CLI execution-boundary candidate behind the
  # same refreshed resource target gate while preserving unfiltered input.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_runner_execution_boundary_preview_parses_a_bounded_request_file
  cargo test -p forge-runtime-cli --bin forge-runtime accepts_instance_projection_and_rejects_partial_options
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_execution_boundary_refreshes_resource_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime foreign_instance_execution_boundary_stops_before_candidate_post
  # §752/§753 keeps the selected Runtime TUI execution-boundary candidate
  # behind the refreshed resource target gate and zero-POST foreign path.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_execution_boundary_requires_selected_resource_pair
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_execution_boundary_blocks_foreign_resource_target
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_execution_boundary_accepts_resource_device_target_before_post
  flutter test test/forge_sessions_gate_runner_execution_intent_candidate_test.dart
  # §708 keeps an explicitly selected TUI Prompt append/retry behind the
  # already-open atomic inventory/resource convergence pair.
  cargo test -p forge-runtime-cli --bin forge-runtime explicit_instance_prompt_retry
  # §709 refreshes an explicitly opened TUI inventory/resource pair before a
  # planning-only scheduler preview while keeping the one-sided path offline.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_scheduler_selection_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_read_authenticated_client_instance
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_show_converged
  cargo test -p forge-runtime-cli --bin forge-runtime execution_consent
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_execution_consent_preview
  cargo test -p forge-runtime-cli --bin forge-runtime execution_reconciliation
  cargo test -p forge-runtime-cli --bin forge-runtime lifecycle_registry
  cargo test -p forge-runtime-cli --bin forge-runtime remote_command::placement_registry::tests
  cargo test -p forge-runtime-cli --bin forge-runtime remote_registry_placement_preview_parses_a_requirements_file
  cargo test -p forge-runtime-cli --bin forge-runtime registry_placement_preview_posts_requirements_once_and_validates_v2_response
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_registry_placement_preview_posts_requirements_and_renders_no_selection
  cargo test -p forge-runtime-cli --bin forge-runtime validates_strict_request_and_receipt
  cargo test -p forge-runtime-cli --bin forge-runtime remote_scheduler_lease_lifecycle_accepts_instance_session_resource_projection
  cargo test -p forge-runtime-cli --bin forge-runtime remote_scheduler_lease_lifecycle_rejects_missing_instance_and_duplicate_projection_options
  cargo test -p forge-runtime-cli --bin forge-runtime scheduler_selection_lease_posts_once_with_explicit_idempotency
  cargo test -p forge-runtime-cli --bin forge-runtime visible_instance_scheduler_lease_claim_reads_converged_pair_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime hidden_instance_scheduler_lease_claim_is_rejected_before_post
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_scheduler_selection_lease_posts_once_and_withholds_fencing_token
  cargo test -p forge-runtime-cli --bin forge-runtime scheduler_selection_lease_release_posts_once_with_explicit_idempotency
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_scheduler_selection_lease_release_posts_once_and_withholds_fencing_token
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_run_execution_evidence_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime run_observed_preview
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_run_observed_without_a_device_request
  cargo test -p forge-runtime-cli --bin forge-runtime device_attempt_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_can_preview_attempt_request_without_a_network_request
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_attempt_request_preview_requires_a_file_path
  cargo test -p forge-runtime-cli --bin forge-runtime tui_sync_refreshes_the_selected_run_timeline_incrementally
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui
  cargo test -p forge-runtime-domain --lib placement_parity
  cargo test -p forge-runtime-domain --lib prompt_append_receipt
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
  cargo test -p forge-runtime-domain --lib runner_command_digest_vectors
  cargo test -p forge-runtime-domain --lib runner_terminal_receipt_vectors
  cargo test -p forge-runtime-domain --lib session_runner_receipt_vectors
  cargo test -p forge-runtime-cli --bin forge-runtime runner_dispatch_plan_preview
  cargo test -p forge-runtime-cli --bin forge-runtime runner_dispatch_admission
  cargo test -p forge-runtime-cli runner_transport_admission -- --nocapture
  cargo test -p forge-runtime-cli runner_execution_boundary -- --nocapture
  cargo test -p forge-runtime-cli --bin forge-runtime device_runner_attempt_boundary
  cargo test -p forge-runtime-cli --bin forge-runtime runner_attempt_boundary
  cargo test -p forge-runtime-cli runner_attempt_boundary_remote -- --nocapture
  cargo test -p forge-runtime-domain --lib session_runner_receipt
  cargo test -p forge-runtime-domain --lib session_runner_receipt_history
  cargo test -p forge-runtime-domain --lib session_runner_reconciliation_projection
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
  # §682 pins the canonical CLI/TUI dispatch-preflight request bytes and rejects
  # unknown, duplicate, trailing, selected-target, lease-target, and authority mutations.
  cargo test -p forge-runtime-cli --bin forge-runtime canonical_request_fixture_bytes_reject_all_unsafe_mutations
  # §684 keeps the Runtime TUI Run/Attempt/lease preflight request-free when its
  # explicit inventory/resource observation pair has drifted.
  cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_blocks_run_attempt_lease_preflight_after_inventory_resource_drift
  # §685 keeps the authenticated CLI Run/Attempt/lease preflight behind the
  # selected owner-bound client-instance pair before its candidate POST.
  cargo test -p forge-runtime-cli --bin forge-runtime instance_run_attempt_preflight
  cargo test -p forge-runtime-domain --lib enrollment_heartbeat_lifecycle_contract
  FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE="$FORGE_DEVICE_ENROLLMENT_HEARTBEAT_LIFECYCLE_PERSISTENCE_FIXTURE" cargo test -p forge-runtime-domain --lib enrollment_heartbeat_lifecycle_persistence_contract
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE="$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" cargo test -p forge-runtime-domain --lib run_attempt_lease_dispatch_preflight
)

(
  cd "$SNAPLINK_CONSOLE_ROOT"
  cmp "$FORGE_SESSION_CONTRACT_FIXTURE" docs/contracts/fixtures/forge-shared-session-v1.json
  cmp "$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" docs/contracts/fixtures/forge-prompt-append-receipt-v1.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_FIXTURE" docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json
  cmp "$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json
  python3 -m py_compile android/tests/run_forge_shared_session_instrumentation.py android/tests/test_forge_shared_session_instrumentation.py android/tests/run_forge_admission_contract.py ios/tests/validate_forge_ios_shared_session_input.py ios/tests/test_forge_native_shared_session_contract.py
  bash -n ios/tests/run_forge_shared_session_acceptance.sh ios/tests/run_forge_admission_contract.sh
  python3 android/tests/run_forge_shared_session_instrumentation.py
  ios/tests/run_forge_shared_session_acceptance.sh
  # §749 keeps native Android/iOS admission evidence metadata-only and
  # request-free while binding candidate targets to the resource image.
  python3 android/tests/run_forge_admission_contract.py
  ios/tests/run_forge_admission_contract.sh
  # §754 keeps Android's opt-in shared-session Prompt input on HTTPS or
  # loopback HTTP before a bearer can enter the explicit emulator path.
  python3 -m unittest android/tests/test_forge_shared_session_instrumentation.py
  flutter test test/forge_preflight_fixture_test.dart
  FORGE_PROMPT_APPEND_RECEIPT_FIXTURE="$FORGE_PROMPT_APPEND_RECEIPT_FIXTURE" flutter test test/forge_prompt_append_receipt_contract_test.dart
  flutter test test/forge_prompt_append_receipt_api_test.dart
  flutter test test/forge_sessions_gate_prompt_append_receipt_candidate_test.dart
  FORGE_RUNNER_DISPATCH_PLAN_FIXTURE="$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" flutter test test/forge_runner_dispatch_plan_preview_contract_test.dart
  cmp "$FORGE_RUNNER_DISPATCH_ADMISSION_FIXTURE" docs/contracts/fixtures/forge-runner-dispatch-admission-v1.json
  flutter test test/forge_runner_dispatch_admission_contract_test.dart
  cmp "$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" docs/contracts/fixtures/forge-runner-transport-admission-v1.json
  cmp "$FORGE_RUNNER_EXECUTION_BOUNDARY_FIXTURE" docs/contracts/fixtures/forge-runner-execution-boundary-v1.json
  cmp "$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" docs/contracts/fixtures/forge-runner-command-digest-v1.json
  cmp "$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" docs/contracts/fixtures/forge-runner-terminal-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" docs/contracts/fixtures/forge-session-runner-receipt-vectors-v1.json
  cmp "$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json
  FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE="$FORGE_RUNNER_TRANSPORT_ADMISSION_FIXTURE" flutter test test/forge_runner_transport_admission_contract_test.dart
  flutter test test/forge_runner_transport_admission_api_test.dart
  flutter test test/forge_runner_transport_admission_api_e2e_test.dart
  flutter test test/forge_sessions_gate_runner_transport_admission_candidate_test.dart
  flutter test test/forge_runner_execution_boundary_contract_test.dart
  FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE="$FORGE_RUNNER_ATTEMPT_BOUNDARY_FIXTURE" flutter test test/forge_runner_attempt_boundary_contract_test.dart
  flutter test test/forge_runner_attempt_boundary_card_test.dart
  flutter test test/forge_sessions_gate_runner_attempt_boundary_projection_test.dart
  flutter test test/forge_runner_attempt_boundary_api_test.dart
  flutter test test/forge_runner_attempt_boundary_api_e2e_test.dart
  flutter test test/forge_runner_attempt_boundary_gate_e2e_test.dart
  flutter test test/forge_sessions_gate_runner_attempt_boundary_candidate_test.dart
  flutter test test/forge_runner_execution_boundary_api_test.dart
  flutter test test/forge_sessions_gate_runner_execution_boundary_candidate_test.dart
  flutter test test/forge_runner_dispatch_admission_api_test.dart
  flutter test test/forge_runner_dispatch_admission_api_e2e_test.dart
  flutter test test/forge_sessions_gate_runner_dispatch_admission_candidate_test.dart
  flutter test test/forge_run_attempt_lease_dispatch_preflight_api_test.dart
  # §679 proves the Console preflight candidate re-reads the selected
  # client-instance pair and remains request-free when that Run is hidden.
  flutter test test/forge_sessions_gate_preflight_candidate_test.dart
  FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE="$FORGE_RUN_ATTEMPT_LEASE_DISPATCH_PREFLIGHT_REQUEST_FIXTURE" FORGE_RUNNER_DISPATCH_PLAN_FIXTURE="$FORGE_RUNNER_DISPATCH_PLAN_FIXTURE" flutter test test/forge_sessions_gate_runner_dispatch_plan_candidate_test.dart
  # §743 binds every dispatch-plan target identity to the owner resource image
  # while preserving the request-compatible path when no resource reader is configured.
  flutter test test/forge_sessions_gate_runner_dispatch_plan_resource_binding_test.dart
  # §748 keeps Console dispatch/transport admission behind the selected
  # client-instance resource image while preserving the no-reader path.
  flutter test test/forge_sessions_gate_runner_admission_resource_binding_test.dart
  # §755 keeps Console Web/App/Mobile execution-boundary previews behind a
  # freshly refreshed selected-instance session/resource image; foreign
  # targets and resource drift must remain zero-POST.
  flutter test test/forge_sessions_gate_runner_execution_boundary_candidate_test.dart
  # §756 drives the accepted Forge Core EXECUTE harness with a real Snaplink
  # JWT and proves Web/App/Mobile session/resource convergence before one
  # display-only execution-boundary preview.
  flutter test test/forge_client_instance_runner_execution_boundary_convergence_e2e_test.dart
  # §757 keeps Console Web/App/Mobile Attempt-boundary previews behind the
  # refreshed selected-instance and inventory/resource images; foreign targets
  # must remain zero-POST.
  flutter test test/forge_sessions_gate_runner_attempt_boundary_candidate_test.dart
  flutter test test/forge_run_attempt_lease_dispatch_preflight_api_e2e_test.dart
  flutter test test/forge_conversations_api_test.dart
  # §689 covers the optional strict owner change SSE/long-poll client; the Sessions UI remains on polling.
  flutter test test/forge_conversation_changes_stream_api_test.dart
  # §691 covers the opt-in Sessions stream consumer, 204 reconnect, and polling fallback.
  flutter test test/forge_sessions_change_stream_widget_test.dart
  flutter test test/forge_conversations_models_test.dart
  FORGE_SESSION_CONTRACT_FIXTURE="$SNAPLINK_CONSOLE_ROOT/docs/contracts/fixtures/forge-shared-session-v1.json" flutter test test/forge_conversations_contract_test.dart
  flutter test test/forge_run_observer_resume_contract_test.dart
  flutter test test/forge_run_observed_contract_test.dart
  flutter test test/forge_run_observed_widget_test.dart
  flutter test test/forge_candidate_resource_status_transport_test.dart
  # Pass the canonical fixture so the Console contract test exercises the
  # same byte-level envelope as the CLI/Rust and Go consumers. Without this
  # variable its fixture-consumption case intentionally skips.
  FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE="$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" flutter test test/forge_run_execution_evidence_contract_test.dart
  flutter test test/forge_run_execution_evidence_widget_test.dart
  FORGE_RUN_OBSERVED_CONTRACT_FIXTURE="$FORGE_RUN_OBSERVED_FIXTURE" FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE" FORGE_RUN_EXECUTION_EVIDENCE_CONTRACT_FIXTURE="$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" flutter test test/forge_sessions_gate_run_execution_evidence_candidate_test.dart
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
  flutter test test/forge_client_instance_session_resource_convergence_api_test.dart
  cmp "$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" docs/contracts/fixtures/forge-client-instance-session-resource-convergence-v1.json
  FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE="$FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_FIXTURE" flutter test test/forge_client_instance_session_resource_convergence_contract_test.dart
  cmp "$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json
  # §700 revalidates manually constructed v2 inventory/resource observations
  # through the strict decoders before accepting a local convergence proof.
  FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE="$FORGE_DEVICE_INVENTORY_RESOURCE_CONVERGENCE_FIXTURE" flutter test test/forge_device_inventory_resource_convergence_contract_test.dart
  flutter test test/forge_sessions_gate_client_instance_session_resource_convergence_test.dart
  # §711 forces a fresh composed inventory/resource read at the Console Prompt
  # write boundary and rejects drift without invoking the Prompt submitter.
  flutter test test/forge_sessions_gate_prompt_inventory_journey_test.dart
  # §712 forces a fresh inventory/resource pair before the planning-only
  # scheduler-preview POST and rejects a revision drift with zero POSTs.
  flutter test test/forge_sessions_gate_scheduler_selection_client_instance_candidate_test.dart
  # §713's real-JWT Console scheduler Gate is exercised by the configured Go
  # activation E2E; this local target keeps the same owner-bound readers.
  flutter test test/forge_scheduler_selection_preview_gate_e2e_test.dart
  flutter test test/forge_sessions_initial_client_instance_test.dart
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
  flutter test test/forge_scheduler_selection_preview_api_test.dart
  flutter test test/forge_sessions_gate_scheduler_selection_candidate_test.dart
  # §675 proves the selected Console instance converges independent session/resource readers before scheduler preview.
  flutter test test/forge_sessions_gate_scheduler_selection_client_instance_candidate_test.dart
  flutter test test/forge_scheduler_selection_lease_api_test.dart
  # §676 proves a composed inventory/resource image converges with the selected client-instance pair before scheduler lease.
  flutter test test/forge_sessions_gate_scheduler_selection_lease_candidate_test.dart
  flutter test test/forge_sessions_gate_scheduler_selection_lease_candidate_test.dart --plain-name 'explicit Gate performs one scheduler lease release POST'
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
  flutter test test/forge_device_inventory_resource_convergence_api_test.dart
  flutter test test/forge_device_inventory_v2_gate_test.dart
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
  FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE="$FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE" flutter test test/forge_runner_execution_intent_request_contract_test.dart
  FORGE_RUNNER_COMMAND_DIGEST_FIXTURE="$FORGE_RUNNER_COMMAND_DIGEST_FIXTURE" flutter test test/forge_runner_command_digest_vectors_contract_test.dart
  FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE="$FORGE_RUNNER_TERMINAL_RECEIPT_VECTORS_FIXTURE" flutter test test/forge_runner_terminal_receipt_vectors_contract_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" flutter test test/forge_session_runner_receipt_vectors_contract_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_VECTORS_FIXTURE" flutter test test/forge_session_runner_receipt_vectors_widget_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" flutter test test/forge_session_runner_receipt_history_contract_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" flutter test test/forge_session_runner_receipt_history_widget_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" flutter test test/forge_session_runner_receipt_history_api_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" flutter test test/forge_sessions_gate_session_runner_receipt_history_candidate_test.dart
  FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE="$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" flutter test test/forge_session_runner_reconciliation_projection_contract_test.dart
  FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE="$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" flutter test test/forge_session_runner_reconciliation_projection_widget_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE="$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" flutter test test/forge_session_runner_reconciliation_projection_api_test.dart
  FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_HISTORY_FIXTURE" FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE="$FORGE_SESSION_RUNNER_RECONCILIATION_PROJECTION_FIXTURE" flutter test test/forge_sessions_gate_session_runner_reconciliation_projection_candidate_test.dart
  flutter test test/forge_runner_execution_intent_api_e2e_test.dart
  flutter test test/forge_runner_execution_intent_gate_e2e_test.dart
  flutter test test/forge_sessions_gate_runner_execution_intent_candidate_test.dart
  flutter test test/forge_runner_lease_fencing_contract_test.dart
  FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE="$FORGE_EXECUTION_LEASE_REGISTRY_FIXTURE" flutter test test/forge_execution_lease_registry_contract_test.dart
  FORGE_LEASE_FENCING_FIXTURE="$FORGE_LEASE_FENCING_FIXTURE" flutter test test/forge_runner_lease_fencing_preview_widget_test.dart
  FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE="$FORGE_EXECUTION_LEASE_CHECKPOINT_FIXTURE" flutter test test/forge_execution_lease_checkpoint_contract_test.dart
  flutter test test/forge_execution_lease_checkpoint_preview_widget_test.dart
  flutter test test/forge_runner_terminal_receipt_contract_test.dart
  flutter test test/forge_session_runner_receipt_observation_contract_test.dart
  flutter test test/forge_session_runner_receipt_observation_api_test.dart
  FORGE_RUN_EXECUTION_EVIDENCE_CONTRACT_FIXTURE="$FORGE_RUN_EXECUTION_EVIDENCE_FIXTURE" FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE="$FORGE_SESSION_RUNNER_RECEIPT_CONTRACT_FIXTURE" flutter test test/forge_run_execution_evidence_api_test.dart
  flutter test test/forge_run_execution_evidence_api_e2e_test.dart
  flutter test test/forge_local_runner_preview_api_test.dart
  flutter test test/forge_sessions_gate_local_runner_preview_candidate_test.dart
  flutter test test/forge_sessions_run_intent_observation_test.dart
  flutter test test/forge_sessions_deep_link_test.dart
  flutter test test/forge_runs_widget_test.dart
  flutter test test/forge_sessions_route_test.dart
  flutter test test/forge_sessions_widget_test.dart
  # §689/§691/§695/§697 cover the strict Console SSE adapter, opt-in Sessions
  # consumer, shared Gate forwarding, and selected-instance revocation while
  # keeping polling the default.
  flutter test test/forge_conversation_changes_stream_api_test.dart
  flutter test test/forge_sessions_change_stream_widget_test.dart
  flutter test test/forge_sessions_gate_change_stream_config_test.dart
  # §719 keeps owner-feed cursors advancing across rows hidden by an
  # explicitly selected client-instance projection while preventing private
  # Prompt/Run hydration for the hidden Conversation.
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime changes_instance)
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime remote_change_)
  flutter test test/forge_sessions_change_stream_widget_test.dart
  # §720 proves the candidate-only heartbeat → persisted lifecycle image →
  # accepted inventory/resource read journey while keeping heartbeat writes
  # outside the accepted route assembly.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestLifecycleHeartbeatCandidateFeedsAcceptedInventoryAndResourceViews$' -count=1)
  # §721 proves that accepted route assembly rechecks ADR and device-fabric
  # review evidence even when a persisted lifecycle image is present.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestAcceptedDeviceFabricAssemblyRechecksReviewEvidence$' -count=1)
  # §722 keeps accepted INVENTORY/OBSERVE lifecycle mutation candidates closed
  # while the independent device-credential transport remains governed.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestAcceptedDeviceFabricAssemblyKeepsLifecycleMutationCandidatesClosed$' -count=1)
  # §723 verifies the injected signed-proof heartbeat transport binds the
  # heartbeat digest to the challenge, applies lifecycle CAS, preserves other
  # devices, and remains absent from ordinary/accepted assemblies.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestLifecycleSignedHeartbeatCandidateVerifiesProofAndBindsHeartbeat$' -count=1)
  # §724 proves a TUI owner-wide create response that is hidden by the selected
  # client-instance projection is never promoted to private Prompt/Run state.
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_create_does_not_select_session_hidden_by_instance_projection)
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_instance_refresh_does_not_detail_read_a_hidden_selection)
  # §725 proves accepted INVENTORY/OBSERVE assemblies keep receipt,
  # reconciliation, Attempt, scheduler, and dispatch candidates closed.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestAcceptedNonExecuteDeviceFabricKeepsExecutionEvidenceClosed$' -count=1)
  # §726 proves candidate challenge persistence, active-reissue rejection,
  # owner scoping, one-time signed-heartbeat consumption, and default-off
  # ordinary/accepted route closure.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestLifecycleChallengeCandidateIssuesPersistsAndScopesOwner$' -count=1)
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestLifecycleSignedHeartbeatCandidateVerifiesProofAndBindsHeartbeat$' -count=1)
  # §727 verifies challenge expiry-at-the-boundary, consumed-challenge
  # replacement, TTL/overflow validation, and a file-CAS race with exactly
  # one successful active challenge.
  (cd "$REPO_ROOT/forge-core" && go test ./internal/appserver -run '^TestLifecycleChallengeCandidate(ReissuesAfterExpiryAndConsumption|TTLAndClockBoundaries|CASAllowsOneConcurrentIssue)$' -count=1)
  # §728 keeps a Console Web/App/Mobile owner-wide create response outside the
  # selected client-instance private projection until the instance declares it.
  (cd "$SNAPLINK_CONSOLE_ROOT" && flutter test test/forge_sessions_create_instance_widget_test.dart)
  # §729 proves Runtime CLI sessions create validates an optional instance
  # projection before its single owner-wide Conversation POST.
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime session_create)
  # §732 keeps a Runtime TUI instance-scoped create request-free until both
  # owner-bound session/resource observations are present and converged.
  # §733 refreshes that pair immediately before the owner-wide Conversation
  # POST and blocks the POST if the fresh observations drift or lose auth while
  # retaining the pending write for explicit retry.
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_instance_create_requires_a_converged_resource_pair)
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_create_blocks_owner_post_when_pair_drifted)
  (cd "$REPO_ROOT/forge-runtime" && cargo test -p forge-runtime-cli --bin forge-runtime remote_tui_create_keeps_pending_write_after_projection_authorization_failure)
)
