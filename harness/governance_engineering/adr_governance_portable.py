"""ADR-0074 portable Proposed ADR v2 validation governance."""

from __future__ import annotations

import hashlib
import subprocess
import sys

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file
from governance_engineering.evidence_claim_portable import EXPECTED_SCOPE


SCHEMA = "docs/contracts/architecture-decision-record-v2.schema.json"
GOLDEN = "docs/contracts/fixtures/ADR-9001-proposed-boundary.md"
SEMANTIC_DECISION = "docs/adr/0067-proposed-only-adr-v2-frontmatter.md"
ADAPTER = ".agent/skills/adr-governance.md"
SKILL = "skills/adr-governance/SKILL.md"
MANIFEST = "skills/adr-governance/references/package-manifest.json"
CHECKER = "skills/adr-governance/scripts/check_package.py"
VALIDATOR = "skills/adr-governance/scripts/validate_declared_proposed_adr.py"
DECISION = (
    "docs/adr/ADR-0074-portable-adr-governance-proposed-document-"
    "validation-skill.md"
)
MANIFEST_SHA256 = "c1f84e909414878eec6ed62e6605ce7c26758f1940fb1a4660ecef7dcb56fab7"
SCHEMA_SHA256 = (
    "ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b"
)
GOLDEN_SHA256 = (
    "b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194"
)
SEMANTIC_DECISION_SHA256 = (
    "78c7d484cfb0e448c4c896440d4ea272a8e32a60f947539a3ad739baaeead71e"
)
DECISION_SHA256 = (
    "21d452845cf0f2889fcc5fa22f450cc4a40d5fb694f5b1f202d4b3cfd79f2eb2"
)
DECISION_BODY_SHA256 = (
    "a18646f93391a1413d690853a35e5a2ca6a17eb498dcf970696e3606074fb875"
)
DECISION_SELF_SHA256 = (
    "15c996fc2286a011a1b99f1d859b506cd6658b0f0e40afbaf97af767dcfb7d65"
)

