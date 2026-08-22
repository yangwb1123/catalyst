"""ADR-0088/0089 Kernel operational reference candidate governance."""

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
from kernel_operational_contract import SUCCESS_MARKER, load_golden

from .evidence_claim_portable import EXPECTED_SCOPE

SEMANTIC_DECISION = "docs/adr/ADR-0088-kernel-operational-reference-core-v1.md"
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0089-kernel-operational-reference-governance-and-source-"
    "distribution.md"
)
SCHEMA = "docs/contracts/kernel-operational-reference-core-v1.schema.json"
GOLDEN = "docs/contracts/fixtures/kernel-operational-reference-closure-v1.json"
PYTHON_PACKAGE = "harness/kernel_operational_contract"
CHECKER = "harness/kernel_operational_contract_check.py"
PYTHON_TESTS = (
    "harness/test_kernel_operational_contract.py",
    "harness/test_kernel_operational_cross_contract.py",
    "harness/test_kernel_operational_reference_graph.py",
)
GO_PACKAGE = "forge-core/internal/kerneloperationalcontract"
RUST_PACKAGE = "forge-runtime/crates/domain/src/kernel_operational_contract"
RUST_REGISTRATION = "forge-runtime/crates/domain/src/lib.rs"
SCOPE_SHA256 = "8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c"
SEMANTIC_DECISION_SHA256 = (
    "e179c3451a28df68051e7dc5f907db5e097c2bca5baab4894700bebafdc9bb77"
)
SEMANTIC_DECISION_BODY_SHA256 = (
    "09ba5bee976a5a25c460b164365bb2b77d80d55e80dc0fece7a438d982686dd9"
)
SEMANTIC_DECISION_SELF_SHA256 = (
    "9c166a1071afa1ef21067f189fdef2634910e07d5a51045f2e6ab64e5ec26195"
)
GOVERNANCE_DECISION_SHA256 = (
    "eccdcd118983be03de27ef8886679d3fb0c73a2b3aee0f4bdeb6e10fdc1007f2"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "97e77a7b735f1aa7e2b188cda7a2ffed346a7292b529d2f0e84aa0795c16f9b8"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "f75f36728628122e23167e7945880c68b0aabba4d3a5c8e629c76cd884765e44"
)
SCHEMA_SHA256 = "4e166659b6e6ed39f157bb514565ebd589ae727183e179448839a974b3fbf2b0"
GOLDEN_SHA256 = "85f8d9887331fe95e52533c228e40b41750f04dfe10f3a7c77e5a4daff785f2f"
CLOSURE_SHA256 = "1db702583b8dae850413b75b80d620a6031ad452071908e33ea551a4f5feae0e"
PYTHON_MANIFEST_SHA256 = "7fe140626edb3fc613a9f986ba088c35faefa54b74970a52727d0abc46a8f08c"
CORE_MANIFEST_SHA256 = "e6b5dd47e35e00916ea2f112d00bb1e5c5c10be5b03895ef5f892c029e39a3fe"
GO_MANIFEST_SHA256 = "009b38f62eb434d66e862761be15b55e8d49a5bdf71a2b685a1f3e9758986709"
RUST_MODULE_MANIFEST_SHA256 = "37e5a2ca8d9896f649a6eef9ba4f146ea5cdd92a1dcdaaa0f2ca5cf97021c2fc"

