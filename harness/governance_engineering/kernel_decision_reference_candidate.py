"""ADR-0090/0091 Kernel decision reference candidate governance."""

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
from kernel_decision_contract import SUCCESS_MARKER, load_golden
from kernel_decision_contract.constants import ATTESTATION_FIELDS

from .evidence_claim_portable import EXPECTED_SCOPE

SEMANTIC_DECISION = "docs/adr/ADR-0090-kernel-decision-reference-core-v1.md"
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0091-kernel-decision-reference-governance-and-source-"
    "distribution.md"
)
SCHEMA = "docs/contracts/kernel-decision-reference-core-v1.schema.json"
GOLDEN = "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
PYTHON_PACKAGE = "harness/kernel_decision_contract"
CHECKER = "harness/kernel_decision_contract_check.py"
PYTHON_TESTS = (
    "harness/test_kernel_decision_contract.py",
    "harness/test_kernel_decision_reference_graph.py",
    "harness/test_kernel_decision_strict.py",
)
GO_PACKAGE = "forge-core/internal/kerneldecisioncontract"
RUST_PACKAGE = "forge-runtime/crates/domain/src/kernel_decision_contract"
RUST_REGISTRATION = "forge-runtime/crates/domain/src/lib.rs"
SCOPE_SHA256 = "8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c"
SEMANTIC_DECISION_SHA256 = (
    "5ebb9bcb4fbce5c0e613fe59de44c19fb7c359e506b6ee3b2a6d66e38afd3210"
)
SEMANTIC_DECISION_BODY_SHA256 = (
    "90fb217528d0a234ebee7ac65a5df91a91a801beed86d3353492634f63f6fa39"
)
SEMANTIC_DECISION_SELF_SHA256 = (
    "a12c58f74c941c32f9e6705ed4fcb8b0b5305c2575b81f262719d908cded44b8"
)
GOVERNANCE_DECISION_SHA256 = (
    "a37dc6d8bce98bae07f5d4e047d52b8625b60c65d3f0129b1e2989d54d2eedde"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "79b88283df34569e902be62e7bdc702e413109c7aa3d66a93b8bb77bdcefaf56"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "8fae0ca867bb3d18fce8a63af9cae277cd1f038a187bf2740e13521c6334c883"
)
SCHEMA_SHA256 = "1add521e4533a0ad41e273d500fd449e9953799e57bdc8df10210d9ebb4238b9"
GOLDEN_SHA256 = "93f6225b745eacf966796cb671d723440890ae3ab02699dd40d6a078f539af1c"
CLOSURE_SHA256 = "cdadf0e5fddbbda429939be4e68dc77dd0b52c0bb7e4fe955f1d485183908e58"
PYTHON_MANIFEST_SHA256 = "0c2dd1383454f605cdcfdf42c6b1d2916d62deccedf8e5f1b05d018202125d8e"
CORE_MANIFEST_SHA256 = "acab4b66f5bf39161265c0d232cbe225e5d3881362620cdafdd8a2634891a7e1"
GO_MANIFEST_SHA256 = "1d32535593206275a101f0edbd58788cc5ae92cbddcb8170b7c653168eba8de0"
RUST_FLAT_MANIFEST_SHA256 = "ce6d042e7b31ee4706626ee7d35e7214b9cd6994a36422cadcdff2ee6b16b187"

