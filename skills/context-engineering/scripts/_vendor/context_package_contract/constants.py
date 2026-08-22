"""Frozen ContextPackage v1 constants and resource bounds."""

from __future__ import annotations

import re

REQUEST_VERSION = "forgeos.context-package-build-request/v1"
PACKAGE_VERSION = "forgeos.context-package/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
ASSEMBLY_MODE = "authority_free_deterministic_context_projection"
NORMALIZATION = "exact_lf_utf8_after_declared_redactions"
RESULT = (
    "ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, "
    "completion, persistence, or effect attestation)"
)

REQUEST_DOMAIN = b"forgeos.context-package-build-request.v1\0"
CACHE_DOMAIN = b"forgeos.context-package-cache-key.v1\0"
CONTEXT_DOMAIN = b"forgeos.context-package.v1\0"
SNIPPET_DOMAIN = b"forgeos.context-snippet.v1\0"
CONTENT_DOMAIN = b"forgeos.context-content.v1\0"
PROJECTION_DOMAIN = b"forgeos.context-package-projection.v1\0"

UTF8_COUNTER_ID = "forgeos.token-counter.utf8-bytes/v1"
UTF8_COUNTER_SHA256 = "44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf"
REDACTION_REPLACEMENT = b"[REDACTED]"

MAX_REQUEST_BYTES = 20 * 1024 * 1024
MAX_PACKAGE_BYTES = 2 * 1024 * 1024
MAX_DEPTH = 16
MAX_OBJECT_FIELDS = 32
MAX_ARRAY_ITEMS = 256
MAX_STRING_BYTES = 131_072
MAX_SOURCES = 64
MAX_SELECTED = 24
MAX_CONTENT_BYTES = 524_288
MAX_TOKENS = 1_000_000
MAX_SOURCE_BYTES = 131_072
MAX_REDACTION_RANGES = 256
MAX_I64 = 9_223_372_036_854_775_807

HASH_RE = re.compile(r"^[a-f0-9]{64}$")

CATEGORIES = (
    "task", "requirement", "acceptance", "hard_constraint", "permission",
    "prohibition", "fact", "decision", "assumption", "unknown", "adr",
    "impact", "api_contract", "data_contract", "deployment_contract", "code",
    "test", "debt", "finding", "runtime_evidence", "history",
)
CATEGORY_RANK = {name: index for index, name in enumerate(CATEGORIES)}
AVAILABILITY = {"available", "missing"}
DECLARED_LANES = {"instruction", "trusted_context", "untrusted_data"}
DECLARED_TRUST = {
    "system_policy", "user_authorized", "project_governance",
    "governance_record", "untrusted",
}
DISPOSITIONS = {"allow", "deny"}
FRESHNESS = {"fresh", "stale", "contested", "unknown"}
INJECTION_RISKS = {"none", "suspected"}
SOURCE_CLASSES = {
    "system_policy", "user_instruction", "repository", "web", "log", "issue",
    "tool_output", "governance_record", "artifact", "other",
}
UNTRUSTED_SOURCE_CLASSES = {
    "repository", "web", "log", "issue", "tool_output", "artifact", "other",
}
TRUNCATION_POLICIES = {"forbidden", "utf8_prefix"}
LANES = ("instruction_candidates", "trusted_context", "untrusted_data")
OMISSION_REASONS = {
    "missing", "denied", "stale", "contested", "unknown_freshness", "expired",
    "quarantined_prompt_injection", "source_limit_exceeded",
    "snippet_budget_exceeded", "content_budget_exceeded", "token_budget_exceeded",
}

REQUEST_FIELDS = {
    "api_version", "budget", "canonicalization", "redactions", "source_binding",
    "sources", "task_binding",
}
BUDGET_FIELDS = {
    "max_content_bytes", "max_snippets", "max_tokens", "tokenizer_id",
    "tokenizer_sha256",
}
TASK_FIELDS = {"change_id", "node_id", "phase", "project_id", "role", "run_id", "task_id"}
SOURCE_BINDING_FIELDS = {
    "as_of_unix_ms", "policy_sha256", "routes_sha256", "source_revision",
    "source_tree_sha256",
}
SOURCE_FIELDS = {
    "availability", "category", "content", "content_sha256", "declared_lane",
    "declared_trust", "disposition", "expires_at_unix_ms", "freshness",
    "injection_risk", "max_bytes", "priority", "required", "source_class",
    "source_id", "source_ref", "source_revision", "truncation",
}
REDACTION_FIELDS = {"ranges", "source_id"}
RANGE_FIELDS = {"end_byte", "rule_id", "start_byte"}

PACKAGE_FIELDS = {
    "accounting", "api_version", "assembly_mode", "budget", "cache_key_sha256",
    "canonicalization", "context_sha256", "freshness", "lanes", "omissions",
    "projection_sha256", "redaction_receipts", "request_sha256", "result",
    "source_binding", "task_binding",
}
ACCOUNTING_FIELDS = {
    "actual_tokens", "candidate_count", "content_bytes", "omitted_source_count",
    "redacted_range_count", "selected_snippet_count", "truncated_snippet_count",
}
FRESHNESS_FIELDS = {"evaluated_at_unix_ms", "expires_at_unix_ms"}
SNIPPET_FIELDS = {
    "category", "content", "declared_lane", "declared_trust", "delimiter",
    "instruction_allowed", "lane", "normalization", "projected_content_sha256", "required",
    "selection_reason", "snippet_sha256", "source_class", "source_content_sha256",
    "source_id", "source_ref", "source_revision", "truncation",
}
DELIMITER = "structured_json_lane_no_text_delimiter"
TRUNCATION_FIELDS = {"original_redacted_bytes", "reason", "retained_bytes"}
OMISSION_FIELDS = {"reason", "source_id", "source_ref"}
