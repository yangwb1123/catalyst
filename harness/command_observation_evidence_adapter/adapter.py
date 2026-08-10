"""Pure command observation to shadow EvidenceRecord v1 adaptation."""

from __future__ import annotations

import hashlib
from pathlib import PurePosixPath

from governance_contract import (ContractError, canonical_json as governance_json,
                                 compute_record_digest)
from governance_contract.codec import decode_json as decode_governance_json
from governance_contract.constants import MAX_RECORD_BYTES
from governance_contract.semantics import validate_record

from .codec import (canonical_command, canonical_json, canonical_observation,
                    decode_request)
from .constants import (ADAPTER_ID, API_VERSION, BINDING_FIELDS,
                        CANONICALIZATION, COMMAND_DOMAIN, COMMAND_FIELDS,
                        EMPTY_SHA256, EVIDENCE_TYPES, HASH_RE, ID_RE, MAX_ARGV_ITEMS,
                        MAX_EXIT_CODE, MAX_I64, MAX_TEXT_CHARS, MAX_TIMEOUT_MS,
                        OBSERVATION_API_VERSION, OBSERVATION_FIELDS, PRODUCER_FIELDS,
                        PRODUCER_TYPES, REQUEST_DOMAIN, REQUEST_FIELDS,
                        SENSITIVITIES, SOURCE_DOMAIN, SOURCE_FIELDS, STREAM_FIELDS,
                        STREAMS_FIELDS, TERMINATION_FIELDS, TERMINATION_KINDS)


def _exact_fields(value: object, fields: set[str], label: str,
                  issues: list[str]) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def _identifier(value: object, label: str, issues: list[str]) -> None:
    if not isinstance(value, str) or ID_RE.fullmatch(value) is None:
        issues.append(f"{label}: expected bounded identifier")


def _digest(value: object, label: str, issues: list[str]) -> None:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        issues.append(f"{label}: expected lowercase bare SHA-256")


def _nonnegative_i64(value: object, label: str, issues: list[str]) -> bool:
    if type(value) is not int or not 0 <= value <= MAX_I64:
        issues.append(f"{label}: expected nonnegative signed-int64 integer")
        return False
    return True


def _identifier_list(value: object, label: str, issues: list[str],
                     *, nonempty: bool = False) -> None:
    if not isinstance(value, list) or len(value) > 256 or (nonempty and not value):
        issues.append(f"{label}: expected {'non-empty ' if nonempty else ''}list <= 256")
        return
    for index, item in enumerate(value):
        _identifier(item, f"{label}[{index}]", issues)
    if all(isinstance(item, str) for item in value):
        byte_sorted = sorted(set(value), key=lambda item: item.encode("utf-8"))
        if value != byte_sorted or len(value) != len(set(value)):
            issues.append(f"{label}: must be UTF-8-byte sorted and unique")


def _safe_cwd(value: object) -> bool:
    if not isinstance(value, str) or not value or len(value) > MAX_TEXT_CHARS:
        return False
    if value == ".":
        return True
    if value.startswith(("/", "\\")) or "\\" in value:
        return False
    if len(value) >= 2 and value[0].isascii() and value[0].isalpha() and value[1] == ":":
        return False
    path = PurePosixPath(value)
    return bool(path.parts) and str(path) == value and all(
        part not in {"", ".", ".."} for part in path.parts)


def _validate_command(command: object, issues: list[str]) -> None:
    label = "request.observation.command"
    if not _exact_fields(command, COMMAND_FIELDS, label, issues):
        return
    argv = command["argv"]
    if not isinstance(argv, list) or not 1 <= len(argv) <= MAX_ARGV_ITEMS:
        issues.append(f"{label}.argv: expected 1..{MAX_ARGV_ITEMS} exact arguments")
    else:
        for index, argument in enumerate(argv):
            if not isinstance(argument, str) or len(argument) > MAX_TEXT_CHARS:
                issues.append(f"{label}.argv[{index}]: expected string <= {MAX_TEXT_CHARS} characters")
        if isinstance(argv[0], str) and not argv[0]:
            issues.append(f"{label}.argv[0]: executable must be non-empty")
    if not _safe_cwd(command["cwd"]):
        issues.append(f"{label}.cwd: expected '.' or normalized repo-relative path")
    for field in {"environment_sha256", "stdin_sha256", "tool_snapshot_sha256"}:
        _digest(command[field], f"{label}.{field}", issues)
    stdin_valid = _nonnegative_i64(command["stdin_bytes"], f"{label}.stdin_bytes", issues)
    if stdin_valid and command["stdin_bytes"] == 0 and command["stdin_sha256"] != EMPTY_SHA256:
        issues.append(f"{label}.stdin_sha256: empty stdin must use SHA-256(empty)")
    timeout = command["timeout_ms"]
    if timeout is not None and (type(timeout) is not int or
                                not 1 <= timeout <= MAX_TIMEOUT_MS):
        issues.append(f"{label}.timeout_ms: expected null or integer 1..{MAX_TIMEOUT_MS}")