PYTHON_SHA256 = {
    f"{PYTHON_PACKAGE}/__init__.py": "fd6c3f87c4e4b587daa1e2d9a3e50575dfe21d6310f63b879d50ccf3b2f03e7e",
    f"{PYTHON_PACKAGE}/atoms.py": "84d8b55ed0807b2fe61d14946d75dd09fd8ea95846562357452d46300fc982ff",
    f"{PYTHON_PACKAGE}/closure.py": "a5a4c0004b5cae9091654e9ebfa1b8173cd911089c17239d6e15b61110f85718",
    f"{PYTHON_PACKAGE}/codec.py": "ca8c9bdb73f42504cd5aadf247bcf1948febf58aa5da12340af4c31c3a21e307",
    f"{PYTHON_PACKAGE}/constants.py": "a68f03e4f4e471cfdc240102f50e578584022793c912753e472ee75f118ed883",
    f"{PYTHON_PACKAGE}/fixture.py": "a1323ea77a1fc5dec3cb627cf845e75935bcf9f67f9255cca40a3612db3b57f8",
    f"{PYTHON_PACKAGE}/graph.py": "ee90aae941ccf398e86bdda2b7682319afa4f342fa63e9a748b956797337a9db",
    f"{PYTHON_PACKAGE}/shape.py": "0cedba5d4ac7b39778893973e6e7650db545a4a96610034a81ef18b56ce1c632",
    f"{PYTHON_PACKAGE}/transaction.py": "cd989fb96c83ba1358eaa2ae6873d389b827e9006796baa13656e79f395ee474",
    CHECKER: "2f0d5e24c085047c04c0bd2fe28046cf43edf476b94db49718ecca29323b1f5a",
    PYTHON_TESTS[0]: "dda52dba4ecc664e8ed5821e510a76435bc349da63d6c3e55eb496649e409185",
    PYTHON_TESTS[1]: "6b625c6a9fbd6c46ae6404a9ce6dc4273434e10146bb11eb50478a2e4ec14409",
    PYTHON_TESTS[2]: "d749583506b1973cf08afc7bf9532b7b53fa819eecccd05d15a4f50e4e899df3",
}
CORE_SHA256 = {
    SEMANTIC_DECISION: SEMANTIC_DECISION_SHA256,
    SCHEMA: SCHEMA_SHA256,
    GOLDEN: GOLDEN_SHA256,
    **PYTHON_SHA256,
}
GO_SHA256 = {
    "atom.go": "fc942b64351581967927795d6c32f7a552c72c61810edbbec26f2c370f6d20d8",
    "clone.go": "9fc9618ed5946d475ad7d56c256d7402dc3dee525816d47497047e7d74c47487",
    "closure.go": "8c88e728230277c3861e4dbb285b9377211177c87667af39293beeab3ff40263",
    "constants.go": "9f627beb29583f46513635cb78a917498b2c10947885cfa28811ff7c0ede253c",
    "contract_test.go": "3c1d42068a07cbec1dad5ed1ff9aa11b05f30f3114b5a524337d2e62c136a674",
    "decision_graph_test.go": "421cea0aa725f32a3f5a622d86e105648c73ba6d7bd0669f9fc5ce1a349c7f90",
    "graph.go": "a8a6c39c9c73d30b2fa644e05d63ead419ad1658f048881ed630d42d6e655910",
    "matrix_test.go": "f0c9f487d7a6f1dc6eacae69ea5972ca180d0552de337e9ab81d40c8f985ad38",
    "model.go": "6b26cc6d80607ffa2de505f1da89a2b98e6ca3765f54339fd1baf100134ec17d",
    "primitives.go": "c302f91d2ed3454202b654355f900a08917a3bfc3f4c61b1814281bbb705ca9a",
    "source.go": "794193e985ed09b1df3acb9a98a516508a4350dba58161524dbc9412a318414b",
    "transaction.go": "6e03056ccc7782b53a09653750335a55598e02d770e662461f96bbd0b0dfc78e",
    "wire.go": "cd2c97e55e570cb17369d3c9f9b62b3097c459b7da7bcf15817818fef21036cf",
}
RUST_SHA256 = {
    "atom.rs": "854da31bb8128f8c90f6255818615e337cc595263aeb4dedd1fbdc5cdc006836",
    "closure.rs": "2aa7fdfa00d77550490b5133f2055bb6e75f2b2a25813b8652364994dd2c87e2",
    "graph.rs": "ab116baafe35cbe11673c242aa83ad0f4c8a5019cb744d997abb28a1d40a9fb3",
    "mod.rs": "3538b6d3a71bf80308adacbce07dff30112f3fb07a3a510582fbc4fe4c9e0536",
    "model.rs": "341adaa0b61f100fa50ffec8c0a7f27ac460b33cad9c33d8948147cffa9fca5a",
    "primitives.rs": "1ee5280826cb346136df9dd67a0329f6a8eacb2faaab27d983b72f6c64b76f73",
    "source.rs": "1a66d89e30c95cfba0ca3b227f845f3250fe92a4af3b81524dbab1c3b6fdb348",
    "transaction.rs": "fa92496649ee74d7cc756b0ef9d9dde58042910df29793caf75c244ba64847a2",
    "wire.rs": "2f654f6aec9ae2391e61a78c272766ae0eec02f7e3b4696aa64b8aaaeb2e64c6",
}
ATTESTATIONS = {name: False for name in ATTESTATION_FIELDS}
CONTRACT = {
    "profile_id": "kernel_decision_reference_core_v1",
    "decision_status": "proposed",
    "delivery": (
        "source_distributed_dependency_free_python_pure_contract_with_"
        "catalyst_repository_only_go_and_rust_parity"
    ),
    "mode": "exact_caller_supplied_decision_and_operational_reference_closure_validation",
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "cognitive_atom_v2_and_decision_transaction_v1": True,
        "one_way_operational_cross_closure": True,
        "domain_separated_atom_transaction_and_closure_seals": True,
        "declared_sources_authority_hardness_and_bindings_are_not_authenticated": True,
    },
    "input_boundary": {
        "checker_argv": ["python3", CHECKER, "--golden", "."],
        "detector_validates_only_the_pinned_golden": True,
        "explicit_file_mode_is_standalone_not_registry_supplied": True,
        "stdin_repository_discovery_clock_database_network_or_cache": False,
    },
    "implementation": {
        "python": "source_distributed_dependency_free_strict_pure_contract",
        "go": "catalyst_repository_only_cross_language_parity_not_scaffolded",
        "rust": "catalyst_repository_only_cross_language_parity_not_scaffolded",
        "rust_registration": "unique_module_declaration_not_whole_file_pinned",
        "rust_pin_scope": "flat_exact9_module_sources_only",
        "exact_three_language_golden": True,
    },
    "source_distribution": {
        "copies_exact_sixteen_file_core_and_three_file_governance": True,
        "copies_go_rust_or_runtime_registration": False,
        "installs_skill_route_adapter_service_product_command_pdp_or_controller": False,
        "adds_registry_scope_kind_evaluator_producer_or_runtime_profile": False,
    },
    "completion": {
        "kernel_structural_reference_family_abi_complete": True,
        "broader_adr_0038_complete": False,
        "decision_capsule_complete": False,
        "authorized_transaction_spec_complete": False,
        "authenticated_pdp_complete": False,
        "controller_complete": False,
        "authority_promotion_complete": False,
    },
    "effective_semantics": {
        "effective_hardness": "none",
        "instruction_allowed": False,
        "declared_authority_is_effective": False,
    },
    "positive_result": SUCCESS_MARKER,
    "attestations": ATTESTATIONS,
    "persistence": "none",
}
CANONICAL_REFS = {
    "kernel_decision_reference_core_v1_schema": SCHEMA,
    "kernel_decision_reference_core_v1_golden_fixture": GOLDEN,
    "kernel_decision_reference_core_v1_checker": CHECKER,
    "kernel_decision_reference_core_v1_semantic_decision": SEMANTIC_DECISION,
    "kernel_decision_reference_core_v1_governance_decision": GOVERNANCE_DECISION,
}
PINS = {
    "kernel_decision_reference_core_v1_schema_sha256": SCHEMA_SHA256,
    "kernel_decision_reference_core_v1_golden_fixture_sha256": GOLDEN_SHA256,
    "kernel_decision_reference_core_v1_decision_sha256": SEMANTIC_DECISION_SHA256,
    "kernel_decision_reference_core_v1_closure_sha256": CLOSURE_SHA256,
    "kernel_decision_reference_core_v1_python_manifest_sha256": PYTHON_MANIFEST_SHA256,
    "kernel_decision_reference_core_v1_source_manifest_sha256": CORE_MANIFEST_SHA256,
    "kernel_decision_reference_core_v1_go_manifest_sha256": GO_MANIFEST_SHA256,
    "kernel_decision_reference_core_v1_rust_flat_manifest_sha256": RUST_FLAT_MANIFEST_SHA256,
}
REFERENCE_IMPLEMENTATIONS = {
    "kernel_decision_reference_core_v1_python": {
        "ref": PYTHON_PACKAGE,
        "projection": "source_distributed_dependency_free_strict_pure_contract",
    },
    "kernel_decision_reference_core_v1_python_checker": {
        "ref": CHECKER,
        "projection": "source_distributed_pinned_golden_or_explicit_file_checker",
    },
    "kernel_decision_reference_core_v1_go": {
        "ref": GO_PACKAGE,
        "projection": "catalyst_repository_only_exact13_cross_language_parity_not_scaffolded",
    },
    "kernel_decision_reference_core_v1_rust": {
        "ref": RUST_PACKAGE,
        "projection": "catalyst_repository_only_flat_exact9_parity_with_unpinned_unique_registration_not_scaffolded",
    },
}
NON_CAPABILITY = (
    "ADR-0090/0091 only validate exact caller-supplied Kernel decision and "
    "operational reference declarations; Registry v39 keeps declared authority and "
    "hardness ineffective, instructions disabled and all twenty-two attestations "
    "false, authenticates no source, principal, Approval, Grant, artifact content or "
    "binding, performs no authorization, CAS, event append, persistence, transition, "
    "execution, outcome, completion, effect or usage measurement, adds no Skill, "
    "route, kind, evaluator, producer, service, PDP, controller or runtime profile, "
    "copies no Catalyst Go or Rust parity or runtime registration, and stages only "
    "the structural reference-family ABI candidate while ADR-0038, DecisionCapsule and "
    "AuthorizedTransactionSpec remain incomplete"
)
DETECTOR_ID = "governance.kernel_decision_reference_core_v1_candidate"
DETECTOR = {
    "argv": ["python3", CHECKER, "--golden", "."],
    "adapter": "standalone.kernelDecisionReferenceCoreV1PinnedGolden",
    "positive": "test_registry_is_v39_scope_neutral_structural_candidate",
    "negative": "test_scope_argv_authority_and_distribution_drift_fail_closed",
}


