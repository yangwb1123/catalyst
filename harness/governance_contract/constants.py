"""Frozen constants for the Governance Evidence/Claim v1 ABI."""

import re

MAX_I64 = 9_223_372_036_854_775_807
MIN_I64 = -9_223_372_036_854_775_808
MAX_RECORD_BYTES = 131_072
MAX_SET_BYTES = 1_048_576
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ITEMS = 256
MAX_STRING_BYTES = 16_384
HASH_RE = re.compile(r"[a-f0-9]{64}\Z")
ID_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]{0,159}\Z")
KEY_RE = re.compile(r"[a-z][a-z0-9_]*\Z")
KINDS = {"EvidenceRecord", "KnowledgeClaim"}
DOMAINS = {
    "EvidenceRecord": b"forgeos.governance.evidence-record.v1\0",
    "KnowledgeClaim": b"forgeos.governance.knowledge-claim.v1\0",
}
TOP_FIELDS = {"api_version", "integrity", "kind", "metadata", "spec", "status"}
METADATA_FIELDS = {
    "aggregate_id", "context_sha256", "created_at_unix_ms", "created_by",
    "policy_sha256", "project_id", "record_id", "scope", "sequence",
    "source_revision", "source_tree_sha256", "supersedes_record_ids",
}
PRINCIPAL_FIELDS = {"authority_domain", "principal_id", "principal_type", "role", "run_id"}
PRINCIPAL_TYPES = {"agent", "human", "operator", "service", "tool"}
STATUS_FIELDS = {"reason_codes", "state", "valid_from_unix_ms", "valid_until_unix_ms"}
EVIDENCE_TYPES = {
    "artifact", "external_source", "gate_result", "human_attestation", "repo_locator",
    "runtime_metric", "test_run",
}
LOCATOR_BY_TYPE = {
    "artifact": "artifact", "external_source": "external", "gate_result": "command",
    "human_attestation": "attestation", "repo_locator": "repo",
    "runtime_metric": "metric", "test_run": "command",
}
CLAIM_STATES = {
    "fact": {"candidate", "confirmed", "contested", "stale", "retracted", "superseded"},
    "constraint": {"candidate", "active", "waived", "expired", "superseded"},
    "decision": {"proposed", "accepted", "rejected", "deprecated", "superseded"},
    "inference": {"candidate", "supported", "contested", "invalidated", "expired"},
    "assumption": {"open", "testing", "validated", "invalidated", "expired"},
    "hypothesis": {"open", "testing", "validated", "invalidated", "expired"},
    "lesson": {"candidate", "observed", "repeated", "retired", "promoted"},
    "proposal": {"draft", "submitted", "adopted", "rejected", "superseded"},
    "unknown": {"open", "investigating", "resolved", "accepted_risk"},
}
SHADOW_STATES = {
    "fact": {"candidate", "contested"}, "constraint": {"candidate"},
    "decision": {"proposed"}, "inference": {"candidate"},
    "assumption": {"open", "testing"}, "hypothesis": {"open", "testing"},
    "lesson": {"candidate"}, "proposal": {"draft", "submitted"},
    "unknown": {"open", "investigating"},
}
