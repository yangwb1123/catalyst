"""Frozen constants for the authority-neutral Kernel operational core v1."""

from __future__ import annotations

import re

CANONICALIZATION = "forgeos.canonical-json/v1"

ARTIFACT_RECEIPT_API = "forgeos.artifact-receipt/v1"
INVOCATION_API = "forgeos.capability-invocation/v1"
EVENT_API = "forgeos.interaction-event/v1"
EXECUTION_RECEIPT_API = "forgeos.execution-receipt/v1"
CLOSURE_API = "forgeos.kernel-operational-reference-closure/v1"

ARTIFACT_RECEIPT_KIND = "ArtifactReceipt"
INVOCATION_KIND = "CapabilityInvocation"
EVENT_KIND = "InteractionEvent"
EXECUTION_RECEIPT_KIND = "ExecutionReceipt"
CLOSURE_KIND = "KernelOperationalReferenceClosure"

ARTIFACT_RECEIPT_DOMAIN = b"forgeos.kernel.artifact-receipt.v1\0"
INVOCATION_DOMAIN = b"forgeos.kernel.capability-invocation.v1\0"
EVENT_DOMAIN = b"forgeos.kernel.interaction-event.v1\0"
EXECUTION_RECEIPT_DOMAIN = b"forgeos.kernel.execution-receipt.v1\0"
CLOSURE_DOMAIN = b"forgeos.kernel-operational-reference-closure.v1\0"

ARTIFACT_RECEIPT_PREFIX = "artifact-receipt-"
INVOCATION_PREFIX = "capability-invocation-"
EVENT_PREFIX = "interaction-event-"
EXECUTION_RECEIPT_PREFIX = "execution-receipt-"
CLOSURE_PREFIX = "kernel-operational-reference-closure-"

MAX_ARTIFACT_RECEIPT_BYTES = 262_144
MAX_INVOCATION_BYTES = 524_288
MAX_EVENT_BYTES = 262_144
MAX_EXECUTION_RECEIPT_BYTES = 1_048_576
MAX_CLOSURE_BYTES = 16_777_216
MAX_ARTIFACT_REF_BYTES = 16_384
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY_ITEMS = 256
MAX_STRING_BYTES = 16_384
MAX_SHORT_BYTES = 160
MAX_REFERENCE_BYTES = 4_096
MAX_ARTIFACTS = 256
MAX_ARTIFACT_RECEIPTS = 64
MAX_INVOCATIONS = 64
MAX_EVENTS = 256
MAX_EXECUTION_RECEIPTS = 64
MAX_ATTEMPT = 64
MAX_IO_ITEMS = 32
MAX_REASON_CODES = 32
MAX_CONFIDENCE_MICROS = 1_000_000
MAX_CALL_COUNT = 1_000_000_000
MAX_COST_USD_MICROS = 1_000_000_000_000_000
MAX_ELAPSED_MS = 86_400_000
MAX_TOKEN_COUNT = 1_000_000_000
MAX_NETWORK_BYTES = 1_073_741_824
MAX_OUTPUT_BYTES = 1_073_741_824
MIN_I64 = -(2**63)
MAX_I64 = 2**63 - 1

HASH_RE = re.compile(r"[0-9a-f]{64}")
IDENTIFIER_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]{0,159}")
KEY_RE = re.compile(r"[a-z][a-z0-9_]*")

ATTESTATION_FIELDS = {
    "authorization_attestation",
    "binding_authentication_attestation",
    "completion_attestation",
    "content_provenance_attestation",
    "effect_attestation",
    "event_append_attestation",
    "execution_attestation",
    "grant_authentication_attestation",
    "outcome_attestation",
    "permission_attestation",
    "persistence_attestation",
    "principal_authentication_attestation",
    "transition_attestation",
    "usage_measurement_attestation",
}