PYTHON_SHA256 = {
    f"{PYTHON_PACKAGE}/__init__.py": "b2c608a88bee2c7c5c01cc93ff667cd4a3d19f112a7218b30765cccaed0f53d7",
    f"{PYTHON_PACKAGE}/closure.py": "c16a27a1f248d7d798cf6e044310cf59588207c7f6692d0a46cce68e8064cb3f",
    f"{PYTHON_PACKAGE}/codec.py": "959be158f96a59f757b5257ac951860d4b3a79c968e0b69d4d95c73b44e866ee",
    f"{PYTHON_PACKAGE}/constants.py": "b78febe4db08a8a6656ee87817fc3403e7e5c73f8605464fda290192ddcc33bf",
    f"{PYTHON_PACKAGE}/fixture.py": "38881bb34ea3cb82dcc81743e08cba2f0e14b0bf4aed4f6a1a2324cde2974342",
    f"{PYTHON_PACKAGE}/graph.py": "c92bb8f5833a291cff8da9fc342062617b6caa4e30675bac7e98fb8997132580",
    f"{PYTHON_PACKAGE}/records.py": "05428bca080f0043ebfc54fd9dc28e60c7b41dd29c78929ebecd593d9cb4d8e4",
    f"{PYTHON_PACKAGE}/shape.py": "31b2372f58867702a4b7895f6a21bdd5bc32da97c9141dfc14a7c37478a08312",
    CHECKER: "2b3945def7f462937c65e29514b3b7e9440468fa35f359f915fa8e88b58d6cf4",
    PYTHON_TESTS[0]: "8ed6d2cd7015c345b7366eed1aa8d67287411c67bd5ebc67f477ced997e1a587",
    PYTHON_TESTS[1]: "25b647d44c832873710964610ad690ebc75bcf7b4b0d458eb13cea700c9b9a85",
    PYTHON_TESTS[2]: "3f5a19d134e24ae32af762ef4fc3e2a9261985f322f7150cc68126e193180a2f",
}
CORE_SHA256 = {
    SEMANTIC_DECISION: SEMANTIC_DECISION_SHA256,
    SCHEMA: SCHEMA_SHA256,
    GOLDEN: GOLDEN_SHA256,
    **PYTHON_SHA256,
}
GO_SHA256 = {
    "closure.go": "a79f8881bc855ae8a4dc4aaa43bca550a11d93410c7c234990c2f94f0a03b07a",
    "constants.go": "ad0b69d7c71d6810f11ee9e9fed07e113eb16eab2a042723fbddb1aa4cdd90e1",
    "contract_test.go": "c0204815182f9b64d7422e1039ae498e7ff35b8cac8e7852c9f71546f5b475e6",
    "graph.go": "6860c065b73e7404f8034fb4713ad6a0823b76d6f3958200db9a69f3f54b22b9",
    "graph_test.go": "4d20d2782e965e5e4fce4da20f64acd4e21e2dddbdb5c1cb003743723ae332d0",
    "primitives.go": "1dd48d8ba37fd00d4e035e1f2dc1f12b5207304321e6d2ba11a1204a6efdc92d",
    "records.go": "89b6f8815941fe4902c730e1551afa0bd914a89b61967a4aee86353fe59fa287",
    "strict_test.go": "b22939a0cb23aceea9c61d9cc67a62679bd3ba1fe3a2770603a91cd263741d36",
    "types.go": "75be1f6f709a806dcff03726bad2e85214938bcf036807e31aa0dd4db6bd9b3a",
    "validation.go": "a3edc5e198e00524eaa3f43cf64087b9d7fed1c5d0135cc34ce7640eb2589dcd",
    "wire.go": "a6cffa73c7e558bcec96f397a5de142ced59b34769d255c975eceee7882ee735",
}
RUST_SHA256 = {
    "closure.rs": "d8e9adc066349b4da38c01c19f86ff838b707139c1b956f06c74ca1a6b8d43e1",
    "codec.rs": "34ec7b20e8e77bb1cc23037622f21ae55852e09da3ff46c74bb922c9d82f9114",
    "constants.rs": "74aa0eeef8529e65644162e954caa814325cd16a2aacee0932cc815689de65a3",
    "graph.rs": "3dfa85b52b7fd420cd5ce2fe18494e12618809c967edfcfb02e5dfacbdbea0ff",
    "mod.rs": "84938a4aa1bc1366dd24e6d7ecf059517517a2535f88d5e81059ad07caa2fa5a",
    "model.rs": "53134a0679206d92ec7a83f41502fb6511b720061de16b8a113b581fada56f60",
    "primitives.rs": "489303da1c7a05b155041ad6a511d8536d8a29bd8088171ee1fb32b203a8d16f",
    "records.rs": "a6380547730090a154f9fb0489f086d7db80e0bff64fa3928decaf3175379c0c",
    "tests/graph.rs": "cd419343938fc4feada06836443b9f2ce4d36c0e1e86a25d8f33dd2749bdd6cf",
    "tests/mod.rs": "f0bce7866ac6e566eacf72ca9cb8a22358592706b3d124b363646e9d5f6d2faf",
    "tests/strict.rs": "641817feda564a40fb1e3890c2761458390ef4ef8ea0e089ac39afd1f1d78163",
    "validation.rs": "fceb7637307173d83b9ca09180e7770059a6668729bcfae2a851fc0788856dd9",
    "wire.rs": "980a666f33720883cb2b5b23d57a46e6fd9031eaea1f0c9d3a240d20eb3657f2",
}
ATTESTATIONS = {
    name: False for name in (
        "authorization_attestation", "binding_authentication_attestation",
        "completion_attestation", "content_provenance_attestation",
        "effect_attestation", "event_append_attestation", "execution_attestation",
        "grant_authentication_attestation", "outcome_attestation",
        "permission_attestation", "persistence_attestation",
        "principal_authentication_attestation", "transition_attestation",
        "usage_measurement_attestation",
    )
}
CONTRACT = {
    "profile_id": "kernel_operational_reference_core_v1",
    "decision_status": "proposed",
    "delivery": (
        "source_distributed_dependency_free_python_pure_contract_with_"
        "catalyst_repository_only_go_and_rust_parity"
    ),
    "mode": "exact_caller_supplied_operational_reference_closure_validation",
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "five_semantic_objects_and_one_nonsemantic_closure": True,
        "domain_separated_record_and_closure_seals": True,
        "opaque_bindings_are_caller_declared_not_authenticated": True,
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
        "exact_three_language_golden": True,
    },
    "source_distribution": {
        "copies_exact_fifteen_file_core_and_three_file_governance": True,
        "copies_go_rust_or_runtime_registration": False,
        "installs_skill_route_adapter_service_or_product_command": False,
        "adds_registry_scope_kind_evaluator_producer_or_runtime_profile": False,
    },
    "completion": {
        "operational_reference_subclosure_only": True,
        "full_kernel_abi_complete": False,
        "cognitive_atom_expansion_complete": False,
        "decision_transaction_complete": False,
        "cross_closure_semantics_complete": False,
    },
    "positive_result": SUCCESS_MARKER,
    "attestations": ATTESTATIONS,
    "persistence": "none",
}
CANONICAL_REFS = {
    "kernel_operational_reference_core_v1_schema": SCHEMA,
    "kernel_operational_reference_core_v1_golden_fixture": GOLDEN,
    "kernel_operational_reference_core_v1_checker": CHECKER,
    "kernel_operational_reference_core_v1_semantic_decision": SEMANTIC_DECISION,
    "kernel_operational_reference_core_v1_governance_decision": GOVERNANCE_DECISION,
}
PINS = {
    "kernel_operational_reference_core_v1_schema_sha256": SCHEMA_SHA256,
    "kernel_operational_reference_core_v1_golden_fixture_sha256": GOLDEN_SHA256,
    "kernel_operational_reference_core_v1_decision_sha256": SEMANTIC_DECISION_SHA256,
    "kernel_operational_reference_core_v1_closure_sha256": CLOSURE_SHA256,
    "kernel_operational_reference_core_v1_python_manifest_sha256": PYTHON_MANIFEST_SHA256,
    "kernel_operational_reference_core_v1_source_manifest_sha256": CORE_MANIFEST_SHA256,
    "kernel_operational_reference_core_v1_go_manifest_sha256": GO_MANIFEST_SHA256,
    "kernel_operational_reference_core_v1_rust_module_manifest_sha256": RUST_MODULE_MANIFEST_SHA256,
}
REFERENCE_IMPLEMENTATIONS = {
    "kernel_operational_reference_core_v1_python": {
        "ref": PYTHON_PACKAGE,
        "projection": "source_distributed_dependency_free_strict_pure_contract",
    },
    "kernel_operational_reference_core_v1_python_checker": {
        "ref": CHECKER,
        "projection": "source_distributed_pinned_golden_or_explicit_file_checker",
    },
    "kernel_operational_reference_core_v1_go": {
        "ref": GO_PACKAGE,
        "projection": "catalyst_repository_only_cross_language_parity_not_scaffolded",
    },
    "kernel_operational_reference_core_v1_rust": {
        "ref": RUST_PACKAGE,
        "projection": "catalyst_repository_only_exact13_module_parity_with_unpinned_unique_registration_not_scaffolded",
    },
}
NON_CAPABILITY = (
    "ADR-0088/0089 only validate exact caller-supplied Kernel operational "
    "reference declarations and an acyclic closure; Registry v39 authenticates no "
    "principal, Grant, artifact content, source/context/environment/policy binding, "
    "authorization, permission, event append, persistence, transition, execution, "
    "outcome, completion, effect or usage measurement, adds no Skill, route, kind, "
    "evaluator, producer, service or runtime profile, copies no Catalyst Go or Rust "
    "parity or runtime registration, and does not complete CognitiveAtom, "
    "DecisionTransaction or the full Kernel ABI"
)
DETECTOR_ID = "governance.kernel_operational_reference_core_v1_candidate"
DETECTOR = {
    "argv": ["python3", CHECKER, "--golden", "."],
    "adapter": "standalone.kernelOperationalReferenceCoreV1PinnedGolden",
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
        issues.append(f"{path}: Kernel operational candidate requires Registry v39")
    if data.get("kernel_operational_reference_core_v1_candidate_contract") != CONTRACT:
        issues.append(f"{path}: Kernel operational candidate contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: Kernel operational candidate cannot expand Registry scope")
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
        issues.append(f"{path}: Kernel operational non-capability drifted")
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
        f"{PYTHON_PACKAGE}: exact eight-file package drifted"]
    issues.extend(_manifest_issues(
        repo_root, PYTHON_SHA256, PYTHON_MANIFEST_SHA256, "Kernel operational Python"))
    issues.extend(_manifest_issues(
        repo_root, CORE_SHA256, CORE_MANIFEST_SHA256, "Kernel operational source core"))
    return issues


