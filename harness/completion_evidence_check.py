#!/usr/bin/env python3
"""Validate TaskEvidencePackage shape without minting a completion verdict."""
import re
from pathlib import Path

from engineering_check_support import header_issues, unknown_field_issues


INVARIANT_IDS = {
    "decision_fields_forbidden", "executed_receipt_is_structured",
    "non_execution_requires_reason", "source_binding_matches",
    "no_duplicate_receipt_ids",
}
INVARIANT_RULES = {
    "decision_fields_forbidden": "Evidence packages cannot contain status, verdict, completed or accepted decision fields.",
    "executed_receipt_is_structured": "Passed and failed observations require argv, exit-code semantics and an output digest.",
    "non_execution_requires_reason": "Not-executed and not-applicable observations carry a reason and no fabricated execution data.",
    "source_binding_matches": "Every receipt is bound to the package source tree digest.",
    "no_duplicate_receipt_ids": "Receipt identifiers are unique within one package.",
}
PACKAGE_CONTRACTS = {
    "task_id": {"type": "string", "non_empty": True, "max_length": 128},
    "summary": {"type": "string", "non_empty": True, "max_length": 4096},
    "source_revision": {"type": "string", "non_empty": True, "max_length": 128},
    "source_tree_sha256": {"type": "sha256"},
    "changed_files": {"type": "list", "items": "repo_relative_path", "max_items": 512},
    "requirements_covered": {"type": "list", "items": "string", "max_items": 512},
    "verification_receipts": {
        "type": "list", "items": "verification_receipt", "min_items": 1, "max_items": 128,
    },
    "residual_risks": {"type": "list", "items": "string", "max_items": 128},
    "assumptions": {"type": "list", "items": "string", "max_items": 128},
}
RECEIPT_CONTRACTS = {
    "id": {"type": "string", "non_empty": True, "max_length": 128},
    "detector_id": {"type": "string", "non_empty": True, "max_length": 128},
    "detector_version": {"type": "semver"},
    "source_tree_sha256": {"type": "sha256"},
    "status": {"type": "enum", "values": ["passed", "failed", "not_executed", "not_applicable"]},
    "argv": {"type": "list", "items": "string", "max_items": 64},
    "cwd": {"type": "repo_relative_path_or_dot"},
    "exit_code": {"type": "integer_or_null"},
    "output_sha256": {"type": "sha256_or_null"},
    "reason": {"type": "string", "max_length": 4096},
}
PACKAGE_FIELDS = set(PACKAGE_CONTRACTS)
RECEIPT_FIELDS = set(RECEIPT_CONTRACTS)
SHA256_RE = re.compile(r"sha256:[0-9a-f]{64}")
SEMVER_RE = re.compile(r"\d+\.\d+\.\d+")
DECISION_FIELDS = {"status", "verdict", "completed", "accepted", "decision"}


def _safe_report_path(raw, label, allow_dot=False):
    if allow_dot and raw == ".":
        return None
    if not isinstance(raw, str) or not raw.strip():
        return f"{label}: expected a non-empty repository-relative path"
    path = Path(raw)
    if path.is_absolute() or ".." in path.parts or "\\" in raw:
        return f"{label}: unsafe repository path {raw!r}"
    return None


def _scalar_issues(value, spec, label):
    field_type = spec.get("type")
    issues = []
    if field_type == "string":
        if not isinstance(value, str):
            return [f"{label} must be a string"]
        if spec.get("non_empty") is True and not value.strip():
            issues.append(f"{label} must be non-empty")
        if len(value) > spec.get("max_length", len(value)):
            issues.append(f"{label} exceeds max_length")
    elif field_type == "enum" and value not in set(spec.get("values") or []):
        issues.append(f"{label} has invalid enum value {value!r}")
    elif field_type == "sha256" and (not isinstance(value, str) or not SHA256_RE.fullmatch(value)):
        issues.append(f"{label} must be sha256:<64 lowercase hex>")
    elif field_type == "sha256_or_null" and value is not None and (
        not isinstance(value, str) or not SHA256_RE.fullmatch(value)
    ):
        issues.append(f"{label} must be null or sha256:<64 lowercase hex>")
    elif field_type == "semver" and (not isinstance(value, str) or not SEMVER_RE.fullmatch(value)):
        issues.append(f"{label} must be semantic version")
    elif field_type == "integer_or_null" and value is not None and (
        isinstance(value, bool) or not isinstance(value, int)
    ):
        issues.append(f"{label} must be an integer or null")
    elif field_type in {"repo_relative_path", "repo_relative_path_or_dot"}:
        issue = _safe_report_path(value, label, field_type.endswith("or_dot"))
        if issue:
            issues.append(issue)
    return issues


