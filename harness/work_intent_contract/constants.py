"""Constants for the authority-neutral WorkIntent v1 candidate contract."""

from __future__ import annotations

import re

API_VERSION = "forgeos.work-intent/v1"
KIND = "WorkIntent"
CANONICALIZATION = "forgeos.canonical-json/v1"
STATUS = "declared_unassessed"
FRESHNESS = "not_evaluated"
DIGEST_DOMAIN = b"forgeos.work-intent.v1\0"

MAX_RECORD_BYTES = 262_144
MAX_DEPTH = 8
MAX_FIELDS = 32
MAX_ARRAY_ITEMS = 256
MAX_STRING_BYTES = 16_384
MAX_SHORT_BYTES = 160
MAX_REFERENCE_BYTES = 4_096
MAX_NARRATIVE_ITEMS = 64
MAX_NARRATIVE_TOTAL = 256
MAX_RECORD_REFS = 64
MAX_RECORD_REF_TOTAL = 128
MAX_ARTIFACTS = 32
MIN_I64 = -(2**63)
MAX_I64 = 2**63 - 1

HASH_RE = re.compile(r"[0-9a-f]{64}")
IDENTIFIER_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]{0,159}")
KEY_RE = re.compile(r"[a-z][a-z0-9_]*")

TOP_FIELDS = {
    "api_version", "attestations", "binding", "canonicalization",
    "declared_at_unix_ms", "declared_owner", "freshness", "intent", "kind",
    "materiality", "origin", "references", "requester", "status",
    "work_intent_id", "work_intent_sha256",
}
ATTESTATION_FIELDS = {
    f"{name}_attestation" for name in (
        "approval", "authentication", "authority", "completion", "effect",
        "execution", "freshness", "materiality", "ownership", "permission",
        "persistence", "reference_resolution", "scope", "truth",
    )
}
PRINCIPAL_FIELDS = {"authority_domain", "principal_id", "principal_type"}
PRINCIPAL_TYPES = {"agent", "human", "operator", "service"}
WORK_TYPES = {
    "architecture_evolution", "defect", "feature", "incident_response",
    "migration", "question", "refactor", "small_change",
}
ORIGIN_KINDS = {
    "incident", "operator_request", "other", "reflection_proposal",
    "runtime_signal", "technical_debt", "user_request",
}
MATERIALITY_LEVELS = {"materiality_not_bound", "L0", "L1", "L2", "L3", "L4"}
SNAPSHOT_TYPES = {"artifact", "external", "repository", "runtime"}
NARRATIVE_ARRAY_FIELDS = {
    "external_constraints", "non_goals", "open_questions", "scope",
    "success_signals",
}
SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_DECLARED_WORK_INTENT_V1 (exact caller-supplied declaration "
    "only; no origin authentication, reference resolution, G0, routing, Run or "
    "RunJournal existence, lifecycle, approval, authentication, authority, completion, "
    "effect, execution, freshness, materiality, ownership, permission, persistence, "
    "scope, or truth attestation)"
)
FIXTURE_PATH = "docs/contracts/fixtures/work-intent-v1.json"
