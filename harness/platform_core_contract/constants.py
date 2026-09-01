"""Frozen Platform Core Envelope v1 constants."""

CANONICALIZATION = "forge.canonical-json/v1"
ENVELOPE_VERSION = 1
RECEIPT_VERSION = 1

MAX_ARTIFACT_REF_BYTES = 16 * 1024
MAX_ENVELOPE_BYTES = 256 * 1024
MAX_RECEIPT_BYTES = 256 * 1024
MAX_PAYLOAD_BYTES = 32 * 1024
MAX_EXTENSIONS_BYTES = 8 * 1024
MAX_JSON_DEPTH = 12
MAX_OBJECT_FIELDS = 64
MAX_ARRAY_ITEMS = 256
MAX_STRING_BYTES = 16 * 1024
MAX_EXTENSION_FIELDS = 16
MAX_ARTIFACT_BYTES = 1 << 40
MAX_UNIX_MILLISECONDS = 253_402_300_799_999
MAX_INPUT_PATH_BYTES = 4096
MAX_INPUT_PATH_COMPONENTS = 64
MAX_RECEIPT_ARTIFACTS = 32
MAX_VERIFICATION_CHECKS = 64
MAX_REASON_CODES = 16
MAX_EVIDENCE_REFS = 16
MAX_EXECUTION_ELAPSED_MS = 31_536_000_000
MAX_OBSERVED_COUNT = 1_000_000_000
MAX_OBSERVED_QUANTITY = 1_000_000_000_000_000

ARTIFACT_DIGEST_DOMAIN = b"forge.platform.artifact-ref.v1\0"
COMMAND_DIGEST_DOMAIN = b"forge.platform.command-envelope.v1\0"
EVENT_DIGEST_DOMAIN = b"forge.platform.event-envelope.v1\0"
EXECUTION_RECEIPT_DIGEST_DOMAIN = b"forge.platform.execution-receipt.v1\0"
VERIFICATION_REQUEST_DIGEST_DOMAIN = b"forge.platform.verification-request.v1\0"
VERIFICATION_RECEIPT_DIGEST_DOMAIN = b"forge.platform.verification-receipt.v1\0"

DOCUMENT_INVALID = "pc_document_invalid"
IDENTIFIER_INVALID = "pc_identifier_invalid"
VALUE_INVALID = "pc_value_invalid"
REFERENCE_MISMATCH = "pc_reference_mismatch"
STATE_INVALID = "pc_state_invalid"
TRANSITION_INVALID = "pc_transition_invalid"
RELATION_MISMATCH = "pc_relation_mismatch"

FIXTURE_PATH = "docs/contracts/fixtures/platform-core-envelope-v1.json"
SCHEMA_PATH = "docs/contracts/platform-core-envelope-v1.schema.json"
FIXTURE_API_VERSION = "forge.platform-core-envelope-fixture/v1"
RECEIPT_FIXTURE_PATH = "docs/contracts/fixtures/platform-core-receipt-v1.json"
RECEIPT_SCHEMA_PATH = "docs/contracts/platform-core-receipt-v1.schema.json"
RECEIPT_FIXTURE_API_VERSION = "forge.platform-core-receipt-fixture/v1"
REJECTION_CORPUS_PATH = "docs/contracts/fixtures/platform-core-rejection-corpus-v1.json"
SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_PLATFORM_CORE_ENVELOPE_V1 "
    "(exact identity, scope, payload and reference declarations only; no "
    "authentication, authorization, idempotency reservation, event append, "
    "content existence, persistence, transition, verification or completion attestation)"
)
RECEIPT_SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_PLATFORM_CORE_RECEIPT_V1 "
    "(supplied execution and verification observations only; no reference "
    "resolution, authority, state mutation, persistence or completion attestation)"
)

ENTITY_PREFIXES = {
    "action": "act", "actor": "acr", "artifact": "art", "attempt": "atm",
    "change": "chg", "objective": "obj", "project": "prj",
    "project_snapshot": "psn", "receipt": "rcp", "session": "ses",
    "space": "spc", "turn": "trn", "work_graph": "wgr", "work_item": "wki",
}
PLATFORM_PREFIXES = set(ENTITY_PREFIXES.values()) | {"cmd", "cor", "evt", "msg", "ver"}

ARTIFACT_FIELDS = {
    "artifact_kind", "canonicalization", "content_digest", "content_id",
    "created_at_unix_ms", "logical_id", "media_type", "producer_attempt_id",
    "provenance_ref", "retention_class", "sensitivity", "size_bytes",
    "source_snapshot_ref",
}
COMMAND_FIELDS = {
    "actor_ref", "authorization_ref", "canonicalization", "causation_id",
    "command_id", "correlation_id", "deadline_unix_ms", "expected_version",
    "envelope_version", "extensions", "idempotency_key", "issued_at_unix_ms", "message_id",
    "payload", "payload_artifact_ref", "schema_name", "schema_version",
    "scope_ref", "target_ref",
}
EVENT_FIELDS = {
    "actor_ref", "aggregate_ref", "aggregate_version", "canonicalization",
    "causation_id", "correlation_id", "envelope_version", "event_id", "extensions", "message_id",
    "occurred_at_unix_ms", "payload", "payload_artifact_ref", "schema_name",
    "schema_version", "scope_ref", "sequence", "source_component",
    "source_snapshot_ref",
}
SCOPE_FIELDS = {
    "action_id", "attempt_id", "change_id", "objective_id", "project_id",
    "project_snapshot_id", "session_id", "space_id", "turn_id",
    "work_graph_id", "work_item_id",
}