def golden_issues(repo_root):
    try:
        closure = load_golden(repo_root)
    except (OSError, ContractError) as error:
        return [f"Kernel operational exact golden failed: {error}"]
    issues = []
    if closure.get("closure_sha256") != CLOSURE_SHA256:
        issues.append("Kernel operational closure semantic seal drifted")
    if closure.get("result") != SUCCESS_MARKER or closure.get("attestations") != ATTESTATIONS:
        issues.append("Kernel operational result or fourteen attestations drifted")
    return issues


def _optional_package_issues(repo_root, package, paths, aggregate, label):
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
        if label == "Catalyst Go parity":
            actual = {entry.name for entry in root.iterdir()}
            expected = set(paths)
        else:
            tests = _real_directory(repo_root, f"{package}/tests", label)
            top = {name for name in paths if "/" not in name}
            actual = {entry.name for entry in root.iterdir()}
            expected = top | {"tests"}
            test_actual = {entry.name for entry in tests.iterdir()}
            test_expected = {name.removeprefix("tests/") for name in paths
                             if name.startswith("tests/")}
            if test_actual != test_expected:
                return [f"{package}: {label} exact tests closure drifted"]
        if actual != expected:
            return [f"{package}: {label} exact lexical closure drifted"]
    except OSError as error:
        return [f"{package}: {label} unreadable: {error}"]
    mapped = {f"{package}/{name}": digest for name, digest in paths.items()}
    return _manifest_issues(repo_root, mapped, aggregate, label)
