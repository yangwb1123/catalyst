"""ADR-0076 portable lexical ImpactPreScan delivery governance."""

from __future__ import annotations

import hashlib
import subprocess
import sys

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file
from governance_engineering.evidence_claim_portable import EXPECTED_SCOPE


SCHEMA = "docs/contracts/local-go-package-impact-prescan-v1.schema.json"
GOLDEN = "docs/contracts/fixtures/local-go-package-impact-prescan-v1.json"
GRAPH_DECISION = (
    "docs/adr/0053-local-go-package-dependency-graph-observation-producer-v1.md"
)
SEMANTIC_DECISION = "docs/adr/0062-local-go-package-impact-prescan-v1.md"
ADAPTER = ".agent/skills/change-impact-cost-risk.md"
SKILL = "skills/change-impact-cost-risk/SKILL.md"
MANIFEST = "skills/change-impact-cost-risk/references/package-manifest.json"
CHECKER = "skills/change-impact-cost-risk/scripts/check_package.py"
PROJECTOR = (
    "skills/change-impact-cost-risk/scripts/"
    "project_local_go_package_impact_prescan.py"
)
DECISION = (
    "docs/adr/ADR-0076-portable-change-impact-cost-risk-"
    "lexical-prescan-skill.md"
)
MANIFEST_SHA256 = (
    "d46202beacc000c6fbdc14afb1c5996476af90d9c0e8927da6f1bf56bf354ad5"
)
COMPATIBILITY_PINS = {
    SCHEMA: "a4592c63a938c090ccc4d6c8187bba8f37909ef6c2d2253fd06f656623c2bb25",
    GOLDEN: "bc364e387705651d307a3ff18137b857a3fad2c518685a358bba169a835a68d9",
    GRAPH_DECISION: (
        "4bd8ed3e14478e3d41c0ecf8d04f50d65e5e14c5362a96b9be1207f6f90fbe99"
    ),
    SEMANTIC_DECISION: (
        "9e4e2cc3b99d78fb26c5b55e23079a38463060667f175b7da2d82950029cb678"
    ),
}
DECISION_SHA256 = "d7df301a4236be84e866a05c54089e79507db13ffba08ab85f955d27c3dc8b01"
DECISION_BODY_SHA256 = "c1097bc6db2f88058f7b4d2af1aeacee0400b035545e01bed0499199525880a5"
DECISION_SELF_SHA256 = "63aa497ce38b8d1182d128cd4227eb45690f9c01cd7c7dbae7c328028418398e"