EXECUTION_RECEIPT_FIELDS = {
    "approval_ref", "attempt_ref", "canonicalization", "ended_at_unix_ms",
    "event_range", "execution_receipt_version", "executor", "grant_ref",
    "input_artifact_refs", "observed_usage", "output_artifact_refs",
    "reason_codes", "receipt_id", "scope_ref", "session_ref",
    "source_snapshot_ref", "started_at_unix_ms", "terminal_state",
}
EXECUTOR_FIELDS = {"actor_ref", "adapter_id", "adapter_version"}
OBSERVED_USAGE_FIELDS = {
    "cost_usd_micros", "elapsed_ms", "input_tokens", "model_calls",
    "network_bytes", "output_bytes", "output_tokens", "tool_calls",
}
EVENT_RANGE_FIELDS = {
    "aggregate_ref", "first_event_id", "first_sequence", "last_event_id",
    "last_sequence",
}
VERIFICATION_REQUEST_FIELDS = {
    "canonicalization", "checks", "input_artifact_ref", "requested_at_unix_ms",
    "requested_by", "scope_ref", "verification_id",
    "verification_request_version",
}
VERIFICATION_CHECK_REQUEST_FIELDS = {
    "check_id", "check_name", "declared_required",
}
VERIFICATION_RECEIPT_FIELDS = {
    "canonicalization", "ended_at_unix_ms", "input_artifact_ref",
    "overall_status", "produced_by", "receipt_id", "request_sha256",
    "results", "scope_ref", "started_at_unix_ms", "verification_id",
    "verification_receipt_version",
}
VERIFICATION_RESULT_FIELDS = {
    "applicability", "applicability_reason", "check_id", "evidence_refs",
    "reason_codes", "status",
}

