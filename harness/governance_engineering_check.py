#!/usr/bin/env python3
"""Governance record, journal, projection and source-adapter integration checks."""
import hashlib
import json
import re

from engineering_check_support import (
    header_issues, load_yaml, mapping_issues, unknown_field_issues,
)
from engineering_detector_check import detector_index
from governance_contract import ContractError, read_bounded_file
from governance_contract_check import validate_golden_fixture
from cognitive_atom_contract_check import (
    validate_golden_fixture as validate_cognitive_atom_golden_fixture,
)
from artifact_evidence_adapter_check import (
    validate_golden_fixture as validate_artifact_evidence_golden_fixture,
)
from command_observation_evidence_adapter_check import (
    validate_golden_fixture as validate_command_evidence_golden_fixture,
)
from evolve_repo_locator_evidence_adapter_check import (
    validate_golden_fixture as validate_evolve_locator_golden_fixture,
)
from governance_engineering import (
    ARTIFACT_CANONICAL_REFS,
    ARTIFACT_EVIDENCE_ADAPTER,
    ARTIFACT_EVIDENCE_DETECTOR,
    ARTIFACT_SCHEMA_CANONICALIZATION,
    ARTIFACT_SCHEMA_LIMITS,
    ARTIFACT_SCHEMA_MAPPING,
    ARTIFACT_SKILL_MARKERS,
    COMMAND_CANONICAL_REFS,
    COMMAND_EVIDENCE_DETECTOR,
    COMMAND_OBSERVATION_EVIDENCE_ADAPTER,
    COMMAND_SCHEMA_CANONICALIZATION,
    COMMAND_SCHEMA_LIMITS,
    COMMAND_SCHEMA_MAPPING,
    COMMAND_SCHEMA_SEMANTIC_VALIDATION,
    COMMAND_SKILL_MARKERS,
    COMMAND_SUCCESS,
    EVOLVE_LOCATOR_CANONICAL_REFS,
    EVOLVE_LOCATOR_EVIDENCE_DETECTOR,
    EVOLVE_LOCATOR_SCHEMA_CANONICALIZATION,
    EVOLVE_LOCATOR_SCHEMA_LIMITS,
    EVOLVE_LOCATOR_SCHEMA_MAPPING,
    EVOLVE_LOCATOR_SCHEMA_SEMANTIC_VALIDATION,
    EVOLVE_LOCATOR_SKILL_MARKERS,
    EVOLVE_LOCATOR_SUCCESS,
    EVOLVE_REPO_LOCATOR_EVIDENCE_ADAPTER,
    artifact_adapter_registry_issues as _artifact_adapter_registry_issues,
    artifact_adapter_schema_issues as _artifact_adapter_schema_issues,
    artifact_detector_issues,
    artifact_skill_marker_issues,
    command_adapter_registry_issues as _command_adapter_registry_issues,
    command_adapter_schema_issues as _command_adapter_schema_issues,
    command_detector_issues,
    command_skill_marker_issues,
    evolve_locator_adapter_registry_issues as _evolve_locator_adapter_registry_issues,
    evolve_locator_adapter_schema_issues as _evolve_locator_adapter_schema_issues,
    evolve_locator_detector_issues,
    evolve_locator_skill_marker_issues,
)


