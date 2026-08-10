"""Frozen constants for Evolve repository locator Evidence adapter v1."""

import re

from governance_contract.constants import (HASH_RE, ID_RE, MAX_DEPTH, MAX_FIELDS,
                                           MAX_I64, MAX_ITEMS, MAX_STRING_BYTES,
                                           MIN_I64)

API_VERSION = "forgeos.governance.evolve-repo-locator-evidence-adapter/v1"
OBSERVATION_API_VERSION = "forgeos.evolve-repo-locator/v1"
SCAN_CONTRACT = "evolve_scan_v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
LOCATOR_DOMAIN = b"forgeos.governance.evolve-repo-locator.locator.v1\0"
SOURCE_DOMAIN = b"forgeos.governance.evolve-repo-locator-source.v1\0"
REQUEST_DOMAIN = b"forgeos.governance.evolve-repo-locator-evidence-adapter.request.v1\0"
SUCCESS = (
    "ADAPTED_SHADOW (locator mapping only; no file/report verification, scan "
    "judgment, completion, truth, authority, claim, atom, persistence, or effect "
    "attestation)"
)
ADAPTER_ID = "forgeos.evolve-repo-locator-evidence-adapter"
MAX_REQUEST_BYTES = 131_072
MAX_CONTENT_BYTES = 1_048_576
MAX_DETAIL_BYTES = 512
MAX_PATH_SCALARS = 4096
EVOLVE_OPPORTUNITY_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")

REQUEST_FIELDS = {"api_version", "binding", "canonicalization", "observation"}
BINDING_FIELDS = {
    "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope",
    "sensitivity", "sequence", "subjects", "supersedes_record_ids",
}
OBSERVATION_FIELDS = {
    "api_version", "canonicalization", "content", "locator",
    "observed_at_unix_ms", "producer", "scan_context", "source",
}
CONTENT_FIELDS = {"bytes", "sha256"}
LOCATOR_FIELDS = {"detail", "line", "path"}
PRODUCER_FIELDS = {
    "parameters_sha256", "producer_id", "producer_type", "producer_version", "run_id",
}
SCAN_CONTEXT_FIELDS = {
    "contract", "depth", "dimension", "opportunity_id", "relation", "report_sha256",
}
SOURCE_FIELDS = {"source_revision", "source_tree_sha256"}
DEPTHS = {"advisory", "opportunistic", "standard", "thorough"}
DIMENSIONS = {
    "architecture_drift", "code", "dependencies", "performance", "security",
    "test_coverage",
}
RELATIONS = {"clear", "finding", "opportunity"}
PRODUCER_TYPES = {"service", "tool"}
SENSITIVITIES = {"confidential", "internal", "public", "restricted"}

__all__ = [name for name in globals() if name.isupper()]
