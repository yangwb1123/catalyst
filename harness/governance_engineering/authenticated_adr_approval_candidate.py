"""ADR-0079/0080 Proposed authenticated ADR approval prerequisite governance."""

from __future__ import annotations

import hashlib
import json

from architecture_decision_record_v2 import (
    ContractError as ADRContractError,
    validate_document_file,
)
from authenticated_adr_approval_contract import (
    ContractError as ApprovalContractError,
    SUCCESS_MARKER,
    load_golden,
)
from engineering_check_support import load_yaml
from governance_contract import ContractError, read_bounded_file

from .evidence_claim_portable import EXPECTED_SCOPE


SCHEMA = "docs/contracts/authenticated-architecture-decision-approval-v1.schema.json"
GOLDEN = (
    "docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json"
)
PROPOSAL = "docs/contracts/fixtures/ADR-9002-authenticated-approval-target.md"
CHECKER = "harness/authenticated_adr_approval_contract_check.py"
SEMANTIC_DECISION = (
    "docs/adr/ADR-0079-authenticated-architecture-decision-approval-v1-"
    "prerequisite.md"
)
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0080-authenticated-architecture-decision-approval-v1-"
    "proposed-candidate-governance-and-source-distribution.md"
)
SCHEMA_SHA256 = "9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198"
GOLDEN_SHA256 = "936b989856ff733e2de848ba9907c10f9f626aa188648fc60372775e44dbc7b5"
PROPOSAL_SHA256 = "6beabf33656998b942036b63c90db99c6a5f9b138cf2e5bd4a5372ec8e1ad1f2"
PROPOSAL_BODY_SHA256 = (
    "9a798ab129919d51d8b2a3c842d281b19c1d1667a180ef8cbd4f690465730a63"
)
PROPOSAL_SELF_SHA256 = (
    "1d2579dafcf152c302e22cfa4d6932e248f1171eef812297f601b60ce7ed208f"
)
SEMANTIC_DECISION_SHA256 = (
    "087eb6f7e669c027802c0f1822c8091d2b2cfb405e72186a430e24dcfd34d194"
)
SEMANTIC_DECISION_BODY_SHA256 = (
    "358c43bf52b6cc2d7026d1b9c8041ab2cb01aa479ac01ddf120ea413d2c20541"
)
SEMANTIC_DECISION_SELF_SHA256 = (
    "f57ea9c1fbc75e0f50fe997683409744dfa4cd001bab9a8189b271526e5b54f9"
)
GOVERNANCE_DECISION_SHA256 = (
    "d91548ab698db4137a7f96bf78070c45303e2201e017a2094c7b6ee48aac6563"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "b7c87f0291cb58dc27098e7a918abfaffa4bd23b1b6290e82b2a5477c4e39b13"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "8db896364d84ccd0f813d81bb816f120802b30d142f582755bac5b105fe292e3"
)

AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE_CONTRACT = {
    "profile_id": "authenticated_architecture_decision_approval_v1",
    "decision_status": "proposed",
    "delivery": (
        "proposed_structural_prerequisite_candidate_with_source_only_python_"
        "distribution"
    ),
    "mode": "exact_caller_supplied_offline_structural_validation_only",
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "proposal_physical_sha256": PROPOSAL_SHA256,
        "proposal_body_sha256": PROPOSAL_BODY_SHA256,
        "proposal_self_sha256": PROPOSAL_SELF_SHA256,
        "schema_alone_sufficient": False,
    },
    "candidate_implementation": {
        "python": "dependency_free_strict_structural_core_and_golden_checker",
        "go_contract_and_authority": (
            "catalyst_repository_only_under_ADR_0081_not_scaffolded"
        ),
        "exact_golden_reconstruction": True,
    },
    "source_distribution": {
        "copies_python_structural_core_and_checker": True,
        "copies_schema_golden_proposal_and_proposed_decisions": True,
        "copies_go_contract_or_authority": False,
        "copies_production_keys_or_state": False,
        "installs_skill_or_adapter": False,
        "adds_authenticated_route": False,
        "adds_kind_evaluator_producer_or_runtime_profile": False,
    },
    "authority_semantics": {
        "applies_to": "source_distributed_python_candidate_only",
        "ed25519_or_sod_proof_verification": False,
        "root_key_principal_proposal_or_publisher_authentication": False,
        "authorization": False,
        "receipt_signing_minting_or_issuance": False,
        "external_root_or_epoch_pin_consumption": False,
        "trusted_time_currentness": False,
        "revocation_currentness": False,
        "cas_durability_or_rollback_resistance": False,
        "adr_acceptance_or_lifecycle_transition": False,
        "semantic_authority": False,
        "g0_closure": False,
        "persistence_execution_or_effect": False,
    },
    "positive_result": SUCCESS_MARKER,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "authenticated_adr_approval_v1_schema": SCHEMA,
    "authenticated_adr_approval_v1_golden_fixture": GOLDEN,
    "authenticated_adr_approval_v1_proposal_fixture": PROPOSAL,
    "authenticated_adr_approval_v1_checker": CHECKER,
    "authenticated_adr_approval_v1_semantic_decision": SEMANTIC_DECISION,
    "authenticated_adr_approval_v1_candidate_governance_decision": (
        GOVERNANCE_DECISION
    ),
    "authenticated_adr_approval_v1_go_authority_decision": (
        "docs/adr/ADR-0081-authenticated-architecture-decision-approval-"
        "authorization-service-v1.md"
    ),
}
REFERENCE_IMPLEMENTATIONS = {
    "authenticated_adr_approval_v1_python": {
        "ref": "harness/authenticated_adr_approval_contract",
        "projection": (
            "source_distributed_dependency_free_strict_proposed_structural_"
            "prerequisite_core_and_golden_checker"
        ),
    },
    "authenticated_adr_approval_v1_go_contract": {
        "ref": "forge-core/internal/authenticatedadrapprovalcontract",
        "projection": (
            "catalyst_repository_only_proposed_exact_structural_contract_"
            "not_scaffolded"
        ),
    },
    "authenticated_adr_approval_v1_go_authority": {
        "ref": "forge-core/internal/authenticatedadrapprovalauthority",
        "projection": (
            "catalyst_repository_only_proposed_external_trust_authenticated_"
            "authorization_and_durable_store_not_scaffolded"
        ),
    },
}
NON_CAPABILITY = (
    "Authenticated Architecture Decision Approval v1 is only a Proposed pure "
    "offline structural prerequisite over caller-supplied proof-shaped bytes; "
    "it does not verify Ed25519 or SoD proofs, authenticate roots, keys, "
    "principals, proposals or publishers, authorize, sign, mint or issue a "
    "receipt, consume an external root or epoch pin, establish trusted time or "
    "current revocation, provide durable CAS or rollback resistance, accept or "
    "transition an ADR, close G0, register a route, kind, evaluator, producer or "
    "runtime profile, persist, execute or effect, install a Skill or adapter, or "
    "implement ADR lifecycle; ADR-0081 Go contract and authority evidence remains "
    "Catalyst-repository-only, while source distribution copies Python only and "
    "no Go contract, authority, production keys or state"
)
DETECTOR = {
    "argv": ["python3", CHECKER, "--golden", "repo_root"],
    "positive": "test_registry_is_v39_scope_neutral_structural_candidate_only",
    "negative": "test_scope_route_authority_and_pin_drift_fail_closed",
}
SCHEMA_AUTHORITY = {
    "delivery": "pure_offline_structural_candidate_only",
    "signature_verification": "not_implemented",
    "external_root_pin": "required_but_not_consumed",
    "trusted_time": "not_consumed",
    "revocation_currentness": "not_evaluated",
    "authorization_decision": "not_attested",
    "acceptance_transition": "not_implemented",
    "cas_and_durability": "not_implemented",
    "permission_attestation": False,
    "persistence_attestation": False,
    "effect_attestation": False,
    "positive_result": SUCCESS_MARKER,
}
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Authenticated ADR approval v1 Proposed structural prerequisite"],
    ".agent/ARCHITECTURE.md": ["ADR-0080 Authenticated ADR approval v1"],
    ".agent/ROADMAP.md": ["Authenticated ADR approval v1 Proposed prerequisite evidence"],
    ".agent/CURRENT_SPRINT.md": ["Authenticated ADR approval v1 Proposed prerequisite"],
    ".agent/DECISIONS.md": ["Authenticated ADR approval v1 Proposed candidate governance"],
    ".agent/engineering/README.md": ["ADR-0080 adds"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0080 Authenticated ADR Approval v1 Proposed Candidate Governance"
    ],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0080 Authenticated ADR Approval v1 Proposed Candidate Governance"
    ],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "Authenticated ADR approval v1 Proposed structural prerequisite evidence"
    ],
}
PROMOTION_SENTINEL = "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: authenticated ADR approval candidate requires Registry v39")
    if data.get("authenticated_adr_approval_v1_candidate_contract") != (
            AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE_CONTRACT):
        issues.append(f"{path}: authenticated ADR approval candidate contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: authenticated ADR approval cannot expand runtime scope")
    for field, expected in CANONICAL_REFS.items():
        if _mapping(data.get("canonical_refs")).get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if _mapping(data.get("reference_implementations")).get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    pins = _mapping(data.get("contract_pins"))
    expected_pins = {
        "authenticated_adr_approval_v1_schema_sha256": SCHEMA_SHA256,
        "authenticated_adr_approval_v1_golden_fixture_sha256": GOLDEN_SHA256,
        "authenticated_adr_approval_v1_proposal_fixture_sha256": PROPOSAL_SHA256,
    }
    issues.extend(f"{path}: contract_pins.{field} drifted"
                  for field, expected in expected_pins.items()
                  if pins.get(field) != expected)
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: authenticated ADR approval non-capability drifted")
    return issues


