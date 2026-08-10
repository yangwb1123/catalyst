"""Pure Evolve repository locator to shadow EvidenceRecord v1 adaptation."""

from __future__ import annotations

import hashlib
import unicodedata
from pathlib import PurePosixPath

from governance_contract import (ContractError, canonical_json as governance_json,
                                 compute_record_digest)
from governance_contract.codec import decode_json as decode_governance_json
from governance_contract.constants import MAX_RECORD_BYTES
from governance_contract.semantics import validate_record

from .codec import (canonical_json, canonical_locator, canonical_observation,
                    decode_request)
from .constants import (ADAPTER_ID, API_VERSION, BINDING_FIELDS,
                        CANONICALIZATION, CONTENT_FIELDS, DEPTHS, DIMENSIONS,
                        EVOLVE_OPPORTUNITY_ID_RE,
                        HASH_RE, ID_RE, LOCATOR_DOMAIN, LOCATOR_FIELDS,
                        MAX_CONTENT_BYTES, MAX_DETAIL_BYTES, MAX_I64,
                        MAX_PATH_SCALARS,
                        OBSERVATION_API_VERSION, OBSERVATION_FIELDS,
                        PRODUCER_FIELDS, PRODUCER_TYPES, RELATIONS, REQUEST_DOMAIN,
                        REQUEST_FIELDS, SCAN_CONTEXT_FIELDS, SCAN_CONTRACT,
                        SENSITIVITIES, SOURCE_DOMAIN, SOURCE_FIELDS)


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


def _contains_unicode_control(value: str) -> bool:
    return any(unicodedata.category(character) == "Cc" for character in value)


def _identifier_list(value: object, label: str, issues: list[str],
                     *, nonempty: bool = False) -> None:
    if not isinstance(value, list):
        issues.append(f"{label}: expected array")
        return
    if nonempty and not value:
        issues.append(f"{label}: must not be empty")
    if any(not isinstance(item, str) or ID_RE.fullmatch(item) is None for item in value):
        issues.append(f"{label}: all values must be bounded identifiers")
        return
    expected = sorted(set(value), key=lambda item: item.encode("utf-8"))
    if value != expected:
        issues.append(f"{label}: must be unique and sorted by UTF-8 bytes")


def _validate_repo_path(value: object, label: str, issues: list[str]) -> None:
    if (not isinstance(value, str) or not value.strip() or
            len(value) > MAX_PATH_SCALARS or
            value.startswith(("/", "\\"))):
        issues.append(f"{label}: expected canonical repository-relative path")
        return
    if "\\" in value or (len(value) >= 2 and value[0].isascii() and
                           value[0].isalpha() and value[1] == ":"):
        issues.append(f"{label}: expected canonical repository-relative path")
        return
    if _contains_unicode_control(value):
        issues.append(f"{label}: Unicode control characters are forbidden")
        return
    path = PurePosixPath(value)
    if (str(path) != value or not path.parts or
            any(part in {"", ".", ".."} for part in path.parts)):
        issues.append(f"{label}: expected canonical repository-relative path")
        return
    if path.parts[0].lower() in {".git", ".forge"}:
        issues.append(f"{label}: protected repository control path is forbidden")


def _validate_content(content: object, issues: list[str]) -> None:
    label = "request.observation.content"
    if not _exact_fields(content, CONTENT_FIELDS, label, issues):
        return
    size = content["bytes"]
    if type(size) is not int or not 1 <= size <= MAX_CONTENT_BYTES:
        issues.append(f"{label}.bytes: expected integer 1..{MAX_CONTENT_BYTES}")
    _digest(content["sha256"], f"{label}.sha256", issues)


def _validate_locator(locator: object, issues: list[str]) -> None:
    label = "request.observation.locator"
    if not _exact_fields(locator, LOCATOR_FIELDS, label, issues):
        return
    detail = locator["detail"]
    if (not isinstance(detail, str) or not detail.strip() or
            _contains_unicode_control(detail) or
            len(detail.encode("utf-8")) > MAX_DETAIL_BYTES):
        issues.append(f"{label}.detail: expected non-blank text up to {MAX_DETAIL_BYTES} UTF-8 bytes")
    line = locator["line"]
    if type(line) is not int or not 0 <= line <= MAX_I64:
        issues.append(f"{label}.line: expected nonnegative signed-int64 integer")
    _validate_repo_path(locator["path"], f"{label}.path", issues)


