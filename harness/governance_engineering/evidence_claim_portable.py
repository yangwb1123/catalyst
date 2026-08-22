"""ADR-0072 portable Evidence/Claim structural-validation governance."""

from __future__ import annotations

import hashlib
import subprocess
import sys

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/governance-evidence-claim-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/governance-evidence-claim-v1.json"
SEMANTIC_DECISION_RELATIVE = "docs/adr/0045-canonical-evidence-claim-contract.md"
ADAPTER_RELATIVE = ".agent/skills/evidence-claim-management.md"
PORTABLE_SKILL_RELATIVE = "skills/evidence-claim-management/SKILL.md"
PACKAGE_MANIFEST_RELATIVE = (
    "skills/evidence-claim-management/references/package-manifest.json"
)
PACKAGE_CHECKER_RELATIVE = (
    "skills/evidence-claim-management/scripts/check_package.py"
)
DELIVERY_DECISION_RELATIVE = (
    "docs/adr/ADR-0072-portable-evidence-claim-validation-skill.md"
)
PACKAGE_MANIFEST_SHA256 = (
    "b5d0d15497f47d4310729e7eadf2df506b0c90a1ae982b30b5b453536e98c771"
)
SCHEMA_SHA256 = "b2f8824c95012d94e71b4643756890a7a23f67dc1b9e0e8ecacf979b016864e8"
FIXTURE_SHA256 = "db111600f93e63b3533b1f06b14d7520eb4cbec0e4c6d0e3a6e0fd7e2740824a"
SEMANTIC_DECISION_SHA256 = (
    "a04479075dc60828176cd7e68857dcc4f3fc92bb4ae4b567f2caddd93f478b81"
)
DELIVERY_DECISION_SHA256 = (
    "5ed33ea8d0a7e44e0ff401fad438c0fce0a875914da1187a64cb6cc3452b4929"
)
DELIVERY_DECISION_BODY_SHA256 = (
    "9aa8871ca9024c163ac83677a7c6f289c0579e1b4a92c8535e950b1d34b4c895"
)
DELIVERY_DECISION_SELF_SHA256 = (
    "4aa14c22cb0c49a701764b611af045baaeabdb4af6a3144a75423fecd076e741"
)

