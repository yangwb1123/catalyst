"""ADR-0092/0093 Decision Capsule structural replay candidate governance."""
from __future__ import annotations
import hashlib
import json
import stat
from architecture_decision_record_v2 import validate_document_bytes
from decision_capsule_contract import SUCCESS_MARKER, load_golden
from decision_capsule_contract.constants import ATTESTATION_FIELDS
from engineering_check_support import (
    load_yaml,
    optional_package_manifest_issues,
    physical_manifest_issues,
    proposed_adr_issues,
    read_regular_file,
    real_directory,
    source_reference_closure_issues,
)
from .evidence_claim_portable import EXPECTED_SCOPE
SEMANTIC_DECISION = "docs/adr/ADR-0092-decision-capsule-structural-replay-core-v1.md"
GOVERNANCE_DECISION = "docs/adr/ADR-0093-decision-capsule-structural-replay-governance-and-source-distribution.md"
SCHEMA = "docs/contracts/decision-capsule-structural-replay-core-v1.schema.json"
GOLDEN = "docs/contracts/fixtures/decision-capsule-structural-replay-v1.json"
PYTHON_PACKAGE = "harness/decision_capsule_contract"
CHECKER = "harness/decision_capsule_contract_check.py"
GOVERNANCE_MODULE, GOVERNANCE_TEST = ("harness/governance_engineering/decision_capsule_structural_replay_candidate.py", "harness/governance_engineering/test_decision_capsule_structural_replay_candidate.py")
PYTHON_TESTS = ("harness/test_decision_capsule_contract.py", "harness/test_decision_capsule_replay_graph.py", "harness/test_decision_capsule_strict.py")
GO_PACKAGE = "forge-core/internal/decisioncapsulecontract"
RUST_PACKAGE = "forge-runtime/crates/domain/src/decision_capsule_contract"
RUST_REGISTRATION = "forge-runtime/crates/domain/src/lib.rs"
SCAFFOLD_STATE = ".agent/scaffold-state.json"
CATALYST_REPOSITORY_FLAVOR = "catalyst_source"
SCAFFOLDED_REPOSITORY_FLAVOR = "scaffolded_project"
ROUTE_INCLUSION_SHA256 = "ad2bab6f4ce95f37875f86ba9c547442c723b5688ad4922df395a8667c064d85"
SCOPE_SHA256 = "8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c"
SEMANTIC_DECISION_SHA256 = "89c5fb87b1abdd7f8b3fe3cf7bc8759aa43739dfad5f4c79102ba2a26bb4e54b"
SEMANTIC_DECISION_BODY_SHA256 = "f2b943ad8ec4eac2ac906f6b177313b8ca4a4cc36e64f3ca2f7240657543f820"
SEMANTIC_DECISION_SELF_SHA256 = "153df82413525b2700879a22d3656cbbf0bd34eb0c145ad9cff6e052292070a8"
GOVERNANCE_DECISION_SHA256 = "6e33f2262a4037a1abb474df3f55f057cb36d6e2ab4fb2d41802da902d11a6eb"
GOVERNANCE_DECISION_BODY_SHA256 = "062db58969ae99328012624293b1216deb74613d1b94ea9ba193b732d3baf6ea"
GOVERNANCE_DECISION_SELF_SHA256 = "45b099b8028b4a9b89fea4458bd73b845cd8dab6d3f6c953f707701cbb6ae315"
SCHEMA_SHA256 = "6145c150c8be7ee3934e9d93aec6ab89ddbe4cb6ba77a69b88d2e586616eae1f"
GOLDEN_SHA256 = "d54494f49851cc4146905bbd64c0815fe7d79704476c0aeb1113f270d5cbb2d0"
MANIFEST_SHA256 = "40d1fa34a2fc9b31856d3f16edd1cc346f47d0b447040539b667279f0f67365c"
CAPSULE_SHA256 = "f02c172fb5d65a36841361a9969dd8ad79eae08c548d1c6d0bbea5a564276b59"
BRANCH_SHA256 = "4442cf99caa21eda32a1c4062cfe66b333dff5188f4b818a9c69bf5cb829949a"
CLOSURE_SHA256 = "38f14574e9a9531371d55800f1f77bbdb79648a121c0f774a2a9c0083cf13497"
PYTHON_MANIFEST_SHA256 = "353c2cce2251225872fe672141e332a30a42eda2f6e37c9df423b246f351c2cc"
CORE_MANIFEST_SHA256 = "358b6e330789c123cbec6c23e51179f1b82de4ca88af0cb25e7c4ce1bf1cf45f"
GO_MANIFEST_SHA256 = "5afddbe8ba871c41c7d4a2ff820309be67c589ba56b8227d7e4e34c6d9f570af"
RUST_MODULE_MANIFEST_SHA256 = "6cab30aca92abe0ddd863bf5b9c88132f9010e466f65b9d83e65124635107caf"
PYTHON_SHA256 = {
    f"{PYTHON_PACKAGE}/__init__.py": "d6f1290e3a350ebb5b26baecb1d3d204daf1608ec764a23d4bc34f42b1baeeb6",
    f"{PYTHON_PACKAGE}/branch.py": "2d60f60e3bebe28a476e45a9690dcb1d26e0bd497b37ac443748a246d6aa200f",
    f"{PYTHON_PACKAGE}/capsule.py": "2134da3b01f726f42ae39ded2b97b0b11deb17d4a4e8d1060c8d53280421f368",
    f"{PYTHON_PACKAGE}/closure.py": "af44e9286ae92708f5bd0f428d1e05b5515fbed857fda37807390492229c3817",
    f"{PYTHON_PACKAGE}/codec.py": "66f322169b8e79e76ab06b64785d7970587e7a9ddcb946b0d91591021bd982c0",
    f"{PYTHON_PACKAGE}/constants.py": "e8861e4ac240fd7e1d66f9f9714c44851b96b35ff0efca44c607c09fa265ca37",
    f"{PYTHON_PACKAGE}/fixture.py": "cd16fac5ded67a19be9b73c78645780fb49b46b88f9aac7a555beef7a175d1cc",
    f"{PYTHON_PACKAGE}/manifest.py": "0f667a2ff6de512984c2cc54161e4b6a62552e7ca82e3634212238829bad28c1",
    f"{PYTHON_PACKAGE}/shape.py": "392abeb6bc6496d3d40d3fc60e7017cc76bc486480aad02d427195be8110d41d",
    CHECKER: "4680982f7e5c29a9df515a5672a73fbccd71dd75b544e5f75561f274bc1c31e0",
    PYTHON_TESTS[0]: "1477af13b229395c277f9c025134ea4c4e5ac5da9fd2696e6272bd9fd43afec2",
    PYTHON_TESTS[1]: "d75db786409da35558a1a950892d1ab7c3f8c04983346bc94a066bf6f06fe4ac",
    PYTHON_TESTS[2]: "862a32d02ccc92f57b37a7dffb85260b5ba6010110784df5a1a74cf46472d727",
}
CORE_SHA256 = {SEMANTIC_DECISION: SEMANTIC_DECISION_SHA256, SCHEMA: SCHEMA_SHA256,
               GOLDEN: GOLDEN_SHA256, **PYTHON_SHA256}