def artifact_issues(repo_root):
    try:
        schema_raw = read_bounded_file(repo_root / SCHEMA, label=SCHEMA,
                                       max_bytes=1_048_576)
        schema = json.loads(schema_raw.decode("utf-8"))
        golden = load_golden(repo_root)
        proposal_raw = read_bounded_file(repo_root / PROPOSAL, label=PROPOSAL,
                                         max_bytes=262_144)
    except (OSError, ContractError, ApprovalContractError, UnicodeDecodeError,
            json.JSONDecodeError) as error:
        return [f"authenticated ADR approval artifacts cannot be validated: {error}"]
    issues = []
    physical = ((SCHEMA, schema_raw, SCHEMA_SHA256),
                (GOLDEN, (repo_root / GOLDEN).read_bytes(), GOLDEN_SHA256),
                (PROPOSAL, proposal_raw, PROPOSAL_SHA256))
    issues.extend(f"{relative}: physical pin drifted"
                  for relative, raw, expected in physical
                  if hashlib.sha256(raw).hexdigest() != expected)
    if schema.get("x-forgeos-authority-semantics") != SCHEMA_AUTHORITY:
        issues.append(f"{SCHEMA}: authority semantics drifted")
    binding = _mapping(golden.get("proposal_binding"))
    expected_binding = {"physical_sha256": PROPOSAL_SHA256,
                        "body_sha256": PROPOSAL_BODY_SHA256,
                        "self_sha256": PROPOSAL_SELF_SHA256,
                        "status": "proposed"}
    issues.extend(f"{GOLDEN}: proposal binding {field} drifted"
                  for field, expected in expected_binding.items()
                  if binding.get(field) != expected)
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get("governance.authenticated_adr_approval_v1_candidate")
    if not isinstance(detector, dict):
        return ["authenticated ADR approval checker-only detector is missing"]
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    issues = []
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("authenticated ADR approval detector argv drifted")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("authenticated ADR approval detector must remain shadow")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"authenticated ADR approval {polarity} test drifted")
    checker_uses = [item for item in detectors.values() if CHECKER in
                    (_mapping(item.get("implementation")).get("argv") or [])]
    if checker_uses != [detector]:
        issues.append("authenticated ADR approval requires one checker-only detector")
    return issues


def wiring_issues(agent_root):
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if a_error or r_error or d_error:
        return ["authenticated ADR approval Agent Engineering wiring is unreadable"]
    refs = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, expected in CANONICAL_REFS.items()
              if refs.get(field) != expected]
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    for required in (SCHEMA, CHECKER):
        if required not in assets:
            issues.append(f"authenticated ADR approval contract asset missing: {required}")
    routed = {item.get("ref") for route in routes.get("routes") or []
              for item in route.get("include") or []}
    issues.extend(f"authenticated ADR approval cannot enter context route: {field}"
                  for field, reference in CANONICAL_REFS.items()
                  if reference in routed)
    return issues


def documentation_issues(repo_root):
    if not (repo_root / PROMOTION_SENTINEL).is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        try:
            text = read_bounded_file(repo_root / relative, label=relative).decode()
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: cannot validate candidate docs: {error}")
            continue
        issues.extend(f"{relative}: missing Proposed marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        roadmap_text = read_bounded_file(roadmap, label=str(roadmap)).decode()
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate candidate roadmap: {error}"]
    required = ("- [x] Authenticated ADR approval v1 Proposed structural prerequisite evidence",
                "full authenticated approval 与 ADR lifecycle 保持开放", "不关闭 G0")
    issues.extend(f"{roadmap}: missing Proposed/open marker {marker!r}"
                  for marker in required if marker not in roadmap_text)
    return issues


def _one_adr_issues(repo_root, relative, physical, body, self_digest, markers):
    path = repo_root / relative
    try:
        raw = read_bounded_file(path, label=relative)
        metadata = validate_document_file(path)
        normalized = " ".join(raw.decode().split())
    except (OSError, ADRContractError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: authenticated ADR approval Proposed ADR failed: {error}"]
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
        ("does not verify Ed25519", "does not accept", "ADR-0079 remains Proposed"),
    )
    issues.extend(_one_adr_issues(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v33 candidate-only metadata", "checker-only",
         "must not copy a future Go service", "Full authenticated approval"),
    ))
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(artifact_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    return issues