RESULT = (
    "LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY (exact ADR-0053 lexical reverse "
    "dependency closure; system impact unknown; no selected-build, truth, "
    "authority, completion, persistence, execution, or effect attestation)"
)
REQUEST_FIELDS = [
    "api_version", "canonicalization", "changed_paths",
    "graph_observation_base64url", "graph_observation_sha256",
    "request_sha256", "run_id",
]
PORTABLE_PROJECTION = {
    "delivery": "shipped_existing_lexical_prescan_and_closed_portable_skill",
    "mode": "source_distributed_existing_ADR_0062_exact_request_projection",
    "semantic_authority": "ADR-0062_only",
    "input": {
        "source": "caller_supplied_exact_canonical_request_bytes",
        "api_version": (
            "forgeos.governance.local-go-package-impact-prescan-request/v1"
        ),
        "exact_fields": REQUEST_FIELDS,
        "explicit_eof_required": True,
        "max_input_bytes": 25_165_824,
        "overflow_detection_bytes": 1,
        "raw_graph_fixture_envelope_wrapper_union_dispatch_or_mode": "forbidden",
    },
    "operation": {
        "adapter_argv": ["python3", "-I", "-B", PROJECTOR],
        "output_api_version": (
            "forgeos.governance.local-go-package-impact-prescan/v1"
        ),
        "result_marker": RESULT,
    },
    "process": {
        "arguments": "zero",
        "stdin": "exact_existing_request_bytes_through_explicit_eof",
        "embedded_request_must_reencode_to_exact_stdin": True,
        "max_output_bytes": 50_331_648,
        "success_exit": 0,
        "rejection_exit": 1,
        "usage_exit": 2,
        "success_stderr": "empty",
        "success_stdout_framing": "exact_canonical_existing_envelope_plus_one_lf",
        "pre_output_failure_stdout": "empty",
        "output_failure_may_be_partial_or_indeterminate": True,
        "success_requires_exit_zero_and_exact_bytes": True,
    },
    "portable_package": {
        "source_distributed": True,
        "closed_manifest_required": True,
        "checker_argv": ["python3", "-I", "-B", CHECKER],
        "checker_package_root_argument": "zero_or_one",
        "schema_and_golden_are_exact_source_copies": True,
        "semantic_leaf_parity": (
            "exact_15_named_runtime_semantic_leaves_required_by_ADR_0062"
        ),
        "lean_initializer_count": 4,
        "lean_initializers_are_not_full_source_tree_parity": True,
        "live_repository_or_graph_capture": False,
        "adds_evaluator_producer_runtime_profile_or_route": False,
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
    "authority": {
        "caller_graph_project_repository_or_publisher": "not_authenticated",
        "lexical_package_closure": (
            "exact_only_within_supplied_ADR_0053_observation"
        ),
        "system_impact": "unknown",
        "full_change_impact_cost_risk_materiality_or_safety": "unavailable",
        "selected_build_compile_test_runtime_or_cross_surface": "unavailable",
    },
    "attests": [],
    "persistence": "none",
}
CANONICAL_REFS = {
    "change_impact_cost_risk_skill": ADAPTER,
    "change_impact_cost_risk_portable_skill": SKILL,
    "change_impact_cost_risk_package_manifest": MANIFEST,
    "change_impact_cost_risk_portable_decision": DECISION,
}
REFERENCE_IMPLEMENTATION = {
    "ref": "skills/change-impact-cost-risk",
    "projection": (
        "source_distributed_closed_existing_ADR_0062_lexical_prescan_projector_"
        "without_live_capture_full_impact_cost_risk_runtime_route_or_authority"
    ),
}
NON_CAPABILITY = (
    "Portable Change Impact Cost Risk only distributes the unchanged ADR-0062 "
    "pure lexical ImpactPreScan projector over one caller-supplied exact request; "
    "it accepts no raw or parsed graph fixture envelope wrapper union dispatcher "
    "or mode, invokes no ADR-0053 producer or live repository build test runtime "
    "or cross-surface capture, authenticates no caller graph project repository "
    "host publisher or interpreter, keeps system impact unknown, never promotes "
    "lexical complete_within_observation or zero dependents to system completeness "
    "safe no-impact low-Cost or low-Risk, provides no final ChangeImpactReport Cost "
    "Risk materiality gate or AssessmentReceipt, installs no host Skill, persists "
    "and executes nothing, provides no atomic check-to-use binding, and attests no "
    "truth identity ownership evidence claim authority approval acceptance "
    "compliance completion permission transition execution or effect"
)
LEAF_MODULES = (
    "governance_contract/codec.py", "governance_contract/constants.py",
    "local_command_observation_producer/codec.py",
    "local_command_observation_producer/constants.py",
    "local_command_observation_producer/profiles.py",
    "go_package_dependency_graph_observation_producer/codec.py",
    "go_package_dependency_graph_observation_producer/constants.py",
    "go_package_dependency_graph_observation_producer/graph_contract.py",
    "go_package_dependency_graph_observation_producer/profiles.py",
    "go_package_dependency_graph_observation_producer/semantics.py",
    "local_go_package_impact_prescan_contract/codec.py",
    "local_go_package_impact_prescan_contract/constants.py",
    "local_go_package_impact_prescan_contract/derive.py",
    "local_go_package_impact_prescan_contract/graph.py",
    "local_go_package_impact_prescan_contract/profiles.py",
)
SKILL_MARKERS = (
    "python3 -I -B scripts/check_package.py",
    "python3 -I -B scripts/project_local_go_package_impact_prescan.py",
    "zero arguments", "explicitly", "system_impact_status", "Cost", "Risk",
)
ADAPTER_MARKERS = (
    "python3 -I -B skills/change-impact-cost-risk/scripts/check_package.py",
    "project_local_go_package_impact_prescan.py < REQUEST.json",
    "zero arguments", "explicit EOF", "system_impact_status", "不安装 host Skill",
)
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Portable Change Impact Cost Risk"],
    ".agent/ARCHITECTURE.md": ["ADR-0076", "Registry v31"],
    ".agent/ROADMAP.md": ["change-impact-cost-risk", "ADR 0076"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 123", "change-impact-cost-risk"],
    ".agent/DECISIONS.md": ["D48 Portable Change Impact Cost Risk"],
    ".agent/engineering/README.md": ["ADR-0076 adds"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0076 Portable Change Impact Cost Risk"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0076 Portable Change Impact Cost Risk"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "`change-impact-cost-risk` narrow package slice"],
}


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("change_impact_cost_risk_portable_projection") != PORTABLE_PROJECTION:
        issues.append(f"{path}: portable Change Impact Cost Risk contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: portable delivery must not expand runtime scope")
    refs = _mapping(data.get("canonical_refs"))
    issues.extend(f"{path}: canonical_refs.{field} drifted"
                  for field, value in CANONICAL_REFS.items()
                  if refs.get(field) != value)
    pins = _mapping(data.get("contract_pins"))
    if pins.get("change_impact_cost_risk_package_manifest_sha256") != MANIFEST_SHA256:
        issues.append(f"{path}: portable Change Impact Cost Risk manifest pin drifted")
    implementations = _mapping(data.get("reference_implementations"))
    expected = implementations.get("change_impact_cost_risk_portable_skill")
    if expected != REFERENCE_IMPLEMENTATION:
        issues.append(f"{path}: portable Change Impact Cost Risk implementation drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: portable Change Impact Cost Risk non-capability drifted")
    return issues


def package_issues(repo_root):
    manifest = repo_root / MANIFEST
    try:
        raw = read_bounded_file(manifest, label=MANIFEST)
    except (OSError, ContractError) as error:
        return [f"{manifest}: cannot validate package manifest: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != MANIFEST_SHA256:
        issues.append(f"{manifest}: package manifest pin drifted")
    try:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(repo_root / CHECKER),
             str(repo_root / "skills/change-impact-cost-risk")],
            cwd=repo_root, capture_output=True, check=False, timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return issues + [f"{repo_root / CHECKER}: package checker failed: {error}"]
    if result.returncode:
        issues.append(f"{repo_root / CHECKER}: package checker rejected")
    return issues


def compatibility_issues(repo_root):
    issues = []
    for relative, digest in COMPATIBILITY_PINS.items():
        try:
            raw = read_bounded_file(repo_root / relative, label=relative)
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: cannot validate compatibility pin: {error}")
            continue
        if hashlib.sha256(raw).hexdigest() != digest:
            issues.append(f"{relative}: ADR-0053/0062 compatibility pin drifted")
    pairs = (
        ("references/local-go-package-impact-prescan-v1.schema.json", SCHEMA),
        ("references/fixtures/local-go-package-impact-prescan-v1.json", GOLDEN),
    )
    for target, source in pairs:
        packaged = repo_root / "skills/change-impact-cost-risk" / target
        try:
            if packaged.read_bytes() != (repo_root / source).read_bytes():
                issues.append(f"{packaged}: packaged semantic artifact is not exact")
        except OSError as error:
            issues.append(f"{packaged}: cannot compare semantic artifact: {error}")
    return issues


def vendor_issues(repo_root):
    source = repo_root / "harness"
    vendor = repo_root / "skills/change-impact-cost-risk/scripts/_vendor"
    issues = []
    for relative in LEAF_MODULES:
        try:
            if (source / relative).read_bytes() != (vendor / relative).read_bytes():
                issues.append(f"{vendor / relative}: vendored semantic leaf drifted")
        except OSError as error:
            issues.append(f"{vendor / relative}: cannot compare semantic leaf: {error}")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get("governance.change_impact_cost_risk_portable_package")
    if not isinstance(detector, dict):
        return ["portable Change Impact Cost Risk package detector is missing"]
    issues = []
    argv = _mapping(detector.get("implementation")).get("argv")
    if argv != ["python3", "-I", "-B", CHECKER]:
        issues.append("portable Change Impact Cost Risk detector argv drifted")
    invocation = _mapping(detector.get("invocation"))
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("portable Change Impact Cost Risk detector must remain shadow")
    tests = _mapping(detector.get("tests"))
    if _mapping(tests.get("positive")).get("contains") != "test_valid_closed_package":
        issues.append("portable Change Impact Cost Risk positive test drifted")
    negative = _mapping(tests.get("negative")).get("contains")
    if negative != "test_missing_descriptor_primitives_fail_closed":
        issues.append("portable Change Impact Cost Risk negative test drifted")
    for candidate in detectors.values():
        command = " ".join(_mapping(candidate.get("implementation")).get("argv") or [])
        if PROJECTOR in command:
            issues.append("portable ImpactPreScan projector cannot be a detector")
    return issues


def skill_issues(repo_root):
    issues = []
    for relative, markers in ((SKILL, SKILL_MARKERS), (ADAPTER, ADAPTER_MARKERS)):
        try:
            text = read_bounded_file(repo_root / relative, label=relative).decode()
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: cannot validate Skill markers: {error}")
            continue
        issues.extend(f"{relative}: missing portable marker {marker!r}"
                      for marker in markers if marker not in text)
    return issues


def wiring_issues(agent_root):
    from engineering_check_support import load_yaml
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if a_error or r_error or d_error:
        return ["portable Change Impact Cost Risk wiring is unreadable"]
    refs = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, value in CANONICAL_REFS.items() if refs.get(field) != value]
    route_map = {route.get("id"): route for route in routes.get("routes") or []}
    all_refs = {item.get("ref") for route in route_map.values()
                for item in route.get("include") or []}
    if SKILL in all_refs:
        issues.append("portable Change Impact Cost Risk Skill cannot be routed")
    for route_id in ("governance", "architecture-boundary"):
        included = {item.get("ref") for item in
                    _mapping(route_map.get(route_id)).get("include") or []}
        if ADAPTER not in included:
            issues.append(f"repository Change Impact adapter missing from {route_id}")
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    required = (("tool", "skills/change-impact-cost-risk"),
                ("contract", ADAPTER), ("contract", SKILL))
    for discipline, asset in required:
        if asset not in (by_id.get(discipline, {}).get("assets") or []):
            issues.append(f"portable Change Impact Cost Risk {discipline} asset missing")
    for discipline in ("context", "memory", "knowledge"):
        assets = by_id.get(discipline, {}).get("assets") or []
        if SKILL in assets or "skills/change-impact-cost-risk" in assets:
            issues.append(f"portable Change Impact Cost Risk cannot enter {discipline}")
    return issues