SCAFFOLD_OWNERS = frozenset((*CORE_SHA256, GOVERNANCE_DECISION, GOVERNANCE_MODULE, GOVERNANCE_TEST))
GO_SHA256 = {
    "attempt_test.go": "b958fc9f5ce74ffc957e8f9485b56dc744a93747e87398675fbdba04c6039129", "branch.go": "529b4752ddd2756ffed33aa2cde01dbf1049320195bb8c85fa7e4a95760510e9",
    "capsule.go": "2799cad32ab62b0c9504a14f56735b1a11855e0afa0ca814c3ac3a32183caa19", "closure.go": "2bd90366cc456265029ff5395d0c99dc70557aa420fc468398177ebb0614c950",
    "codec.go": "f5db1c3e2df52cc0c981d1d9a525179b7c9518fa82cdc11d179fb91082adf498",
    "constants.go": "577d87d87676d1d351c5d39a06767fc2ab310c284718346f55ecb6f6868eaa74", "contract_test.go": "502fc00270c30bb7a0a26032f6bf660581bb94fc1e3393357ac88e8a2ef29547",
    "envelope_test.go": "81c6489a4e6d6ef2db4abd1733879095bad853cc09fbc4896a72ac42a40cdfcd", "graph_test.go": "e7345301167a168c3e72ef6bac866279984d45baae2e6d81d8617a7369bcb505",
    "manifest.go": "f8fdd68e00995d8a7e73bec7fc8d9a8536cff43503b426f4975fe0d22d49e080",
    "measure.go": "9fe730e577fb0f5470762f47422f1f3c824cfba95f7e38aaba523cde944b5d7d", "model.go": "3cd7c0ed93ecbc4027537a297123dac360e0bee116c9ff076f3dcb5bfa6936b3",
    "primitives.go": "3de937afd06ce5f3d10ebb191a00b3c43126c292ecb3e34e31cf86bc0b234377",
    "public_api_test.go": "181ab7e6357e788a44705166f3c30aef2baff7a5b9caa880613d16fa3bf0764a",
    "strict_test.go": "d935c2464555aa9c0e7a824b6b65a693ecabe3fd2e848f82fb42ce4fda22a66e",
}
RUST_SHA256 = {
    "branch.rs": "63816f9a9406769ce2c5af55fe9f1dc5cfc4f93fe7de9d2788b93f4d1d3adc51", "capsule.rs": "e75ee98a3060b7be8ec2c24fa192ef27136d11bcfbd9af476970e711be825ea4",
    "closure.rs": "8fb03b3d3ec2551512744c6168d5e671186160e5e04745a46fcd8bcfb7ce6c73", "manifest.rs": "b80d00acaffa10a505ca6af1b3b54eae2cc1464fee4a781e46ca98f08e13cd35",
    "mod.rs": "e3f692e91d7c175deb56e82a180280f4671c3f3e3c1e351dea391204a6aaab17",
    "model.rs": "f42ea5ef65aa60542dc31adbd65e9ffe46b06432885ecc17ee55e6a47f20f7ca", "primitives.rs": "bc47e3593529e6929f1afedc73e4f0de8218fd641c5dfa81a284a8999dcd40fd",
    "profile.rs": "bd6ad6d67591c39526764262ad3d22fbe9bbda0473d63126aac52e4563380128", "profile_key.rs": "686b98bea60510b50382da1902b526795d338d7a7353ea602551410657b58963",
    "tests/contract.rs": "e63a121a404424b14cd1e8596e563304bc1004751d194c97a10eb5261ed43288", "tests/graph.rs": "4878b5c5120919afa6705dba5366a14c13634fe533cdefa3211b9cde36f40677",
    "tests/mod.rs": "963c118047d2a75b2d2f00a428038ab356418114192f636de741ec2a6e15df41",
    "tests/strict.rs": "8a16dbafa848ad55678bb2e54917bcbb0c22f79c66e10d37805a8935d7f5d3bb",
    "wire.rs": "d6c016878884ad779fce55cd49ff00eab7b66759cf7662cc31415a8dc155a87d",
}
RUNTIME_REFERENCE_TOKENS = ("decision_capsule_contract", "decisioncapsulecontract")
RUNTIME_REFERENCE_ALLOWED_COUNTS = {
    **{f"{GO_PACKAGE}/{name}": (0, 1) for name in GO_SHA256},
    f"{GO_PACKAGE}/public_api_test.go": (0, 2),
    **{f"{RUST_PACKAGE}/{name}": (0, count) for name, count in {
        "branch.rs": 13, "capsule.rs": 12, "closure.rs": 10, "manifest.rs": 19,
        "mod.rs": 5, "primitives.rs": 6, "profile.rs": 3,
        "tests/contract.rs": 2, "tests/strict.rs": 1, "wire.rs": 7,
    }.items()},
    RUST_REGISTRATION: (1, 0), CHECKER: (3, 0),
    PYTHON_TESTS[0]: (13, 1), PYTHON_TESTS[1]: (4, 0), PYTHON_TESTS[2]: (19, 0),
    GOVERNANCE_MODULE: (9, 2), GOVERNANCE_TEST: (7, 1),
    "harness/agent_engineering/contract.py": (1, 0),
    "harness/agent_engineering/support.py": (1, 1),
    "harness/scaffold/decision-capsule-structural-replay-copy-fragment.mjs": (3, 0),
    "harness/scaffold/decision-capsule-structural-replay-upgrade-verification.mjs": (8, 1),
    "harness/scaffold/decision-capsule-structural-replay-v38-projection.mjs": (2, 0),
    "harness/scaffold/test_decision-capsule-structural-replay-upgrade-verification.mjs": (4, 1),
    "harness/scaffold/test_upgrade_transaction_authority.mjs": (2, 0),
    "harness/scaffold/test_upgrade_transaction_recovery.mjs": (1, 0),
    "harness/test_governance_engineering_integration.py": (1, 0),
}
ATTESTATIONS = {name: False for name in ATTESTATION_FIELDS}
CONTRACT = {
    "profile_id": "decision_capsule_structural_replay_core_v1",
    "decision_status": "proposed",
    "delivery": ("source_distributed_dependency_free_python_pure_contract_with_"
                 "catalyst_repository_only_go_and_rust_parity"),
    "mode": "exact_caller_supplied_decision_capsule_structural_replay_closure_validation",
    "identity": {
        "four_object_one_way_structural_replay_dag": True,
        "complete_ordered_manifest_projection": True,
        "separately_sealed_compare_only_branch": True,
        "dedicated_reflection_report_refs_are_unresolved_and_outer_attached": True,
        "domain_separated_object_seals": True,
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
        "rust_pin_scope": "exact14_nested_module_sources_and_tests",
        "exact_three_language_golden": True,
    },
    "source_distribution": {
        "copies_exact_sixteen_file_core_and_three_file_governance": True,
        "copies_go_rust_or_runtime_registration": False,
        "installs_skill_route_adapter_service_product_command_pdp_controller_or_reflection_runtime": False,
        "adds_registry_scope_kind_evaluator_producer_or_runtime_profile": False,
    },
    "completion": {
        "decision_capsule_structural_replay_repository_slice_complete": False,
        "broader_adr_0038_complete": False,
        "decision_capsule_complete": False,
        "authorized_transaction_spec_complete": False,
        "authenticated_pdp_complete": False,
        "controller_complete": False,
        "authority_promotion_complete": False,
    },
    "effective_semantics": {
        "comparison": "exact_structural_reference_match_only",
        "effect_replay_allowed": False,
        "history_rewrite_allowed": False,
        "instruction_allowed": False,
    },
    "positive_result": SUCCESS_MARKER,
    "attestations": ATTESTATIONS,
    "persistence": "none",
}
CANONICAL_REFS = {
    "decision_capsule_structural_replay_core_v1_schema": SCHEMA,
    "decision_capsule_structural_replay_core_v1_golden_fixture": GOLDEN,
    "decision_capsule_structural_replay_core_v1_checker": CHECKER,
    "decision_capsule_structural_replay_core_v1_semantic_decision": SEMANTIC_DECISION,
    "decision_capsule_structural_replay_core_v1_governance_decision": GOVERNANCE_DECISION,
}
PINS = {
    "decision_capsule_structural_replay_core_v1_schema_sha256": SCHEMA_SHA256,
    "decision_capsule_structural_replay_core_v1_golden_fixture_sha256": GOLDEN_SHA256,
    "decision_capsule_structural_replay_core_v1_decision_sha256": SEMANTIC_DECISION_SHA256,
    "decision_capsule_structural_replay_core_v1_manifest_sha256": MANIFEST_SHA256,
    "decision_capsule_structural_replay_core_v1_capsule_sha256": CAPSULE_SHA256,
    "decision_capsule_structural_replay_core_v1_branch_sha256": BRANCH_SHA256,
    "decision_capsule_structural_replay_core_v1_closure_sha256": CLOSURE_SHA256,
    "decision_capsule_structural_replay_core_v1_python_manifest_sha256": PYTHON_MANIFEST_SHA256,
    "decision_capsule_structural_replay_core_v1_source_manifest_sha256": CORE_MANIFEST_SHA256,
    "decision_capsule_structural_replay_core_v1_go_manifest_sha256": GO_MANIFEST_SHA256,
    "decision_capsule_structural_replay_core_v1_rust_module_manifest_sha256": RUST_MODULE_MANIFEST_SHA256,
}
REFERENCE_IMPLEMENTATIONS = {
    "decision_capsule_structural_replay_core_v1_python": {
        "ref": PYTHON_PACKAGE,
        "projection": "source_distributed_dependency_free_strict_pure_contract",
    },
    "decision_capsule_structural_replay_core_v1_python_checker": {
        "ref": CHECKER,
        "projection": "source_distributed_pinned_golden_or_explicit_file_checker",
    },
    "decision_capsule_structural_replay_core_v1_go": {
        "ref": GO_PACKAGE,
        "projection": "catalyst_repository_only_exact15_cross_language_parity_not_scaffolded",
    },
    "decision_capsule_structural_replay_core_v1_rust": {
        "ref": RUST_PACKAGE,
        "projection": "catalyst_repository_only_exact14_nested_module_parity_with_unpinned_unique_registration_not_scaffolded",
    },
}
NON_CAPABILITY = (
    "ADR-0092/0093 only validate an exact caller-supplied Decision Capsule "
    "structural replay DAG; Registry v39 keeps both replay controls and all "
    "thirty-two attestations false, resolves no external history or Reflection "
    "report, evaluates no model, rule or world state, authenticates no source, "
    "principal, authority, Approval, Grant, result or binding, performs no "
    "authorization, CAS, event append, persistence, transition, execution, "
    "outcome, completion, effect or usage measurement, adds no Skill, route, "
    "kind, evaluator, producer, service, PDP, controller, Reflection runtime or "
    "runtime profile, copies no Catalyst Go or Rust parity or registration, and "
    "stages only a structural replay repository candidate while ADR-0038, "
    "DecisionCapsule and AuthorizedTransactionSpec remain incomplete"
)
DETECTOR_ID = "governance.decision_capsule_structural_replay_core_v1_candidate"
DETECTOR = {
    "argv": ["python3", CHECKER, "--golden", "."],
    "adapter": "standalone.decisionCapsuleStructuralReplayCoreV1PinnedGolden",
    "positive": "test_registry_is_v39_scope_neutral_structural_candidate",
    "negative": "test_scope_argv_authority_completion_and_distribution_drift_fail_closed",
}
def _mapping(value): return value if isinstance(value, dict) else {}
def _repository_flavor(repo_root):
    data, error = load_yaml(repo_root / ".agent/project.yml")
    if error or not isinstance(data, dict): return None
    flavor = data.get("repository_flavor")
    try:
        (repo_root / SCAFFOLD_STATE).lstat()
        state_present = True
    except FileNotFoundError:
        state_present = False
    except OSError:
        return None
    if flavor == CATALYST_REPOSITORY_FLAVOR and not state_present: return CATALYST_REPOSITORY_FLAVOR
    if flavor == SCAFFOLDED_REPOSITORY_FLAVOR and state_present: return SCAFFOLDED_REPOSITORY_FLAVOR
    return None
