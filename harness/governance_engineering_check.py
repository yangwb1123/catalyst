#!/usr/bin/env python3
"""Evidence/Claim codec and local-journal registry integration checks."""
import hashlib
import json
import re

from engineering_check_support import (
    header_issues, load_yaml, mapping_issues, unknown_field_issues,
)
from engineering_detector_check import detector_index
from governance_contract import ContractError, read_bounded_file
from governance_contract_check import validate_golden_fixture


POLICY_RELATIVE = "engineering/governance-contracts.yml"
POLICY_SHA256 = "db91c548482ba23159bc507f167378e7dce1449b8981d8f4f5d0a05f3ee764df"
POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "scope", "canonicalization", "identity",
    "claim_states", "shadow_admissibility", "evidence_semantics", "journal", "legacy",
    "canonical_refs", "contract_pins", "reference_implementations", "non_capabilities",
}
PIN_TARGETS = {
    "schema_sha256": "docs/contracts/governance-evidence-claim-v1.schema.json",
    "journal_schema_sha256": "docs/contracts/governance-record-journal-v1.schema.json",
    "golden_fixture_sha256":
        "docs/contracts/fixtures/governance-evidence-claim-v1.json",
}
SKILL_RELATIVE = ".agent/skills/evidence-claim-management.md"
SKILL_MARKERS = [
    "职责与触发", "输入契约", "执行 SOP", "输出契约", "规则、禁止与权限", "自动化与验收",
]
REFERENCE_CLOSURE_LIMITS = {
    "max_dependency_records": 1024,
    "max_dependency_bytes": 16777216,
    "max_derivation_depth": 256,
    "reference_closure_classification": "resource_exhaustion_admissibility",
}
SCHEMA_CLOSURE_LIMITS = {
    "classification": "resource_exhaustion_admissibility",
    "max_dependency_records": 1024,
    "max_dependency_bytes": 16777216,
    "max_derivation_depth": 256,
}
RUNTIME_DELIVERY = {
    "command": "forge-runtime",
    "compatible_api": "forgeos.governance-journal/v1",
    "compatible_binary_required": True,
    "scaffold_inherits": ["contract", "skill", "shadow_checker"],
    "scaffold_installs_rust_binary": False,
    "unavailable_result": "not_executed",
    "persistence_claim_requires_receipt": True,
}


def _pin_issues(repo_root, policy_path, pins):
    issues = []
    for field, relative in PIN_TARGETS.items():
        target = repo_root / relative
        if not target.is_file():
            issues.append(f"{policy_path}: required pin target missing: {relative}")
            continue
        try:
            raw = read_bounded_file(target, label=relative)
        except (OSError, ContractError) as error:
            issues.append(f"{policy_path}: cannot validate {relative}: {error}")
            continue
        if hashlib.sha256(raw).hexdigest() != pins.get(field):
            issues.append(f"{policy_path}: {field} does not match {relative}")
    return issues


def _skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    if not path.is_file():
        return [f"{path}: required Evidence/Claim Skill missing"]
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate Skill: {error}"]
    issues = [f"{path}: missing required section {marker!r}" for marker in SKILL_MARKERS
              if not re.search(rf"^##\s+.*{re.escape(marker)}", text, re.MULTILINE)]
    commands = (
        "forge-runtime --idempotency-key KEY governance journal append",
        "forge-runtime governance journal show", "forge-runtime governance journal list",
        "forge-runtime governance journal head",
    )
    if any(command not in text for command in commands) or "forge governance journal" in text:
        issues.append(f"{path}: journal automation requires the compatible forge-runtime CLI")
    if "Scaffold/upgrade" not in text or "not_executed" not in text:
        issues.append(f"{path}: scaffold must not claim an installed journal runtime")
    if not all(value in text for value in ("1,024", "16,777,216", "admissibility limits")):
        issues.append(f"{path}: reference-closure resource limits are missing")
    return issues


def _journal_registry_issues(data, path):
    journal = data.get("journal") if isinstance(data, dict) else None
    if not isinstance(journal, dict):
        return [f"{path}: journal contract is missing"]
    limits = journal.get("limits") if isinstance(journal.get("limits"), dict) else {}
    issues = [f"{path}: journal.limits.{field} must remain {expected!r}"
              for field, expected in REFERENCE_CLOSURE_LIMITS.items()
              if limits.get(field) != expected]
    if journal.get("runtime_delivery") != RUNTIME_DELIVERY:
        issues.append(f"{path}: journal runtime/scaffold delivery boundary drifted")
    return issues


def _journal_schema_issues(repo_root):
    relative = PIN_TARGETS["journal_schema_sha256"]
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate journal Schema limits: {error}"]
    if schema.get("x-forgeos-reference-closure-limits") != SCHEMA_CLOSURE_LIMITS:
        return [f"{path}: reference-closure resource limits drifted"]
    return []


def _detector_issues(agent_root):
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.evidence_claim_contract"
    )
    expected = [
        "python3", "harness/governance_contract_check.py",
        "repo_root", "governance_record_set",
    ]
    if not isinstance(detector, dict):
        return ["governance Evidence/Claim detector is missing"]
    implementation = detector.get("implementation")
    if not isinstance(implementation, dict) or implementation.get("argv") != expected:
        return ["governance Evidence/Claim detector requires exact record-set arguments"]
    return []


def check_governance_evidence_claim_contract(agent_root):
    path = agent_root / POLICY_RELATIVE
    data, error = load_yaml(path)
    if error:
        return [f"{path}: invalid YAML ({error})"]
    issues = mapping_issues(data, path, "governance Evidence/Claim policy")
    if issues:
        return issues
    issues.extend(unknown_field_issues(data, POLICY_FIELDS, path))
    issues.extend(header_issues(data, path, "GovernanceContractRegistry"))
    if set(data) != POLICY_FIELDS:
        issues.append(f"{path}: governance contract policy fields drifted")
    expected = {
        "status": "active_contract",
        "runtime_binding": "cross_language_codec_local_journal_shadow",
        "version": 3, "completion_authority": "forge_accept",
    }
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{path}: {field} must remain the canonical v3 value")
    repo_root = agent_root.parent
    issues.extend(_journal_registry_issues(data, path))
    issues.extend(_journal_schema_issues(repo_root))
    pins = data.get("contract_pins") if isinstance(data.get("contract_pins"), dict) else {}
    issues.extend(_pin_issues(repo_root, path, pins))
    try:
        policy_raw = read_bounded_file(path, label=POLICY_RELATIVE)
    except (OSError, ContractError) as error:
        issues.append(f"{path}: cannot validate protected policy: {error}")
    else:
        if hashlib.sha256(policy_raw).hexdigest() != POLICY_SHA256:
            issues.append(f"{path}: protected governance contract policy changed without a version update")
    issues.extend(_skill_issues(repo_root))
    issues.extend(_detector_issues(agent_root))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