def _mapping(value):
    return value if isinstance(value, dict) else {}
def _read_regular(path, label, maximum=128 * 1024 * 1024):
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise OSError("O_NOFOLLOW is unavailable")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | nofollow)
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
def _real_directory(repo_root, relative, label):
    current = repo_root
    for part in relative.split("/"):
        current /= part
        info = current.lstat()
        if not stat.S_ISDIR(info.st_mode):
            raise OSError(f"{label} component {current} must be a real directory")
    return current


def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: Kernel decision candidate requires Registry v39")
    if data.get("kernel_decision_reference_core_v1_candidate_contract") != CONTRACT:
        issues.append(f"{path}: Kernel decision candidate contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: Kernel decision candidate cannot expand Registry scope")
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
        issues.append(f"{path}: Kernel decision non-capability drifted")
    return issues


def _manifest_issues(repo_root, paths, expected_aggregate, label):
    issues, rows = [], []
    for relative, expected in paths.items():
        try:
            raw = _read_regular(repo_root / relative, relative)
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: {label} unreadable: {error}")
            continue
        digest = hashlib.sha256(raw).hexdigest()
        rows.append((relative, digest))
        if digest != expected:
            issues.append(f"{relative}: physical pin drifted")
    if len(rows) != len(paths) or _aggregate(rows) != expected_aggregate:
        issues.append(f"{label} exact{len(paths)} aggregate drifted")
    return issues