def repository_flavor_issues(repo_root):
    flavor = _repository_flavor(repo_root)
    if flavor is None: return ["repository flavor requires explicit Catalyst identity or scaffold ledger"]
    if flavor == CATALYST_REPOSITORY_FLAVOR: return []
    try:
        raw = read_regular_file(repo_root / SCAFFOLD_STATE, SCAFFOLD_STATE,
                                16 * 1024 * 1024, (0o600, 0o640, 0o644))
        state = json.loads(raw)
    except (OSError, UnicodeError, ValueError) as error:
        return [f"{SCAFFOLD_STATE}: scaffold repository identity unreadable: {error}"]
    if (not isinstance(state, dict) or set(state) != {"version", "copied"}
            or state.get("version") != 1 or not isinstance(state.get("copied"), list)):
        return [f"{SCAFFOLD_STATE}: scaffold repository identity drifted"]
    copied = state["copied"]
    canonical = lambda value: (isinstance(value, str) and value and "\\" not in value and
                               "\0" not in value and not value.startswith("/") and all(part not in
                               ("", ".", "..") and ":" not in part and not part.endswith(
                                   (" ", ".")) for part in value.split("/")))
    if any(not canonical(value) for value in copied) or len(copied) != len(set(copied)):
        return [f"{SCAFFOLD_STATE}: copied paths must be canonical and unique"]
    missing = sorted(SCAFFOLD_OWNERS - set(copied))
    if missing: return [f"{SCAFFOLD_STATE}: exact19 scaffold identity missing {', '.join(missing)}"]
    return []