SUCCESS = (
    "STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document "
    "bytes only; no identity, ownership, approver, evidence, claim, graph, "
    "acceptance, compliance, persistence, transition, execution, or effect "
    "attestation)"
)
PORTABLE_VALIDATION = {
    "delivery": "shipped_pure_contract_and_closed_portable_skill",
    "mode": "validation_only_caller_supplied_basename_and_exact_document_bytes",
    "semantic_authority": "ADR-0067_only",
    "input": {
        "source": "caller_supplied_lexical_basename_and_exact_document_bytes",
        "basename_argument": "exactly_one",
        "basename_semantics": "lexical_label_only_not_physical_identity_or_proof",
        "stdin": "exact_document_bytes_through_explicit_eof",
        "explicit_eof_required": True,
        "max_document_bytes": 262_144,
        "overflow_detection_bytes": 1,
        "request_envelope": "none_preserves_ADR_0067_document_wire",
    },
    "process": {
        "validator_argv": ["python3", "-I", "-B", VALIDATOR,
                           "ADR-NNNN-slug.md"],
        "success_exit": 0, "success_stdout_marker": SUCCESS,
        "success_stdout_framing": "marker_plus_one_lf",
        "success_stderr": "empty", "rejection_exit": 1, "usage_exit": 2,
        "pre_output_failure_stdout": "empty",
        "output_failure_may_be_partial_or_indeterminate": True,
        "success_requires_exit_zero_and_exact_bytes": True,
    },
    "portable_package": {
        "source_distributed": True, "closed_manifest_required": True,
        "checker_argv": ["python3", "-I", "-B", CHECKER],
        "checker_package_root_argument": "zero_or_one",
        "vendored_python_parity": "exact_ADR_0067_reference_implementation",
        "repository_workspace_environment_network_clock_identity_approval_claim_evidence_graph_policy_database_provider_model_subprocess_lifecycle_reads": False,
        "authors_repairs_normalizes_sorts_reseals_accepts_or_supersedes": False,
        "copies_catalyst_go_writes_adr_runtime": False,
        "installs_host_skill": False,
        "python_isolation_boundary":
            "excludes_script_current_directory_pythonpath_and_user_site_only",
        "system_site_stdlib_interpreter_startup_host_publisher":
            "not_disabled_authenticated_or_isolated",
        "package_integrity_nofollow_unavailable_result": "exit_1_fail_closed",
        "check_to_use_atomicity": "not_provided",
    },
    "authority": {
        "physical_filename_or_repository_identity": "not_attested",
        "owner_approver_claim_evidence_or_graph": "not_authenticated_or_resolved",
        "acceptance_compliance_or_lifecycle": "unavailable",
    },
    "attests": [], "persistence": "none",
}
CANONICAL_REFS = {
    "adr_governance_portable_skill": SKILL,
    "adr_governance_package_manifest": MANIFEST,
    "adr_governance_portable_decision": DECISION,
}
REFERENCE_IMPLEMENTATION = {
    "ref": "skills/adr-governance",
    "projection": (
        "source_distributed_closed_proposed_document_validator_without_"
        "authoring_acceptance_lifecycle_or_authority"
    ),
}
NON_CAPABILITY = (
    "Portable ADR Governance validates only caller-supplied exact Proposed ADR "
    "v2 bytes against one caller-supplied lexical basename; the label proves no "
    "physical file or repository identity, and the package authors repairs "
    "normalizes reseals accepts supersedes persists or executes nothing, reads "
    "no ambient repository workspace environment clock identity ApprovalRecord "
    "Claim Evidence graph policy database network provider model or lifecycle "
    "state, installs no host Skill, provides no atomic check-to-use binding, and "
    "attests no identity ownership approver truth acceptance compliance "
    "immutability transition execution completion or effect authority"
)
SKILL_MARKERS = (
    "python3 -I -B scripts/check_package.py",
    "scripts/validate_declared_proposed_adr.py ADR-NNNN-slug.md",
    "caller-provided lexical label", "stdin without explicit EOF",
    "without both `-I` and `-B`", "Do not normalize Markdown",
    "Do not read a repository", "Do not treat the Schema alone",
)
ADAPTER_MARKERS = (
    "ADR-0074 portable Proposed-document validation branch",
    "从 repository root 执行以下 exact argv",
    "python3 -I -B skills/adr-governance/scripts/check_package.py",
    "python3 -I -B skills/adr-governance/scripts/"
    "validate_declared_proposed_adr.py ADR-NNNN-slug.md",
    "caller-supplied lexical basename", "explicit EOF", "不安装 host Skill",
)
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Portable ADR Governance validation"],
    ".agent/ARCHITECTURE.md": ["ADR-0074", "Registry v29"],
    ".agent/ROADMAP.md": ["Wave 4–B5 `adr-governance`"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 121", "adr-governance"],
    ".agent/DECISIONS.md": ["D46 Portable ADR Governance"],
    ".agent/engineering/README.md": ["ADR-0074 adds", "exactly one basename"],
    "docs/design/ai-engineering-os/README.md": ["ADR-0074 Portable ADR Governance"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0074 Portable ADR Governance"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "`adr-governance` narrow package slice"],
}
ADR_VENDOR = ("__init__.py", "codec.py", "constants.py", "document.py",
              "fixture.py", "shape.py")
GOVERNANCE_VENDOR = ("__init__.py", "codec.py", "constants.py", "fixture.py",
                     "record_set.py", "semantics.py", "shape.py")


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("adr_governance_portable_proposed_document_validation") != PORTABLE_VALIDATION:
        issues.append(f"{path}: portable ADR Governance contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: portable delivery must not expand runtime scope")
    refs = _mapping(data.get("canonical_refs"))
    issues.extend(f"{path}: canonical_refs.{field} drifted"
                  for field, value in CANONICAL_REFS.items()
                  if refs.get(field) != value)
    pins = _mapping(data.get("contract_pins"))
    if pins.get("adr_governance_package_manifest_sha256") != MANIFEST_SHA256:
        issues.append(f"{path}: portable ADR Governance manifest pin drifted")
    implementations = _mapping(data.get("reference_implementations"))
    if implementations.get("adr_governance_portable_skill") != REFERENCE_IMPLEMENTATION:
        issues.append(f"{path}: portable ADR Governance implementation drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: portable ADR Governance non-capability drifted")
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
             str(repo_root / "skills/adr-governance")], cwd=repo_root,
            capture_output=True, check=False, timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return issues + [f"{repo_root / CHECKER}: package checker failed: {error}"]
    if result.returncode:
        issues.append(f"{repo_root / CHECKER}: package checker rejected")
    return issues


