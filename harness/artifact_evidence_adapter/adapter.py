"""Pure Artifact v1 to shadow EvidenceRecord v1 adaptation."""

from __future__ import annotations

import hashlib
from datetime import datetime, timedelta
from pathlib import PurePosixPath

from governance_contract import (ContractError, canonical_json as governance_json,
                                 compute_record_digest)
from governance_contract.codec import decode_json as decode_governance_json
from governance_contract.semantics import validate_record

from .codec import canonical_artifact, canonical_json, decode_request
from .constants import (ADAPTER_ID, API_VERSION, ARTIFACT_FIELDS, ARTIFACT_FORMAT,
                        BINDING_FIELDS, CANONICALIZATION, HASH_RE, ID_RE, MAX_I64,
                        MAX_RECORD_BYTES, MAX_TEXT_CHARS, REQUEST_DOMAIN,
                        REQUEST_FIELDS, RFC3339_NANO_RE, SENSITIVITIES,
                        SOURCE_DOMAIN)


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


def _text(value: object, label: str, issues: list[str]) -> None:
    if not isinstance(value, str) or not value.strip() or len(value) > MAX_TEXT_CHARS:
        issues.append(f"{label}: expected non-empty text <= {MAX_TEXT_CHARS} characters")


def _identifier_list(value: object, label: str, issues: list[str],
                     *, nonempty: bool = False) -> None:
    if not isinstance(value, list) or len(value) > 256 or (nonempty and not value):
        issues.append(f"{label}: expected {'non-empty ' if nonempty else ''}list <= 256")
        return
    for index, item in enumerate(value):
        _identifier(item, f"{label}[{index}]", issues)
    if all(isinstance(item, str) for item in value) and value != sorted(set(value)):
        issues.append(f"{label}: must be sorted and unique")


def _safe_artifact_path(value: object) -> bool:
    if (not isinstance(value, str) or not value.strip() or
            len(value) > MAX_TEXT_CHARS):
        return False
    if value.startswith(("/", "\\")) or "\\" in value:
        return False
    if len(value) >= 2 and value[0].isascii() and value[0].isalpha() and value[1] == ":":
        return False
    path = PurePosixPath(value)
    return bool(path.parts) and str(path) == value and all(
        part not in {"", ".", ".."} for part in path.parts)


def _timestamp_components(value: object) -> tuple[datetime, int, int]:
    if not isinstance(value, str) or len(value) > 40:
        raise ContractError("artifact.created_at must be bounded RFC3339Nano")
    match = RFC3339_NANO_RE.fullmatch(value)
    if match is None:
        raise ContractError("artifact.created_at must be RFC3339Nano with timezone")
    parts = {name: int(match[name]) for name in
             ("year", "month", "day", "hour", "minute", "second")}
    try:
        local = datetime(**parts)
    except ValueError as error:
        raise ContractError(f"artifact.created_at is not a valid time: {error}") from error
    fraction = (match["fraction"] or ".0")[1:].ljust(9, "0")
    zone = match["zone"]
    if zone == "Z":
        return local, int(fraction), 0
    hours, minutes = int(zone[1:3]), int(zone[4:6])
    if hours > 23 or minutes > 59:
        raise ContractError("artifact.created_at has invalid UTC offset")
    sign = 1 if zone[0] == "+" else -1
    return local, int(fraction), sign * (hours * 60 + minutes)