def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: Decision Capsule candidate requires Registry v39")
    if data.get("decision_capsule_structural_replay_core_v1_candidate_contract") != CONTRACT:
        issues.append(f"{path}: Decision Capsule candidate contract drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: Decision Capsule candidate cannot expand Registry scope")
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
        issues.append(f"{path}: Decision Capsule non-capability drifted")
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
    issues.extend(physical_manifest_issues(
        repo_root, PYTHON_SHA256, PYTHON_MANIFEST_SHA256, "Decision Capsule Python"))
    issues.extend(physical_manifest_issues(
        repo_root, CORE_SHA256, CORE_MANIFEST_SHA256, "Decision Capsule source core"))
    return issues
def golden_issues(repo_root):
    try:
        closure = load_golden(repo_root)
    except (OSError, ValueError) as error:
        return [f"Decision Capsule exact golden failed: {error}"]
    capsule, branch = closure.get("decision_capsule", {}), closure.get("evaluation_branch", {})
    manifest = capsule.get("replay_manifest", {}) if isinstance(capsule, dict) else {}
    expected = ((manifest, "manifest_sha256", MANIFEST_SHA256),
                (capsule, "capsule_sha256", CAPSULE_SHA256),
                (branch, "branch_sha256", BRANCH_SHA256),
                (closure, "closure_sha256", CLOSURE_SHA256))
    issues = [f"Decision Capsule {field} semantic seal drifted" for owner, field, value
              in expected if _mapping(owner).get(field) != value]
    if closure.get("result") != SUCCESS_MARKER or closure.get("attestations") != ATTESTATIONS:
        issues.append("Decision Capsule result or thirty-two attestations drifted")
    return issues