RESULT = "STRUCTURALLY_VALID (shadow; no truth or authority attestation)"
PORTABLE_VALIDATION = {
    "record_api_version": "forgeos.governance/v1",
    "delivery": "shipped_pure_contract_and_closed_portable_skill",
    "mode": "authority_free_exact_record_set_structural_validation",
    "semantic_authority": "ADR-0045_only",
    "input": {
        "source": "caller_supplied_exact_canonical_record_set_bytes",
        "explicit_eof_required": True,
        "min_records": 1,
        "max_records": 256,
        "max_input_bytes": 1_048_576,
        "overflow_detection_bytes": 1,
        "fixture_envelope_is_adapter_input": False,
    },
    "result": {
        "success_exit": 0,
        "success_stdout_marker": RESULT,
        "success_stdout_framing": "marker_plus_one_lf",
        "success_stderr": "empty",
        "record_output": "none",
        "rejection_exit": 1,
        "usage_exit": 2,
    },
    "portable_package": {
        "source_distributed": True,
        "closed_manifest_required": True,
        "validator_argv": ["python3", "-I", "-B",
                           "skills/evidence-claim-management/scripts/validate.py"],
        "validator_arguments": "zero",
        "validator_input": "exact_canonical_stdin_through_explicit_eof",
        "checker_argv": ["python3", "-I", "-B", PACKAGE_CHECKER_RELATIVE],
        "checker_package_root_argument": "zero_or_one",
        "vendored_python_parity": "exact_ADR_0045_reference_implementation",
        "fixture_envelope_is_adapter_input": False,
        "authors_defaults_repairs_sorts_inserts_digests_or_seals": False,
        "repository_environment_network_clock_provider_model_database_subprocess_reads": False,
        "journal_semantic_view_or_knowledge_update_proposal_access": False,
        "copies_catalyst_go_or_rust_runtime": False,
        "installs_host_skill": False,
        "python_isolation_boundary": (
            "excludes_script_current_directory_pythonpath_and_user_site_only"
        ),
        "system_site_stdlib_interpreter_startup_host_publisher": (
            "not_disabled_authenticated_or_isolated"
        ),
        "package_integrity_nofollow_unavailable_result": "exit_1_fail_closed",
        "check_to_use_atomicity": "not_provided",
    },
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "evidence_claim_portable_skill": PORTABLE_SKILL_RELATIVE,
    "evidence_claim_package_manifest": PACKAGE_MANIFEST_RELATIVE,
    "evidence_claim_validation_skill_decision": DELIVERY_DECISION_RELATIVE,
}
REFERENCE_IMPLEMENTATION = {
    "ref": "skills/evidence-claim-management",
    "projection": (
        "source_distributed_closed_validation_only_stdin_adapter_without_"
        "authorship_persistence_or_authority"
    ),
}
NON_CAPABILITY = (
    "Portable Evidence Claim validation accepts only already-authored caller-"
    "supplied exact bytes and performs structural validation only; it observes "
    "or authors nothing, repairs no records, emits no record bytes, reads no "
    "ambient repository environment network clock provider model database "
    "subprocess journal semantic view or proposal source, persists nothing, "
    "authenticates no provenance identity host publisher or interpreter, installs "
    "no host Skill, provides no atomic check-to-use binding, and grants no truth "
    "instruction completion Grant PDP Approval permission routing transition "
    "execution or effect authority"
)
DETECTOR = {
    "argv": ["python3", "-I", "-B", PACKAGE_CHECKER_RELATIVE],
    "positive": "test_valid_closed_package",
    "negative": "test_missing_descriptor_primitives_fail_closed",
}
EXPECTED_SCOPE = {
    "shipped_kinds": ["EvidenceRecord", "KnowledgeClaim", "ContextPackage"],
    "shipped_contract_only_kinds": [
        "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
        "TransitionReceipt",
    ],
    "shipped_runtime_profiles": [
        "authenticated_bootstrap_repo_read_grant_issuance_v1",
        "authenticated_bootstrap_repo_read_execution_v1",
    ],
    "candidate_runtime_profiles": [],
    "shipped_projections": ["CognitiveAtom", "GovernanceSemanticView",
                            "GraphSnapshot"],
    "shipped_adapters": ["artifact_provenance_to_evidence",
                         "command_observation_to_evidence",
                         "evolve_repo_locator_to_evidence"],
    "shipped_producers": ["local_gate_command_observation_producer",
                          "local_evolve_repo_locator_observation_producer",
                          "local_go_package_dependency_graph_observation_producer",
                          "local_project_source_snapshot_producer"],
    "shipped_projectors": ["graph_snapshot", "graph_snapshot_test_source"],
    "shipped_evaluators": ["local_go_package_impact_prescan", "graph_snapshot",
                           "graph_snapshot_test_source",
                           "architecture_decision_record_v2", "capability_registry",
                           "planning_capability_ownership", "project_source_snapshot"],
    "staged_producers": [],
    "planned_kinds": [],
    "persistence": (
        "local_append_only_exact_evidence_claim_journal_with_rebuildable_semantic_view"
    ),
    "authority_attestation": "available_only_in_shipped_runtime_profiles",
    "trusted_instruction_lane": "unavailable",
    "production_effects": "forbidden",
}
PORTABLE_SKILL_MARKERS = [
    "python3 -I -B scripts/check_package.py",
    "python3 -I -B scripts/validate.py",
    "explicit EOF",
    "non-isolated Python startup",
    "Treat package checking and validation as separate observations",
    "truth, authority, completion, persistence, and effect attestations false",
]
ADAPTER_MARKERS = [
    "ADR-0072 portable structural-validation branch",
    "already-authored exact canonical record-set bytes",
    "explicit EOF",
    "STRUCTURALLY_VALID (shadow; no truth or authority attestation)",
    "不安装 host Skill",
]
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Portable Evidence Claim validation"],
    ".agent/ARCHITECTURE.md": ["Portable Evidence Claim Validation Skill"],
    ".agent/ROADMAP.md": ["Wave 4–B3 `evidence-claim-management`"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 119", "evidence-claim-management"],
    ".agent/DECISIONS.md": ["D44 Portable Evidence Claim Validation Skill"],
    ".agent/engineering/README.md": ["ADR-0072 adds"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0072 Portable Evidence Claim Validation Skill"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0072 Portable Evidence Claim Validation Skill"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "`evidence-claim-management` narrow package slice"],
}
VENDOR_MODULES = (
    "__init__.py", "codec.py", "constants.py", "fixture.py",
    "record_set.py", "semantics.py", "shape.py",
)


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("evidence_claim_portable_validation") != PORTABLE_VALIDATION:
        issues.append(f"{path}: portable Evidence/Claim validation contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: portable delivery must not expand runtime scope")
    refs = _mapping(data.get("canonical_refs"))
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    pins = _mapping(data.get("contract_pins"))
    if pins.get("evidence_claim_package_manifest_sha256") != PACKAGE_MANIFEST_SHA256:
        issues.append(f"{path}: portable Evidence/Claim manifest pin drifted")
    implementations = _mapping(data.get("reference_implementations"))
    if implementations.get("evidence_claim_portable_skill") != REFERENCE_IMPLEMENTATION:
        issues.append(f"{path}: portable Evidence/Claim reference implementation drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: portable Evidence/Claim non-capability drifted")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get(
        "governance.evidence_claim_portable_package"
    )
    if not isinstance(detector, dict):
        return ["portable Evidence/Claim package detector is missing"]
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    issues = []
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("portable Evidence/Claim detector argv drifted")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("portable Evidence/Claim detector must remain shadow")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"portable Evidence/Claim {polarity} detector test drifted")
    validator = "skills/evidence-claim-management/scripts/validate.py"
    for candidate in detectors.values():
        argv = _mapping(candidate.get("implementation")).get("argv") or []
        if validator in argv:
            issues.append("portable Evidence/Claim validator cannot be a detector")
    return issues


