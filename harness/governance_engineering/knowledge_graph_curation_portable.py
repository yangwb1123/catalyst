"""ADR-0075 portable Knowledge Graph Curation projection governance."""

from __future__ import annotations

import hashlib
import subprocess
import sys

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file
from governance_engineering.evidence_claim_portable import EXPECTED_SCOPE


MODULE_SCHEMA = "docs/contracts/graph-snapshot-v1.schema.json"
TEST_SCHEMA = "docs/contracts/graph-snapshot-go-test-source-v1.schema.json"
MODULE_GOLDEN = "docs/contracts/fixtures/graph-snapshot-v1.json"
TEST_GOLDEN = "docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json"
MODULE_DECISION = "docs/adr/0065-authority-free-graph-snapshot-v1-contract.md"
TEST_DECISION = "docs/adr/0066-local-go-lexical-test-source-graph-snapshot.md"
ADAPTER = ".agent/skills/knowledge-graph-curation.md"
SKILL = "skills/knowledge-graph-curation/SKILL.md"
MANIFEST = "skills/knowledge-graph-curation/references/package-manifest.json"
CHECKER = "skills/knowledge-graph-curation/scripts/check_package.py"
MODULE_PROJECTOR = (
    "skills/knowledge-graph-curation/scripts/project_module_package_snapshot.py"
)
TEST_PROJECTOR = (
    "skills/knowledge-graph-curation/scripts/project_go_test_source_snapshot.py"
)
DECISION = (
    "docs/adr/ADR-0075-portable-knowledge-graph-curation-"
    "partial-projectors-skill.md"
)
MANIFEST_SHA256 = "5f3f240f7ebaaacc9b06945367e9f6bef9cfeb03df4dcae6d2fbbd7f36ae393f"
COMPATIBILITY_PINS = {
    MODULE_SCHEMA: "9dcaf66cff5b6d10338af6d295c75b2a5925604238cc276f80b68d3783d72bff",
    TEST_SCHEMA: "bfada8bb3d183061f2758bfc3645b56dc038b35d38c3c0b779a8ef32afcd17be",
    MODULE_GOLDEN: "8ce8418e840c97ef28ed77dfd5112c4c4b7d7ae8d843b714674e102d6322b03e",
    TEST_GOLDEN: "df1b25a933ffa2503f750e2209c9866bfe126e273b28c1181bb211ce48cae5e9",
    MODULE_DECISION: "c8e6cc3cb67d847d9b01b45d8043d132168d64eb42ff453b75864a46679ab11a",
    TEST_DECISION: "5c3c59521e27f19d202639bf14d953e6fbe76559f7159de3ccaf7790510d141d",
}
DECISION_SHA256 = (
    "81c0690f1f305dbf714a7ee0afd8dae0f2226d95a015fe019f087c3429761f91"
)
DECISION_BODY_SHA256 = (
    "0feec210c2bf8b9162b35b149ce74df58011784385534a72763d79d89a12f3d6"
)
DECISION_SELF_SHA256 = (
    "3f04faa5e28661adec1ed2cb87984c5d670a792a57cf9663836f6a00ff5d7819"
)

