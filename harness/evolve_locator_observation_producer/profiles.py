"""Closed report, parameter, and shared source profile validation."""

from __future__ import annotations

import hashlib
import json
from pathlib import PurePosixPath

from governance_contract import ContractError
from local_command_observation_producer.profiles import validate_source

from .codec import decode_json, forbidden_scalar
from .constants import (CANONICALIZATION, DEPTHS, DIMENSION_FIELDS,
                        DIMENSION_RANK, DIMENSIONS, EVIDENCE_FIELDS,
                        EVOLVE_ID_RE, HASH_RE, MARKER_PREFIX,
                        MAX_EVIDENCE_FILE_BYTES, MAX_I64,
                        MAX_REPORT_BYTES, MAX_REPORT_PAYLOAD_BYTES,
                        OPPORTUNITY_FIELDS, PARAMETERS_API, PARAMETERS_FIELDS,
                        REPORT_API, REPORT_FIELDS, REPORT_PROFILE,
                        REPORT_VALUE_FIELDS, SCAN_CONTRACT, SOURCE_PROFILE)


def exact_fields(value: object, fields: set[str], label: str,
                 issues: list[str], optional: set[str] | None = None) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    optional = optional or set()
    unknown, missing = set(value) - fields - optional, fields - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def bounded_text(value: object, label: str, issues: list[str], max_bytes: int,
                 *, allow_empty: bool = False) -> bool:
    if not isinstance(value, str) or (not allow_empty and not value.strip()):
        issues.append(f"{label}: expected bounded non-blank text")
        return False
    try:
        encoded = value.encode("utf-8")
    except UnicodeError:
        issues.append(f"{label}: expected valid UTF-8")
        return False
    if len(encoded) > max_bytes or any(forbidden_scalar(char) for char in value):
        issues.append(f"{label}: bounded UTF-8 or Unicode profile violated")
        return False
    return True


def validate_parameters(value: object) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, PARAMETERS_FIELDS, "parameters", issues):
        return issues
    if (value["api_version"] != PARAMETERS_API or
            value["canonicalization"] != CANONICALIZATION or
            value["contract"] != SCAN_CONTRACT or
            value["report_profile_id"] != REPORT_PROFILE or
            value["source_profile_id"] != SOURCE_PROFILE):
        issues.append("parameters: fixed fields drifted")
    if value["expected_depth"] not in DEPTHS:
        issues.append("parameters.expected_depth: unsupported Evolve depth")
    return issues


def _go_json(value: object) -> str:
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    return encoded.replace("&", "\\u0026").replace("<", "\\u003c").replace(
        ">", "\\u003e")


def _canonical_evidence(item: dict[str, object]) -> dict[str, object]:
    result = {"path": item["path"]}
    if item.get("line", 0) != 0:
        result["line"] = item["line"]
    result["detail"] = item["detail"]
    return result


def canonical_report(report: dict[str, object]) -> str:
    dimensions = sorted(report["dimensions"],
                        key=lambda item: DIMENSION_RANK.get(item["name"], 99))
    opportunities = sorted(report["opportunities"], key=lambda item: item["id"])
    encoded_dimensions = []
    for item in dimensions:
        value = {"name": item["name"], "status": item["status"],
                 "evidence": [_canonical_evidence(e) for e in item["evidence"]]}
        if item.get("unavailable_reason", "") != "":
            value["unavailable_reason"] = item["unavailable_reason"]
        encoded_dimensions.append(value)
    encoded_opportunities = []
    for item in opportunities:
        value = {"id": item["id"], "dimension": item["dimension"],
                 "title": item["title"],
                 "evidence": [_canonical_evidence(e) for e in item["evidence"]],
                 "obvious": item["obvious"]}
        if item.get("candidate_task", "") != "":
            value["candidate_task"] = item["candidate_task"]
        encoded_opportunities.append(value)
    value = {"version": report["version"], "depth": report["depth"],
             "dimensions": encoded_dimensions,
             "opportunities": encoded_opportunities}
    return MARKER_PREFIX + _go_json(value)


def _safe_path(value: object) -> bool:
    if (not isinstance(value, str) or not value or value.startswith(("/", "\\")) or
            "\\" in value or len(value) > 4096):
        return False
    if len(value) >= 2 and value[0].isascii() and value[0].isalpha() and value[1] == ":":
        return False
    path = PurePosixPath(value)
    return (str(path) == value and value != "." and
            all(part not in {"", ".", ".."} for part in path.parts) and
            bool(path.parts) and path.parts[0].lower() not in {".git", ".forge"})