def go_parity_issues(repo_root):
    return optional_package_manifest_issues(
        repo_root, GO_PACKAGE, GO_SHA256, GO_MANIFEST_SHA256,
        "Catalyst Go parity", _repository_flavor(repo_root),
        CATALYST_REPOSITORY_FLAVOR, SCAFFOLDED_REPOSITORY_FLAVOR)
def rust_parity_issues(repo_root):
    issues = optional_package_manifest_issues(
        repo_root, RUST_PACKAGE, RUST_SHA256, RUST_MODULE_MANIFEST_SHA256,
        "Catalyst Rust parity", _repository_flavor(repo_root),
        CATALYST_REPOSITORY_FLAVOR, SCAFFOLDED_REPOSITORY_FLAVOR)
    flavor, registration = _repository_flavor(repo_root), repo_root / RUST_REGISTRATION
    if flavor == SCAFFOLDED_REPOSITORY_FLAVOR:
        try: raw = read_regular_file(registration, RUST_REGISTRATION)
        except FileNotFoundError: return issues
        except (OSError, ValueError) as error:
            return issues + [f"{RUST_REGISTRATION}: registration unreadable: {error}"]
        if b"pub mod decision_capsule_contract;" in raw.splitlines():
            issues.append(f"{RUST_REGISTRATION}: scaffold cannot register Catalyst parity")
        return issues
    try:
        (repo_root / RUST_PACKAGE).lstat()
        real_directory(repo_root, RUST_REGISTRATION.rsplit("/", 1)[0],
                       "Catalyst Rust registration")
        raw = read_regular_file(repo_root / RUST_REGISTRATION, RUST_REGISTRATION)
        if raw.decode("utf-8").splitlines().count("pub mod decision_capsule_contract;") != 1:
            issues.append(f"{RUST_REGISTRATION}: exact unique registration drifted")
    except (OSError, UnicodeError) as error:
        if flavor == CATALYST_REPOSITORY_FLAVOR: issues.append(
            f"{RUST_REGISTRATION}: registration unreadable: {error}")
    return issues