MODULE_RESULT = (
    "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical "
    "module/package subgraph only; coverage partial and system/freshness unknown; "
    "no selected-build, cross-surface completeness, truth, authority, completion, "
    "persistence, execution, impact, or effect attestation)"
)
TEST_RESULT = (
    "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical "
    "module/package/test-source subgraph only; test nodes are source sets, not tests "
    "or outcomes; coverage partial and system/freshness unknown; no selected-build, "
    "cross-surface completeness, truth, authority, completion, persistence, "
    "execution, verification, impact, or effect attestation)"
)
PORTABLE_PROJECTION = {
    "delivery": "shipped_existing_projectors_and_closed_portable_skill",
    "mode": "source_distributed_two_existing_partial_projection_wires",
    "semantic_authorities": ["ADR-0065_only", "ADR-0066_only"],
    "input": {
        "source": "caller_supplied_exact_canonical_request_bytes",
        "explicit_eof_required": True, "max_input_bytes": 25_165_824,
        "overflow_detection_bytes": 1,
        "wrapper_union_dispatch_or_profile_argument": "forbidden",
    },
    "operations": {
        "go_test_source": {
            "adapter_argv": ["python3", "-I", "-B", TEST_PROJECTOR],
            "request_api_version": (
                "forgeos.governance.local-go-test-source-graph-snapshot-"
                "projection-request/v1"
            ),
            "projector_profile_id": (
                "adr-0053-selected-go-module-lexical-package-test-source-"
                "partial-graph-snapshot-v1"
            ),
            "result_marker": TEST_RESULT,
        },
        "module_package": {
            "adapter_argv": ["python3", "-I", "-B", MODULE_PROJECTOR],
            "request_api_version": (
                "forgeos.governance.local-go-graph-snapshot-projection-request/v1"
            ),
            "projector_profile_id": (
                "adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1"
            ),
            "result_marker": MODULE_RESULT,
        },
    },
    "process": {
        "arguments": "zero",
        "stdin": "exact_existing_request_bytes_through_explicit_eof",
        "embedded_request_must_reencode_to_exact_stdin": True,
        "max_output_bytes": 100_663_296,
        "success_exit": 0, "rejection_exit": 1, "usage_exit": 2,
        "success_stderr": "empty",
        "success_stdout_framing": "exact_canonical_existing_envelope_plus_one_lf",
        "pre_output_failure_stdout": "empty",
        "output_failure_may_be_partial_or_indeterminate": True,
        "success_requires_exit_zero_and_exact_bytes": True,
    },
    "portable_package": {
        "source_distributed": True, "closed_manifest_required": True,
        "checker_argv": ["python3", "-I", "-B", CHECKER],
        "checker_package_root_argument": "zero_or_one",
        "semantic_leaf_parity": "exact_26_named_ADR_0065_and_ADR_0066_runtime_leaves",
        "lean_initializer_count": 4,
        "lean_initializers_are_not_full_source_tree_parity": True,
        "live_repository_build_test_or_graph_capture": False,
        "adds_projector_evaluator_producer_runtime_profile_or_route": False,
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
        "coverage_and_system_knowledge": "partial_and_unknown",
        "test_source_nodes": "lexical_source_sets_not_tests_outcomes_or_coverage",
        "impact_verification_acceptance_or_compliance": "unavailable",
    },
    "attests": [], "persistence": "none",
}
CANONICAL_REFS = {
    "knowledge_graph_curation_portable_skill": SKILL,
    "knowledge_graph_curation_package_manifest": MANIFEST,
    "knowledge_graph_curation_portable_decision": DECISION,
}
REFERENCE_IMPLEMENTATION = {
    "ref": "skills/knowledge-graph-curation",
    "projection": (
        "source_distributed_closed_two_existing_partial_projectors_without_"
        "live_capture_runtime_route_or_authority"
    ),
}
NON_CAPABILITY = (
    "Portable Knowledge Graph Curation only distributes the two unchanged ADR-0065 "
    "and ADR-0066 pure partial projectors over caller-supplied exact request bytes; "
    "it adds no third projector ABI wrapper union dispatcher profile route evaluator "
    "producer live repository build or test capture, authenticates no caller graph "
    "project repository host publisher or interpreter, keeps coverage partial and "
    "system freshness unknown, treats test nodes only as lexical source sets rather "
    "than tests outcomes verification or coverage, installs no host Skill, persists "
    "and executes nothing, provides no atomic check-to-use binding, and attests no "
    "truth authority completion acceptance compliance impact transition execution "
    "or effect"
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
    "graph_snapshot_contract/codec.py", "graph_snapshot_contract/constants.py",
    "graph_snapshot_contract/coverage.py", "graph_snapshot_contract/derive.py",
    "graph_snapshot_contract/lexical_test_source_constants.py",
    "graph_snapshot_contract/lexical_test_source_coverage.py",
    "graph_snapshot_contract/lexical_test_source_derive.py",
    "graph_snapshot_contract/lexical_test_source_provenance.py",
    "graph_snapshot_contract/lexical_test_source_snapshot.py",
    "graph_snapshot_contract/lexical_test_source_topology.py",
    "graph_snapshot_contract/profiles.py", "graph_snapshot_contract/provenance.py",
    "graph_snapshot_contract/records.py", "graph_snapshot_contract/snapshot.py",
    "graph_snapshot_contract/topology.py", "graph_snapshot_contract/unresolved.py",
)
SKILL_MARKERS = (
    "python3 -I -B scripts/check_package.py",
    "python3 -I -B scripts/project_module_package_snapshot.py",
    "python3 -I -B scripts/project_go_test_source_snapshot.py",
    "zero arguments", "Do not cross-feed", "accept a raw graph observation",
)
ADAPTER_MARKERS = (
    "ADR-0075 portable partial-projector branch",
    f"python3 -I -B {CHECKER}", f"python3 -I -B {MODULE_PROJECTOR}",
    f"python3 -I -B {TEST_PROJECTOR}", "zero-argument", "explicit EOF",
    "不安装 host Skill", "不新增 authenticated context route",
)
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Portable Knowledge Graph Curation"],
    ".agent/ARCHITECTURE.md": ["ADR-0075", "Registry v30"],
    ".agent/ROADMAP.md": ["knowledge-graph-curation", "ADR 0075"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 122", "knowledge-graph-curation"],
    ".agent/DECISIONS.md": ["D47 Portable Knowledge Graph Curation"],
    ".agent/engineering/README.md": ["ADR-0075 adds"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0075 Portable Knowledge Graph Curation"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0075 Portable Knowledge Graph Curation"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "`knowledge-graph-curation` narrow package slice"],
}


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("knowledge_graph_curation_portable_projection") != PORTABLE_PROJECTION:
        issues.append(f"{path}: portable Knowledge Graph Curation contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: portable delivery must not expand runtime scope")
    refs = _mapping(data.get("canonical_refs"))
    issues.extend(f"{path}: canonical_refs.{field} drifted"
                  for field, value in CANONICAL_REFS.items()
                  if refs.get(field) != value)
    pins = _mapping(data.get("contract_pins"))
    if pins.get("knowledge_graph_curation_package_manifest_sha256") != MANIFEST_SHA256:
        issues.append(f"{path}: portable Knowledge Graph Curation manifest pin drifted")
    implementations = _mapping(data.get("reference_implementations"))
    if implementations.get("knowledge_graph_curation_portable_skill") != REFERENCE_IMPLEMENTATION:
        issues.append(f"{path}: portable Knowledge Graph Curation implementation drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: portable Knowledge Graph Curation non-capability drifted")
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
             str(repo_root / "skills/knowledge-graph-curation")],
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
            issues.append(f"{relative}: ADR-0065/0066 compatibility pin drifted")
    pairs = (("references/graph-snapshot-v1.schema.json", MODULE_SCHEMA),
             ("references/graph-snapshot-go-test-source-v1.schema.json", TEST_SCHEMA),
             ("references/fixtures/graph-snapshot-v1.json", MODULE_GOLDEN),
             ("references/fixtures/graph-snapshot-go-test-source-v1.json", TEST_GOLDEN))
    for target, source in pairs:
        packaged = repo_root / "skills/knowledge-graph-curation" / target
        try:
            if packaged.read_bytes() != (repo_root / source).read_bytes():
                issues.append(f"{packaged}: packaged semantic artifact is not exact")
        except OSError as error:
            issues.append(f"{packaged}: cannot compare semantic artifact: {error}")
    return issues