PRINCIPAL_FIELDS = {"authority_domain", "principal_id", "principal_type"}
PRINCIPAL_TYPES = {"agent", "human", "operator", "service"}
TASK_BINDING_FIELDS = {
    "attempt_id", "change_id", "environment_class", "environment_id",
    "node_id", "project_id", "role", "run_id", "target_id", "task_id",
}
ENVIRONMENT_CLASSES = {"development", "local", "production", "staging", "test"}
CAPABILITY_FIELDS = {
    "capability_contract_sha256", "capability_id", "capability_version",
}
GRANT_REF_FIELDS = {"authority_domain", "grant_id", "grant_sha256"}
BINDING_FIELDS = {
    "context_sha256", "environment_profile_id", "environment_sha256",
    "policy_sha256", "source_profile_id", "source_revision",
    "source_tree_sha256",
}
ARTIFACT_FIELDS = {"artifact_kind", "artifact_ref", "artifact_sha256"}
OBSERVED_USAGE_FIELDS = {
    "call_count", "cost_usd_micros", "elapsed_ms", "input_tokens",
    "network_bytes", "output_bytes", "output_tokens",
}

ARTIFACT_RECEIPT_REF_FIELDS = {"artifact_receipt_id", "artifact_receipt_sha256"}
INVOCATION_REF_FIELDS = {"invocation_id", "invocation_sha256"}
EVENT_REF_FIELDS = {"event_id", "event_sha256"}
EXECUTION_RECEIPT_REF_FIELDS = {
    "execution_receipt_id", "execution_receipt_sha256",
}

ARTIFACT_RECEIPT_FIELDS = {
    "api_version", "artifact", "artifact_receipt_id", "artifact_receipt_sha256",
    "attestations", "bindings", "canonicalization", "content_bytes",
    "created_at_unix_ms", "kind", "producer", "producer_invocation_ref",
    "receipt_role", "slot", "task_binding",
}
INVOCATION_FIELDS = {
    "api_version", "attempt", "attestations", "bindings", "canonicalization",
    "capability", "capability_grant_ref", "correlation_id",
    "declared_output_slots", "idempotency_key", "input_artifact_receipt_refs",
    "invocation_id", "invocation_sha256", "kind",
    "prior_execution_receipt_ref", "requested_action_sha256",
    "requested_at_unix_ms", "subject", "task_binding",
}
EVENT_FIELDS = {
    "actor", "api_version", "artifact_refs", "attestations", "bindings",
    "canonicalization", "causation_event_ref", "confidence_micros",
    "correlation_id", "event_id", "event_sha256", "invocation_ref", "kind",
    "logical_sequence", "object_ref", "occurred_at_unix_ms", "target",
    "task_binding", "verb",
}
EXECUTION_RECEIPT_FIELDS = {
    "api_version", "attempt", "attestations", "bindings", "canonicalization",
    "correlation_id", "ended_at_unix_ms", "event_refs", "execution_receipt_id",
    "execution_receipt_sha256", "input_artifacts", "invocation_ref", "kind",
    "observed_usage", "outcome", "output_artifact_receipt_refs",
    "prior_execution_receipt_ref", "reason_codes", "executor",
    "started_at_unix_ms", "task_binding",
}
CLOSURE_FIELDS = {
    "api_version", "artifact_receipts", "artifacts", "attestations",
    "canonicalization", "capability_invocations", "closure_id",
    "closure_sha256", "execution_receipts", "interaction_events", "kind",
    "result",
}

RECEIPT_ROLES = {"declared_input", "declared_output"}
EVENT_VERBS = {
    "approve", "execute", "observe", "propose", "reject", "request",
    "rollback", "verify",
}
OUTCOMES = {"cancelled", "failed", "inconclusive", "lost", "succeeded"}

SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_KERNEL_OPERATIONAL_REFERENCE_CLOSURE_V1 (exact "
    "caller-supplied records and acyclic references only; no content provenance, "
    "principal, Grant, or source/context/environment/policy binding authentication; "
    "no authorization, permission, event append, persistence, transition, execution, "
    "outcome, completion, effect, or usage measurement attestation)"
)

FIXTURE_PATH = "docs/contracts/fixtures/kernel-operational-reference-closure-v1.json"