def _validate_evidence(item: object, label: str, issues: list[str]) -> None:
    if not exact_fields(item, EVIDENCE_FIELDS, label, issues, {"line"}):
        return
    if not _safe_path(item["path"]):
        issues.append(f"{label}.path: expected canonical repository-relative path")
    bounded_text(item["path"], f"{label}.path", issues, 16_384)
    bounded_text(item["detail"], f"{label}.detail", issues, 512)
    line = item.get("line", 0)
    if type(line) is not int or not 0 <= line <= MAX_I64:
        issues.append(f"{label}.line: expected nonnegative signed-int64 integer")


def _validate_evidence_set(value: object, label: str, issues: list[str]) -> None:
    if not isinstance(value, list) or len(value) > 8:
        issues.append(f"{label}: expected array with at most 8 items")
        return
    seen = set()
    for index, item in enumerate(value):
        _validate_evidence(item, f"{label}[{index}]", issues)
        if isinstance(item, dict):
            key = (item.get("path"), item.get("line", 0), item.get("detail"))
            if key in seen:
                issues.append(f"{label}[{index}]: duplicate locator")
            seen.add(key)


def _validate_dimensions(report: dict[str, object], issues: list[str]) -> dict[str, object]:
    values = report["dimensions"]
    if not isinstance(values, list) or not 1 <= len(values) <= len(DIMENSIONS):
        issues.append("report.dimensions: expected array with 1..6 items")
        return {}
    records: dict[str, object] = {}
    for index, item in enumerate(values):
        label = f"report.dimensions[{index}]"
        if not exact_fields(item, DIMENSION_FIELDS, label, issues,
                            {"unavailable_reason"}):
            continue
        name, status, evidence = item["name"], item["status"], item["evidence"]
        if name not in DIMENSION_RANK or name in records:
            issues.append(f"{label}.name: unknown or duplicate dimension")
        else:
            records[name] = item
        _validate_evidence_set(evidence, f"{label}.evidence", issues)
        reason = item.get("unavailable_reason", "")
        if status in {"finding", "clear"}:
            if not isinstance(evidence, list) or not evidence or reason != "":
                issues.append(f"{label}: finding/clear requires evidence and no reason")
        elif status == "unavailable":
            if evidence != [] or not bounded_text(reason, f"{label}.unavailable_reason",
                                                   issues, 512):
                issues.append(f"{label}: unavailable requires only a reason")
        else:
            issues.append(f"{label}.status: unsupported status")
    names = [item.get("name") for item in values if isinstance(item, dict)]
    expected = sorted(names, key=lambda name: DIMENSION_RANK.get(name, 99))
    if names != expected:
        issues.append("report.dimensions: not in canonical dimension order")
    return records


def _intersects(left: list[object], right: list[object]) -> bool:
    for a in left:
        for b in right:
            if (isinstance(a, dict) and isinstance(b, dict) and
                    a.get("path") == b.get("path") and
                    (a.get("line", 0) == 0 or b.get("line", 0) == 0 or
                     a.get("line", 0) == b.get("line", 0))):
                return True
    return False


def _validate_opportunities(report: dict[str, object], records: dict[str, object],
                            issues: list[str]) -> None:
    values = report["opportunities"]
    if not isinstance(values, list) or len(values) > 24:
        issues.append("report.opportunities: expected array with at most 24 items")
        return
    ids, mapped = [], set()
    for index, item in enumerate(values):
        label = f"report.opportunities[{index}]"
        if not exact_fields(item, OPPORTUNITY_FIELDS, label, issues,
                            {"candidate_task"}):
            continue
        identifier = item["id"]
        if not isinstance(identifier, str) or EVOLVE_ID_RE.fullmatch(identifier) is None:
            issues.append(f"{label}.id: invalid Evolve identifier")
        ids.append(identifier)
        dimension = item["dimension"]
        finding = records.get(dimension)
        if not isinstance(finding, dict) or finding.get("status") != "finding":
            issues.append(f"{label}.dimension: must reference a finding")
        else:
            mapped.add(dimension)
        bounded_text(item["title"], f"{label}.title", issues, 512)
        _validate_evidence_set(item["evidence"], f"{label}.evidence", issues)
        if not isinstance(item["evidence"], list) or not item["evidence"]:
            issues.append(f"{label}.evidence: required")
        elif isinstance(finding, dict) and not _intersects(
                finding["evidence"], item["evidence"]):
            issues.append(f"{label}.evidence: must intersect its finding")
        if type(item["obvious"]) is not bool:
            issues.append(f"{label}.obvious: expected boolean")
        if report["depth"] == "opportunistic" and item["obvious"] is not True:
            issues.append(f"{label}.obvious: opportunistic requires true")
        task = item.get("candidate_task", "")
        if report["depth"] == "thorough" or task != "":
            bounded_text(task, f"{label}.candidate_task", issues, 2048)
    if ids != sorted(set(ids)):
        issues.append("report.opportunities: IDs must be sorted and unique")
    for name, item in records.items():
        if item.get("status") == "finding" and name not in mapped:
            issues.append(f"report: finding dimension {name!r} has no opportunity")