def package_issues(repo_root):
    manifest = repo_root / PACKAGE_MANIFEST_RELATIVE
    try:
        raw = read_bounded_file(manifest, label=PACKAGE_MANIFEST_RELATIVE)
    except (OSError, ContractError) as error:
        return [f"{manifest}: cannot validate package manifest: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != PACKAGE_MANIFEST_SHA256:
        issues.append(f"{manifest}: package manifest pin drifted")
    checker = repo_root / PACKAGE_CHECKER_RELATIVE
    try:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(checker),
             str(repo_root / "skills/evidence-claim-management")],
            cwd=repo_root, capture_output=True, check=False, timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return issues + [f"{checker}: package checker failed: {error}"]
    if result.returncode:
        detail = result.stderr.decode("utf-8", "replace").strip()
        issues.append(f"{checker}: package checker rejected: {detail}")
    return issues


def compatibility_issues(repo_root):
    expected = {
        SCHEMA_RELATIVE: SCHEMA_SHA256,
        FIXTURE_RELATIVE: FIXTURE_SHA256,
        SEMANTIC_DECISION_RELATIVE: SEMANTIC_DECISION_SHA256,
    }
    issues = []
    for relative, digest in expected.items():
        path = repo_root / relative
        try:
            raw = read_bounded_file(path, label=relative)
        except (OSError, ContractError) as error:
            issues.append(f"{path}: cannot validate compatibility pin: {error}")
            continue
        if hashlib.sha256(raw).hexdigest() != digest:
            issues.append(f"{path}: ADR-0045 compatibility pin drifted")
    package_fixture = repo_root / (
        "skills/evidence-claim-management/references/fixtures/"
        "governance-evidence-claim-v1.json"
    )
    try:
        if package_fixture.read_bytes() != (repo_root / FIXTURE_RELATIVE).read_bytes():
            issues.append(f"{package_fixture}: package fixture is not exact")
    except OSError as error:
        issues.append(f"{package_fixture}: cannot compare package fixture: {error}")
    return issues


