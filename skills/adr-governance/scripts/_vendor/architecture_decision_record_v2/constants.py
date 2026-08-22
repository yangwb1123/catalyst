"""Frozen constants for the proposed-only ArchitectureDecisionRecord v2 wire."""

from __future__ import annotations

import re
from pathlib import Path


API_VERSION = "forgeos.architecture-decision-record/v2"
KIND = "ArchitectureDecisionRecord"
CANONICALIZATION = "forgeos.canonical-json/v1"
STATUS = "proposed"
BODY_DOMAIN = b"forgeos.architecture-decision-record-body.v2\0"
SELF_DOMAIN = b"forgeos.architecture-decision-record.v2\0"

MAX_DOCUMENT_BYTES = 256 * 1024
MAX_FRONTMATTER_BYTES = 64 * 1024
MAX_BODY_BYTES = 192 * 1024
MAX_DEPTH = 16
MAX_ARRAY_ITEMS = 64
MAX_FIELDS = 64
MAX_ID_BYTES = 160
MAX_TITLE_BYTES = 160
MAX_DOCUMENT_NAME_BYTES = 255
MAX_NARRATIVE_BYTES = 4096
MAX_IMPLEMENTATION_REF_BYTES = 4096
MAX_LINE_NUMBER = 2_147_483_647
MAX_I64 = 9_223_372_036_854_775_807

GOLDEN = Path("docs/contracts/fixtures/ADR-9001-proposed-boundary.md")
GOLDEN_SHA256 = "b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194"
SUCCESS = "VALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2"

HASH_RE = re.compile(r"[a-f0-9]{64}\Z")
IDENTIFIER_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]{0,159}\Z")
ADR_ID_RE = re.compile(
    r"ADR-(?:000[1-9]|00[1-9][0-9]|0[1-9][0-9]{2}|[1-9][0-9]{3})\Z"
)
DOCUMENT_NAME_RE = re.compile(
    r"ADR-([0-9]{4})-[a-z0-9]+(?:-[a-z0-9]+)*\.md\Z"
)
GRAPH_NODE_RE = re.compile(r"graph-node-[a-f0-9]{64}\Z")
IMPLEMENTATION_REF_RE = re.compile(
    r"[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*(?:#L[1-9][0-9]{0,9})?\Z"
)

TOP_FIELDS = {
    "accepted_at_unix_ms", "acceptance_id", "adr_id", "affected_node_ids",
    "alternatives", "api_version", "approver_refs", "assumption_claim_ids",
    "body_sha256", "canonicalization", "compatibility", "consequences",
    "context_claim_ids", "decision", "decision_driver_claim_ids",
    "document_name", "evidence_record_ids", "expires_at_unix_ms",
    "implementation_refs", "kind", "owner_refs", "proposed_at_unix_ms",
    "revisit_triggers", "risks", "rollback", "rollout", "scope_refs",
    "self_sha256", "status", "superseded_by", "supersedes", "title",
    "validation_plan",
}

SET_FIELDS = {
    "affected_node_ids", "approver_refs", "assumption_claim_ids",
    "context_claim_ids", "decision_driver_claim_ids", "evidence_record_ids",
    "implementation_refs", "owner_refs", "scope_refs", "superseded_by",
    "supersedes",
}
REQUIRED_SET_FIELDS = {
    "approver_refs", "owner_refs", "scope_refs",
}
NARRATIVE_FIELDS = {"compatibility", "decision", "rollback", "rollout"}
BODY_SECTIONS = ("Context", "Decision", "Consequences", "Validation", "Limitations")

ALTERNATIVE_FIELDS = {"alternative_id", "description", "disposition", "rationale"}
RISK_FIELDS = {"description", "mitigation", "risk_id"}
VALIDATION_FIELDS = {
    "description", "due_trigger", "evidence_required", "owner_ref",
    "success_criteria", "validation_id",
}
REVISIT_FIELDS = {"condition", "evidence_required", "trigger_id"}