POLICY_RELATIVE = "engineering/governance-contracts.yml"
POLICY_SHA256 = "b9dc9112876374f9c168ecbc2b545b36234e79d5a963a982b031679f20e68c6f"
POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "scope", "canonicalization", "identity",
    "claim_states", "shadow_admissibility", "evidence_semantics", "journal",
    "cognitive_atom_projection", "artifact_evidence_adapter",
    "command_observation_evidence_adapter", "legacy",
    "evolve_repo_locator_evidence_adapter",
    "canonical_refs", "contract_pins", "reference_implementations",
    "non_capabilities",
}
PIN_TARGETS = {
    "schema_sha256": "docs/contracts/governance-evidence-claim-v1.schema.json",
    "journal_schema_sha256": "docs/contracts/governance-record-journal-v1.schema.json",
    "golden_fixture_sha256":
        "docs/contracts/fixtures/governance-evidence-claim-v1.json",
    "cognitive_atom_schema_sha256":
        "docs/contracts/cognitive-atom-projection-v1.schema.json",
    "cognitive_atom_golden_fixture_sha256":
        "docs/contracts/fixtures/cognitive-atom-projection-v1.json",
    "artifact_evidence_adapter_schema_sha256":
        "docs/contracts/artifact-evidence-adapter-v1.schema.json",
    "artifact_evidence_adapter_golden_fixture_sha256":
        "docs/contracts/fixtures/artifact-evidence-adapter-v1.json",
    "command_observation_evidence_adapter_schema_sha256":
        "docs/contracts/command-observation-evidence-adapter-v1.schema.json",
    "command_observation_evidence_adapter_golden_fixture_sha256":
        "docs/contracts/fixtures/command-observation-evidence-adapter-v1.json",
    "evolve_repo_locator_evidence_adapter_schema_sha256":
        "docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json",
    "evolve_repo_locator_evidence_adapter_golden_fixture_sha256":
        "docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json",
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
COGNITIVE_ATOM_PROJECTION = {
    "api_version": "forgeos.aadm.cognitive-atom/v1",
    "mode": "deterministic_claim_to_atom_shadow",
    "source_kinds": ["KnowledgeClaim"],
    "projectable_claim_types": [
        "assumption", "constraint", "decision", "fact", "hypothesis", "inference",
        "unknown",
    ],
    "ignored_claim_types": ["lesson", "proposal"],
    "constants": {
        "authority_ref": None, "hardness": "none", "instruction_allowed": False,
        "projection_mode": "shadow",
    },
    "source_record_set": {
        "exact_canonical_required": True, "closed_world_required": True,
        "max_records": 256, "max_bytes": 1048576,
    },
    "closure_edges": [
        "contradicting_evidence", "derived_from_claim", "supporting_evidence",
        "supersedes",
    ],
    "identity": {
        "atom_id_domain": "forgeos.aadm.cognitive-atom-id.v1\0",
        "atom_digest_domain": "forgeos.aadm.cognitive-atom.v1\0",
        "atom_set_digest_domain": "forgeos.aadm.cognitive-atom-set.v1\0",
        "source_closure_digest_domain": "forgeos.governance.record-set.v1\0",
        "length_framing": "unsigned_u64_big_endian",
    },
    "limits": {
        "max_atom_bytes": 131072, "max_atom_set_bytes": 1048576, "max_atoms": 256,
    },
    "positive_result": "PROJECTED_SHADOW", "attests": [], "persistence": "none",
}
COGNITIVE_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "atom_digest_domain": "forgeos.aadm.cognitive-atom.v1\0",
    "atom_id_domain": "forgeos.aadm.cognitive-atom-id.v1\0",
    "atom_set_digest_domain": "forgeos.aadm.cognitive-atom-set.v1\0",
    "source_closure_digest_domain": "forgeos.governance.record-set.v1\0",
    "self_digest_rule": "integrity.canonical_sha256 is empty while hashing",
}
COGNITIVE_SCHEMA_LIMITS = {
    "max_atom_bytes": 131072, "max_atom_set_bytes": 1048576, "max_atoms": 256,
    "max_source_records": 256, "max_source_record_set_bytes": 1048576,
    "max_depth": 16, "max_object_fields": 64, "max_array_items": 256,
    "max_string_bytes": 16384, "integer_domain": "signed_int64",
}
COGNITIVE_SCHEMA_PROJECTION = {
    "mode": "shadow", "source_kind": "KnowledgeClaim",
    "projectable_claim_types": [
        "assumption", "constraint", "decision", "fact", "hypothesis", "inference",
        "unknown",
    ],
    "ignored_claim_types": ["lesson", "proposal"],
    "hardness": "none", "authority_ref": None, "instruction_allowed": False,
    "persistence": "none", "positive_result": "PROJECTED_SHADOW", "attestations": [],
}
COGNITIVE_CANONICAL_REFS = {
    "cognitive_atom_schema": "docs/contracts/cognitive-atom-projection-v1.schema.json",
    "cognitive_atom_golden_fixture":
        "docs/contracts/fixtures/cognitive-atom-projection-v1.json",
    "cognitive_atom_checker": "harness/cognitive_atom_contract_check.py",
    "cognitive_atom_decision": "docs/adr/0047-shadow-cognitive-atom-projection-v1.md",
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
    issues.extend(artifact_skill_marker_issues(text, path))
    issues.extend(command_skill_marker_issues(text, path))
    issues.extend(evolve_locator_skill_marker_issues(text, path))
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


def _cognitive_registry_issues(data, path):
    projection = data.get("cognitive_atom_projection") if isinstance(data, dict) else None
    issues = []
    if projection != COGNITIVE_ATOM_PROJECTION:
        issues.append(f"{path}: cognitive_atom_projection contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in COGNITIVE_CANONICAL_REFS.items():
            if refs.get(field) != expected:
                issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    return issues


def _cognitive_schema_issues(repo_root):
    relative = PIN_TARGETS["cognitive_atom_schema_sha256"]
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate CognitiveAtom Schema contract: {error}"]
    expected = {
        "x-forgeos-canonicalization": COGNITIVE_SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": COGNITIVE_SCHEMA_LIMITS,
        "x-forgeos-projection": COGNITIVE_SCHEMA_PROJECTION,
    }
    return [f"{path}: {field} drifted" for field, value in expected.items()
            if schema.get(field) != value]


def _detector_issues(agent_root):
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    evidence = detectors.get("governance.evidence_claim_contract")
    evidence_argv = [
        "python3", "harness/governance_contract_check.py",
        "repo_root", "governance_record_set",
    ]
    atom = detectors.get("aadm.cognitive_atom_projection")
    atom_argv = [
        "python3", "harness/cognitive_atom_contract_check.py", "repo_root", "task_id",
        "governance_record_set", "cognitive_atom_set",
    ]
    issues = []
    if not isinstance(evidence, dict):
        issues.append("governance Evidence/Claim detector is missing")
    else:
        implementation = evidence.get("implementation")
        if not isinstance(implementation, dict) or implementation.get("argv") != evidence_argv:
            issues.append(
                "governance Evidence/Claim detector requires exact record-set arguments"
            )
    if not isinstance(atom, dict):
        issues.append("CognitiveAtom projection detector is missing")
    else:
        implementation = atom.get("implementation")
        if not isinstance(implementation, dict) or implementation.get("argv") != atom_argv:
            issues.append("CognitiveAtom projection detector requires exact projection arguments")
    issues.extend(artifact_detector_issues(detectors))
    issues.extend(command_detector_issues(detectors))
    issues.extend(evolve_locator_detector_issues(detectors))
    return issues


def _source_adapter_issues(data, path, repo_root):
    issues = []
    for registry_check, schema_check in (
        (_artifact_adapter_registry_issues, _artifact_adapter_schema_issues),
        (_command_adapter_registry_issues, _command_adapter_schema_issues),
        (_evolve_locator_adapter_registry_issues,
         _evolve_locator_adapter_schema_issues),
    ):
        issues.extend(registry_check(data, path))
        issues.extend(schema_check(repo_root))
    return issues


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
        "runtime_binding": (
            "cross_language_codec_local_journal_atom_projection_"
            "artifact_command_evolve_locator_adapters_shadow"
        ),
        "version": 7, "completion_authority": "forge_accept",
    }
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{path}: {field} must remain the canonical v7 value")
    repo_root = agent_root.parent
    issues.extend(_journal_registry_issues(data, path))
    issues.extend(_journal_schema_issues(repo_root))
    issues.extend(_cognitive_registry_issues(data, path))
    issues.extend(_cognitive_schema_issues(repo_root))
    issues.extend(_source_adapter_issues(data, path, repo_root))
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
    issues.extend(validate_cognitive_atom_golden_fixture(repo_root))
    issues.extend(validate_artifact_evidence_golden_fixture(repo_root))
    issues.extend(validate_command_evidence_golden_fixture(repo_root))
    issues.extend(validate_evolve_locator_golden_fixture(repo_root))
    return issues