def vendor_issues(repo_root):
    source = repo_root / "harness/governance_contract"
    vendor = (repo_root / "skills/evidence-claim-management/scripts/_vendor/"
              "governance_contract")
    issues = []
    for name in VENDOR_MODULES:
        try:
            if (source / name).read_bytes() != (vendor / name).read_bytes():
                issues.append(f"{vendor / name}: vendored ADR-0045 module drifted")
        except OSError as error:
            issues.append(f"{vendor / name}: cannot compare vendored module: {error}")
    return issues


def skill_issues(repo_root):
    issues = []
    for relative, markers in {
            PORTABLE_SKILL_RELATIVE: PORTABLE_SKILL_MARKERS,
            ADAPTER_RELATIVE: ADAPTER_MARKERS,
    }.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate Skill markers: {error}")
            continue
        issues.extend(f"{path}: missing portable marker {marker!r}"
                      for marker in markers if marker not in text)
    return issues


def wiring_issues(agent_root):
    from engineering_check_support import load_yaml
    activation, activation_error = load_yaml(agent_root / "engineering/activation.yml")
    routes, routes_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, disciplines_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if activation_error or routes_error or disciplines_error:
        return ["portable Evidence/Claim activation/route/discipline unreadable"]
    extension = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, expected in CANONICAL_REFS.items()
              if extension.get(field) != expected]
    all_refs = {item.get("ref") for route in routes.get("routes") or []
                for item in route.get("include") or []}
    if PORTABLE_SKILL_RELATIVE in all_refs:
        issues.append("portable Evidence/Claim Skill cannot be routed")
    if ADAPTER_RELATIVE not in all_refs:
        issues.append("repository Evidence/Claim adapter route is missing")
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    required = {"tool": "skills/evidence-claim-management",
                "contract": PORTABLE_SKILL_RELATIVE}
    for discipline, asset in required.items():
        if asset not in (by_id.get(discipline, {}).get("assets") or []):
            issues.append(f"portable Evidence/Claim {discipline} asset is missing")
    for discipline in ("context", "memory", "knowledge"):
        assets = by_id.get(discipline, {}).get("assets") or []
        if PORTABLE_SKILL_RELATIVE in assets or "skills/evidence-claim-management" in assets:
            issues.append(f"portable Evidence/Claim cannot enter {discipline} assets")
    return issues


def documentation_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate documentation: {error}")
            continue
        issues.extend(f"{path}: missing delivery marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(roadmap, label=str(roadmap)).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate roadmap: {error}"]
    if "- [ ] 按 `implementation_wave` 逐 package 实现 Skill" not in text:
        issues.append(f"{roadmap}: 38-package parent must remain unchecked")
    if "  - [x] `evidence-claim-management` 窄切片" not in text:
        issues.append(f"{roadmap}: Evidence/Claim nested item must be checked")
    return issues


def adr_issues(repo_root):
    path = repo_root / DELIVERY_DECISION_RELATIVE
    try:
        raw = read_bounded_file(path, label=DELIVERY_DECISION_RELATIVE)
        metadata = validate_document_file(path)
        text = raw.decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: ADR-0072 validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != DELIVERY_DECISION_SHA256:
        issues.append(f"{path}: ADR-0072 physical pin drifted")
    expected = {"status": "proposed", "body_sha256": DELIVERY_DECISION_BODY_SHA256,
                "self_sha256": DELIVERY_DECISION_SELF_SHA256}
    issues.extend(f"{path}: ADR-0072 {field} drifted"
                  for field, value in expected.items() if metadata.get(field) != value)
    normalized = " ".join(text.split())
    for marker in ("registry v27", "explicit EOF", "source-only scaffold",
                   "deliberately absent from authenticated context routes",
                   "does not install a host Skill"):
        if marker not in normalized:
            issues.append(f"{path}: missing ADR-0072 marker {marker!r}")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(detector_issues(agent_root))
    issues.extend(package_issues(repo_root))
    issues.extend(compatibility_issues(repo_root))
    issues.extend(vendor_issues(repo_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    return issues
