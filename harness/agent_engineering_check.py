#!/usr/bin/env python3
"""Validate the active Agent Engineering governance contracts.

The module is imported by ``harness/check.py`` and can also validate a concrete
task evidence package. It intentionally validates contracts and references only;
it does not pretend that planned AADM, reflection or device runtimes exist.
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

from completion_evidence_check import (
    validate_completion_contract,
    validate_evidence_package,
)
from backend_decision_check import check_backend_decision_contract as _check_backend_decision_contract
from frontend_design_check import check_frontend_design_contract as _check_frontend_design_contract
from engineering_detector_check import (
    check_capability_ownership as _check_capability_ownership,
    check_engineering_activation as _check_engineering_activation,
    check_engineering_detectors as _check_engineering_detectors,
    check_engineering_project_binding as _check_engineering_project_binding,
    detector_index as _detector_index,
)
from engineering_check_support import (
    header_issues as _header_issues,
    load_yaml as _load_yaml,
    mapping_issues as _mapping,
    repo_path_issue as _repo_path_issue,
    unique_id_issues as _unique_ids,
    unknown_field_issues as _unknown_fields,
)
from engineering_routing_check import (
    check_engineering_context_routes as _check_engineering_context_routes,
    check_engineering_workflow_profiles as _check_engineering_workflow_profiles,
)
from governance_engineering_check import check_governance_evidence_claim_contract


SPEC_FILES = {
    "activation": "engineering/activation.yml",
    "disciplines": "engineering/disciplines.yml",
    "rules": "engineering/rules.yml",
    "detectors": "engineering/detectors.yml",
    "contexts": "engineering/context-routes.yml",
    "profiles": "engineering/workflow-profiles.yml",
    "completion": "eval/completion-evidence.schema.yml",
    "backend_policy": "engineering/backend-decision-gates.yml",
    "backend_package": "eval/backend-decision-package.schema.yml",
    "frontend_policy": "engineering/frontend-design-gates.yml",
    "frontend_profiles": "engineering/frontend-profiles.yml",
    "frontend_package": "eval/frontend-design-package.schema.yml",
    "frontend_architecture_policy": "engineering/frontend-code-architecture.yml",
    "governance_contracts": "engineering/governance-contracts.yml",
}
PROJECT_REFS = {
    "activation": ".agent/engineering/activation.yml",
    "disciplines": ".agent/engineering/disciplines.yml",
    "rules": ".agent/engineering/rules.yml",
    "detectors": ".agent/engineering/detectors.yml",
    "context_routes": ".agent/engineering/context-routes.yml",
    "workflow_profiles": ".agent/engineering/workflow-profiles.yml",
    "capability_catalog": "docs/design/ai-engineering-os/capability-catalog.v1.yml",
    "capability_skill_map": "docs/design/ai-engineering-os/capability-skill-map.v1.yml",
    "acceptance_policy": ".agent/eval/acceptance.schema.yml",
    "completion_contract": ".agent/eval/completion-evidence.schema.yml",
}
EXTENSION_REFS = {
    "backend_policy": ".agent/engineering/backend-decision-gates.yml",
    "backend_package": ".agent/eval/backend-decision-package.schema.yml",
    "backend_standard": "docs/design/ai-engineering-os/backend-decision-standard.md",
    "frontend_policy": ".agent/engineering/frontend-design-gates.yml",
    "frontend_profiles": ".agent/engineering/frontend-profiles.yml",
    "frontend_package": ".agent/eval/frontend-design-package.schema.yml",
    "frontend_standard": "docs/design/ai-engineering-os/frontend-design-standard.md",
    "frontend_architecture_policy": ".agent/engineering/frontend-code-architecture.yml",
    "frontend_architecture_contract": ".arch/frontend-architecture.v1.json",
    "frontend_architecture_baseline": ".arch/frontend-architecture-baseline.v1.json",
    "frontend_architecture_waivers": ".arch/frontend-architecture-waivers.v1.json",
    "frontend_architecture_standard": "docs/design/ai-engineering-os/frontend-code-architecture-standard.md",
    "governance_contract_registry": ".agent/engineering/governance-contracts.yml",
    "governance_contract_schema": "docs/contracts/governance-evidence-claim-v1.schema.json",
    "governance_journal_schema": "docs/contracts/governance-record-journal-v1.schema.json",
    "governance_contract_fixture": "docs/contracts/fixtures/governance-evidence-claim-v1.json",
    "cognitive_atom_schema": "docs/contracts/cognitive-atom-projection-v1.schema.json",
    "cognitive_atom_fixture": "docs/contracts/fixtures/cognitive-atom-projection-v1.json",
    "cognitive_atom_checker": "harness/cognitive_atom_contract_check.py",
    "artifact_evidence_adapter_schema": "docs/contracts/artifact-evidence-adapter-v1.schema.json",
    "artifact_evidence_adapter_fixture": "docs/contracts/fixtures/artifact-evidence-adapter-v1.json",
    "artifact_evidence_adapter_checker": "harness/artifact_evidence_adapter_check.py",
    "command_observation_evidence_adapter_schema": "docs/contracts/command-observation-evidence-adapter-v1.schema.json",
    "command_observation_evidence_adapter_fixture": "docs/contracts/fixtures/command-observation-evidence-adapter-v1.json",
    "command_observation_evidence_adapter_checker": "harness/command_observation_evidence_adapter_check.py",
    "evolve_repo_locator_evidence_adapter_schema": "docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json",
    "evolve_repo_locator_evidence_adapter_fixture": "docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json",
    "evolve_repo_locator_evidence_adapter_checker": "harness/evolve_repo_locator_evidence_adapter_check.py",
    "local_gate_command_observation_producer_schema": "docs/contracts/local-gate-command-observation-producer-v1.schema.json",
    "local_gate_command_observation_producer_fixture": "docs/contracts/fixtures/local-gate-command-observation-producer-v1.json",
    "local_gate_command_observation_producer_checker": "harness/local_command_observation_producer_check.py",
    "local_evolve_repo_locator_observation_producer_schema": "docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json",
    "local_evolve_repo_locator_observation_producer_fixture": "docs/contracts/fixtures/local-evolve-repo-locator-observation-producer-v1.json",
    "local_evolve_repo_locator_observation_producer_checker": "harness/evolve_locator_observation_producer/check.py",
    "governance_contract_skill": ".agent/skills/evidence-claim-management.md",
    "governance_contract_decision": "docs/adr/0045-canonical-evidence-claim-contract.md",
    "governance_journal_decision": "docs/adr/0046-local-governance-record-journal.md",
    "cognitive_atom_decision": "docs/adr/0047-shadow-cognitive-atom-projection-v1.md",
    "artifact_evidence_adapter_decision": "docs/adr/0048-artifact-provenance-evidence-adapter-v1.md",
    "command_observation_evidence_adapter_decision": "docs/adr/0049-command-observation-evidence-adapter-v1.md",
    "evolve_repo_locator_evidence_adapter_decision": "docs/adr/0050-evolve-repo-locator-evidence-adapter-v1.md",
    "local_gate_command_observation_producer_decision": "docs/adr/0051-local-gate-command-observation-producer-v1.md",
    "local_evolve_repo_locator_observation_producer_decision": "docs/adr/0052-local-evolve-repo-locator-observation-producer-v1.md",
    "governance_contract_standard": "docs/design/ai-engineering-os/governance-contracts.md",
}
DISCIPLINES = {
    "prompt", "context", "memory", "tool", "planning", "loop",
    "reflection", "graph", "harness", "evaluation", "knowledge",
    "evolution", "state", "contract",
}
RULE_FIELDS = {
    "id", "title", "level", "severity", "scope", "trigger", "rationale",
    "requirements", "forbidden", "verification", "exceptions", "owner", "version",
}
PROTECTED_RULE_DIGESTS = {
    "SEC-001": "3c8dae3b4f9d80072c65f708536fd033a3df81717fbc3344fb1657ef22f2cf2b",
    "ARCH-001": "59f527997f552053e78549731447d37a83c3facdcebddeff0dda5cedb9fa221b",
    "QUAL-001": "b0c5358218fe557587f2e0ff2a1331ff9f31bddb77ac8efe72adc696c59a0f90",
    "GOV-001": "82a2ddf7d268bbac46bec8c14328ed083f683b6fd423e9a57bc764fbb9d04ee7",
}
FRONTEND_ARCH_POLICY_SHA256 = "2bc6dcd6e40b670cfea0a52e5ba248ffca74adba75b4216eceb7db2028b583ab"
FRONTEND_ARCH_POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "capability_relationship", "applicability",
    "decision_sequence", "architecture_profiles", "hard_invariants",
    "review_lenses", "god_file_risk", "exception_contract",
    "evidence_contract", "canonical_refs",
}
def check_engineering_files(agent_root):
    issues = []
    for name, relative in SPEC_FILES.items():
        path = agent_root / relative
        if not path.is_file():
            issues.append(f"{path}: required Agent Engineering {name} contract missing")
    return issues


def check_engineering_activation(agent_root):
    return _check_engineering_activation(
        agent_root, SPEC_FILES["activation"], PROJECT_REFS, EXTENSION_REFS,
    )


def check_engineering_project_binding(agent_root):
    issues = _check_engineering_project_binding(agent_root, PROJECT_REFS)
    for name, raw in PROJECT_REFS.items():
        issue = _repo_path_issue(agent_root.parent, raw, f"canonical Agent Engineering ref {name}")
        if issue:
            issues.append(issue)
    for name, raw in EXTENSION_REFS.items():
        issue = _repo_path_issue(agent_root.parent, raw, f"canonical Agent Engineering extension {name}")
        if issue:
            issues.append(issue)
    return issues


def check_engineering_detectors(agent_root):
    return _check_engineering_detectors(agent_root, SPEC_FILES["detectors"])


def check_capability_ownership(agent_root):
    return _check_capability_ownership(
        agent_root, PROJECT_REFS["capability_catalog"], PROJECT_REFS["capability_skill_map"],
    )


def check_engineering_disciplines(agent_root):
    path = agent_root / SPEC_FILES["disciplines"]
    data, err = _load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = _mapping(data, path, "discipline registry")
    if issues:
        return issues
    issues.extend(_unknown_fields(
        data, {"api_version", "kind", "status", "runtime_binding", "owner", "allowed_states", "disciplines"}, path,
    ))
    issues.extend(_header_issues(data, path, "DisciplineRegistry"))
    items = data.get("disciplines")
    id_issues, ids = _unique_ids(items, path, "discipline")
    issues.extend(id_issues)
    if ids != DISCIPLINES:
        issues.append(f"{path}: disciplines must be exactly {sorted(DISCIPLINES)}")
    states = set(data.get("allowed_states") or [])
    if states != {"enforced", "partial", "planned"}:
        issues.append(f"{path}: allowed_states vocabulary is invalid")
    repo_root = agent_root.parent
    for item in items if isinstance(items, list) else []:
        if not isinstance(item, dict):
            continue
        issues.extend(_unknown_fields(item, {"id", "purpose", "state", "assets"}, f"{path}: discipline"))
        if item.get("state") not in states:
            issues.append(f"{path}: discipline {item.get('id')!r} has invalid state")
        if not str(item.get("purpose") or "").strip():
            issues.append(f"{path}: discipline {item.get('id')!r} requires purpose")
        for raw in item.get("assets") or []:
            issue = _repo_path_issue(repo_root, raw, f"{path}: {item.get('id')} asset")
            if issue:
                issues.append(issue)
    return issues


def _rule_shape_issues(rule, path):
    rule_id = rule.get("id", "<unknown>")
    issues = []
    missing = RULE_FIELDS - set(rule)
    if missing:
        issues.append(f"{path}: rule {rule_id!r} missing fields {sorted(missing)}")
    issues.extend(_unknown_fields(rule, RULE_FIELDS, f"{path}: rule {rule_id!r}"))
    for field in ("scope", "requirements", "forbidden"):
        if not isinstance(rule.get(field), list) or not rule.get(field):
            issues.append(f"{path}: rule {rule_id!r} requires non-empty {field}")
        elif not all(isinstance(value, str) and value.strip() for value in rule[field]):
            issues.append(f"{path}: rule {rule_id!r} {field} must contain non-empty strings")
    if not isinstance(rule.get("trigger"), dict) or not rule.get("trigger"):
        issues.append(f"{path}: rule {rule_id!r} requires a trigger mapping")
    if not re.fullmatch(r"[A-Z]+-\d{3}", str(rule_id)):
        issues.append(f"{path}: invalid rule id {rule_id!r}")
    if not re.fullmatch(r"\d+\.\d+\.\d+", str(rule.get("version", ""))):
        issues.append(f"{path}: rule {rule_id!r} requires semantic version")
    exceptions = rule.get("exceptions")
    if not isinstance(exceptions, dict) or "allowed" not in exceptions:
        issues.append(f"{path}: rule {rule_id!r} requires an exceptions policy")
    elif not isinstance(exceptions.get("allowed"), bool):
        issues.append(f"{path}: rule {rule_id!r} exceptions.allowed must be boolean")
    elif exceptions.get("allowed") is False and set(exceptions) != {"allowed"}:
        issues.append(f"{path}: rule {rule_id!r} disallowed exceptions cannot declare a process")
    elif exceptions.get("allowed") is True and (
        set(exceptions) != {"allowed", "process"}
        or not isinstance(exceptions.get("process"), str)
        or not exceptions["process"].strip()
    ):
        issues.append(f"{path}: rule {rule_id!r} allowed exceptions require a non-empty process")
    return issues


def _rule_verification_issues(rule, data, path, detectors):
    rule_id = rule.get("id", "<unknown>")
    verification = rule.get("verification")
    if not isinstance(verification, dict):
        return [f"{path}: rule {rule_id!r} requires verification mapping"]
    mode = verification.get("mode")
    issues = []
    if mode not in set(data.get("enforcement_modes") or []):
        issues.append(f"{path}: rule {rule_id!r} has invalid verification mode")
    allowed = {"mode", "detector_refs", "reviewer_refs", "proof_types"}
    issues.extend(_unknown_fields(verification, allowed, f"{path}: rule {rule_id!r} verification"))
    detector_refs = verification.get("detector_refs")
    reviewer_refs = verification.get("reviewer_refs")
    proof_types = verification.get("proof_types")
    if rule.get("severity") == "error" and mode != "automatic":
        issues.append(f"{path}: error rule {rule_id!r} must be automatic")
    if mode == "automatic":
        if not isinstance(detector_refs, list) or not detector_refs:
            issues.append(f"{path}: automatic rule {rule_id!r} requires detector_refs")
        if reviewer_refs is not None:
            issues.append(f"{path}: automatic rule {rule_id!r} cannot use reviewer_refs")
        for detector_id in detector_refs or []:
            detector = detectors.get(detector_id)
            if not detector or detector.get("state") != "enforced":
                issues.append(f"{path}: rule {rule_id!r} references a non-enforced detector {detector_id!r}")
            elif rule_id not in set(detector.get("rule_refs") or []):
                issues.append(f"{path}: detector {detector_id!r} does not declare coverage for rule {rule_id!r}")
    else:
        if not isinstance(reviewer_refs, list) or not reviewer_refs:
            issues.append(f"{path}: {mode} rule {rule_id!r} requires reviewer_refs")
        if detector_refs is not None:
            issues.append(f"{path}: non-automatic rule {rule_id!r} cannot claim detector_refs")
    if not isinstance(proof_types, list) or not proof_types or not all(
        isinstance(value, str) and value.strip() for value in proof_types
    ):
        issues.append(f"{path}: rule {rule_id!r} requires non-empty proof_types")
    return issues


def _protected_rule_issues(rules, path):
    index = {rule.get("id"): rule for rule in rules if isinstance(rule, dict)}
    issues = []
    for rule_id, expected in PROTECTED_RULE_DIGESTS.items():
        rule = index.get(rule_id)
        if rule is None:
            issues.append(f"{path}: protected automatic rule {rule_id!r} is missing")
            continue
        payload = json.dumps(
            rule, sort_keys=True, separators=(",", ":"), ensure_ascii=False, default=str,
        ).encode("utf-8")
        if hashlib.sha256(payload).hexdigest() != expected:
            issues.append(
                f"{path}: protected automatic rule {rule_id!r} changed without a v1 governance update"
            )
    return issues


def check_engineering_rules(agent_root):
    path = agent_root / SPEC_FILES["rules"]
    data, err = _load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = _mapping(data, path, "rule registry")
    if issues:
        return issues
    issues.extend(_unknown_fields(
        data, {"api_version", "kind", "status", "owner", "levels", "severities", "enforcement_modes", "rules"}, path,
    ))
    issues.extend(_header_issues(data, path, "EngineeringRuleRegistry"))
    rules = data.get("rules")
    if not isinstance(rules, list) or not rules:
        return issues + [f"{path}: rules must be a non-empty list"]
    id_issues, _ = _unique_ids(rules, path, "rule")
    issues.extend(id_issues)
    issues.extend(_protected_rule_issues(rules, path))
    levels, severities = set(data.get("levels") or []), set(data.get("severities") or [])
    if levels != {"invariant", "contract", "policy", "heuristic", "suggestion"}:
        issues.append(f"{path}: rule level vocabulary is invalid")
    if severities != {"error", "warning", "advice"}:
        issues.append(f"{path}: rule severity vocabulary is invalid")
    detectors = _detector_index(agent_root, SPEC_FILES["detectors"])
    for rule in rules if isinstance(rules, list) else []:
        if not isinstance(rule, dict):
            continue
        issues.extend(_rule_shape_issues(rule, path))
        if rule.get("level") not in levels or rule.get("severity") not in severities:
            issues.append(f"{path}: rule {rule.get('id')!r} has invalid level/severity")
        issues.extend(_rule_verification_issues(rule, data, path, detectors))
    return issues


def check_engineering_context_routes(agent_root):
    return _check_engineering_context_routes(agent_root, SPEC_FILES["contexts"])


def check_engineering_workflow_profiles(agent_root):
    return _check_engineering_workflow_profiles(agent_root, SPEC_FILES["profiles"])


def check_completion_evidence_schema(agent_root):
    path = agent_root / SPEC_FILES["completion"]
    data, err = _load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    return validate_completion_contract(data, str(path))


def check_backend_decision_contract(agent_root):
    return _check_backend_decision_contract(agent_root.parent)


def check_frontend_design_contract(agent_root):
    return _check_frontend_design_contract(agent_root.parent)


def _frontend_architecture_policy_issues(agent_root):
    path = agent_root / SPEC_FILES["frontend_architecture_policy"]
    data, err = _load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = _mapping(data, path, "frontend code architecture policy")
    if issues:
        return issues
    issues.extend(_unknown_fields(data, FRONTEND_ARCH_POLICY_FIELDS, path))
    issues.extend(_header_issues(data, path, "FrontendCodeArchitecturePolicy"))
    if set(data) != FRONTEND_ARCH_POLICY_FIELDS:
        issues.append(f"{path}: frontend architecture policy fields drifted")
    expected = {
        "runtime_binding": "standalone_shadow_detector_plus_review",
        "completion_authority": "forge_accept", "version": 1,
        "canonical_refs": {
            "skill": ".agent/skills/frontend-code-architecture.md",
            "architecture_contract": ".arch/frontend-architecture.v1.json",
            "architecture_baseline": ".arch/frontend-architecture-baseline.v1.json",
            "architecture_waivers": ".arch/frontend-architecture-waivers.v1.json",
            "detector": "harness/frontend-architecture/check.mjs",
            "detector_contract": "harness/frontend-architecture/contract.mjs",
            "standard": "docs/design/ai-engineering-os/frontend-code-architecture-standard.md",
            "review_skill": ".agent/skills/code-review.md",
            "refactor_skill": ".agent/skills/god-object-refactoring.md",
        },
    }
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{path}: {field} must remain the canonical v1 value")
    if hashlib.sha256(path.read_bytes()).hexdigest() != FRONTEND_ARCH_POLICY_SHA256:
        issues.append(f"{path}: protected frontend architecture policy changed without a v1 update")
    return issues


def check_frontend_code_architecture_contract(agent_root):
    issues = _frontend_architecture_policy_issues(agent_root)
    repo_root = agent_root.parent
    skill = repo_root / ".agent" / "skills" / "frontend-code-architecture.md"
    markers = ["职责与触发", "执行 SOP", "自动硬约束", "审查启发式", "例外合同", "完成条件"]
    if not skill.is_file():
        issues.append(f"{skill}: required frontend architecture Skill missing")
    else:
        text = skill.read_text(encoding="utf-8")
        for marker in markers:
            if not re.search(rf"^##\s+.*{re.escape(marker)}", text, re.MULTILINE):
                issues.append(f"{skill}: missing required section {marker!r}")
    try:
        result = subprocess.run(
            ["node", "harness/frontend-architecture/check.mjs", "--contract-only", str(repo_root)],
            cwd=repo_root, capture_output=True, text=True, timeout=30, check=False,
        )
        if result.returncode != 0:
            detail = (result.stdout + result.stderr).strip().replace("\n", "; ")
            issues.append(f"frontend architecture contract validator failed: {detail}")
    except (OSError, subprocess.TimeoutExpired) as exc:
        issues.append(f"frontend architecture contract validator unavailable: {exc}")
    return issues


def check_agent_engineering_spec(agent_root):
    """One composed check for ``harness/check.py``'s CHECKS registry."""
    issues = check_engineering_files(agent_root)
    if issues:
        return issues
    checks = (
        check_engineering_activation, check_engineering_project_binding,
        check_engineering_disciplines, check_engineering_detectors,
        check_capability_ownership, check_engineering_rules,
        check_engineering_context_routes, check_engineering_workflow_profiles,
        check_completion_evidence_schema, check_backend_decision_contract,
        check_frontend_design_contract, check_frontend_code_architecture_contract,
        check_governance_evidence_claim_contract,
    )
    return [issue for check in checks for issue in check(agent_root)]


def main(argv):
    repo_root = Path(argv[1] if len(argv) > 1 else ".")
    agent_root = repo_root / ".agent"
    issues = check_agent_engineering_spec(agent_root)
    if len(argv) > 2 and not issues:
        report, err = _load_yaml(Path(argv[2]))
        if err:
            issues.append(f"{argv[2]}: invalid YAML ({err})")
        else:
            schema, _ = _load_yaml(agent_root / SPEC_FILES["completion"])
            issues.extend(validate_evidence_package(report, schema))
    if not issues:
        print("agent-engineering-check: PASS")
        return 0
    print(f"agent-engineering-check: FAIL - {len(issues)} issue(s):")
    for issue in issues:
        print(f"  {issue}")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
