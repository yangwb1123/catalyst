"""ADR-0086/0087 unverified legacy read-import candidate governance."""

from __future__ import annotations
import hashlib
import json
import os
import stat

from architecture_decision_record_v2 import (
    ContractError as ADRContractError,
    validate_document_bytes,
)
from engineering_check_support import load_yaml
from governance_contract import ContractError
from legacy_governance_read_import_contract import (
    ContractError as ImportContractError,
    project_request,
    validate_view_against_request,
)
from legacy_governance_read_import_contract.constants import (
    RESULT,
    SUCCESS_MARKER,
)

from .evidence_claim_portable import EXPECTED_SCOPE
SEMANTIC_DECISION = "docs/adr/ADR-0086-legacy-governance-read-only-import-v1.md"
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0087-legacy-governance-read-import-governance-and-source-"
    "distribution.md"
)
SCHEMA = "docs/contracts/legacy-governance-read-import-v1.schema.json"
MEMORY_FIXTURE = (
    "docs/contracts/fixtures/legacy-governance-read-import-memory-v1.jsonl"
)
ADR_FIXTURES = (
    "docs/contracts/fixtures/legacy-governance-read-import-ADR-0001.md",
    "docs/contracts/fixtures/legacy-governance-read-import-ADR-0002.md",
)
REQUEST_FIXTURE = (
    "docs/contracts/fixtures/legacy-governance-read-import-request-v1.json"
)
VIEW_FIXTURE = "docs/contracts/fixtures/legacy-governance-read-import-view-v1.json"
PYTHON_PACKAGE = "harness/legacy_governance_read_import_contract"
CHECKER = "harness/legacy_governance_read_import_contract_check.py"
CORE_TEST = "harness/test_legacy_governance_read_import_contract.py"
GO_PACKAGE = "forge-core/internal/legacygovernanceimportcontract"
SCOPE_SHA256 = "8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c"