def _validate_producer(producer: object, issues: list[str]) -> None:
    label = "request.observation.producer"
    if not _exact_fields(producer, PRODUCER_FIELDS, label, issues):
        return
    for field in {"producer_id", "producer_version", "run_id"}:
        _identifier(producer[field], f"{label}.{field}", issues)
    if (not isinstance(producer["producer_type"], str) or
            producer["producer_type"] not in PRODUCER_TYPES):
        issues.append(f"{label}.producer_type: expected service or tool")
    _digest(producer["parameters_sha256"], f"{label}.parameters_sha256", issues)


def _validate_scan_context(context: object, issues: list[str]) -> None:
    label = "request.observation.scan_context"
    if not _exact_fields(context, SCAN_CONTEXT_FIELDS, label, issues):
        return
    if context["contract"] != SCAN_CONTRACT:
        issues.append(f"{label}.contract: unsupported scan contract")
    if not isinstance(context["depth"], str) or context["depth"] not in DEPTHS:
        issues.append(f"{label}.depth: unsupported Evolve depth")
    if (not isinstance(context["dimension"], str) or
            context["dimension"] not in DIMENSIONS):
        issues.append(f"{label}.dimension: unsupported Evolve dimension")
    relation, opportunity_id = context["relation"], context["opportunity_id"]
    if not isinstance(relation, str) or relation not in RELATIONS:
        issues.append(f"{label}.relation: unsupported locator relation")
    elif relation == "opportunity":
        if (not isinstance(opportunity_id, str) or
                EVOLVE_OPPORTUNITY_ID_RE.fullmatch(opportunity_id) is None):
            issues.append(
                f"{label}.opportunity_id: expected Evolve identifier up to 64 bytes"
            )
    elif opportunity_id is not None:
        issues.append(f"{label}.opportunity_id: must be null outside opportunity relation")
    _digest(context["report_sha256"], f"{label}.report_sha256", issues)


def _validate_source(source: object, issues: list[str]) -> None:
    label = "request.observation.source"
    if not _exact_fields(source, SOURCE_FIELDS, label, issues):
        return
    _identifier(source["source_revision"], f"{label}.source_revision", issues)
    _digest(source["source_tree_sha256"], f"{label}.source_tree_sha256", issues)


def _validate_observation(observation: object, issues: list[str]) -> None:
    label = "request.observation"
    if not _exact_fields(observation, OBSERVATION_FIELDS, label, issues):
        return
    if observation["api_version"] != OBSERVATION_API_VERSION:
        issues.append(f"{label}.api_version: unsupported version")
    if observation["canonicalization"] != CANONICALIZATION:
        issues.append(f"{label}.canonicalization: unsupported format")
    _validate_content(observation["content"], issues)
    _validate_locator(observation["locator"], issues)
    observed = observation["observed_at_unix_ms"]
    if type(observed) is not int or not 0 <= observed <= MAX_I64:
        issues.append(f"{label}.observed_at_unix_ms: expected nonnegative signed-int64 integer")
    _validate_producer(observation["producer"], issues)
    _validate_scan_context(observation["scan_context"], issues)
    _validate_source(observation["source"], issues)


def validate_observation(observation: object) -> list[str]:
    """Validate one observation without reading its locator or report."""
    try:
        issues: list[str] = []
        _validate_observation(observation, issues)
        canonical_observation(observation)
        return issues
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["Evolve repository locator observation validation exhausted memory"]


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
    """Validate one exact request without granting truth or authority."""
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
        canonical_json(request)
        return issues
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["Evolve repository locator adapter request validation exhausted memory"]


def _require_valid_request(request: object) -> dict[str, object]:
    issues = validate_request(request)
    if issues:
        raise ContractError("; ".join(issues))
    if not isinstance(request, dict):
        raise ContractError("request: expected object")
    return request


def _locator_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(
        LOCATOR_DOMAIN + canonical_locator(request["observation"]["locator"])
    ).hexdigest()


def _source_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(
        SOURCE_DOMAIN + canonical_observation(request["observation"])
    ).hexdigest()


def _request_digest_unchecked(request: dict[str, object]) -> str:
    return hashlib.sha256(REQUEST_DOMAIN + canonical_json(request)).hexdigest()


def compute_locator_digest(request: dict[str, object]) -> str:
    return _locator_digest_unchecked(_require_valid_request(request))


def compute_source_digest(request: dict[str, object]) -> str:
    return _source_digest_unchecked(_require_valid_request(request))


def compute_request_digest(request: dict[str, object]) -> str:
    return _request_digest_unchecked(_require_valid_request(request))