def compatibility_issues(repo_root):
    expected = {SCHEMA: SCHEMA_SHA256, GOLDEN: GOLDEN_SHA256,
                SEMANTIC_DECISION: SEMANTIC_DECISION_SHA256}
    issues = []
    for relative, digest in expected.items():
        try:
            raw = read_bounded_file(repo_root / relative, label=relative)
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: cannot validate compatibility pin: {error}")
            continue
        if hashlib.sha256(raw).hexdigest() != digest:
            issues.append(f"{relative}: semantic compatibility pin drifted")
    pairs = (("references/architecture-decision-record-v2.schema.json", SCHEMA),
             ("references/fixtures/ADR-9001-proposed-boundary.md", GOLDEN))
    for target, source in pairs:
        package = repo_root / "skills/adr-governance" / target
        try:
            if package.read_bytes() != (repo_root / source).read_bytes():
                issues.append(f"{package}: packaged contract artifact is not exact")
        except OSError as error:
            issues.append(f"{package}: cannot compare artifact: {error}")
    return issues


def vendor_issues(repo_root):
    issues = []
    pairs = (("architecture_decision_record_v2", ADR_VENDOR),
             ("governance_contract", GOVERNANCE_VENDOR))
    for package, names in pairs:
        source = repo_root / "harness" / package
        vendor = repo_root / "skills/adr-governance/scripts/_vendor" / package
        for name in names:
            try:
                if (source / name).read_bytes() != (vendor / name).read_bytes():
                    issues.append(f"{vendor / name}: vendored contract drifted")
            except OSError as error:
                issues.append(f"{vendor / name}: cannot compare module: {error}")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get("governance.adr_governance_portable_package")
    if not isinstance(detector, dict):
        return ["portable ADR Governance package detector is missing"]
    argv = _mapping(detector.get("implementation")).get("argv")
    issues = []
    if argv != ["python3", "-I", "-B", CHECKER]:
        issues.append("portable ADR Governance detector argv drifted")
    invocation = _mapping(detector.get("invocation"))
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("portable ADR Governance detector must remain shadow")
    for candidate in detectors.values():
        command = " ".join(_mapping(candidate.get("implementation")).get("argv") or [])
        if "validate_declared_proposed_adr.py" in command:
            issues.append("portable ADR validator cannot be a detector")
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
    activation, error = load_yaml(agent_root / "engineering/activation.yml")
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if error or route_error or discipline_error:
        return ["portable ADR Governance wiring is unreadable"]
    extension = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, value in CANONICAL_REFS.items()
              if extension.get(field) != value]
    all_refs = {item.get("ref") for route in routes.get("routes") or []
                for item in route.get("include") or []}
    if SKILL in all_refs:
        issues.append("portable ADR Governance Skill cannot be routed")
    if ADAPTER not in all_refs:
        issues.append("repository ADR Governance adapter route is missing")
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    for discipline, asset in (("tool", "skills/adr-governance"),
                              ("contract", SKILL)):
        if asset not in (by_id.get(discipline, {}).get("assets") or []):
            issues.append(f"portable ADR Governance {discipline} asset missing")
    for discipline in ("context", "memory"):
        assets = by_id.get(discipline, {}).get("assets") or []
        if SKILL in assets or "skills/adr-governance" in assets:
            issues.append(f"portable ADR Governance cannot enter {discipline} assets")
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
        "  - [x] `adr-governance` 窄切片",
        "- [ ] 实现 Accepted ADR immutable + supersede 状态机和 Architecture Compliance",
        "- [ ] 合并 `.agent/DECISIONS` 与 ADR 的查询视图",
        "- [x] 设计旧 memory/ADR 的只读导入",
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
        return [f"{path}: ADR-0074 validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != DECISION_SHA256:
        issues.append(f"{path}: ADR-0074 physical pin drifted")
    expected = {"status": "proposed", "body_sha256": DECISION_BODY_SHA256,
                "self_sha256": DECISION_SELF_SHA256}
    issues.extend(f"{path}: ADR-0074 {field} drifted"
                  for field, value in expected.items()
                  if metadata.get(field) != value)
    for marker in ("Registry v29", "exactly one caller-supplied lexical basename",
                   "explicit EOF", "deliberately absent from authenticated context routes",
                   "install a host Skill", "Source-only fresh and legacy scaffold"):
        if marker not in normalized:
            issues.append(f"{path}: missing ADR-0074 marker {marker!r}")
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