SEMANTIC_DECISION_SHA256 = (
    "6dbe26fd38c6d64294f673da9d132d4d28d6dec29d7283aceb26a5b03593701f"
)
SEMANTIC_DECISION_BODY_SHA256 = (
    "a8ab72b17237c1f379251669f3a14c280c13b01a1c59d4b005427bf02afe69d1"
)
SEMANTIC_DECISION_SELF_SHA256 = (
    "86cdfaba1d28dde72d60adc3187adc07a3489cde64e366f890f5fe45fc72dbf1"
)
GOVERNANCE_DECISION_SHA256 = (
    "5e1cf6054347d5bd15384adb649c7f011b4969da725b8bf1f188418ea1a84a68"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "70e3231eded364574763aafaf16b5d08a6f56151b5a9f1192b62909e3515b6a5"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "5640720e42790e54dde663c1fc1bac030771dec1b6974236753840e76529fc31"
)
CORE_MANIFEST_SHA256 = ("2c8aa8d3f5cb47463975377dbc5668b7c935c33d510d07cfdeca6cdb280044aa")
GO_MANIFEST_SHA256 = (
    "72d8f40e8c04792c3d22ee3a6abd950ba4425d7e0958a87476b3d4e537b395a0"
)
CORE_SHA256 = {
    SEMANTIC_DECISION: SEMANTIC_DECISION_SHA256,
    SCHEMA: "0bc8f6dca9898eebc6efefc2eca3c097fcf1f7a2d5ca5ea3dd11784585216083",
    ADR_FIXTURES[0]: "412064c813fcab0740827df6c6cd0eae2bb1b4837401d9224e1a29a5867b3fe4",
    ADR_FIXTURES[1]: "a2a5bf66e3090557edd46e1fcc6bd45900d341fd84082dd5490275b0af01c08e",
    MEMORY_FIXTURE: "18401766587cb448f605e39bdf43a8a45116247e5f98b65bddfbe9a9cf766ad1",
    REQUEST_FIXTURE: "661e024a100e34e86922c88e4bbc02cf86b57af411f2b8af772bfb4be659dccc",
    VIEW_FIXTURE: "6d864e4cb2f02930d8a27d10312ee22fb3695dc6d699ef15969d38ac4cee266c",
    f"{PYTHON_PACKAGE}/__init__.py":
        "340d7088346a46ae35e71210fa484026ec30e6f972c373de7dea0725ee91d10b",
    f"{PYTHON_PACKAGE}/canonical.py":
        "dda4c0e3ab7288865926e3de69fb77222ef5ce1d637a97edc5f55caa35a28c7b",
    f"{PYTHON_PACKAGE}/constants.py":
        "19c927c5b5fdfb0db3a26c2fd88461775af09e060c15d7685acabf4410203593",
    f"{PYTHON_PACKAGE}/memory.py":
        "cdbd85d33365dc55f9b1c779976285c53dc9750aa98fd31794236e055768d4cc",
    f"{PYTHON_PACKAGE}/projection.py":
        "dde48f1dd9d503cdeaa390b0ed5e267e015f8c9b55007d61654a59b9e82598f5",
    f"{PYTHON_PACKAGE}/source.py":
        "26a4c3bf20a60754084582d8313478fa30606dc7696eecedefd8d68b19c9efde",
    CHECKER: "f5f225a31ae765e4110fc4d298940012c23310194860a71c0b7795515e785b12",
    CORE_TEST: "69b75407bcb6575c0453281ccc5a3fefbff5cc9bf6251f8ea676aa2a723db23f",
}
GO_SHA256 = {
    "constants.go": "b0643a7ae26f1d0a809593aa18c90111ca85fe54af303d1a01ab650b272cd356",
    "doc.go": "5b2ec633a4009934d001d4f13468c397f3ef2ef9a58986acc6726718deaf3c07",
    "golden_test.go": "30550cc7f1c84bc2cfe38715a2d2b222b0df02b6a0ec38ecef274e022135363a",
    "helpers.go": "4fe78c0939e573c8df68ba1e9df7cd0a4538d6796fc9f53d864dd28618e456bd",
    "json_decode.go": "252f4c7ade63cb7ece55b94502ae69d9456aa6022445ad56b45f075fd6301cee",
    "json_encode.go": "890827c25fc3687ffc5df8bdd3a4c4982ab89e44da7dc02b22d69375f553d92c",
    "memory.go": "06097f4bc43b5445c48ede50a109ad2d3ac938b1a343b545d8c8acddeb7799e2",
    "projection.go": "82d40c35b4406faa16f75ca74f5c587d9bb22a936a4b724d9e2936da5b182cd3",
    "request.go": "1bd44d63d8f8c74aea1a615207aff1499b2cf8c68b02d522e1a1c15b0570d233",
    "view.go": "7f4473db594dc2a25750b139cc163387190f9b05f2c60fbdb523a9c281f02435",
}
GO_MARKERS = {
    "constants.go": ("maxMemoryEntries    = 4096", "result           ="),
    "memory.go": ("func parseMemoryJSONL", "confidence"),
    "projection.go": ("func Project", "buildView"),
    "request.go": ("func DecodeRequest", 'selfDigest(requestDomain, request, "request_sha256",'),
    "view.go": ("func DecodeView", "func ValidateViewAgainstRequest", "len(members) > maxMemoryEntries"),
}
ATTESTATIONS = {
    "acceptance": False,
    "authority": False,
    "confidence_interpretation": False,
    "conflict_resolution": False,
    "currentness": False,
    "instruction_eligibility": False,
    "persistence": False,
    "runtime_effect": False,
    "source_authentication": False,
    "source_completeness": False,
    "status_interpretation": False,
    "truth": False,
    "winner_selection": False,
}
LEGACY_POLICY = {
    "rejected_kind_aliases": [
        "Evidence", "Claim", "ContextManifest", "AuthorityGrant",
        "AgentCapabilityGrant",
    ],
    "memory_import": "explicit_supplied_bytes_unverified_read_only_projection_v1",
    "adr_import": "explicit_supplied_bytes_unverified_read_only_no_parse_projection_v1",
    "missing_confidence_default": "forbidden",
    "legacy_status_is_authority": False,
}
CONTRACT = {
    "profile_id": "legacy_governance_read_import_v1",
    "decision_status": "proposed",
    "delivery": (
        "source_distributed_dependency_free_python_pure_projector_with_"
        "catalyst_repository_only_go_parity"
    ),
    "mode": "exact_explicit_supplied_bytes_to_unverified_read_only_view",
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "project_snapshot_binding_is_caller_declared_not_authenticated": True,
        "request_and_candidate_identity_is_snapshot_bound": True,
        "raw_source_reconstruction_required": True,
    },
    "input_boundary": {
        "checker_argv": ["python3", CHECKER],
        "operator_supplies_exact_request_on_stdin_and_closes_eof": True,
        "detector_registry_supplies_stdin_or_request_path": False,
        "ambient_repository_path_clock_database_network_or_cache": False,
    },
    "implementation": {
        "python": "dependency_free_strict_pure_core_and_stdin_checker",
        "go": "catalyst_repository_only_cross_language_parity_not_scaffolded",
        "exact_cross_language_golden": True,
    },
    "source_distribution": {
        "copies_exact_fifteen_file_core_and_three_file_governance": True,
        "copies_go_parity_package_or_runtime": False,
        "installs_skill_route_adapter_service_or_product_command": False,
        "adds_registry_scope_evaluator_producer_or_runtime_profile": False,
    },
    "view_result": RESULT,
    "positive_result": SUCCESS_MARKER,
    "attestations": ATTESTATIONS,
    "persistence": "none",
}