def runtime_reference_issues(repo_root):
    allowed = RUNTIME_REFERENCE_ALLOWED_COUNTS
    if _repository_flavor(repo_root) == SCAFFOLDED_REPOSITORY_FLAVOR:
        prefixes = (f"{GO_PACKAGE}/", f"{RUST_PACKAGE}/", "harness/scaffold/")
        allowed = {relative: counts for relative, counts in allowed.items()
                   if relative != RUST_REGISTRATION
                   and not relative.startswith(prefixes)}
    return source_reference_closure_issues(
        repo_root, RUNTIME_REFERENCE_TOKENS, allowed,
        "Decision Capsule")
def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    item = detectors.get(DETECTOR_ID)
    if not isinstance(item, dict):
        return ["Decision Capsule checker-only detector is missing"]
    implementation = _mapping(item.get("implementation"))
    invocation, tests = _mapping(item.get("invocation")), _mapping(item.get("tests"))
    issues = []
    if implementation != {"argv": DETECTOR["argv"], "cwd": "repo_root", "shell": False}:
        issues.append("Decision Capsule pinned-golden detector argv drifted")
    expected = {"owner": "operator", "adapter": DETECTOR["adapter"],
                "acceptance_criterion": None, "load_bearing": False}
    if item.get("state") != "shadow" or invocation != expected:
        issues.append("Decision Capsule detector must remain operator shadow only")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"Decision Capsule detector {polarity} sentinel drifted")
    uses = [candidate for candidate in detectors.values() if CHECKER in
            (_mapping(candidate.get("implementation")).get("argv") or [])]
    if uses != [item]:
        issues.append("Decision Capsule checker requires exactly one detector")
    return issues
