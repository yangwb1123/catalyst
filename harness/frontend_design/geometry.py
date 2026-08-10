#!/usr/bin/env python3
"""Bind declarative DOM geometry reports to exact AFDS capture contexts."""
import math

from engineering_check_support import repo_path_issue, unknown_field_issues
from .composition_support import canonical_sha256, strict_json_file
from .contract import GEOMETRY_REPORT_MEDIA_TYPE

REPORT_FIELDS = {
    "api_version", "contract_sha256", "case_id", "source_tree_sha256",
    "build_sha256", "fixture_id", "environment_sha256", "coordinate_space",
    "runner", "assertions",
}
COORDINATE_SPACE_FIELDS = {"unit", "origin", "axis_orientation", "device_pixels_per_unit"}
COORDINATE_UNITS = {"css_px", "logical_dp", "device_px"}
RUNNER_FIELDS = {"name", "version"}
ASSERTION_FIELDS = {
    "id", "type", "subject_refs", "required", "tolerance", "observations", "result",
}
TOLERANCE_FIELDS = {"value", "policy_ref"}
OBSERVATION_FIELDS = {"subject_ref", "value"}
ASSERTION_TYPES = {
    "axis_alignment", "spacing", "boundary", "dimension", "overflow",
    "responsive_disposition", "stroke_anchor",
}
RESULTS = {"passed", "failed", "inconclusive", "not_executed"}


def _shape(value, fields, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, fields, label)
    missing = fields - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _text(value, label, maximum=256):
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        return [f"{label}: must be non-empty string <= {maximum} characters"]
    return []


def _strings(value, label, *, non_empty=False):
    if not isinstance(value, list) or (non_empty and not value) or len(value) > 256:
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list <= 256 items"]
    if not all(isinstance(item, str) and item.strip() for item in value):
        return [f"{label}: values must be non-empty strings"]
    return [f"{label}: values must be unique"] if len(value) != len(set(value)) else []


def _report_binding_issues(data, label, case):
    issues = []
    if data.get("api_version") != "forgeos.ui-geometry-report/v1":
        issues.append(f"{label}.api_version: unsupported")
    if data.get("case_id") != case.get("id"):
        issues.append(f"{label}.case_id: does not match capture case")
    for field in ("source_tree_sha256", "build_sha256", "fixture_id"):
        if data.get(field) != case.get(field):
            issues.append(f"{label}.{field}: does not match capture case")
    try:
        expected_environment = canonical_sha256(case.get("environment"))
    except (TypeError, ValueError):
        expected_environment = None  # The capture environment validator reports the malformed value.
    if expected_environment is not None and data.get("environment_sha256") != expected_environment:
        issues.append(f"{label}.environment_sha256: does not match capture environment")
    return issues


def _runner_issues(runner, label):
    issues = _shape(runner, RUNNER_FIELDS, f"{label}.runner")
    if isinstance(runner, dict):
        for field in RUNNER_FIELDS:
            issues.extend(_text(runner.get(field), f"{label}.runner.{field}"))
    return issues


def _coordinate_space_issues(space, label, case):
    coordinate_label = f"{label}.coordinate_space"
    issues = _shape(space, COORDINATE_SPACE_FIELDS, coordinate_label)
    if not isinstance(space, dict):
        return issues
    unit = space.get("unit")
    if unit not in COORDINATE_UNITS:
        issues.append(f"{coordinate_label}.unit: invalid")
    if space.get("origin") != "capture_viewport_top_left":
        issues.append(f"{coordinate_label}.origin: invalid")
    if space.get("axis_orientation") != "x_right_y_down":
        issues.append(f"{coordinate_label}.axis_orientation: invalid")
    scale = space.get("device_pixels_per_unit")
    if not isinstance(scale, (int, float)) or isinstance(scale, bool) \
            or not math.isfinite(scale) or not 0 < scale <= 8:
        issues.append(f"{coordinate_label}.device_pixels_per_unit: expected finite positive number <= 8")
        return issues
    if unit == "device_px" and scale != 1:
        issues.append(f"{coordinate_label}.device_pixels_per_unit: device_px requires 1")
    dpr = case.get("environment", {}).get("dpr") if isinstance(case, dict) else None
    if unit in {"css_px", "logical_dp"} and isinstance(dpr, (int, float)) \
            and not isinstance(dpr, bool) and math.isfinite(dpr) and scale != dpr:
        issues.append(f"{coordinate_label}.device_pixels_per_unit: must equal capture environment dpr")
    return issues