def _validate_producer(producer: object, issues: list[str]) -> None:
    label = "request.observation.producer"
    if not _exact_fields(producer, PRODUCER_FIELDS, label, issues):
        return
    for field in {"producer_id", "producer_version", "run_id"}:
        _identifier(producer[field], f"{label}.{field}", issues)
    if (not isinstance(producer["producer_type"], str) or
            producer["producer_type"] not in PRODUCER_TYPES):
        issues.append(f"{label}.producer_type: unsupported producer type")


def _validate_source(source: object, issues: list[str]) -> None:
    label = "request.observation.source"
    if not _exact_fields(source, SOURCE_FIELDS, label, issues):
        return
    _identifier(source["source_revision"], f"{label}.source_revision", issues)
    _digest(source["source_tree_sha256"], f"{label}.source_tree_sha256", issues)


def _validate_stream(stream: object, label: str, issues: list[str]) -> None:
    if not _exact_fields(stream, STREAM_FIELDS, label, issues):
        return
    bytes_valid = _nonnegative_i64(stream["bytes"], f"{label}.bytes", issues)
    retained_valid = _nonnegative_i64(stream["retained_bytes"],
                                      f"{label}.retained_bytes", issues)
    _digest(stream["retained_sha256"], f"{label}.retained_sha256", issues)
    _digest(stream["sha256"], f"{label}.sha256", issues)
    if bytes_valid and retained_valid:
        total, retained = stream["bytes"], stream["retained_bytes"]
        if retained > total:
            issues.append(f"{label}.retained_bytes: cannot exceed bytes")
        if total == 0 and (stream["sha256"] != EMPTY_SHA256 or
                           stream["retained_sha256"] != EMPTY_SHA256):
            issues.append(f"{label}: empty stream must use SHA-256(empty)")
        if retained == 0 and stream["retained_sha256"] != EMPTY_SHA256:
            issues.append(f"{label}.retained_sha256: empty retained prefix must use SHA-256(empty)")
        if retained == total and stream["retained_sha256"] != stream["sha256"]:
            issues.append(f"{label}: fully retained stream digests must match")


def _validate_streams(streams: object, issues: list[str]) -> None:
    label = "request.observation.streams"
    if not _exact_fields(streams, STREAMS_FIELDS, label, issues):
        return
    for name in ("combined", "stderr", "stdout"):
        _validate_stream(streams[name], f"{label}.{name}", issues)
    if all(isinstance(streams.get(name), dict) and
           type(streams[name].get("bytes")) is int and
           0 <= streams[name]["bytes"] <= MAX_I64
           for name in ("combined", "stderr", "stdout")):
        split_total = streams["stdout"]["bytes"] + streams["stderr"]["bytes"]
        if split_total > MAX_I64:
            issues.append(f"{label}: stdout plus stderr bytes exceeds signed int64")
        elif streams["combined"]["bytes"] != split_total:
            issues.append(f"{label}.combined.bytes: must equal stdout.bytes + stderr.bytes")


def _validate_termination(termination: object, issues: list[str]) -> None:
    label = "request.observation.termination"
    if not _exact_fields(termination, TERMINATION_FIELDS, label, issues):
        return
    kind, exit_code = termination["kind"], termination["exit_code"]
    if not isinstance(kind, str) or kind not in TERMINATION_KINDS:
        issues.append(f"{label}.kind: unsupported termination kind")
    elif kind == "exited":
        if type(exit_code) is not int or not 0 <= exit_code <= MAX_EXIT_CODE:
            issues.append(f"{label}.exit_code: exited requires integer 0..{MAX_EXIT_CODE}")
    elif exit_code is not None:
        issues.append(f"{label}.exit_code: {kind} requires null")


