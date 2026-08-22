"""ADR-0077/0078 Proposed WorkIntent v1 candidate governance."""

from __future__ import annotations

import hashlib
import json
import re

from architecture_decision_record_v2 import (
    ContractError as ADRContractError,
    validate_document_file,
)
from engineering_check_support import load_yaml
from governance_contract import ContractError, read_bounded_file
from work_intent_contract import SUCCESS_MARKER, load_golden

from .evidence_claim_portable import EXPECTED_SCOPE


SCHEMA = "docs/contracts/work-intent-v1.schema.json"
GOLDEN = "docs/contracts/fixtures/work-intent-v1.json"
CHECKER = "harness/work_intent_contract_check.py"
SEMANTIC_DECISION = "docs/adr/ADR-0077-authority-neutral-work-intent-v1-contract.md"
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0078-work-intent-v1-proposed-candidate-governance-and-"
    "source-distribution.md"
)
SCHEMA_SHA256 = "3b02fab59eae8767c86caaa73d0830adcbd92825045b7f27db0c3eca5ee10e01"
GOLDEN_SHA256 = "8e80553677ebf9f6548a15be4c3cb4ccc8aa6825010a20f2e890e91d1cd7ed7b"
RECORD_SHA256 = "2fe0424d30405a8b1d716afc99bbd38d602375f3316fd1c54c472890d520a225"
SEMANTIC_DECISION_SHA256 = (
    "f7bfbe26a4786c42c6d89666d42353fd26414097f2158e223c26f842457b06d5"
)
SEMANTIC_DECISION_BODY_SHA256 = (
    "c39002066f9f593d44dd16b09e0745cb27d2f890e914554ee1e55307e6c91878"
)
SEMANTIC_DECISION_SELF_SHA256 = (
    "4f960415f0d1a83e8898c77f0af5df12843a0ce4f12de378401ceece4d21c048"
)
GOVERNANCE_DECISION_SHA256 = (
    "af03daac138bab353ae81827317e76df241807f87eb5b32fcdd1de8bd535f363"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "4726be6001d0ac4ed40d0f11c7b05ea1a8a2250aa21f4b47395fe5f8538d345b"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "21aeed431e92f42fd44d24fa748be4cf80fd2e4f3ae855fd879ef6d196d82cc1"
)