def _tolerance_issues(tolerance, label):
    issues = _shape(tolerance, TOLERANCE_FIELDS, f"{label}.tolerance")
    if not isinstance(tolerance, dict):
        return issues
    value = tolerance.get("value")
    if not isinstance(value, (int, float)) or isinstance(value, bool) \
            or not math.isfinite(value) or not 0 <= value <= 10000:
        issues.append(f"{label}.tolerance.value: expected finite number within 0..10000")
    policy_ref = tolerance.get("policy_ref")
    if not isinstance(policy_ref, str) or not policy_ref.startswith(("token:", "policy:", "profile:")):
        issues.append(f"{label}.tolerance.policy_ref: requires a symbolic policy source")
    return issues


def _observation_issues(observations, result, label, known_refs, subject_refs):
    if not isinstance(observations, list) or len(observations) > 512:
        return [f"{label}.observations: expected list <= 512 observations"]
    if result != "not_executed" and not observations:
        return [f"{label}.observations: executed assertions require 1..512 observations"]
    issues, observed_refs = [], []
    if result == "not_executed" and observations:
        issues.append(f"{label}.observations: not_executed assertion cannot contain observations")
    for index, observation in enumerate(observations):
        observation_label = f"{label}.observations[{index}]"
        issues.extend(_shape(observation, OBSERVATION_FIELDS, observation_label))
        if not isinstance(observation, dict):
            continue
        subject_ref = observation.get("subject_ref")
        if subject_ref not in known_refs:
            issues.append(f"{observation_label}.subject_ref: unknown composition reference")
        if isinstance(subject_ref, str):
            observed_refs.append(subject_ref)
        value = observation.get("value")
        if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value):
            issues.append(f"{observation_label}.value: expected finite number")
    if len(observed_refs) != len(set(observed_refs)):
        issues.append(f"{label}.observations: subject_ref values must be unique")
    expected = set(subject_refs) if isinstance(subject_refs, list) \
        and all(isinstance(ref, str) for ref in subject_refs) else set()
    observed = set(observed_refs)
    if result != "not_executed" and expected - observed:
        issues.append(f"{label}.observations: missing asserted subjects {sorted(expected - observed)}")
    if observed - expected:
        issues.append(f"{label}.observations: contains subjects outside assertion {sorted(observed - expected)}")
    return issues