def timestamp_unix_ms(value: object) -> int:
    """Parse exact RFC3339Nano and floor the instant to nonnegative Unix ms."""
    local, nanoseconds, offset_minutes = _timestamp_components(value)
    epoch = datetime(1970, 1, 1)
    delta = local - epoch - timedelta(minutes=offset_minutes)
    milliseconds = (delta.days * 86_400_000 + delta.seconds * 1_000 +
                    nanoseconds // 1_000_000)
    if not 0 <= milliseconds <= MAX_I64:
        raise ContractError("artifact.created_at is outside nonnegative signed-int64 Unix ms")
    return milliseconds


def _validate_artifact(artifact: object, issues: list[str]) -> None:
    if not _exact_fields(artifact, ARTIFACT_FIELDS, "request.artifact", issues):
        return
    if artifact["_format"] != ARTIFACT_FORMAT:
        issues.append("request.artifact._format: unsupported artifact format")
    for field in {"agent", "model", "phase", "workflow"}:
        _text(artifact[field], f"request.artifact.{field}", issues)
    _identifier(artifact["run_id"], "request.artifact.run_id", issues)
    for field in {"prompt_sha256", "sha256"}:
        _digest(artifact[field], f"request.artifact.{field}", issues)
    if not _safe_artifact_path(artifact["path"]):
        issues.append("request.artifact.path: expected normalized repo-relative path")
    if type(artifact["size"]) is not int or not 1 <= artifact["size"] <= MAX_I64:
        issues.append("request.artifact.size: expected positive signed-int64 integer")
    try:
        timestamp_unix_ms(artifact["created_at"])
    except ContractError as error:
        issues.append(str(error))


def _validate_binding(binding: object, issues: list[str]) -> None:
    if not _exact_fields(binding, BINDING_FIELDS, "request.binding", issues):
        return
    for field in {"aggregate_id", "project_id", "scope", "source_revision"}:
        _identifier(binding[field], f"request.binding.{field}", issues)
    for field in {"context_sha256", "policy_sha256", "source_tree_sha256"}:
        _digest(binding[field], f"request.binding.{field}", issues)
    if type(binding["sequence"]) is not int or not 1 <= binding["sequence"] <= MAX_I64:
        issues.append("request.binding.sequence: expected positive signed-int64 integer")
    if (not isinstance(binding["sensitivity"], str) or
            binding["sensitivity"] not in SENSITIVITIES):
        issues.append("request.binding.sensitivity: unsupported sensitivity")
    _identifier_list(binding["subjects"], "request.binding.subjects", issues, nonempty=True)
    _identifier_list(binding["supersedes_record_ids"],
                     "request.binding.supersedes_record_ids", issues)


def validate_request(request: object) -> list[str]:
    """Validate exact adapter semantics without granting provenance authority."""
    try:
        issues: list[str] = []
        if not _exact_fields(request, REQUEST_FIELDS, "request", issues):
            return issues
        if request["api_version"] != API_VERSION:
            issues.append("request.api_version: unsupported version")
        if request["canonicalization"] != CANONICALIZATION:
            issues.append("request.canonicalization: unsupported format")
        _validate_artifact(request["artifact"], issues)
        _validate_binding(request["binding"], issues)
        canonical_json(request)
        return issues
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["adapter request validation exhausted memory"]


def _compute_source_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(SOURCE_DOMAIN + canonical_artifact(request["artifact"])).hexdigest()


def _compute_request_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(REQUEST_DOMAIN + canonical_json(request)).hexdigest()


def _require_valid_request(request: object) -> dict[str, object]:
    issues = validate_request(request)
    if issues:
        raise ContractError("; ".join(issues))
    if not isinstance(request, dict):  # narrowed by validate_request; keeps type/runtime explicit
        raise ContractError("request: expected object")
    return request


def compute_source_digest(request: dict[str, object]) -> str:
    """Digest only a strict Artifact v1 provenance source from a valid request."""
    return _compute_source_digest_unchecked(_require_valid_request(request))


def compute_request_digest(request: dict[str, object]) -> str:
    """Digest one strict complete adapter request and binding."""
    return _compute_request_digest_unchecked(_require_valid_request(request))


def _principal(run_id: str) -> dict[str, object]:
    return {"authority_domain": "shadow", "principal_id": ADAPTER_ID,
            "principal_type": "tool", "role": "evidence-adapter", "run_id": run_id}


def _collector(run_id: str, request_digest: str) -> dict[str, object]:
    return {"collector_id": ADAPTER_ID, "collector_type": "tool",
            "collector_version": "v1", "parameters_sha256": request_digest,
            "run_id": run_id}


def _build_record(request: dict[str, object]) -> dict[str, object]:
    artifact, binding = request["artifact"], request["binding"]
    source_digest = _compute_source_digest_unchecked(request)
    request_digest = _compute_request_digest_unchecked(request)
    observed_at = timestamp_unix_ms(artifact["created_at"])
    metadata = {
        "aggregate_id": binding["aggregate_id"], "context_sha256": binding["context_sha256"],
        "created_at_unix_ms": observed_at, "created_by": _principal(artifact["run_id"]),
        "policy_sha256": binding["policy_sha256"], "project_id": binding["project_id"],
        "record_id": f"artifact-evidence-{request_digest}", "scope": binding["scope"],
        "sequence": binding["sequence"], "source_revision": binding["source_revision"],
        "source_tree_sha256": binding["source_tree_sha256"],
        "supersedes_record_ids": list(binding["supersedes_record_ids"]),
    }
    spec = {
        "artifact_sha256": artifact["sha256"],
        "collector": _collector(artifact["run_id"], request_digest),
        "content_role": "untrusted_data", "directness": "direct",
        "evidence_type": "artifact",
        "locator": {"content_sha256": artifact["sha256"], "exit_code": None,
                    "line_end": None, "line_start": None, "locator_ref": artifact["path"],
                    "locator_type": "artifact"},
        "observed_at_unix_ms": observed_at, "sensitivity": binding["sensitivity"],
        "source_snapshot": {"snapshot_id": f"artifact-snapshot-{source_digest}",
                            "snapshot_sha256": source_digest, "snapshot_type": "artifact"},
        "source_trust": "observed", "subjects": list(binding["subjects"]),
    }
    return {"api_version": "forgeos.governance/v1",
            "integrity": {"canonical_sha256": "", "canonicalization": CANONICALIZATION},
            "kind": "EvidenceRecord", "metadata": metadata, "spec": spec,
            "status": {"reason_codes": [], "state": "valid",
                       "valid_from_unix_ms": observed_at, "valid_until_unix_ms": None}}


def validate_evidence_record(record: object) -> list[str]:
    """Re-run the existing Governance v1 record validator."""
    if not isinstance(record, dict):
        return ["evidence record: expected object"]
    if record.get("kind") != "EvidenceRecord":
        return ["evidence record.kind: must be EvidenceRecord"]
    issues: list[str] = []
    validate_record(record, 0, issues)
    return issues


def adapt_request(request: dict[str, object]) -> dict[str, object]:
    """Purely adapt one validated request into one sealed shadow EvidenceRecord."""
    issues = validate_request(request)
    if issues:
        raise ContractError("; ".join(issues))
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
        return ["artifact evidence projection validation exhausted memory"]


def check_projection_bytes(request_raw: bytes, evidence_raw: bytes) -> list[str]:
    """Check exact canonical request and output bytes with bounded failures."""
    try:
        return validate_projection(decode_request(request_raw),
                                   decode_evidence_record(evidence_raw))
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["artifact evidence projection processing exhausted memory"]