def _list_issues(value, spec, label, receipt_schema):
    if not isinstance(value, list):
        return [f"{label} must be a list"]
    issues = []
    if len(value) < spec.get("min_items", 0):
        issues.append(f"{label} is below min_items")
    if len(value) > spec.get("max_items", len(value)):
        issues.append(f"{label} exceeds max_items")
    item_type = spec.get("items")
    for index, item in enumerate(value):
        item_label = f"{label}[{index}]"
        if item_type == "verification_receipt":
            issues.extend(_object_issues(item, receipt_schema, item_label, receipt_schema))
        elif item_type == "string":
            issues.extend(_scalar_issues(item, {"type": "string", "non_empty": True}, item_label))
        else:
            issues.extend(_scalar_issues(item, {"type": item_type}, item_label))
    return issues


def _object_issues(value, schema, label, receipt_schema):
    if not isinstance(value, dict):
        return [f"{label} must be a mapping"]
    required, fields = set(schema.get("required_fields") or []), schema.get("fields") or {}
    issues = []
    missing = required - set(value)
    if missing:
        issues.append(f"{label} missing fields {sorted(missing)}")
    if schema.get("additional_properties") is False:
        issues.extend(unknown_field_issues(value, set(fields), label))
    for name, field_value in value.items():
        spec = fields.get(name)
        if not isinstance(spec, dict):
            continue
        if spec.get("type") == "list":
            issues.extend(_list_issues(field_value, spec, f"{label}.{name}", receipt_schema))
        else:
            issues.extend(_scalar_issues(field_value, spec, f"{label}.{name}"))
    return issues


def _receipt_semantic_issues(receipt, index, package_digest):
    if not isinstance(receipt, dict):
        return []
    label, issues = f"verification_receipts[{index}]", []
    status = receipt.get("status")
    argv, code, digest, reason = (
        receipt.get("argv"), receipt.get("exit_code"),
        receipt.get("output_sha256"), receipt.get("reason"),
    )
    if status in {"passed", "failed"}:
        if not isinstance(argv, list) or not argv or not all(isinstance(v, str) and v for v in argv):
            issues.append(f"{label}: executed result requires non-empty argv")
        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            issues.append(f"{label}: executed result requires output_sha256")
        if status == "passed" and (code != 0 or reason):
            issues.append(f"{label}: passed requires exit_code 0 and an empty reason")
        if status == "failed" and (not isinstance(code, int) or isinstance(code, bool) or code == 0 or not reason):
            issues.append(f"{label}: failed requires non-zero exit_code and a reason")
    elif status in {"not_executed", "not_applicable"}:
        if argv or code is not None or digest is not None or not isinstance(reason, str) or not reason.strip():
            issues.append(f"{label}: {status} requires reason and no execution claim")
    if receipt.get("source_tree_sha256") != package_digest:
        issues.append(f"{label}: source tree digest does not match the package")
    return issues


def _field_contract_issues(fields, label):
    issues = []
    common = {"type"}
    optional = {
        "string": {"non_empty", "max_length"}, "list": {"items", "min_items", "max_items"},
        "enum": {"values"}, "sha256": set(), "sha256_or_null": set(),
        "semver": set(), "integer_or_null": set(), "repo_relative_path": set(),
        "repo_relative_path_or_dot": set(),
    }
    for name, spec in fields.items() if isinstance(fields, dict) else []:
        if not isinstance(spec, dict) or spec.get("type") not in optional:
            issues.append(f"{label}.{name}: unsupported field contract")
            continue
        unknown = set(spec) - common - optional[spec["type"]]
        if unknown:
            issues.append(f"{label}.{name}: unknown field contract keys {sorted(unknown)}")
    return issues