WORK_INTENT_V1_CANDIDATE_CONTRACT = {
    "api_version": "forgeos.work-intent/v1",
    "kind": "WorkIntent",
    "decision_status": "proposed",
    "delivery": (
        "cross_language_proposed_candidate_semantic_evidence_with_source_only_"
        "python_distribution"
    ),
    "mode": "exact_caller_supplied_structural_validation_only",
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "digest_domain": "forgeos.work-intent.v1\0",
        "golden_record_sha256": RECORD_SHA256,
        "schema_alone_sufficient": False,
    },
    "candidate_implementations": {
        "python": "strict_semantic_core_and_golden_checker_source_distributed",
        "go": "strict_semantic_core_catalyst_repository_only_not_scaffolded",
        "rust": "strict_semantic_core_catalyst_repository_only_not_scaffolded",
        "exact_golden_parity": True,
    },
    "source_distribution": {
        "copies_python_semantic_core_and_checker": True,
        "copies_go": False,
        "copies_rust": False,
        "installs_skill_or_adapter": False,
        "adds_authenticated_route": False,
        "adds_kind_evaluator_producer_or_runtime_profile": False,
    },
    "authority_semantics": {
        "origin_authentication": False,
        "requester_or_owner_authentication": False,
        "reference_resolution": False,
        "freshness_materiality_or_scope_assessment": False,
        "g0_closure": False,
        "semantic_authority": False,
        "run_or_run_journal_existence": False,
        "lifecycle_approval_permission_or_grant": False,
        "routing_or_runtime_consumer": False,
        "persistence_execution_or_effect": False,
    },
    "positive_result": SUCCESS_MARKER,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "work_intent_v1_schema": SCHEMA,
    "work_intent_v1_golden_fixture": GOLDEN,
    "work_intent_v1_checker": CHECKER,
    "work_intent_v1_semantic_decision": SEMANTIC_DECISION,
    "work_intent_v1_candidate_governance_decision": GOVERNANCE_DECISION,
}
REFERENCE_IMPLEMENTATIONS = {
    "work_intent_v1_python": {
        "ref": "harness/work_intent_contract",
        "projection": (
            "source_distributed_strict_proposed_candidate_semantic_core_and_"
            "golden_checker"
        ),
    },
    "work_intent_v1_go": {
        "ref": "forge-core/internal/workintentcontract",
        "projection": (
            "catalyst_repository_only_strict_proposed_candidate_semantic_core_"
            "not_scaffolded"
        ),
    },
    "work_intent_v1_rust": {
        "ref": "forge-runtime/crates/domain/src/work_intent_contract",
        "projection": (
            "catalyst_repository_only_strict_proposed_candidate_semantic_core_"
            "not_scaffolded"
        ),
    },
}
NON_CAPABILITY = (
    "WorkIntent v1 is only Proposed cross-language candidate structural validation "
    "of a caller-supplied lexical declaration; valid or digest parity does not "
    "authenticate origin, requester or owner, resolve references, determine "
    "freshness, materiality or scope, close G0, register, route, produce, evaluate "
    "or consume work, create Run, RunJournal or lifecycle, approve, authorize or "
    "permit, persist, execute or effect, install a Skill, or implement change-"
    "intake-orchestration; Go and Rust remain Catalyst-only and source distribution "
    "copies Python only"
)
DETECTOR = {
    "argv": ["python3", CHECKER, "--golden", "repo_root"],
    "positive": "test_registry_is_v39_scope_neutral_candidate_only",
    "negative": "test_scope_route_authority_and_pin_drift_fail_closed",
}
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["WorkIntent v1 Proposed candidate"],
    ".agent/ARCHITECTURE.md": ["ADR-0078 WorkIntent v1 Proposed candidate governance"],
    ".agent/ROADMAP.md": ["WorkIntent v1 Proposed candidate evidence"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 124", "WorkIntent v1 Proposed candidate"],
    ".agent/DECISIONS.md": ["D49 WorkIntent v1 Proposed candidate governance"],
    ".agent/engineering/README.md": ["ADR-0078 adds"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0078 WorkIntent v1 Proposed Candidate Governance"
    ],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0078 WorkIntent v1 Proposed Candidate Governance"
    ],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "WorkIntent v1 Proposed candidate evidence"
    ],
}
PROMOTION_SENTINEL = "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: WorkIntent candidate requires Registry v39")
    if data.get("work_intent_v1_candidate_contract") != WORK_INTENT_V1_CANDIDATE_CONTRACT:
        issues.append(f"{path}: WorkIntent v1 Proposed candidate contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: WorkIntent candidate must not expand runtime scope")
    for field, expected in CANONICAL_REFS.items():
        if _mapping(data.get("canonical_refs")).get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if _mapping(data.get("reference_implementations")).get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    pins = _mapping(data.get("contract_pins"))
    if pins.get("work_intent_v1_schema_sha256") != SCHEMA_SHA256:
        issues.append(f"{path}: WorkIntent Schema pin drifted")
    if pins.get("work_intent_v1_golden_fixture_sha256") != GOLDEN_SHA256:
        issues.append(f"{path}: WorkIntent golden pin drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: WorkIntent non-capability boundary drifted")
    return issues