CANONICAL_REFS = {
    "legacy_governance_read_import_v1_schema": SCHEMA,
    "legacy_governance_read_import_v1_memory_fixture": MEMORY_FIXTURE,
    "legacy_governance_read_import_v1_adr_0001_fixture": ADR_FIXTURES[0],
    "legacy_governance_read_import_v1_adr_0002_fixture": ADR_FIXTURES[1],
    "legacy_governance_read_import_v1_request_fixture": REQUEST_FIXTURE,
    "legacy_governance_read_import_v1_view_fixture": VIEW_FIXTURE,
    "legacy_governance_read_import_v1_checker": CHECKER,
    "legacy_governance_read_import_v1_semantic_decision": SEMANTIC_DECISION,
    "legacy_governance_read_import_v1_governance_decision": GOVERNANCE_DECISION,
}
PINS = {
    "legacy_governance_read_import_v1_schema_sha256": CORE_SHA256[SCHEMA],
    "legacy_governance_read_import_v1_memory_fixture_sha256": CORE_SHA256[MEMORY_FIXTURE],
    "legacy_governance_read_import_v1_adr_0001_fixture_sha256": CORE_SHA256[ADR_FIXTURES[0]],
    "legacy_governance_read_import_v1_adr_0002_fixture_sha256": CORE_SHA256[ADR_FIXTURES[1]],
    "legacy_governance_read_import_v1_request_fixture_sha256": CORE_SHA256[REQUEST_FIXTURE],
    "legacy_governance_read_import_v1_view_fixture_sha256": CORE_SHA256[VIEW_FIXTURE],
    "legacy_governance_read_import_v1_decision_sha256": SEMANTIC_DECISION_SHA256,
    "legacy_governance_read_import_v1_core_manifest_sha256": CORE_MANIFEST_SHA256,
    "legacy_governance_read_import_v1_go_manifest_sha256": GO_MANIFEST_SHA256,
}
REFERENCE_IMPLEMENTATIONS = {
    "legacy_governance_read_import_v1_python": {
        "ref": PYTHON_PACKAGE,
        "projection": "source_distributed_dependency_free_strict_pure_core",
    },
    "legacy_governance_read_import_v1_python_checker": {
        "ref": CHECKER,
        "projection": "source_distributed_zero_argument_explicit_stdin_only",
    },
    "legacy_governance_read_import_v1_go": {
        "ref": GO_PACKAGE,
        "projection": "catalyst_repository_only_cross_language_parity_not_scaffolded",
    },
}
NON_CAPABILITY = (
    "ADR-0086/0087 only project caller-supplied legacy Memory and ADR bytes into "
    "an unverified read-only view; Registry v39 supplies no stdin data, source "
    "authentication or completeness, confidence or status interpretation, conflict "
    "resolution, truth, currentness, instruction, acceptance, winner, persistence "
    "or runtime effect, adds no Skill, route, evaluator, producer, service or runtime "
    "profile, and source distribution copies Python governance only, never the "
    "Catalyst Go parity package, ambient path reader, database, state or authority"
)
DETECTOR_ID = "governance.legacy_governance_read_import_v1_candidate"
DETECTOR = {
    "argv": ["python3", CHECKER],
    "adapter": "standalone.legacyGovernanceReadImportV1ExplicitStdin",
    "positive": "test_registry_is_v39_scope_neutral_unverified_candidate",
    "negative": "test_scope_argv_authority_and_distribution_drift_fail_closed",
}
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["ADR-0087 unverified legacy read import"],
    ".agent/ARCHITECTURE.md": ["Legacy governance read-only import boundary"],
    ".agent/ROADMAP.md": ["Legacy Memory/ADR unverified read-only import"],
    ".agent/CURRENT_SPRINT.md": ["Registry v36 legacy governance read import"],
    ".agent/DECISIONS.md": ["D53 Legacy governance read import"],
    ".agent/engineering/README.md": ["ADR-0087 adds Registry v36"],
    "docs/design/ai-engineering-os/README.md": ["ADR-0087 Legacy Governance Read Import"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0087 Legacy Governance Read Import Governance"
    ],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "Legacy Memory/ADR unverified read-only import"
    ],
}
PROMOTION_SENTINEL = "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
def _mapping(value):
    return value if isinstance(value, dict) else {}

