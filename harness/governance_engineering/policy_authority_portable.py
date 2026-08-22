"""ADR-0073 portable declaration-only Grant/Approval governance."""

from __future__ import annotations

import hashlib
import subprocess
import sys

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file
from governance_engineering.evidence_claim_portable import EXPECTED_SCOPE


GRANT_SCHEMA = "docs/contracts/capability-grant-v1.schema.json"
GRANT_FIXTURE = "docs/contracts/fixtures/capability-grant-v1.json"
GRANT_DECISION = "docs/adr/0056-capability-grant-v1-contract-only.md"
APPROVAL_SCHEMA = "docs/contracts/approval-record-v1.schema.json"
APPROVAL_FIXTURE = "docs/contracts/fixtures/approval-record-v1.json"
APPROVAL_DECISION = "docs/adr/0059-approval-record-v1-contract-only.md"
ADAPTER = ".agent/skills/policy-authority.md"
SKILL = "skills/policy-authority/SKILL.md"
MANIFEST = "skills/policy-authority/references/package-manifest.json"
CHECKER = "skills/policy-authority/scripts/check_package.py"
DECISION = (
    "docs/adr/ADR-0073-portable-policy-authority-declaration-assessment-skill.md"
)
MANIFEST_SHA256 = (
    "feb21737424b0133e8b57f553ff342b51583917f83e1d47b4b83cd6c3a667132"
)
GRANT_SCHEMA_SHA256 = (
    "dd26568ec430ae5e444ae851ba2b58087528a17e84794137268be3860d9c3209"
)
GRANT_FIXTURE_SHA256 = (
    "0261a682bddca2f27976a9cd663350e8cf222685389fecc7ad8ae536083fef35"
)
GRANT_DECISION_SHA256 = (
    "3b3aa0d0b2f456370bdf2b137f2697454d6a5ff0c705d66881180ceeae8ae9f1"
)
APPROVAL_SCHEMA_SHA256 = (
    "bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64"
)
APPROVAL_FIXTURE_SHA256 = (
    "501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978"
)
APPROVAL_DECISION_SHA256 = (
    "155312825d6a706d8d6bc927d590ec8d50d19b06d7b977e6546a2a42c3dc741d"
)
DECISION_SHA256 = (
    "cb1a9adff937e39f3d42b052e19e7e0e1516968da967948508b45dd735bed619"
)
DECISION_BODY_SHA256 = (
    "729fd91714d43244f3ac23f182007289ee4cd21a4abd0bf7fe51253eefadbf86"
)
DECISION_SELF_SHA256 = (
    "a92f4ef3d22ceab5264316863e396182eadc84a9530803a43af3ed723144cecd"
)