def core_artifact_issues(repo_root):
    package = repo_root / PYTHON_PACKAGE
    try:
        info = package.lstat()
        names = {entry.name for entry in package.iterdir()}
    except OSError as error:
        return [f"{PYTHON_PACKAGE}: exact package unreadable: {error}"]
    expected = {path.removeprefix(f"{PYTHON_PACKAGE}/") for path in PYTHON_SHA256
                if path.startswith(f"{PYTHON_PACKAGE}/")}
    issues = [] if stat.S_ISDIR(info.st_mode) and names == expected else [
        f"{PYTHON_PACKAGE}: exact nine-file package drifted"]
    issues.extend(_manifest_issues(
        repo_root, PYTHON_SHA256, PYTHON_MANIFEST_SHA256, "Kernel decision Python"))
    issues.extend(_manifest_issues(
        repo_root, CORE_SHA256, CORE_MANIFEST_SHA256, "Kernel decision source core"))
    return issues


def golden_issues(repo_root):
    try:
        closure = load_golden(repo_root)
    except (OSError, ContractError) as error:
        return [f"Kernel decision exact golden failed: {error}"]
    issues = []
    if closure.get("closure_sha256") != CLOSURE_SHA256:
        issues.append("Kernel decision closure semantic seal drifted")
    if closure.get("result") != SUCCESS_MARKER or closure.get("attestations") != ATTESTATIONS:
        issues.append("Kernel decision result or twenty-two attestations drifted")
    return issues