def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: legacy read import requires Registry v39")
    if data.get("legacy") != LEGACY_POLICY:
        issues.append(f"{path}: legacy import policy drifted")
    if data.get("legacy_governance_read_import_v1_candidate_contract") != CONTRACT:
        issues.append(f"{path}: legacy read import candidate contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: legacy read import cannot expand Registry scope")
    canonical = json.dumps(data.get("scope"), sort_keys=True,
                           separators=(",", ":")).encode()
    if hashlib.sha256(canonical).hexdigest() != SCOPE_SHA256:
        issues.append(f"{path}: complete scope digest drifted")
    for owner, expected in (("canonical_refs", CANONICAL_REFS),
                            ("contract_pins", PINS),
                            ("reference_implementations", REFERENCE_IMPLEMENTATIONS)):
        actual = _mapping(data.get(owner))
        issues.extend(f"{path}: {owner}.{field} drifted" for field, value in
                      expected.items() if actual.get(field) != value)
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: legacy read import non-capability drifted")
    return issues


def _read_regular(path, label, maximum=128 * 1024 * 1024):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise OSError("O_NOFOLLOW is unavailable")
    descriptor = os.open(path, flags | nofollow)
    try:
        info = os.fstat(descriptor)
        if (not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o644
                or info.st_nlink != 1):
            raise OSError(f"{label} must be regular 0644 with link count one")
        if info.st_size > maximum:
            raise ContractError(f"{label} exceeds {maximum} bytes")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(maximum + 1)
        if len(raw) > maximum:
            raise ContractError(f"{label} exceeds {maximum} bytes")
        return raw
    finally:
        os.close(descriptor)

def _aggregate(rows):
    manifest = "".join(f"{digest}  {relative}\n" for relative, digest in rows)
    return hashlib.sha256(manifest.encode()).hexdigest()

