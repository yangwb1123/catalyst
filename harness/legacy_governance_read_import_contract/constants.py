"""Frozen identifiers and bounds for the ADR-0086 read-only import core."""

CANONICALIZATION = "forgeos.canonical-json/v1"
REQUEST_API = "forgeos.legacy-governance-read-import-request/v1"
VIEW_API = "forgeos.legacy-governance-read-import-view/v1"
REQUEST_KIND = "legacy_governance_read_import_request"
VIEW_KIND = "legacy_governance_read_import_view"
RESULT = "PROJECTED_UNVERIFIED_LEGACY_READ_ONLY"
MEMORY_KIND = "forgeos_memory_jsonl_v1"
ADR_KIND = "legacy_adr_markdown"

REQUEST_DOMAIN = b"forgeos.legacy-governance-read-import-request.v1\0"
SOURCE_SET_DOMAIN = b"forgeos.legacy-governance-read-import-source-set.v1\0"
CANDIDATE_ID_DOMAIN = b"forgeos.legacy-governance-read-import-candidate-id.v1\0"
CANDIDATE_DOMAIN = b"forgeos.legacy-governance-read-import-candidate.v1\0"
CONFLICT_DOMAIN = b"forgeos.legacy-governance-read-import-conflict-set.v1\0"
SUPERSESSION_DOMAIN = b"forgeos.legacy-governance-read-import-supersession.v1\0"
VIEW_DOMAIN = b"forgeos.legacy-governance-read-import-view.v1\0"

MAX_MEMORY_BYTES = 32 << 20
MAX_MEMORY_LINE_BYTES = 16 << 20
MAX_MEMORY_ENTRIES = 4096
MAX_ADR_BYTES = 1 << 20
MAX_ADR_SOURCES = 256
MAX_RAW_BYTES = 64 << 20
MAX_REQUEST_BYTES = 96 << 20
MAX_VIEW_BYTES = 128 << 20
MAX_STRING_BYTES = 48 << 20
MAX_SOURCE_REF_BYTES = 4096
MAX_BINDING_BYTES = 160
MAX_CONFIDENCE_LEXEME_BYTES = 128
MAX_DEPTH = 16
MAX_FIELDS = 32
MAX_ARRAY_ITEMS = MAX_MEMORY_ENTRIES + MAX_ADR_SOURCES

SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_LEGACY_GOVERNANCE_READ_IMPORT_V1 "
    "(unverified legacy read-only projection only; no source authentication or "
    "completeness, confidence or status interpretation, conflict resolution, truth, "
    "authority, currentness, instruction, acceptance, persistence, winner, or "
    "runtime-effect attestation)"
)