def documentation_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        try:
            text = read_bounded_file(repo_root / relative, label=relative).decode()
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: cannot validate documentation: {error}")
            continue
        issues.extend(f"{relative}: missing delivery marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(roadmap, label=str(roadmap)).decode()
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate roadmap: {error}"]
    required = (
        "- [ ] 按 `implementation_wave` 逐 package 实现 Skill",
        "  - [x] `change-impact-cost-risk` 窄切片",
        "其余 31 个 package items 保持开放",
        "- [ ] 从模块/import/call、API/event schema、DB migration/schema、test、deployment、ADR/owner 建确定性 extractor",
        "- [ ] 记录 extractor coverage、unresolved edge、staleness",
    )
    issues.extend(f"{roadmap}: missing open/closed marker {marker!r}"
                  for marker in required if marker not in text)
    return issues


def adr_issues(repo_root):
    path = repo_root / DECISION
    try:
        raw = read_bounded_file(path, label=DECISION)
        metadata = validate_document_file(path)
        normalized = " ".join(raw.decode().split())
    except (OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: ADR-0076 validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != DECISION_SHA256:
        issues.append(f"{path}: ADR-0076 physical pin drifted")
    expected = {"status": "proposed", "body_sha256": DECISION_BODY_SHA256,
                "self_sha256": DECISION_SELF_SHA256}
    issues.extend(f"{path}: ADR-0076 {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    for marker in ("Registry v31", "one zero-argument exact-request projector",
                   "system impact remains UNKNOWN", "no raw graph or wrapper",
                   "deliberately absent from authenticated context routes",
                   "source-only fresh and legacy scaffold"):
        if marker not in normalized:
            issues.append(f"{path}: missing ADR-0076 marker {marker!r}")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(package_issues(repo_root))
    issues.extend(compatibility_issues(repo_root))
    issues.extend(vendor_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    return issues