def _validate_observation(observation: object, issues: list[str]) -> None:
    label = "request.observation"
    if not _exact_fields(observation, OBSERVATION_FIELDS, label, issues):
        return
    if observation["api_version"] != OBSERVATION_API_VERSION:
        issues.append(f"{label}.api_version: unsupported version")
    if observation["canonicalization"] != CANONICALIZATION:
        issues.append(f"{label}.canonicalization: unsupported format")
    _validate_command(observation["command"], issues)
    start_valid = _nonnegative_i64(observation["started_at_unix_ms"],
                                   f"{label}.started_at_unix_ms", issues)
    end_valid = _nonnegative_i64(observation["ended_at_unix_ms"],
                                 f"{label}.ended_at_unix_ms", issues)
    if start_valid and end_valid and observation["ended_at_unix_ms"] < observation["started_at_unix_ms"]:
        issues.append(f"{label}.ended_at_unix_ms: cannot precede started_at_unix_ms")
    if (not isinstance(observation["evidence_type"], str) or
            observation["evidence_type"] not in EVIDENCE_TYPES):
        issues.append(f"{label}.evidence_type: unsupported evidence type")
    _validate_producer(observation["producer"], issues)
    _validate_source(observation["source"], issues)
    _validate_streams(observation["streams"], issues)
    _validate_termination(observation["termination"], issues)


def validate_observation(observation: object) -> list[str]:
    """Validate the observation wire, including non-projectable terminations."""
    try:
        issues: list[str] = []
        _validate_observation(observation, issues)
        canonical_observation(observation)
        return issues
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["command observation validation exhausted memory"]


def _validate_binding(binding: object, issues: list[str]) -> None:
    label = "request.binding"
    if not _exact_fields(binding, BINDING_FIELDS, label, issues):
        return
    for field in {"aggregate_id", "project_id", "scope"}:
        _identifier(binding[field], f"{label}.{field}", issues)
    for field in {"context_sha256", "policy_sha256"}:
        _digest(binding[field], f"{label}.{field}", issues)
    if type(binding["sequence"]) is not int or not 1 <= binding["sequence"] <= MAX_I64:
        issues.append(f"{label}.sequence: expected positive signed-int64 integer")
    if (not isinstance(binding["sensitivity"], str) or
            binding["sensitivity"] not in SENSITIVITIES):
        issues.append(f"{label}.sensitivity: unsupported sensitivity")
    _identifier_list(binding["subjects"], f"{label}.subjects", issues, nonempty=True)
    _identifier_list(binding["supersedes_record_ids"],
                     f"{label}.supersedes_record_ids", issues)


def validate_request(request: object) -> list[str]:
    """Validate one exact projectable request without granting authority."""
    try:
        issues: list[str] = []
        if not _exact_fields(request, REQUEST_FIELDS, "request", issues):
            return issues
        if request["api_version"] != API_VERSION:
            issues.append("request.api_version: unsupported version")
        if request["canonicalization"] != CANONICALIZATION:
            issues.append("request.canonicalization: unsupported format")
        _validate_binding(request["binding"], issues)
        _validate_observation(request["observation"], issues)
        observation = request["observation"]
        if (isinstance(observation, dict) and
                isinstance(observation.get("termination"), dict) and
                observation["termination"].get("kind") != "exited"):
            issues.append("request.observation.termination: only exited observations are projectable")
        canonical_json(request)
        return issues
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["command observation adapter request validation exhausted memory"]


def _require_valid_request(request: object) -> dict[str, object]:
    issues = validate_request(request)
    if issues:
        raise ContractError("; ".join(issues))
    if not isinstance(request, dict):
        raise ContractError("request: expected object")
    return request


def _compute_command_digest_unchecked(request: dict[str, object]) -> str:
    observation = request["observation"]
    return hashlib.sha256(COMMAND_DOMAIN + canonical_command(observation["command"])).hexdigest()


def _compute_source_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(
        SOURCE_DOMAIN + canonical_observation(request["observation"])
    ).hexdigest()


def _compute_request_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(REQUEST_DOMAIN + canonical_json(request)).hexdigest()


def compute_command_digest(request: dict[str, object]) -> str:
    """Digest the exact command only after validating the complete request."""
    return _compute_command_digest_unchecked(_require_valid_request(request))


def compute_source_digest(request: dict[str, object]) -> str:
    """Digest the exact observation only after validating the complete request."""
    return _compute_source_digest_unchecked(_require_valid_request(request))


def compute_request_digest(request: dict[str, object]) -> str:
    """Digest the complete request and Governance binding."""
    return _compute_request_digest_unchecked(_require_valid_request(request))


def _principal(request_digest: str) -> dict[str, object]:
    return {"authority_domain": "shadow", "principal_id": ADAPTER_ID,
            "principal_type": "tool", "role": "evidence-adapter",
            "run_id": f"command-adaptation-{request_digest}"}