def _optional_package_issues(repo_root, package, paths, aggregate, label, rust=False):
    root = repo_root / package
    try:
        root.lstat()
    except FileNotFoundError as error:
        if (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
            return [f"{package}: required {label} unavailable: {error}"]
        return []
    except OSError as error:
        return [f"{package}: {label} unreadable: {error}"]
    try:
        root = _real_directory(repo_root, package, label)
        actual = {entry.name for entry in root.iterdir()}
        expected = set(paths) | ({"tests"} if rust else set())
        if actual != expected:
            return [f"{package}: {label} exact lexical closure drifted"]
        if rust:
            _real_directory(repo_root, f"{package}/tests", label)
    except OSError as error:
        return [f"{package}: {label} unreadable: {error}"]
    mapped = {f"{package}/{name}": digest for name, digest in paths.items()}
    return _manifest_issues(repo_root, mapped, aggregate, label)


def go_parity_issues(repo_root):
    return _optional_package_issues(
        repo_root, GO_PACKAGE, GO_SHA256, GO_MANIFEST_SHA256, "Catalyst Go parity")
def rust_parity_issues(repo_root):
    issues = _optional_package_issues(
        repo_root, RUST_PACKAGE, RUST_SHA256, RUST_FLAT_MANIFEST_SHA256,
        "Catalyst Rust flat parity", rust=True)
    try:
        (repo_root / RUST_PACKAGE).lstat()
        _real_directory(repo_root, RUST_REGISTRATION.rsplit("/", 1)[0],
                        "Catalyst Rust registration")
        raw = _read_regular(repo_root / RUST_REGISTRATION, RUST_REGISTRATION)
        if raw.decode("utf-8").splitlines().count(
                "pub mod kernel_decision_contract;") != 1:
            issues.append(f"{RUST_REGISTRATION}: exact unique registration drifted")
    except (OSError, UnicodeError) as error:
        if (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
            issues.append(f"{RUST_REGISTRATION}: registration unreadable: {error}")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    item = detectors.get(DETECTOR_ID)
    if not isinstance(item, dict):
        return ["Kernel decision checker-only detector is missing"]
    implementation = _mapping(item.get("implementation"))
    invocation, tests = _mapping(item.get("invocation")), _mapping(item.get("tests"))
    issues = []
    expected = {"argv": DETECTOR["argv"], "cwd": "repo_root", "shell": False}
    if implementation != expected:
        issues.append("Kernel decision pinned-golden detector argv drifted")
    expected = {"owner": "operator", "adapter": DETECTOR["adapter"],
                "acceptance_criterion": None, "load_bearing": False}
    if item.get("state") != "shadow" or invocation != expected:
        issues.append("Kernel decision detector must remain operator shadow only")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"Kernel decision detector {polarity} sentinel drifted")
    uses = [candidate for candidate in detectors.values() if CHECKER in
            (_mapping(candidate.get("implementation")).get("argv") or [])]
    if uses != [item]:
        issues.append("Kernel decision checker requires exactly one detector")
    return issues


def wiring_issues(agent_root):
    from agent_engineering.contract import EXTENSION_REFS
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    if a_error or d_error or r_error:
        return ["Kernel decision Agent Engineering wiring is unreadable"]
    extensions = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"Kernel decision activation ref {field} drifted" for field, value
              in CANONICAL_REFS.items() if extensions.get(field) != value or
              EXTENSION_REFS.get(field) != value]
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    issues.extend(f"Kernel decision contract asset missing: {value}" for value in
                  CANONICAL_REFS.values() if value not in assets)
    encoded = json.dumps(routes, sort_keys=True)
    forbidden = tuple(CANONICAL_REFS.values()) + (
        DETECTOR_ID, GO_PACKAGE, RUST_PACKAGE, "kernelDecisionReference")
    if any(token in encoded for token in forbidden):
        issues.append("Kernel decision candidate cannot enter a context route")
    if any("kernel_decision_reference" in field and field.endswith("_skill")
           for field in EXTENSION_REFS):
        issues.append("Kernel decision candidate cannot install a Skill")
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
        ("CognitiveAtom v2", "DecisionTransaction v1", "twenty-two"),
    )
    issues.extend(_adr_one(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v38", "exactly nineteen files", "ADOPTED-PARTIAL"),
    ))
    return issues


def roadmap_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    relative = "docs/design/ai-engineering-os/implementation-roadmap.md"
    expected = (
        "- [x] 冻结 Kernel structural reference-family ABI（structural only）：扩展 "
        "CognitiveAtom source/type/authority/hardness，并定义 DecisionTransaction "
        "及其对 InteractionEvent、CapabilityInvocation、ArtifactReceipt、"
        "ExecutionReceipt 的单向引用闭包；"
    )
    try:
        text = _read_regular(repo_root / relative, relative).decode()
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{relative}: Kernel structural roadmap unreadable: {error}"]
    entries = [line for line in text.splitlines()
               if "Kernel structural reference-family ABI" in line]
    issues = [] if entries == [expected] else [
        f"{relative}: Kernel structural reference-family ABI must remain one exact completed item"]
    required_open = (
        "DecisionCapsule 与 AuthorizedTransactionSpec",
        "DecisionTransaction 与有界 rolling controller",
        "完成 Governance Kernel/PDP",
    )
    if any(not any(line.startswith("- [ ]") and marker in line
                   for line in text.splitlines()) for marker in required_open):
        issues.append(f"{relative}: broader DecisionCapsule/AuthorizedSpec/PDP/controller work must remain open")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(core_artifact_issues(repo_root))
    issues.extend(golden_issues(repo_root))
    issues.extend(go_parity_issues(repo_root))
    issues.extend(rust_parity_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(adr_issues(repo_root))
    issues.extend(roadmap_issues(repo_root))
    return issues
