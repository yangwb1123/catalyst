#!/usr/bin/env python3
"""Evidence/Claim v1 registry, pin, Skill, and detector integration checks."""
import hashlib
import re

from engineering_check_support import (
    header_issues, load_yaml, mapping_issues, unknown_field_issues,
)
from engineering_detector_check import detector_index
from governance_contract import ContractError, read_bounded_file
from governance_contract_check import validate_golden_fixture


POLICY_RELATIVE = "engineering/governance-contracts.yml"
POLICY_SHA256 = "0b24f82848a9e0e1ce41b6418b9f09afbfff4728b1135bb4242a3c8b2b9d562f"
POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "scope", "canonicalization", "identity",
    "claim_states", "shadow_admissibility", "evidence_semantics", "legacy",
    "canonical_refs", "contract_pins", "reference_implementations", "non_capabilities",
}
PIN_TARGETS = {
    "schema_sha256": "docs/contracts/governance-evidence-claim-v1.schema.json",
    "golden_fixture_sha256":
        "docs/contracts/fixtures/governance-evidence-claim-v1.json",
}
SKILL_RELATIVE = ".agent/skills/evidence-claim-management.md"
SKILL_MARKERS = [
    "职责与触发", "输入契约", "执行 SOP", "输出契约", "规则、禁止与权限", "自动化与验收",
]


def _pin_issues(repo_root, policy_path, pins):
    issues = []
    for field, relative in PIN_TARGETS.items():
        target = repo_root / relative
        if not target.is_file():
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
    return [f"{path}: missing required section {marker!r}" for marker in SKILL_MARKERS
            if not re.search(rf"^##\s+.*{re.escape(marker)}", text, re.MULTILINE)]


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
        "status": "active_contract", "runtime_binding": "cross_language_codec_shadow",
        "version": 1, "completion_authority": "forge_accept",
    }
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{path}: {field} must remain the canonical v1 value")
    repo_root = agent_root.parent
    pins = data.get("contract_pins") if isinstance(data.get("contract_pins"), dict) else {}
    issues.extend(_pin_issues(repo_root, path, pins))
    try:
        policy_raw = read_bounded_file(path, label=POLICY_RELATIVE)
    except (OSError, ContractError) as error:
        issues.append(f"{path}: cannot validate protected policy: {error}")
    else:
        if hashlib.sha256(policy_raw).hexdigest() != POLICY_SHA256:
            issues.append(f"{path}: protected governance contract policy changed without a v1 update")
    issues.extend(_skill_issues(repo_root))
    issues.extend(_detector_issues(agent_root))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
