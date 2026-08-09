"""Frozen constants for the Artifact provenance to Evidence adapter v1."""

import re

from governance_contract.constants import (HASH_RE, ID_RE, MAX_DEPTH, MAX_FIELDS,
                                           MAX_I64, MAX_ITEMS, MAX_RECORD_BYTES,
                                           MAX_STRING_BYTES, MIN_I64)

API_VERSION = "forgeos.governance.artifact-evidence-adapter/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
ARTIFACT_FORMAT = "forgeos.artifact.v1"
SOURCE_DOMAIN = b"forgeos.governance.artifact-provenance-source.v1\0"
REQUEST_DOMAIN = b"forgeos.governance.artifact-evidence-adapter.request.v1\0"
SUCCESS = (
    "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect "
    "attestation)"
)
ADAPTER_ID = "forgeos.artifact-evidence-adapter"
MAX_REQUEST_BYTES = MAX_RECORD_BYTES
MAX_TEXT_CHARS = 4096

REQUEST_FIELDS = {"api_version", "artifact", "binding", "canonicalization"}
ARTIFACT_FIELDS = {
    "_format", "agent", "created_at", "model", "path", "phase",
    "prompt_sha256", "run_id", "sha256", "size", "workflow",
}
BINDING_FIELDS = {
    "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope",
    "sensitivity", "sequence", "source_revision", "source_tree_sha256",
    "subjects", "supersedes_record_ids",
}
SENSITIVITIES = {"confidential", "internal", "public", "restricted"}
RFC3339_NANO_RE = re.compile(
    r"(?P<year>[0-9]{4})-(?P<month>[0-9]{2})-(?P<day>[0-9]{2})"
    r"T(?P<hour>[0-9]{2}):(?P<minute>[0-9]{2}):(?P<second>[0-9]{2})"
    r"(?P<fraction>\.[0-9]{1,9})?(?P<zone>Z|[+-][0-9]{2}:[0-9]{2})\Z"
)

__all__ = [name for name in globals() if name.isupper()]