def _canonical_type_issues(package, receipt, label):
    issues = []
    package_fields = ((package or {}).get("fields") or {})
    receipt_fields = ((receipt or {}).get("fields") or {})
    for field, expected in PACKAGE_CONTRACTS.items():
        if package_fields.get(field) != expected:
            issues.append(f"{label}: package.{field} contract must remain {expected!r}")
    for field, expected in RECEIPT_CONTRACTS.items():
        if receipt_fields.get(field) != expected:
            issues.append(f"{label}: receipt.{field} contract must remain {expected!r}")
    return issues


def validate_completion_contract(schema, label="completion evidence schema"):
    """Validate the executable schema itself; report validation consumes it."""
    if not isinstance(schema, dict):
        return [f"{label} must be a mapping"]
    allowed = {
        "api_version", "kind", "status", "owner", "schema_format",
        "package", "verification_receipt", "invariants", "example",
    }
    issues = unknown_field_issues(schema, allowed, label)
    issues.extend(header_issues(schema, label, "TaskEvidencePackageSchema"))
    if schema.get("schema_format") != "forgeos.simple-schema/v1":
        issues.append(f"{label}: unsupported schema_format")
    package, receipt = schema.get("package"), schema.get("verification_receipt")
    if not isinstance(package, dict) or set(package.get("required_fields") or []) != PACKAGE_FIELDS:
        issues.append(f"{label}: package required_fields are not the v1 evidence ABI")
    if not isinstance(receipt, dict) or set(receipt.get("required_fields") or []) != RECEIPT_FIELDS:
        issues.append(f"{label}: receipt required_fields are not the v1 evidence ABI")
    for name, contract, expected in (("package", package, PACKAGE_FIELDS), ("receipt", receipt, RECEIPT_FIELDS)):
        if not isinstance(contract, dict) or set(contract) != {"additional_properties", "required_fields", "fields"}:
            issues.append(f"{label}: {name} contract shape is invalid")
        elif contract.get("additional_properties") is not False or set(contract.get("fields") or {}) != expected:
            issues.append(f"{label}: {name} must fail closed on exactly the v1 fields")
        if isinstance(contract, dict):
            issues.extend(_field_contract_issues(contract.get("fields") or {}, f"{label}: {name}"))
    issues.extend(_canonical_type_issues(package, receipt, label))
    invariants = schema.get("invariants")
    invariant_ids = [item.get("id") for item in invariants or [] if isinstance(item, dict)]
    if len(invariant_ids) != len(set(invariant_ids)):
        issues.append(f"{label}: invariant ids must be unique")
    if set(invariant_ids) != INVARIANT_IDS:
        issues.append(f"{label}: invariants must be exactly {sorted(INVARIANT_IDS)}")
    for item in invariants or []:
        if not isinstance(item, dict) or set(item) != {"id", "rule"}:
            issues.append(f"{label}: each invariant requires exactly id/rule")
        elif INVARIANT_RULES.get(item.get("id")) != item.get("rule"):
            issues.append(f"{label}: invariant {item.get('id')!r} prose contradicts its executable meaning")
    if isinstance(schema.get("example"), dict):
        issues.extend(validate_evidence_package(schema["example"], schema, validate_contract=False))
    return issues


def validate_evidence_package(report, schema, validate_contract=True):
    """Validate one evidence package; never returns ACCEPTED/completed."""
    issues = validate_completion_contract(schema) if validate_contract else []
    if not isinstance(report, dict):
        return issues + ["evidence package must be a YAML mapping"]
    forbidden = DECISION_FIELDS & set(report)
    if forbidden:
        issues.append(f"evidence package contains forbidden decision fields {sorted(forbidden)}")
    package, receipt_schema = schema.get("package") or {}, schema.get("verification_receipt") or {}
    issues.extend(_object_issues(report, package, "evidence package", receipt_schema))
    receipts = report.get("verification_receipts")
    receipts = receipts if isinstance(receipts, list) else []
    ids = [item.get("id") for item in receipts if isinstance(item, dict)]
    if len(ids) != len(set(ids)):
        issues.append("evidence package has duplicate receipt ids")
    for index, receipt in enumerate(receipts):
        issues.extend(_receipt_semantic_issues(receipt, index, report.get("source_tree_sha256")))
    return issues