def _skill_inventory_issues(repo_root, agent_root):
    issues = []
    for root, label in ((agent_root / "skills", ".agent/skills"),
                        (repo_root / "skills", "skills")):
        if not root.exists(): continue
        for path in root.rglob("*"):
            if stat.S_ISDIR(path.lstat().st_mode): continue
            relative = f"{label}/{path.relative_to(root).as_posix()}"
            try: raw = read_regular_file(path, relative, 1024 * 1024, (0o600, 0o640, 0o644, 0o664, 0o755))
            except (OSError, ValueError) as error:
                issues.append(f"{relative}: Skill inventory unreadable: {error}"); continue
            text = f"{relative}\n{raw.decode('utf-8', errors='replace')}".lower()
            normalized = "_".join("".join(c if c.isalnum() else " " for c in text).split())
            if ("decision_capsule_structural_replay" in normalized or any(token.lower() in text for
                    token in (*CANONICAL_REFS.values(), DETECTOR_ID, CHECKER))):
                issues.append(f"{relative}: Decision Capsule candidate cannot install a Skill")
    return issues
def wiring_issues(agent_root, repo_root=None):
    from agent_engineering.contract import EXTENSION_REFS
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    if a_error or d_error or r_error:
        return ["Decision Capsule Agent Engineering wiring is unreadable"]
    extensions = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"Decision Capsule activation ref {field} drifted" for field, value
              in CANONICAL_REFS.items() if extensions.get(field) != value or
              EXTENSION_REFS.get(field) != value]
    if extensions != EXTENSION_REFS:
        issues.append("Decision Capsule activation projection drifted")
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    issues.extend(f"Decision Capsule contract asset missing: {value}" for value in
                  CANONICAL_REFS.values() if value not in assets)
    route_projection = {
        "base_required": _mapping(routes.get("selection")).get("base_required"),
        "routes": [{"id": item.get("id"), "include": item.get("include", [])}
                   for item in routes.get("routes") or [] if isinstance(item, dict)],
    }
    encoded_projection = json.dumps(
        route_projection, sort_keys=True, separators=(",", ":")).encode()
    if hashlib.sha256(encoded_projection).hexdigest() != ROUTE_INCLUSION_SHA256:
        issues.append("Decision Capsule context-route inclusion projection drifted")
    encoded = json.dumps(routes, sort_keys=True)
    forbidden = tuple(CANONICAL_REFS.values()) + (
        DETECTOR_ID, GO_PACKAGE, RUST_PACKAGE, PYTHON_PACKAGE,
        "decisionCapsuleStructuralReplay")
    if any(token in encoded for token in forbidden):
        issues.append("Decision Capsule candidate cannot enter a context route")
    if any("decision_capsule_structural_replay" in field and field.endswith("_skill")
           for field in EXTENSION_REFS):
        issues.append("Decision Capsule candidate cannot install a Skill")
    issues.extend(_skill_inventory_issues(repo_root or agent_root.parent, agent_root))
    return issues