def vendor_issues(repo_root):
    source = repo_root / "harness"
    vendor = repo_root / "skills/knowledge-graph-curation/scripts/_vendor"
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
    detector = detectors.get("governance.knowledge_graph_curation_portable_package")
    if not isinstance(detector, dict):
        return ["portable Knowledge Graph Curation package detector is missing"]
    issues = []
    argv = _mapping(detector.get("implementation")).get("argv")
    if argv != ["python3", "-I", "-B", CHECKER]:
        issues.append("portable Knowledge Graph Curation detector argv drifted")
    invocation = _mapping(detector.get("invocation"))
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("portable Knowledge Graph Curation detector must remain shadow")
    tests = _mapping(detector.get("tests"))
    if _mapping(tests.get("positive")).get("contains") != "test_valid_closed_package":
        issues.append("portable Knowledge Graph Curation positive detector test drifted")
    if _mapping(tests.get("negative")).get("contains") != "test_missing_descriptor_primitives_fail_closed":
        issues.append("portable Knowledge Graph Curation negative detector test drifted")
    for candidate in detectors.values():
        command = " ".join(_mapping(candidate.get("implementation")).get("argv") or [])
        if MODULE_PROJECTOR in command or TEST_PROJECTOR in command:
            issues.append("portable GraphSnapshot projectors cannot be detectors")
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
        return ["portable Knowledge Graph Curation wiring is unreadable"]
    refs = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, value in CANONICAL_REFS.items() if refs.get(field) != value]
    route_refs = {item.get("ref") for route in routes.get("routes") or []
                  for item in route.get("include") or []}
    if SKILL in route_refs:
        issues.append("portable Knowledge Graph Curation Skill cannot be routed")
    if ADAPTER not in route_refs:
        issues.append("repository Knowledge Graph Curation adapter route is missing")
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    for discipline, asset in (("tool", "skills/knowledge-graph-curation"),
                              ("contract", SKILL), ("graph", SKILL)):
        if asset not in (by_id.get(discipline, {}).get("assets") or []):
            issues.append(f"portable Knowledge Graph Curation {discipline} asset missing")
    for discipline in ("context", "memory"):
        assets = by_id.get(discipline, {}).get("assets") or []
        if SKILL in assets or "skills/knowledge-graph-curation" in assets:
            issues.append(f"portable Knowledge Graph Curation cannot enter {discipline} assets")
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
        "  - [x] `knowledge-graph-curation` 窄切片",
        "其余 32 个 package items 保持开放",
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
        return [f"{path}: ADR-0075 validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != DECISION_SHA256:
        issues.append(f"{path}: ADR-0075 physical pin drifted")
    expected = {"status": "proposed", "body_sha256": DECISION_BODY_SHA256,
                "self_sha256": DECISION_SELF_SHA256}
    issues.extend(f"{path}: ADR-0075 {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    for marker in ("Registry v30", "two independent zero-argument projectors",
                   "no wrapper, tagged union or profile dispatcher",
                   "deliberately absent from authenticated context routes",
                   "source-only fresh and legacy scaffold", "coverage remains PARTIAL"):
        if marker not in normalized:
            issues.append(f"{path}: missing ADR-0075 marker {marker!r}")
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