def go_parity_issues(repo_root):
    return _optional_package_issues(
        repo_root, GO_PACKAGE, GO_SHA256, GO_MANIFEST_SHA256, "Catalyst Go parity")
def rust_parity_issues(repo_root):
    issues = _optional_package_issues(
        repo_root, RUST_PACKAGE, RUST_SHA256, RUST_MODULE_MANIFEST_SHA256,
        "Catalyst Rust parity")
    try:
        (repo_root / RUST_PACKAGE).lstat()
        _real_directory(repo_root, RUST_REGISTRATION.rsplit("/", 1)[0],
                        "Catalyst Rust registration")
        raw = _read_regular(repo_root / RUST_REGISTRATION, RUST_REGISTRATION)
        if raw.decode("utf-8").splitlines().count(
                "pub mod kernel_operational_contract;") != 1:
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
        return ["Kernel operational checker-only detector is missing"]
    implementation = _mapping(item.get("implementation"))
    invocation, tests = _mapping(item.get("invocation")), _mapping(item.get("tests"))
    issues = []
    expected = {"argv": DETECTOR["argv"], "cwd": "repo_root", "shell": False}
    if implementation != expected:
        issues.append("Kernel operational pinned-golden detector argv drifted")
    expected = {"owner": "operator", "adapter": DETECTOR["adapter"],
                "acceptance_criterion": None, "load_bearing": False}
    if item.get("state") != "shadow" or invocation != expected:
        issues.append("Kernel operational detector must remain operator shadow only")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"Kernel operational detector {polarity} sentinel drifted")
    uses = [candidate for candidate in detectors.values() if CHECKER in
            (_mapping(candidate.get("implementation")).get("argv") or [])]
    if uses != [item]:
        issues.append("Kernel operational checker requires exactly one detector")
    return issues


def wiring_issues(agent_root):
    from agent_engineering.contract import EXTENSION_REFS
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    if a_error or d_error or r_error:
        return ["Kernel operational Agent Engineering wiring is unreadable"]
    extensions = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"Kernel operational activation ref {field} drifted" for field, value
              in CANONICAL_REFS.items() if extensions.get(field) != value or
              EXTENSION_REFS.get(field) != value]
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    issues.extend(f"Kernel operational contract asset missing: {value}" for value in
                  CANONICAL_REFS.values() if value not in assets)
    encoded = json.dumps(routes, sort_keys=True)
    forbidden = tuple(CANONICAL_REFS.values()) + (
        DETECTOR_ID, GO_PACKAGE, RUST_PACKAGE, "kernelOperationalReference")
    if any(token in encoded for token in forbidden):
        issues.append("Kernel operational candidate cannot enter a context route")
    if any("kernel_operational_reference" in field and field.endswith("_skill")
           for field in EXTENSION_REFS):
        issues.append("Kernel operational candidate cannot install a Skill")
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
        ("five semantic objects", "fourteen booleans", "full Kernel ABI"),
    )
    issues.extend(_adr_one(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v37", "exactly eighteen files", "remains unchecked"),
    ))
    return issues


def roadmap_issues(repo_root):
    sentinel = repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
    if not sentinel.is_file():
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
    return [] if entries == [expected] else [
        f"{relative}: narrow structural reference-family ABI must remain one exact completed item"]


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