GRANT_RESULT = (
    "ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, "
    "approval, revocation, usage, preflight, authorization, permission, "
    "persistence, execution, or effect attestation)"
)
APPROVAL_RESULT = (
    "ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority "
    "authentication, attestation or SoD proof verification, condition or "
    "RiskAcceptance validation, revocation evaluation, policy decision, "
    "effective approval, authorization, permission, persistence, transition, "
    "execution, or effect attestation)"
)
PORTABLE_ASSESSMENT = {
    "delivery": "shipped_pure_contracts_and_closed_portable_skill",
    "mode": "authority_neutral_caller_supplied_declaration_assessment_only",
    "semantic_authorities": ["ADR-0056_only", "ADR-0059_only"],
    "input": {
        "source": "caller_supplied_exact_canonical_request_bytes",
        "explicit_eof_required": True,
        "max_input_bytes": 2_097_152,
        "overflow_detection_bytes": 1,
        "combined_dispatch_envelope": "forbidden",
    },
    "operations": {
        "capability_grant": {
            "request_api_version":
                "forgeos.capability-grant-declared-assessment-request/v1",
            "assessment_api_version":
                "forgeos.capability-grant-declared-assessment/v1",
            "adapter_argv": ["python3", "-I", "-B", (
                "skills/policy-authority/scripts/"
                "assess_declared_capability_grant.py")],
            "result_marker": GRANT_RESULT,
        },
        "approval_record": {
            "request_api_version":
                "forgeos.approval-record-declared-assessment-request/v1",
            "assessment_api_version":
                "forgeos.approval-record-declared-assessment/v1",
            "adapter_argv": ["python3", "-I", "-B", (
                "skills/policy-authority/scripts/"
                "assess_declared_approval_record.py")],
            "result_marker": APPROVAL_RESULT,
        },
    },
    "process": {
        "adapter_arguments": "zero", "success_exit": 0,
        "success_stdout": "exact_canonical_computed_assessment_plus_one_lf",
        "success_stderr": "empty", "rejection_exit": 1, "usage_exit": 2,
        "pre_output_failure_stdout": "empty",
        "output_failure_may_be_partial_or_indeterminate": True,
        "success_requires_exit_zero_and_exact_bytes": True,
    },
    "portable_package": {
        "source_distributed": True, "closed_manifest_required": True,
        "checker_argv": ["python3", "-I", "-B", CHECKER],
        "checker_package_root_argument": "zero_or_one",
        "vendored_python_parity":
            "exact_ADR_0056_and_ADR_0059_reference_implementations",
        "repository_environment_network_clock_identity_policy_approval_store_ledger_reads": False,
        "copies_catalyst_go_or_rust_runtime": False,
        "installs_host_skill": False,
        "python_isolation_boundary":
            "excludes_script_current_directory_pythonpath_and_user_site_only",
        "system_site_stdlib_interpreter_startup_host_publisher":
            "not_disabled_authenticated_or_isolated",
        "package_integrity_nofollow_unavailable_result": "exit_1_fail_closed",
        "check_to_use_atomicity": "not_provided",
    },
    "authority": {
        "policy_decision": "none", "authorization_decision": "none",
        "unavailable_states": "not_evaluated",
        "assessment_defined_attestation_fields": False,
        "execution": "unavailable_and_unattested",
    },
    "attests": [], "persistence": "none",
}
CANONICAL_REFS = {
    "policy_authority_portable_skill": SKILL,
    "policy_authority_package_manifest": MANIFEST,
    "policy_authority_portable_decision": DECISION,
}
REFERENCE_IMPLEMENTATION = {
    "ref": "skills/policy-authority",
    "projection": (
        "source_distributed_closed_dual_pure_stdin_adapters_without_live_"
        "policy_approval_or_authority"
    ),
}
NON_CAPABILITY = (
    "Portable Policy Authority assessment compares only caller-supplied exact "
    "CapabilityGrant or ApprovalRecord declarations; it issues approves "
    "activates revokes reserves consumes persists or executes nothing, "
    "authenticates no issuer approver proof identity key policy host publisher "
    "or interpreter, invokes no ADR-0057 or ADR-0058 runtime Governance Kernel "
    "PDP PEP approval store revocation registry usage ledger or effect, installs "
    "no host Skill, provides no atomic check-to-use binding, and grants no "
    "effective approval authorization permission routing transition completion "
    "or production authority"
)
SKILL_MARKERS = (
    "python3 -I -B scripts/check_package.py",
    "scripts/assess_declared_capability_grant.py",
    "scripts/assess_declared_approval_record.py",
    "stdin without explicit EOF", "without both `-I` and `-B`",
    "every assessment-defined attestation field false",
    "execution unavailable and unattested",
)
ADAPTER_MARKERS = (
    "ADR-0073 portable declaration-assessment branch",
    "从 repository root 执行以下 exact argv",
    "python3 -I -B skills/policy-authority/scripts/check_package.py",
    "python3 -I -B skills/policy-authority/scripts/"
    "assess_declared_capability_grant.py",
    "python3 -I -B skills/policy-authority/scripts/"
    "assess_declared_approval_record.py",
    "explicit EOF",
    "不安装 host Skill",
)
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Portable Policy Authority assessment"],
    ".agent/ARCHITECTURE.md": ["Portable Policy Authority Declaration Assessment"],
    ".agent/ROADMAP.md": ["Wave 4–B4 `policy-authority`"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 120", "policy-authority"],
    ".agent/DECISIONS.md": ["D45 Portable Policy Authority Declaration Assessment"],
    ".agent/engineering/README.md": [
        "ADR-0073 adds", "Source-only fresh and legacy scaffold now copies",
        "installs no host Skill or runtime and grants no authority"],
    "docs/design/ai-engineering-os/README.md": ["ADR-0073 Portable Policy Authority"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0073 Portable Policy Authority"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "`policy-authority` narrow package slice"],
}
GRANT_VENDOR = (
    "__init__.py", "assessment.py", "canonical.py", "constants.py",
    "contract.py", "grant.py", "scope.py", "shape.py", "vocabulary.py",
)
APPROVAL_VENDOR = (
    "__init__.py", "assessment.py", "canonical.py", "constants.py",
    "contract.py", "fixture.py", "record.py", "shape.py",
)


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("policy_authority_portable_declaration_assessment") != PORTABLE_ASSESSMENT:
        issues.append(f"{path}: portable Policy Authority contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: portable delivery must not expand runtime scope")
    refs = _mapping(data.get("canonical_refs"))
    issues.extend(f"{path}: canonical_refs.{field} drifted"
                  for field, value in CANONICAL_REFS.items()
                  if refs.get(field) != value)
    pins = _mapping(data.get("contract_pins"))
    if pins.get("policy_authority_package_manifest_sha256") != MANIFEST_SHA256:
        issues.append(f"{path}: portable Policy Authority manifest pin drifted")
    implementations = _mapping(data.get("reference_implementations"))
    if implementations.get("policy_authority_portable_skill") != REFERENCE_IMPLEMENTATION:
        issues.append(f"{path}: portable Policy Authority implementation drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: portable Policy Authority non-capability drifted")
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
    checker = repo_root / CHECKER
    try:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(checker),
             str(repo_root / "skills/policy-authority")],
            cwd=repo_root, capture_output=True, check=False, timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return issues + [f"{checker}: package checker failed: {error}"]
    if result.returncode:
        issues.append(f"{checker}: package checker rejected")
    return issues