def schema_and_fixture_issues(repo_root):
    schema_path = repo_root / SCHEMA
    try:
        schema_raw = read_bounded_file(schema_path, label=SCHEMA, max_bytes=1_048_576)
        schema = json.loads(schema_raw.decode("utf-8"))
        record = load_golden(repo_root)
    except (OSError, ContractError, UnicodeDecodeError,
            json.JSONDecodeError) as error:
        return [f"WorkIntent candidate artifacts cannot be validated: {error}"]
    issues = []
    if hashlib.sha256(schema_raw).hexdigest() != SCHEMA_SHA256:
        issues.append(f"{schema_path}: WorkIntent Schema physical pin drifted")
    authority = _mapping(schema.get("x-forgeos-authority-semantics"))
    for field in ("origin_authentication", "reference_resolution", "g0_closure",
                  "routing", "run_or_run_journal_existence",
                  "lifecycle_transition", "semantic_authority"):
        if authority.get(field) is not False:
            issues.append(f"{schema_path}: authority semantics {field} drifted")
    canonicalization = _mapping(schema.get("x-forgeos-canonicalization"))
    if canonicalization.get("digest_domain_ascii") != "forgeos.work-intent.v1\0":
        issues.append(f"{schema_path}: WorkIntent digest domain drifted")
    if authority.get("positive_result") != SUCCESS_MARKER:
        issues.append(f"{schema_path}: WorkIntent success marker drifted")
    if record.get("work_intent_sha256") != RECORD_SHA256:
        issues.append(f"{repo_root / GOLDEN}: WorkIntent record digest drifted")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get("governance.work_intent_v1_candidate_contract")
    if not isinstance(detector, dict):
        return ["WorkIntent v1 candidate checker-only detector is missing"]
    issues = []
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("WorkIntent v1 candidate detector argv drifted")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("WorkIntent v1 candidate detector must remain shadow")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"WorkIntent v1 {polarity} detector test drifted")
    checker_uses = [item for item in detectors.values() if CHECKER in
                    (_mapping(item.get("implementation")).get("argv") or [])]
    if checker_uses != [detector]:
        issues.append("WorkIntent v1 requires exactly one checker-only detector")
    commands = "\n".join(" ".join(_mapping(item.get("implementation")).get("argv") or [])
                         for item in detectors.values())
    if ("forge-core/internal/workintentcontract" in commands or
            "forge-runtime/crates/domain/src/work_intent_contract" in commands):
        issues.append("WorkIntent Go/Rust semantic cores cannot be detectors")
    return issues


def wiring_issues(agent_root):
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if a_error or r_error or d_error:
        return ["WorkIntent v1 candidate Agent Engineering wiring is unreadable"]
    issues = []
    refs = _mapping(activation.get("canonical_extension_refs"))
    issues.extend(f"activation canonical_extension_refs.{field} drifted"
                  for field, expected in CANONICAL_REFS.items()
                  if refs.get(field) != expected)
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    for required in (SCHEMA, CHECKER):
        if required not in assets:
            issues.append(f"WorkIntent v1 contract asset missing: {required}")
    routed = {item.get("ref") for route in routes.get("routes") or []
              for item in route.get("include") or []}
    for field, reference in CANONICAL_REFS.items():
        if reference in routed:
            issues.append(f"WorkIntent candidate cannot enter a context route: {field}")
    return issues


def documentation_issues(repo_root):
    if not (repo_root / PROMOTION_SENTINEL).is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        try:
            text = read_bounded_file(repo_root / relative, label=relative).decode()
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: cannot validate WorkIntent docs: {error}")
            continue
        issues.extend(f"{relative}: missing candidate marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        roadmap_text = read_bounded_file(roadmap, label=str(roadmap)).decode()
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate WorkIntent roadmap: {error}"]
    required = ("- [x] WorkIntent v1 Proposed candidate evidence",
                "- [ ] 按 `implementation_wave` 逐 package 实现 Skill",
                "parent 与其余 31 个 package items 保持开放", "不关闭 G0")
    issues.extend(f"{roadmap}: missing candidate/open marker {marker!r}"
                  for marker in required if marker not in roadmap_text)
    if re.search(r"^\s*- \[x\].*change-intake-orchestration", roadmap_text, re.MULTILINE):
        issues.append(f"{roadmap}: change-intake-orchestration cannot be checked")
    return issues


def _one_adr_issues(repo_root, relative, physical, body, self_digest, markers):
    path = repo_root / relative
    try:
        raw = read_bounded_file(path, label=relative)
        metadata = validate_document_file(path)
        normalized = " ".join(raw.decode().split())
    except (OSError, ADRContractError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: WorkIntent Proposed ADR validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != physical:
        issues.append(f"{path}: physical pin drifted")
    expected = {"status": "proposed", "body_sha256": body,
                "self_sha256": self_digest}
    issues.extend(f"{path}: {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    issues.extend(f"{path}: missing boundary marker {marker!r}"
                  for marker in markers if marker not in normalized)
    return issues


def adr_issues(repo_root):
    issues = _one_adr_issues(
        repo_root, SEMANTIC_DECISION, SEMANTIC_DECISION_SHA256,
        SEMANTIC_DECISION_BODY_SHA256, SEMANTIC_DECISION_SELF_SHA256,
        ("Python, Go and Rust", "does not adopt a route, registry entry",
         "ADR-0077 remains Proposed"),
    )
    issues.extend(_one_adr_issues(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v32 candidate-only metadata", "checker-only shadow",
         "source-only fresh and legacy scaffold", "remaining 31 items"),
    ))
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(schema_and_fixture_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    return issues