def _principal(request_digest: str) -> dict[str, object]:
    return {"authority_domain": "shadow", "principal_id": ADAPTER_ID,
            "principal_type": "tool", "role": "evidence-adapter",
            "run_id": f"evolve-locator-adaptation-{request_digest}"}


def _collector(producer: dict[str, object]) -> dict[str, object]:
    return {"collector_id": producer["producer_id"],
            "collector_type": producer["producer_type"],
            "collector_version": producer["producer_version"],
            "parameters_sha256": producer["parameters_sha256"],
            "run_id": producer["run_id"]}


def _build_record(request: dict[str, object]) -> dict[str, object]:
    binding, observation = request["binding"], request["observation"]
    content, locator = observation["content"], observation["locator"]
    producer, source = observation["producer"], observation["source"]
    source_digest = _source_digest_unchecked(request)
    request_digest = _request_digest_unchecked(request)
    observed_at = observation["observed_at_unix_ms"]
    line = locator["line"] or None
    metadata = {
        "aggregate_id": binding["aggregate_id"], "context_sha256": binding["context_sha256"],
        "created_at_unix_ms": observed_at, "created_by": _principal(request_digest),
        "policy_sha256": binding["policy_sha256"], "project_id": binding["project_id"],
        "record_id": f"evolve-locator-evidence-{request_digest}",
        "scope": binding["scope"], "sequence": binding["sequence"],
        "source_revision": source["source_revision"],
        "source_tree_sha256": source["source_tree_sha256"],
        "supersedes_record_ids": list(binding["supersedes_record_ids"]),
    }
    spec = {
        "artifact_sha256": content["sha256"], "collector": _collector(producer),
        "content_role": "untrusted_data", "directness": "direct",
        "evidence_type": "repo_locator",
        "locator": {"content_sha256": content["sha256"], "exit_code": None,
                    "line_end": line, "line_start": line,
                    "locator_ref": locator["path"], "locator_type": "repo"},
        "observed_at_unix_ms": observed_at, "sensitivity": binding["sensitivity"],
        "source_snapshot": {"snapshot_id": f"evolve-locator-{source_digest}",
                            "snapshot_sha256": source_digest,
                            "snapshot_type": "repository"},
        "source_trust": "observed", "subjects": list(binding["subjects"]),
    }
    return {"api_version": "forgeos.governance/v1",
            "integrity": {"canonical_sha256": "", "canonicalization": CANONICALIZATION},
            "kind": "EvidenceRecord", "metadata": metadata, "spec": spec,
            "status": {"reason_codes": [], "state": "valid",
                       "valid_from_unix_ms": observed_at, "valid_until_unix_ms": None}}


def validate_evidence_record(record: object) -> list[str]:
    if not isinstance(record, dict):
        return ["evidence record: expected object"]
    if record.get("kind") != "EvidenceRecord":
        return ["evidence record.kind: must be EvidenceRecord"]
    issues: list[str] = []
    validate_record(record, 0, issues)
    return issues


def adapt_request(request: dict[str, object]) -> dict[str, object]:
    request = _require_valid_request(request)
    record = _build_record(request)
    record["integrity"]["canonical_sha256"] = compute_record_digest(record)
    issues = validate_evidence_record(record)
    if issues:
        raise ContractError("adapter produced invalid EvidenceRecord: " + "; ".join(issues))
    return record


def decode_evidence_record(raw: bytes) -> dict[str, object]:
    if len(raw) > MAX_RECORD_BYTES:
        raise ContractError(f"evidence record exceeds {MAX_RECORD_BYTES} bytes")
    value = decode_governance_json(raw)
    if not isinstance(value, dict):
        raise ContractError("evidence record root must be an object")
    if governance_json(value) != raw:
        raise ContractError("evidence record is not exact compact canonical JSON")
    return value


def validate_projection(request: object, record: object) -> list[str]:
    request_issues = validate_request(request)
    if request_issues:
        return request_issues
    record_issues = validate_evidence_record(record)
    if record_issues:
        return record_issues
    try:
        if governance_json(record) != governance_json(adapt_request(request)):
            return ["evidence record does not exactly match deterministic adaptation"]
        return []
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["Evolve repository locator Evidence projection validation exhausted memory"]


def check_projection_bytes(request_raw: bytes, evidence_raw: bytes) -> list[str]:
    try:
        return validate_projection(decode_request(request_raw),
                                   decode_evidence_record(evidence_raw))
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["Evolve repository locator Evidence processing exhausted memory"]
