"""Frozen constants for command observation to Evidence adapter v1."""

from governance_contract.constants import (HASH_RE, ID_RE, MAX_DEPTH, MAX_FIELDS,
                                           MAX_I64, MAX_ITEMS, MAX_STRING_BYTES,
                                           MIN_I64)

API_VERSION = "forgeos.governance.command-observation-evidence-adapter/v1"
OBSERVATION_API_VERSION = "forgeos.command-observation/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
COMMAND_DOMAIN = b"forgeos.governance.command-observation.command.v1\0"
SOURCE_DOMAIN = b"forgeos.governance.command-observation-source.v1\0"
REQUEST_DOMAIN = b"forgeos.governance.command-observation-evidence-adapter.request.v1\0"
SUCCESS = (
    "ADAPTED_SHADOW (observation mapping only; no execution, pass, completion, "
    "truth, authority, claim, atom, persistence, or effect attestation)"
)
ADAPTER_ID = "forgeos.command-observation-evidence-adapter"
MAX_REQUEST_BYTES = 131_072
MAX_TEXT_CHARS = 4096
MAX_ARGV_ITEMS = 64
MAX_TIMEOUT_MS = 86_400_000
MAX_EXIT_CODE = 2_147_483_647
EMPTY_SHA256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

REQUEST_FIELDS = {"api_version", "binding", "canonicalization", "observation"}
BINDING_FIELDS = {
    "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope",
    "sensitivity", "sequence", "subjects", "supersedes_record_ids",
}
OBSERVATION_FIELDS = {
    "api_version", "canonicalization", "command", "ended_at_unix_ms",
    "evidence_type", "producer", "source", "started_at_unix_ms", "streams",
    "termination",
}
COMMAND_FIELDS = {
    "argv", "cwd", "environment_sha256", "stdin_bytes", "stdin_sha256",
    "timeout_ms", "tool_snapshot_sha256",
}
PRODUCER_FIELDS = {"producer_id", "producer_type", "producer_version", "run_id"}
SOURCE_FIELDS = {"source_revision", "source_tree_sha256"}
STREAMS_FIELDS = {"combined", "stderr", "stdout"}
STREAM_FIELDS = {"bytes", "retained_bytes", "retained_sha256", "sha256"}
TERMINATION_FIELDS = {"exit_code", "kind"}
EVIDENCE_TYPES = {"gate_result", "test_run"}
PRODUCER_TYPES = {"service", "tool"}
TERMINATION_KINDS = {"cancelled", "exited", "timed_out"}
SENSITIVITIES = {"confidential", "internal", "public", "restricted"}

__all__ = [name for name in globals() if name.isupper()]