def core_artifact_issues(repo_root):
    package, issues, rows = repo_root / PYTHON_PACKAGE, [], []
    try:
        info = package.lstat()
    except OSError as error:
        return [f"{PYTHON_PACKAGE}: exact package unreadable: {error}"]
    if not stat.S_ISDIR(info.st_mode):
        return [f"{PYTHON_PACKAGE}: exact package must be a real directory"]
    for relative, expected in CORE_SHA256.items():
        try:
            raw = _read_regular(repo_root / relative, relative)
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: frozen core unreadable: {error}")
            continue
        digest = hashlib.sha256(raw).hexdigest()
        rows.append((relative, digest))
        if digest != expected:
            issues.append(f"{relative}: physical pin drifted")
    try:
        names = {entry.name for entry in package.iterdir()}
    except OSError as error:
        issues.append(f"{PYTHON_PACKAGE}: exact package unreadable: {error}")
    else:
        expected = {path.removeprefix(f"{PYTHON_PACKAGE}/") for path in CORE_SHA256
                    if path.startswith(f"{PYTHON_PACKAGE}/")}
        if names != expected:
            issues.append(f"{PYTHON_PACKAGE}: exact six-file package drifted")
    if len(CORE_SHA256) != 15 or _aggregate(rows) != CORE_MANIFEST_SHA256:
        issues.append("legacy read import exact15 aggregate drifted")
    return issues

def golden_issues(repo_root):
    try:
        request = _read_regular(repo_root / REQUEST_FIXTURE, REQUEST_FIXTURE)
        view = _read_regular(repo_root / VIEW_FIXTURE, VIEW_FIXTURE)
        if project_request(request) + b"\n" != view:
            return ["legacy read import Python projection differs from exact view"]
        decoded = validate_view_against_request(view[:-1], request)
    except (OSError, ContractError, ImportContractError) as error:
        return [f"legacy read import exact golden failed: {error}"]
    issues = []
    if decoded.get("result") != RESULT or decoded.get("attestations") != ATTESTATIONS:
        issues.append("legacy read import result or thirteen attestations drifted")
    return issues

def go_parity_issues(repo_root, optional=False):
    package = repo_root / GO_PACKAGE
    try:
        info = package.lstat()
    except FileNotFoundError as error:
        return [] if optional else [f"{GO_PACKAGE}: Catalyst Go parity unavailable: {error}"]
    except OSError as error:
        return [f"{GO_PACKAGE}: Catalyst Go parity unavailable: {error}"]
    if not stat.S_ISDIR(info.st_mode):
        return [f"{GO_PACKAGE}: Catalyst Go parity must be a real directory"]
    try:
        names = {entry.name for entry in package.iterdir()}
    except OSError as error:
        return [f"{GO_PACKAGE}: Catalyst Go parity unreadable: {error}"]
    if names != set(GO_SHA256):
        return [f"{GO_PACKAGE}: exact ten-file Go closure drifted"]
    issues, rows = [], []
    for name, expected in GO_SHA256.items():
        relative = f"{GO_PACKAGE}/{name}"
        try:
            raw = _read_regular(repo_root / relative, relative, 2 * 1024 * 1024)
            text = raw.decode()
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: Go parity unreadable: {error}")
            continue
        digest = hashlib.sha256(raw).hexdigest()
        rows.append((relative, digest))
        if digest != expected:
            issues.append(f"{relative}: physical pin drifted")
        issues.extend(f"{relative}: semantic marker {marker!r} missing"
                      for marker in GO_MARKERS.get(name, ()) if marker not in text)
    if _aggregate(rows) != GO_MANIFEST_SHA256:
        issues.append(f"{GO_PACKAGE}: exact10 aggregate drifted")
    return issues