def adr_issues(repo_root):
    issues = proposed_adr_issues(
        repo_root, SEMANTIC_DECISION, SEMANTIC_DECISION_SHA256,
        SEMANTIC_DECISION_BODY_SHA256, SEMANTIC_DECISION_SELF_SHA256,
        ("four-object", "thirty-two", "ReflectionReport"), validate_document_bytes,
    )
    issues.extend(proposed_adr_issues(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v39", "exactly nineteen files", "ADOPTED-PARTIAL"),
        validate_document_bytes,
    ))
    return issues
def roadmap_issues(repo_root):
    if _repository_flavor(repo_root) != CATALYST_REPOSITORY_FLAVOR:
        return []
    relative = "docs/design/ai-engineering-os/implementation-roadmap.md"
    expected = (
        "- [ ] 交付 Decision Capsule structural replay repository slice（structural only）："
        "分发 ADR-0092 四对象 pure validate/reseal/compare closure；"
    )
    try:
        text = read_regular_file(repo_root / relative, relative).decode()
    except (OSError, ValueError, UnicodeDecodeError) as error:
        return [f"{relative}: Decision Capsule roadmap unreadable: {error}"]
    entries = [line for line in text.splitlines()
               if "Decision Capsule structural replay repository slice" in line]
    issues = [] if entries == [expected] else [
        f"{relative}: Decision Capsule structural replay item must remain one exact pending item"]
    required_open = ("完成 Governance Kernel/PDP", "DecisionCapsule 与 AuthorizedTransactionSpec",
                     "DecisionTransaction 与有界 rolling controller")
    if any(not any(line.startswith("- [ ]") and marker in line
                   for line in text.splitlines()) for marker in required_open):
        issues.append(f"{relative}: broader DecisionCapsule/AuthorizedSpec/PDP/controller work must remain open")
    return issues
def integration_issues(data, path, repo_root, agent_root):
    return (registry_issues(data, path) + repository_flavor_issues(repo_root) +
            core_artifact_issues(repo_root) + golden_issues(repo_root) +
            go_parity_issues(repo_root) + rust_parity_issues(repo_root) +
            runtime_reference_issues(repo_root) +
            detector_issues(agent_root) + wiring_issues(agent_root, repo_root) +
            adr_issues(repo_root) + roadmap_issues(repo_root))