def _assertion_record_issues(record, label, known_refs, seen):
    issues = _shape(record, ASSERTION_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    assertion_id = record.get("id")
    issues.extend(_text(assertion_id, f"{label}.id", 128))
    if isinstance(assertion_id, str):
        if assertion_id in seen:
            issues.append(f"{label}.id: duplicate assertion id {assertion_id!r}")
        seen.add(assertion_id)
    if record.get("type") not in ASSERTION_TYPES:
        issues.append(f"{label}.type: invalid")
    refs = record.get("subject_refs")
    issues.extend(_strings(refs, f"{label}.subject_refs", non_empty=True))
    if isinstance(refs, list) and all(isinstance(ref, str) for ref in refs):
        missing = set(refs) - set(known_refs)
        if missing:
            issues.append(f"{label}.subject_refs: unknown composition references {sorted(missing)}")
    if not isinstance(record.get("required"), bool):
        issues.append(f"{label}.required: expected boolean")
    issues.extend(_tolerance_issues(record.get("tolerance"), label))
    result = record.get("result")
    if result not in RESULTS:
        issues.append(f"{label}.result: invalid")
    issues.extend(_observation_issues(
        record.get("observations"), result, label, known_refs, refs,
    ))
    return issues


def _required_assertion_result(assertions):
    required = [record for record in assertions
                if isinstance(record, dict) and record.get("required") is True]
    if not required or any(record.get("result") not in RESULTS for record in required):
        return None
    results = {record["result"] for record in required}
    for result in ("failed", "inconclusive", "not_executed"):
        if result in results:
            return result
    return "passed"


def _report_record_issues(data, label, case, contract):
    issues = _shape(data, REPORT_FIELDS, label)
    if not isinstance(data, dict):
        return issues, None
    issues.extend(_report_binding_issues(data, label, case))
    issues.extend(_coordinate_space_issues(data.get("coordinate_space"), label, case))
    runner = data.get("runner")
    issues.extend(_runner_issues(runner, label))
    assertions = data.get("assertions")
    if not isinstance(assertions, list) or not assertions or len(assertions) > 256:
        return issues + [f"{label}.assertions: expected 1..256 records"], None
    seen = set()
    has_required = any(isinstance(item, dict) and item.get("required") is True
                       for item in assertions)
    if not has_required:
        issues.append(f"{label}.assertions: at least one assertion must be required")
    known_refs = contract.get("refs", set()) if isinstance(contract, dict) else set()
    for position, record in enumerate(assertions):
        item = f"{label}.assertions[{position}]"
        issues.extend(_assertion_record_issues(record, item, known_refs, seen))
    return issues, _required_assertion_result(assertions)


def _geometry_indexes(package, artifacts, claims):
    cases = {item.get("id"): item for item in package.get("verification_cases", [])
             if isinstance(item, dict) and isinstance(item.get("id"), str)} \
        if isinstance(package.get("verification_cases"), list) else {}
    reports = {item_id: record for item_id, record in artifacts.items()
               if isinstance(record, dict) and record.get("media_type") == GEOMETRY_REPORT_MEDIA_TYPE}
    geometry_claims = {item_id: claim for item_id, claim in claims.items()
                       if isinstance(claim, dict)
                       and "geometry_measurement_receipts" in set(claim.get("proof_types", []))}
    report_claims = {}
    for claim_id, claim in geometry_claims.items():
        for artifact_id in claim.get("artifact_ids", []) if isinstance(claim.get("artifact_ids"), list) else []:
            if artifact_id in reports:
                report_claims.setdefault(artifact_id, []).append((claim_id, claim))
    return cases, reports, geometry_claims, report_claims


def _capture_binding_issues(artifact_id, artifact, cases, report_claims):
    label = f"FrontendDesignPackage.evidence_artifacts[{artifact_id!r}]"
    issues = []
    if artifact.get("kind") != "tool_output":
        issues.append(f"{label}: geometry report must be a tool_output artifact")
    bindings = report_claims.get(artifact_id, [])
    if len(bindings) != 1:
        issues.append(f"{label}: geometry report requires exactly one measurement proof claim")
        return issues, label, None, None
    claim_id, claim = bindings[0]
    case = cases.get(claim.get("subject_id")) if claim.get("subject_type") == "verification_case" else None
    if not isinstance(case, dict) or case.get("kind") != "capture":
        issues.append(f"{label}: geometry report claim must target one capture case")
        return issues, label, None, None
    if claim_id not in set(case.get("proof_claim_ids", [])) \
            or artifact_id not in set(case.get("artifact_ids", [])):
        issues.append(f"{label}: geometry report is not declared by its capture case")
    return issues, label, claim, case


def _read_report(repo_root, artifact, label, compositions):
    locator = artifact.get("locator")
    path_issue = repo_path_issue(repo_root, locator, f"{label}.locator")
    if path_issue:
        return [path_issue], None, None
    data, issues = strict_json_file(repo_root / locator, label)
    digest = data.get("contract_sha256") if isinstance(data, dict) else None
    contract = compositions.get(digest)
    if not isinstance(contract, dict):
        issues.append(f"{label}.contract_sha256: does not resolve to a composition artifact")
        contract = {}
    return issues, data, contract


def _claim_result_issues(claim, expected_result, label):
    actual = claim.get("result")
    if actual == "passed" and expected_result != "passed":
        return [f"{label}: passed claim cannot hide failed/inconclusive required assertions"]
    if expected_result == "passed" and actual != "passed":
        return [f"{label}: claim result disagrees with required assertion results"]
    if expected_result is not None and actual != expected_result:
        return [f"{label}: claim result {actual!r} does not match required assertion result "
                f"{expected_result!r}"]
    return []


def _geometry_artifact_issues(artifact_id, artifact, repo_root, cases,
                              report_claims, compositions):
    issues, label, claim, case = _capture_binding_issues(
        artifact_id, artifact, cases, report_claims,
    )
    if claim is None:
        return issues, None, False
    read_issues, data, contract = _read_report(repo_root, artifact, label, compositions)
    issues.extend(read_issues)
    if data is None:
        return issues, None, False
    record_issues, required_result = _report_record_issues(data, label, case, contract)
    issues.extend(record_issues)
    issues.extend(_claim_result_issues(claim, required_result, label))
    supports_readiness = required_result == "passed" and claim.get("result") == "passed"
    return issues, case.get("id"), supports_readiness


def _claim_artifact_issues(geometry_claims, artifacts):
    issues = []
    for claim_id, claim in geometry_claims.items():
        kinds = [artifacts.get(ref, {}).get("media_type") for ref in claim.get("artifact_ids", [])
                 if isinstance(ref, str)] if isinstance(claim.get("artifact_ids"), list) else []
        if GEOMETRY_REPORT_MEDIA_TYPE not in kinds:
            issues.append(f"FrontendDesignPackage.proof_claims[{claim_id!r}]: geometry proof lacks geometry report")
    return issues


def _visual_readiness_issues(package, compositions, cases, capture_report_ids):
    readiness = {item.get("id"): item for item in package.get("readiness", []) if isinstance(item, dict)} \
        if isinstance(package.get("readiness"), list) else {}
    if not compositions or readiness.get("visual_evidence", {}).get("result") != "ready":
        return []
    capture_ids = {item_id for item_id, case in cases.items() if case.get("kind") == "capture"}
    missing = capture_ids - capture_report_ids
    if missing:
        return [f"FrontendDesignPackage: visual readiness lacks geometry reports for capture cases {sorted(missing)}"]
    return []


def geometry_report_issues(package, repo_root, artifacts, claims, compositions):
    """Validate report bytes, exact case binding, and no score-based bypass."""
    issues = []
    cases, reports, geometry_claims, report_claims = _geometry_indexes(package, artifacts, claims)
    fingerprints = {}
    ready_capture_report_ids = set()
    for artifact_id, artifact in reports.items():
        artifact_issues, case_id, supports_readiness = _geometry_artifact_issues(
            artifact_id, artifact, repo_root, cases, report_claims, compositions,
        )
        issues.extend(artifact_issues)
        if case_id is None:
            continue
        fingerprint = artifact.get("content_sha256")
        if isinstance(fingerprint, str):
            prior = fingerprints.setdefault(fingerprint, case_id)
            if prior != case_id:
                label = f"FrontendDesignPackage.evidence_artifacts[{artifact_id!r}]"
                issues.append(f"{label}: identical geometry report bytes reused from case {prior!r}")
        if supports_readiness:
            ready_capture_report_ids.add(case_id)
    issues.extend(_claim_artifact_issues(geometry_claims, artifacts))
    issues.extend(_visual_readiness_issues(package, compositions, cases, ready_capture_report_ids))
    return issues