def validate_report(report: object, expected_depth: object) -> list[str]:
    issues: list[str] = []
    if not exact_fields(report, REPORT_VALUE_FIELDS, "report", issues):
        return issues
    if report["version"] != SCAN_CONTRACT or report["depth"] != expected_depth:
        issues.append("report: version or expected depth drifted")
    if report["depth"] not in DEPTHS:
        issues.append("report.depth: unsupported Evolve depth")
    records = _validate_dimensions(report, issues)
    _validate_opportunities(report, records, issues)
    if report["depth"] == "thorough":
        if set(records) != set(DIMENSIONS):
            issues.append("report: thorough requires all six dimensions")
        if any(item.get("status") == "unavailable" for item in records.values()):
            issues.append("report: thorough cannot contain unavailable")
    return issues


def parse_report_manifest(value: object, expected_depth: object) -> tuple[dict[str, object] | None, list[str]]:
    issues: list[str] = []
    if not exact_fields(value, REPORT_FIELDS, "report_manifest", issues):
        return None, issues
    if (value["api_version"] != REPORT_API or
            value["canonicalization"] != CANONICALIZATION or
            value["profile_id"] != REPORT_PROFILE):
        issues.append("report_manifest: fixed fields drifted")
    raw = value["canonical_report"]
    if not isinstance(raw, str) or not raw.startswith(MARKER_PREFIX) or "\n" in raw or "\r" in raw:
        issues.append("report_manifest.canonical_report: invalid exact marker line")
        return None, issues
    encoded = raw.encode("utf-8")
    if len(encoded) > MAX_REPORT_BYTES or type(value["bytes"]) is not int or value["bytes"] != len(encoded):
        issues.append("report_manifest.bytes: report size mismatch or limit exceeded")
    digest = hashlib.sha256(encoded).hexdigest()
    if not isinstance(value["sha256"], str) or value["sha256"] != digest:
        issues.append("report_manifest.sha256: report digest mismatch")
    try:
        report = decode_json(encoded[len(MARKER_PREFIX):],
                             max_bytes=MAX_REPORT_PAYLOAD_BYTES)
    except ContractError as error:
        issues.append(f"report_manifest.canonical_report: {error}")
        return None, issues
    if not isinstance(report, dict):
        issues.append("report_manifest.canonical_report: report must be an object")
        return None, issues
    issues.extend(validate_report(report, expected_depth))
    if not issues and canonical_report(report) != raw:
        issues.append("report_manifest.canonical_report: not exact canonical marker bytes")
    return report, issues


def source_index(source: object) -> tuple[dict[str, dict[str, object]], list[str]]:
    issues = validate_source(source)
    if issues or not isinstance(source, dict):
        return {}, issues
    return {entry["path"]: entry for entry in source["entries"]}, issues


def validate_locator_sources(report: dict[str, object], source: object) -> list[str]:
    index, issues = source_index(source)
    for relation in list(report["dimensions"]) + list(report["opportunities"]):
        if relation.get("status") == "unavailable":
            continue
        for evidence in relation["evidence"]:
            entry = index.get(evidence["path"])
            if (not isinstance(entry, dict) or entry.get("kind") != "regular" or
                    type(entry.get("bytes")) is not int or
                    not 1 <= entry["bytes"] <= MAX_EVIDENCE_FILE_BYTES or
                    not isinstance(entry.get("content_sha256"), str) or
                    HASH_RE.fullmatch(entry["content_sha256"]) is None):
                issues.append(
                    f"report locator {evidence['path']!r}: no bounded regular source entry"
                )
    return issues