TEXT_TYPE = "text"
INTEGER_TYPE = "integer"
BOOLEAN_TYPE = "boolean"
OBJECT_TYPE = "object"
ENTITY_TYPE_PROFILE = {"entity_id": TEXT_TYPE, "entity_type": TEXT_TYPE}
ACTOR_TYPE_PROFILE = {"actor_id": TEXT_TYPE, "actor_type": TEXT_TYPE}
RECORD_TYPE_PROFILE = {
    "record_id": TEXT_TYPE, "record_sha256": TEXT_TYPE, "record_type": TEXT_TYPE,
}
SCOPE_TYPE_PROFILE = {
    "space_id": TEXT_TYPE,
    **{field: ("nullable", TEXT_TYPE) for field in (
        "action_id", "attempt_id", "change_id", "objective_id", "project_id",
        "project_snapshot_id", "session_id", "turn_id", "work_graph_id",
        "work_item_id")},
}
ARTIFACT_TYPE_PROFILE = {
    "artifact_kind": TEXT_TYPE, "canonicalization": TEXT_TYPE,
    "content_digest": TEXT_TYPE, "content_id": TEXT_TYPE,
    "created_at_unix_ms": INTEGER_TYPE, "logical_id": TEXT_TYPE,
    "media_type": TEXT_TYPE, "producer_attempt_id": TEXT_TYPE,
    "provenance_ref": RECORD_TYPE_PROFILE, "retention_class": TEXT_TYPE,
    "sensitivity": TEXT_TYPE, "size_bytes": INTEGER_TYPE,
    "source_snapshot_ref": ENTITY_TYPE_PROFILE,
}
COMMAND_TYPE_PROFILE = {
    "actor_ref": ACTOR_TYPE_PROFILE,
    "authorization_ref": ("nullable", RECORD_TYPE_PROFILE),
    "canonicalization": TEXT_TYPE, "causation_id": ("nullable", TEXT_TYPE),
    "command_id": TEXT_TYPE, "correlation_id": TEXT_TYPE,
    "deadline_unix_ms": ("nullable", INTEGER_TYPE),
    "envelope_version": INTEGER_TYPE,
    "expected_version": ("nullable", INTEGER_TYPE), "extensions": OBJECT_TYPE,
    "idempotency_key": TEXT_TYPE, "issued_at_unix_ms": INTEGER_TYPE,
    "message_id": TEXT_TYPE, "payload": ("nullable", OBJECT_TYPE),
    "payload_artifact_ref": ("nullable", ARTIFACT_TYPE_PROFILE),
    "schema_name": TEXT_TYPE, "schema_version": INTEGER_TYPE,
    "scope_ref": SCOPE_TYPE_PROFILE, "target_ref": ENTITY_TYPE_PROFILE,
}
EVENT_TYPE_PROFILE = {
    "actor_ref": ACTOR_TYPE_PROFILE, "aggregate_ref": ENTITY_TYPE_PROFILE,
    "aggregate_version": INTEGER_TYPE, "canonicalization": TEXT_TYPE,
    "causation_id": ("nullable", TEXT_TYPE), "correlation_id": TEXT_TYPE,
    "envelope_version": INTEGER_TYPE, "event_id": TEXT_TYPE,
    "extensions": OBJECT_TYPE, "message_id": TEXT_TYPE,
    "occurred_at_unix_ms": INTEGER_TYPE,
    "payload": ("nullable", OBJECT_TYPE),
    "payload_artifact_ref": ("nullable", ARTIFACT_TYPE_PROFILE),
    "schema_name": TEXT_TYPE, "schema_version": INTEGER_TYPE,
    "scope_ref": SCOPE_TYPE_PROFILE, "sequence": INTEGER_TYPE,
    "source_component": TEXT_TYPE,
    "source_snapshot_ref": ("nullable", ENTITY_TYPE_PROFILE),
}
EXECUTION_RECEIPT_TYPE_PROFILE = {
    "approval_ref": ("nullable", RECORD_TYPE_PROFILE),
    "attempt_ref": ENTITY_TYPE_PROFILE, "canonicalization": TEXT_TYPE,
    "ended_at_unix_ms": INTEGER_TYPE,
    "event_range": ("nullable", {
        "aggregate_ref": ENTITY_TYPE_PROFILE, "first_event_id": TEXT_TYPE,
        "first_sequence": INTEGER_TYPE, "last_event_id": TEXT_TYPE,
        "last_sequence": INTEGER_TYPE}),
    "execution_receipt_version": INTEGER_TYPE,
    "executor": {"actor_ref": ACTOR_TYPE_PROFILE, "adapter_id": TEXT_TYPE,
                 "adapter_version": TEXT_TYPE},
    "grant_ref": ("nullable", RECORD_TYPE_PROFILE),
    "input_artifact_refs": ("array", ARTIFACT_TYPE_PROFILE),
    "observed_usage": {field: INTEGER_TYPE for field in (
        "cost_usd_micros", "elapsed_ms", "input_tokens", "model_calls",
        "network_bytes", "output_bytes", "output_tokens", "tool_calls")},
    "output_artifact_refs": ("array", ARTIFACT_TYPE_PROFILE),
    "reason_codes": ("array", TEXT_TYPE), "receipt_id": TEXT_TYPE,
    "scope_ref": SCOPE_TYPE_PROFILE, "session_ref": ENTITY_TYPE_PROFILE,
    "source_snapshot_ref": ENTITY_TYPE_PROFILE,
    "started_at_unix_ms": INTEGER_TYPE, "terminal_state": TEXT_TYPE,
}
VERIFICATION_CHECK_REQUEST_TYPE_PROFILE = {
    "check_id": TEXT_TYPE, "check_name": TEXT_TYPE,
    "declared_required": BOOLEAN_TYPE,
}
VERIFICATION_REQUEST_TYPE_PROFILE = {
    "canonicalization": TEXT_TYPE,
    "checks": ("array", VERIFICATION_CHECK_REQUEST_TYPE_PROFILE),
    "input_artifact_ref": ARTIFACT_TYPE_PROFILE,
    "requested_at_unix_ms": INTEGER_TYPE, "requested_by": ACTOR_TYPE_PROFILE,
    "scope_ref": SCOPE_TYPE_PROFILE, "verification_id": TEXT_TYPE,
    "verification_request_version": INTEGER_TYPE,
}
VERIFICATION_RESULT_TYPE_PROFILE = {
    "applicability": TEXT_TYPE,
    "applicability_reason": ("nullable", TEXT_TYPE), "check_id": TEXT_TYPE,
    "evidence_refs": ("array", RECORD_TYPE_PROFILE),
    "reason_codes": ("array", TEXT_TYPE), "status": TEXT_TYPE,
}
VERIFICATION_RECEIPT_TYPE_PROFILE = {
    "canonicalization": TEXT_TYPE, "ended_at_unix_ms": INTEGER_TYPE,
    "input_artifact_ref": ARTIFACT_TYPE_PROFILE, "overall_status": TEXT_TYPE,
    "produced_by": ACTOR_TYPE_PROFILE, "receipt_id": TEXT_TYPE,
    "request_sha256": TEXT_TYPE,
    "results": ("array", VERIFICATION_RESULT_TYPE_PROFILE),
    "scope_ref": SCOPE_TYPE_PROFILE, "started_at_unix_ms": INTEGER_TYPE,
    "verification_id": TEXT_TYPE, "verification_receipt_version": INTEGER_TYPE,
}