def _collector(producer: dict[str, object], command_digest: str) -> dict[str, object]:
    return {"collector_id": producer["producer_id"],
            "collector_type": producer["producer_type"],
            "collector_version": producer["producer_version"],
            "parameters_sha256": command_digest, "run_id": producer["run_id"]}


def _build_record(request: dict[str, object]) -> dict[str, object]:
    binding, observation = request["binding"], request["observation"]
    producer, source = observation["producer"], observation["source"]
    source_digest = _compute_source_digest_unchecked(request)
    request_digest = _compute_request_digest_unchecked(request)
    command_digest = _compute_command_digest_unchecked(request)
    observed_at = observation["ended_at_unix_ms"]
    metadata = {
        "aggregate_id": binding["aggregate_id"], "context_sha256": binding["context_sha256"],
        "created_at_unix_ms": observed_at, "created_by": _principal(request_digest),
        "policy_sha256": binding["policy_sha256"], "project_id": binding["project_id"],
        "record_id": f"command-evidence-{request_digest}", "scope": binding["scope"],
        "sequence": binding["sequence"], "source_revision": source["source_revision"],
        "source_tree_sha256": source["source_tree_sha256"],
        "supersedes_record_ids": list(binding["supersedes_record_ids"]),
    }
    spec = {
        # EvidenceRecord v1 names its valid captured-material digest
        # artifact_sha256 even when the source snapshot kind is runtime.
        "artifact_sha256": source_digest,
        "collector": _collector(producer, command_digest),
        "content_role": "untrusted_data", "directness": "direct",
        "evidence_type": observation["evidence_type"],
        "locator": {"content_sha256": observation["streams"]["combined"]["sha256"],
                    "exit_code": observation["termination"]["exit_code"],
                    "line_end": None, "line_start": None,
                    "locator_ref": f"command-observation:{source_digest}",
                    "locator_type": "command"},
        "observed_at_unix_ms": observed_at, "sensitivity": binding["sensitivity"],
        "source_snapshot": {"snapshot_id": f"command-observation-{source_digest}",
                            "snapshot_sha256": source_digest, "snapshot_type": "runtime"},
        "source_trust": "observed", "subjects": list(binding["subjects"]),
    }
    return {"api_version": "forgeos.governance/v1",
            "integrity": {"canonical_sha256": "", "canonicalization": CANONICALIZATION},
            "kind": "EvidenceRecord", "metadata": metadata, "spec": spec,
            "status": {"reason_codes": [], "state": "valid",
                       "valid_from_unix_ms": observed_at, "valid_until_unix_ms": None}}


def validate_evidence_record(record: object) -> list[str]:
    """Re-run the existing Governance v1 strict record validator."""
    if not isinstance(record, dict):
        return ["evidence record: expected object"]
    if record.get("kind") != "EvidenceRecord":
        return ["evidence record.kind: must be EvidenceRecord"]
    issues: list[str] = []
    validate_record(record, 0, issues)
    return issues


def adapt_request(request: dict[str, object]) -> dict[str, object]:
    """Purely adapt one validated request into one sealed shadow EvidenceRecord."""
    request = _require_valid_request(request)
    record = _build_record(request)
    record["integrity"]["canonical_sha256"] = compute_record_digest(record)
    issues = validate_evidence_record(record)
    if issues:
        raise ContractError("adapter produced invalid EvidenceRecord: " + "; ".join(issues))
    return record


def decode_evidence_record(raw: bytes) -> dict[str, object]:
    """Decode one exact canonical Governance EvidenceRecord object."""
    if len(raw) > MAX_RECORD_BYTES:
        raise ContractError(f"evidence record exceeds {MAX_RECORD_BYTES} bytes")
    value = decode_governance_json(raw)
    if not isinstance(value, dict):
        raise ContractError("evidence record root must be an object")
    if governance_json(value) != raw:
        raise ContractError("evidence record is not exact compact canonical JSON")
    return value


def validate_projection(request: object, record: object) -> list[str]:
    """Validate output and require exact deterministic reprojection."""
    request_issues = validate_request(request)
    if request_issues:
        return request_issues
    record_issues = validate_evidence_record(record)
    if record_issues:
        return record_issues
    try:
        expected = adapt_request(request)
        if governance_json(record) != governance_json(expected):
            return ["evidence record does not exactly match deterministic adaptation"]
        return []
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["command observation evidence projection validation exhausted memory"]


def check_projection_bytes(request_raw: bytes, evidence_raw: bytes) -> list[str]:
    """Check exact canonical request and output bytes with bounded failures."""
    try:
        return validate_projection(decode_request(request_raw),
                                   decode_evidence_record(evidence_raw))
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["command observation evidence projection processing exhausted memory"]