def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    item = detectors.get(DETECTOR_ID)
    if not isinstance(item, dict):
        return ["legacy read import checker-only detector is missing"]
    implementation = _mapping(item.get("implementation"))
    invocation, tests = _mapping(item.get("invocation")), _mapping(item.get("tests"))
    issues = []
    expected_implementation = {"argv": DETECTOR["argv"], "cwd": "repo_root", "shell": False}
    if implementation != expected_implementation:
        issues.append("legacy read import zero-argument detector argv drifted")
    expected_invocation = {"owner": "operator", "adapter": DETECTOR["adapter"],
                           "acceptance_criterion": None, "load_bearing": False}
    if item.get("state") != "shadow" or invocation != expected_invocation:
        issues.append("legacy read import detector must remain operator shadow only")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"legacy read import detector {polarity} sentinel drifted")
    uses = [candidate for candidate in detectors.values() if CHECKER in
            (_mapping(candidate.get("implementation")).get("argv") or [])]
    if uses != [item]:
        issues.append("legacy read import checker requires exactly one detector")
    return issues

def wiring_issues(agent_root):
    from agent_engineering.contract import EXTENSION_REFS
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    if a_error or d_error or r_error:
        return ["legacy read import Agent Engineering wiring is unreadable"]
    extensions = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"legacy read import activation ref {field} drifted" for field, value
              in CANONICAL_REFS.items() if extensions.get(field) != value or
              EXTENSION_REFS.get(field) != value]
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    issues.extend(f"legacy read import contract asset missing: {value}" for value in
                  CANONICAL_REFS.values() if value not in assets)
    encoded = json.dumps(routes, sort_keys=True)
    forbidden = tuple(CANONICAL_REFS.values()) + (DETECTOR_ID, GO_PACKAGE,
                                                  "legacyGovernanceReadImport")
    if any(token in encoded for token in forbidden):
        issues.append("legacy read import candidate cannot enter a context route")
    if any("legacy_governance_read_import" in field and field.endswith("_skill")
           for field in EXTENSION_REFS):
        issues.append("legacy read import candidate cannot install a Skill")
    return issues

def _adr_one(repo_root, relative, physical, body, self_digest, markers):
    try:
        raw = _read_regular(repo_root / relative, relative, 256 * 1024)
        metadata = validate_document_bytes(raw, relative.rsplit("/", 1)[-1])
        normalized = " ".join(raw.decode().split())
    except (OSError, ADRContractError, ContractError, UnicodeDecodeError) as error:
        return [f"{relative}: Proposed ADR failed: {error}"]
    issues = []
    expected = {"status": "proposed", "body_sha256": body, "self_sha256": self_digest}
    if hashlib.sha256(raw).hexdigest() != physical:
        issues.append(f"{relative}: physical pin drifted")
    issues.extend(f"{relative}: {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    issues.extend(f"{relative}: boundary marker {marker!r} missing" for marker in
                  markers if marker not in normalized)
    return issues

def adr_issues(repo_root):
    issues = _adr_one(
        repo_root, SEMANTIC_DECISION, SEMANTIC_DECISION_SHA256,
        SEMANTIC_DECISION_BODY_SHA256, SEMANTIC_DECISION_SELF_SHA256,
        ("unverified_legacy", "thirteen false semantics", "stdin"),
    )
    issues.extend(_adr_one(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v36", "exact eighteen-file closure", "zero-argument stdin"),
    ))
    return issues

def documentation_issues(repo_root):
    if not (repo_root / PROMOTION_SENTINEL).is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        try:
            text = (repo_root / relative).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: v36 documentation unavailable: {error}")
            continue
        issues.extend(f"{relative}: v36 marker {marker!r} missing"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = roadmap.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: v36 roadmap unavailable: {error}"]
    for marker in ("设计旧 memory/ADR 的只读导入",
                   "unverified_legacy", "绝不自动确认"):
        if marker not in text:
            issues.append(f"{roadmap}: v36 roadmap marker {marker!r} missing")
    return issues

def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(core_artifact_issues(repo_root))
    issues.extend(golden_issues(repo_root))
    issues.extend(go_parity_issues(repo_root, optional=True))
    issues.extend(detector_issues(agent_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(adr_issues(repo_root))
    issues.extend(documentation_issues(repo_root))
    return issues