def compatibility_issues(repo_root):
    expected = {
        GRANT_SCHEMA: GRANT_SCHEMA_SHA256, GRANT_FIXTURE: GRANT_FIXTURE_SHA256,
        GRANT_DECISION: GRANT_DECISION_SHA256,
        APPROVAL_SCHEMA: APPROVAL_SCHEMA_SHA256,
        APPROVAL_FIXTURE: APPROVAL_FIXTURE_SHA256,
        APPROVAL_DECISION: APPROVAL_DECISION_SHA256,
    }
    issues = []
    for relative, digest in expected.items():
        try:
            raw = read_bounded_file(repo_root / relative, label=relative)
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: cannot validate compatibility pin: {error}")
            continue
        if hashlib.sha256(raw).hexdigest() != digest:
            issues.append(f"{relative}: semantic compatibility pin drifted")
    for target, source in (("capability-grant-v1.json", GRANT_FIXTURE),
                           ("approval-record-v1.json", APPROVAL_FIXTURE)):
        package = repo_root / "skills/policy-authority/references/fixtures" / target
        try:
            if package.read_bytes() != (repo_root / source).read_bytes():
                issues.append(f"{package}: package fixture is not exact")
        except OSError as error:
            issues.append(f"{package}: cannot compare fixture: {error}")
    return issues


def vendor_issues(repo_root):
    issues = []
    pairs = (("capability_grant_contract", GRANT_VENDOR),
             ("approval_record_contract", APPROVAL_VENDOR))
    for package, names in pairs:
        source = repo_root / "harness" / package
        vendor = repo_root / "skills/policy-authority/scripts/_vendor" / package
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
    detector = detectors.get("governance.policy_authority_portable_package")
    if not isinstance(detector, dict):
        return ["portable Policy Authority package detector is missing"]
    issues = []
    argv = _mapping(detector.get("implementation")).get("argv")
    if argv != ["python3", "-I", "-B", CHECKER]:
        issues.append("portable Policy Authority detector argv drifted")
    invocation = _mapping(detector.get("invocation"))
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("portable Policy Authority detector must remain shadow")
    forbidden = ("assess_declared_capability_grant.py",
                 "assess_declared_approval_record.py")
    for candidate in detectors.values():
        command = " ".join(_mapping(candidate.get("implementation")).get("argv") or [])
        if any(name in command for name in forbidden):
            issues.append("portable assessment adapters cannot be detectors")
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
    if error:
        return ["portable Policy Authority activation unreadable"]
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if route_error or discipline_error:
        return ["portable Policy Authority route/discipline unreadable"]
    extension = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, value in CANONICAL_REFS.items()
              if extension.get(field) != value]
    all_refs = {item.get("ref") for route in routes.get("routes") or []
                for item in route.get("include") or []}
    if SKILL in all_refs:
        issues.append("portable Policy Authority Skill cannot be routed")
    if ADAPTER not in all_refs:
        issues.append("repository Policy Authority adapter route is missing")
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    for discipline, asset in (("tool", "skills/policy-authority"),
                              ("contract", SKILL)):
        if asset not in (by_id.get(discipline, {}).get("assets") or []):
            issues.append(f"portable Policy Authority {discipline} asset missing")
    for discipline in ("context", "memory", "knowledge"):
        assets = by_id.get(discipline, {}).get("assets") or []
        if SKILL in assets or "skills/policy-authority" in assets:
            issues.append(f"portable Policy Authority cannot enter {discipline} assets")
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
        if (relative == ".agent/engineering/README.md" and
                "Source-only scaffold remains pending" in text):
            issues.append(f"{relative}: stale pending scaffold claim is forbidden")
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(roadmap, label=str(roadmap)).decode()
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate roadmap: {error}"]
    if "- [ ] 按 `implementation_wave` 逐 package 实现 Skill" not in text:
        issues.append(f"{roadmap}: 38-package parent must remain unchecked")
    if "  - [x] `policy-authority` 窄切片" not in text:
        issues.append(f"{roadmap}: Policy Authority nested item must be checked")
    return issues


def adr_issues(repo_root):
    path = repo_root / DECISION
    try:
        raw = read_bounded_file(path, label=DECISION)
        metadata = validate_document_file(path)
        normalized = " ".join(raw.decode().split())
    except (OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: ADR-0073 validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != DECISION_SHA256:
        issues.append(f"{path}: ADR-0073 physical pin drifted")
    expected = {"status": "proposed", "body_sha256": DECISION_BODY_SHA256,
                "self_sha256": DECISION_SELF_SHA256}
    issues.extend(f"{path}: ADR-0073 {field} drifted"
                  for field, value in expected.items()
                  if metadata.get(field) != value)
    for marker in ("registry v28", "explicit EOF",
                   "deliberately absent from authenticated context routes",
                   "does not install a host Skill",
                   "Source-only fresh and legacy scaffold"):
        if marker not in normalized:
            issues.append(f"{path}: missing ADR-0073 marker {marker!r}")
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
