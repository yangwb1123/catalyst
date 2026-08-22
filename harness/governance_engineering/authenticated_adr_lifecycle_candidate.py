"""ADR-0081/0082/0083 authority-evidence and lifecycle candidate governance."""

from __future__ import annotations

import hashlib
import json
import stat

from architecture_decision_record_v2 import (
    ContractError as ADRContractError,
    validate_document_file,
)
from authenticated_adr_lifecycle_contract import (
    ContractError as LifecycleContractError,
    SUCCESS_MARKER,
    load_golden,
)
from engineering_check_support import load_yaml
from governance_contract import ContractError, read_bounded_file

from .evidence_claim_portable import EXPECTED_SCOPE

APPROVAL_AUTHORITY_DECISION = (
    "docs/adr/ADR-0081-authenticated-architecture-decision-approval-"
    "authorization-service-v1.md"
)
SEMANTIC_DECISION = (
    "docs/adr/ADR-0082-authenticated-architecture-decision-lifecycle-v1-"
    "prerequisite.md"
)
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0083-authenticated-architecture-decision-lifecycle-v1-"
    "proposed-candidate-governance-and-source-distribution.md"
)
SCHEMA = "docs/contracts/authenticated-architecture-decision-lifecycle-v1.schema.json"
GOLDEN = (
    "docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json"
)
PROPOSALS = (
    "docs/contracts/fixtures/ADR-9003-lifecycle-head-a.md",
    "docs/contracts/fixtures/ADR-9004-lifecycle-head-b.md",
    "docs/contracts/fixtures/ADR-9005-lifecycle-join.md",
)
CHECKER = "harness/authenticated_adr_lifecycle_contract_check.py"
SCOPE_SHA256 = "8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c"
SCHEMA_SHA256 = "17f0f3f79680fd5d7825f574cc20f279f1fc9061ab33a73ef2e86e075d59bcf1"
GOLDEN_SHA256 = "47f8ceb9c4362f37fe5c48e17342a9ec3bedbb9ccfb87b585cabd3aa7c71dccb"
PROPOSAL_SHA256 = (
    "6c9cd0e4b95c968bb280d51b72d74f08a79620077d72611b5634a77b181a0a0b",
    "a76e566c7e18801dbc70c42b8b04ce9190cbc3d18892c80cd40c2ff4ec448bf0",
    "c96d2ef2db3311c16572ed5d753c2435193ab58a1abb7c1fc8a9c45d4d9c5dee",
)
APPROVAL_AUTHORITY_DECISION_SHA256 = (
    "e5a8742a3f49757151ade8df8637ed7fdb9f8d5af1cbbe236e18f474982336bd"
)
APPROVAL_AUTHORITY_DECISION_BODY_SHA256 = (
    "4e73ee42144b37651e77ac53847e43af21d50deb997528283fc003b74825ae2f"
)
APPROVAL_AUTHORITY_DECISION_SELF_SHA256 = (
    "ace1e54255a9e4f4c1a83e357997c28434fa44b9530f4c8dd27e6e0c16637483"
)
SEMANTIC_DECISION_SHA256 = (
    "ab19a1135829432eed1984e859681a9dc84372d9178431afda59df0653d06298"
)
SEMANTIC_DECISION_BODY_SHA256 = (
    "eedf743692fb721825e2ccbbdbbd06fc3c0ac67ee25469c003b51231c376b592"
)
SEMANTIC_DECISION_SELF_SHA256 = (
    "cbc02e51661cc5e195aa63b44af1eef34693a966f0aabfa3eef3d476a15bfce9"
)
GOVERNANCE_DECISION_SHA256 = (
    "bb79f21073d3d972f2b4493173d64056f915327dc03b9ca8f7497c2bc98e598e"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "ed0b0c467118595719654928f963f18d4d740f41a369fd5fa23f61d5279ec533"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "205765efff8bada13dbb28fd0fbe9f73c7ef088713b14a14598b1db56ba9ab1f"
)

CORE_SHA256 = {
    SEMANTIC_DECISION: SEMANTIC_DECISION_SHA256,
    SCHEMA: SCHEMA_SHA256,
    GOLDEN: GOLDEN_SHA256,
    PROPOSALS[0]: PROPOSAL_SHA256[0],
    PROPOSALS[1]: PROPOSAL_SHA256[1],
    PROPOSALS[2]: PROPOSAL_SHA256[2],
    "harness/authenticated_adr_lifecycle_contract/__init__.py":
        "3c385a41796cd8086712a7bb0725955e7751e050aa0afbb2a7883bf42da434d9",
    "harness/authenticated_adr_lifecycle_contract/authority.py":
        "b202f30e18c4a17ee0998989139c2c4f5f54239d82f625473b6cb60d7a963d7e",
    "harness/authenticated_adr_lifecycle_contract/canonical.py":
        "c4fa3d2fdae0a0ade62877bbf0a607eb5095fcf213d20ee7b0879c6646f67e7a",
    "harness/authenticated_adr_lifecycle_contract/constants.py":
        "d0c9de1dbd66e6dc0ba08548bd8d1fd5692e653121c32b35593943c0b18736d4",
    "harness/authenticated_adr_lifecycle_contract/contract.py":
        "78b011b1941d59abb23d5a23150256f280b6e5fbbf2aa6052d4b122c245293a0",
    "harness/authenticated_adr_lifecycle_contract/documents.py":
        "eb7859ce70e180c1077a33caafe3c559ed1c6978272b4b4c5924cf82f6bdc993",
    "harness/authenticated_adr_lifecycle_contract/fixture.py":
        "3784171264d402d7abc9a1cd85550c05c5257acfc1106c567efc36565068eed4",
    "harness/authenticated_adr_lifecycle_contract/ledger.py":
        "c0200840a908e354cfd1301676f44956af3362c2b21dc098f75dd86de5d7a1c7",
    "harness/authenticated_adr_lifecycle_contract/prerequisite.py":
        "b3e828654f6566b4a2a8f1edbffd379f3805450dbcb7ac536a9d3016a40c5794",
    "harness/authenticated_adr_lifecycle_contract/proposal.py":
        "23ce021f9c9bebd9b824b500035950cfeebba7625ae3812ec523af846c275143",
    "harness/authenticated_adr_lifecycle_contract/shape.py":
        "9ec3814dc051f4770fdf8f5d4cc59900dd589a045b28c233d1f6d3fb00cc1b82",
    "harness/authenticated_adr_lifecycle_contract/state.py":
        "68d8770bf8ce80a4a920f97e643c6f410b64c9633940c7aa498e9ae6fa572315",
    CHECKER: "22e6f4c9561a4726f3c208479e255d09039d7524a7d7f42ea8eee288c39c6626",
    "harness/test_authenticated_adr_lifecycle_contract.py":
        "5f0b40332a70a74548402b2163ba3269eabdfa732312aabaedcd976fe8aed543",
}

GO_AUTHORITY_EVIDENCE = {
    "profile_id": "authenticated_architecture_decision_approval_v1",
    "decision_status": "proposed",
    "delivery": "catalyst_repository_only_go_authority_evidence_not_source_distributed",
    "semantic_authority": "ADR-0081_only",
    "implementation": {
        "contract": "forge-core/internal/authenticatedadrapprovalcontract",
        "authority": "forge-core/internal/authenticatedadrapprovalauthority",
        "language_and_dependencies": "go_standard_library_only",
        "availability": "catalyst_repository_internal_api_only_not_scaffolded",
    },
    "evidence": {
        "exact_v1_structural_parity": True,
        "external_root_epoch_time_and_revocation_pin_consumption": True,
        "ed25519_policy_sod_request_receipt_and_ledger_verification": True,
        "authorization_decision_and_state_signing": True,
        "durable_cas_sync_atomic_rename_and_authenticated_reopen": True,
        "exact_idempotent_replay_without_resigning_or_rewrite": True,
        "stored_authorization_required_for_prerequisite_projection": True,
    },
    "boundary": {
        "production_root_key_state_clock_revocation_or_transport_supplied": False,
        "copies_go_contract_or_authority": False,
        "copies_production_keys_or_state": False,
        "adds_command_socket_route_registry_scope_or_runtime_profile": False,
        "adr_acceptance_rejection_supersession_or_repository_mutation": False,
        "lifecycle_permission_effect_or_g0_closure": False,
        "administrator_rollback_resistance": False,
    },
    "positive_evidence": (
        "PROPOSED_CATALYST_ONLY_AUTHENTICATED_ADR_APPROVAL_V1_GO_AUTHORITY "
        "(authenticated only relative to explicit external trust inputs and "
        "opaque StoredAuthorization; no production provisioning, lifecycle "
        "transition, repository mutation, route, runtime registration, scaffold "
        "distribution, permission, effect, or G0 attestation)"
    ),
    "attests": [],
    "persistence": (
        "repository_external_authority_state_only_when_explicitly_provisioned_"
        "by_an_in_process_caller"
    ),
}

LIFECYCLE_CANDIDATE_CONTRACT = {
    "profile_id": "authenticated_architecture_decision_lifecycle_v1",
    "decision_status": "proposed",
    "delivery": (
        "proposed_structural_prerequisite_candidate_with_source_only_python_"
        "distribution"
    ),
    "mode": "exact_caller_supplied_offline_structural_validation_only",
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "proposal_head_a_physical_sha256": PROPOSAL_SHA256[0],
        "proposal_head_b_physical_sha256": PROPOSAL_SHA256[1],
        "proposal_join_physical_sha256": PROPOSAL_SHA256[2],
        "schema_alone_sufficient": False,
    },
    "candidate_implementation": {
        "python": "dependency_free_strict_structural_core_and_golden_checker",
        "go_lifecycle_authority": "absent_not_scaffolded",
        "exact_golden_reconstruction": True,
    },
    "source_distribution": {
        "copies_python_structural_core_and_checker": True,
        "copies_schema_golden_proposals_and_proposed_decisions": True,
        "reuses_authenticated_approval_python_prerequisite_closure": True,
        "copies_go_contract_or_authority": False,
        "copies_production_keys_or_state": False,
        "installs_skill_or_adapter": False,
        "adds_authenticated_route": False,
        "adds_kind_evaluator_producer_or_runtime_profile": False,
    },
    "authority_semantics": {
        "ed25519_or_signature_verification": False,
        "external_root_epoch_time_or_revocation_currentness": False,
        "authorization_or_stored_authorization_authentication": False,
        "adr_acceptance_rejection_or_supersession_execution": False,
        "repository_source_mutation_or_accepted_document_generation": False,
        "atomicity_cas_durability_or_rollback_resistance": False,
        "architecture_compliance_or_immutability_enforcement": False,
        "semantic_authority_permission_effect_or_g0_closure": False,
        "persistence_execution_or_effect": False,
    },
    "positive_result": SUCCESS_MARKER,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "authenticated_adr_approval_v1_go_authority_decision": APPROVAL_AUTHORITY_DECISION,
    "authenticated_adr_lifecycle_v1_schema": SCHEMA,
    "authenticated_adr_lifecycle_v1_golden_fixture": GOLDEN,
    "authenticated_adr_lifecycle_v1_proposal_head_a_fixture": PROPOSALS[0],
    "authenticated_adr_lifecycle_v1_proposal_head_b_fixture": PROPOSALS[1],
    "authenticated_adr_lifecycle_v1_proposal_join_fixture": PROPOSALS[2],
    "authenticated_adr_lifecycle_v1_checker": CHECKER,
    "authenticated_adr_lifecycle_v1_semantic_decision": SEMANTIC_DECISION,
    "authenticated_adr_lifecycle_v1_candidate_governance_decision": GOVERNANCE_DECISION,
}
REFERENCE_IMPLEMENTATIONS = {
    "authenticated_adr_approval_v1_go_contract": {
        "ref": "forge-core/internal/authenticatedadrapprovalcontract",
        "projection": "catalyst_repository_only_proposed_exact_structural_contract_not_scaffolded",
    },
    "authenticated_adr_approval_v1_go_authority": {
        "ref": "forge-core/internal/authenticatedadrapprovalauthority",
        "projection": "catalyst_repository_only_proposed_external_trust_authenticated_authorization_and_durable_store_not_scaffolded",
    },
    "authenticated_adr_lifecycle_v1_python": {
        "ref": "harness/authenticated_adr_lifecycle_contract",
        "projection": "source_distributed_dependency_free_strict_proposed_structural_prerequisite_core_and_golden_checker",
    },
}
PINS = dict(zip((
    "authenticated_adr_lifecycle_v1_schema_sha256",
    "authenticated_adr_lifecycle_v1_golden_fixture_sha256",
    "authenticated_adr_lifecycle_v1_proposal_head_a_fixture_sha256",
    "authenticated_adr_lifecycle_v1_proposal_head_b_fixture_sha256",
    "authenticated_adr_lifecycle_v1_proposal_join_fixture_sha256",
), (SCHEMA_SHA256, GOLDEN_SHA256, *PROPOSAL_SHA256)))
GO_NON_CAPABILITY = (
    "ADR-0081 records a Proposed Catalyst-repository-only Go approval authority "
    "that can authenticate and durably store authorization relative to explicit "
    "external trust inputs, but Registry evidence does not provision a production "
    "root, key, state, clock, revocation publisher or transport, register a command, "
    "socket, route, scope or runtime profile, accept or mutate an ADR, close G0, or "
    "copy any Go authority, production key or state into source distribution"
)
LIFECYCLE_NON_CAPABILITY = (
    "Authenticated Architecture Decision Lifecycle v1 is only a Proposed "
    "dependency-free Python structural prerequisite over caller-supplied "
    "proof-shaped bytes; valid structure or a Go StoredAuthorization prerequisite "
    "does not verify lifecycle signatures, authenticate external root, time or "
    "revocation currentness, authorize or perform acceptance, rejection, atomic "
    "supersession or repository mutation, generate Accepted source, enforce "
    "immutability or architecture compliance, provide CAS, durability or rollback "
    "resistance, close G0, register a Skill, adapter, route, kind, evaluator, "
    "producer or runtime profile, persist, execute or effect; source distribution "
    "copies Python only and no Go contract, authority, production key or state"
)
DETECTOR = {
    "argv": ["python3", CHECKER, "--golden", "repo_root"],
    "positive": (
        "test_registry_is_v39_scope_neutral_lifecycle_candidate_and_go_evidence"
    ),
    "negative": (
        "test_scope_route_authority_go_distribution_and_pin_drift_fail_closed"
    ),
}
GO_WIRING_TOKENS = (
    "authenticated_adr_approval_v1_go_contract", "authenticated_adr_approval_v1_go_authority",
    "authenticatedadrapprovalcontract", "authenticatedadrapprovalauthority",
)
LIFECYCLE_WIRING_TOKENS = (
    "authenticated_adr_lifecycle", "authenticatedADRLifecycle", "architecture_decision_lifecycle",
)
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Authenticated ADR lifecycle v1 Proposed candidate"],
    ".agent/ARCHITECTURE.md": ["ADR-0083 Authenticated ADR lifecycle v1"],
    ".agent/ROADMAP.md": ["Authenticated ADR lifecycle v1 Proposed candidate evidence"],
    ".agent/CURRENT_SPRINT.md": ["Authenticated ADR lifecycle v1 Proposed candidate"],
    ".agent/DECISIONS.md": ["Authenticated ADR lifecycle v1 Proposed candidate governance"],
    ".agent/engineering/README.md": ["ADR-0083 adds"],
    "docs/design/ai-engineering-os/README.md": ["ADR-0083 Authenticated ADR Lifecycle"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0083 Authenticated ADR Lifecycle v1 Proposed Candidate Governance"
    ],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "Authenticated ADR lifecycle v1 Proposed candidate evidence"
    ],
}
PROMOTION_SENTINEL = "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
def _mapping(value):
    return value if isinstance(value, dict) else {}
def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: lifecycle candidate requires Registry v39")
    expected_blocks = {
        "authenticated_adr_approval_v1_go_authority_evidence": GO_AUTHORITY_EVIDENCE,
        "authenticated_adr_lifecycle_v1_candidate_contract": LIFECYCLE_CANDIDATE_CONTRACT,
    }
    issues.extend(f"{path}: {field} drifted" for field, expected in
                  expected_blocks.items() if data.get(field) != expected)
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: lifecycle candidate or Go evidence expanded scope")
    encoded_scope = json.dumps(data.get("scope"), sort_keys=True,
                               separators=(",", ":")).encode()
    if hashlib.sha256(encoded_scope).hexdigest() != SCOPE_SHA256:
        issues.append(f"{path}: canonical scope SHA-256 drifted")
    for section, expected in (("canonical_refs", CANONICAL_REFS),
                              ("reference_implementations", REFERENCE_IMPLEMENTATIONS),
                              ("contract_pins", PINS)):
        actual = _mapping(data.get(section))
        issues.extend(f"{path}: {section}.{field} drifted" for field, value in
                      expected.items() if actual.get(field) != value)
    noncaps = data.get("non_capabilities") or []
    for value in (GO_NON_CAPABILITY, LIFECYCLE_NON_CAPABILITY):
        if value not in noncaps:
            issues.append(f"{path}: lifecycle/Go evidence non-capability drifted")
    return issues


def artifact_issues(repo_root):
    try:
        load_golden(repo_root)
    except (OSError, ContractError, LifecycleContractError) as error:
        return [f"authenticated ADR lifecycle artifacts cannot be validated: {error}"]
    issues = []
    for relative, expected in CORE_SHA256.items():
        path = repo_root / relative
        try:
            mode = path.lstat()
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: cannot validate frozen source: {error}")
            continue
        if (not stat.S_ISREG(mode.st_mode) or stat.S_IMODE(mode.st_mode) != 0o644 or
                mode.st_nlink != 1):
            issues.append(f"{relative}: must remain regular 0644 with nlink 1")
            continue
        try:
            raw = read_bounded_file(path, label=relative, max_bytes=100_663_296)
        except (OSError, ContractError) as error:
            issues.append(f"{relative}: cannot validate frozen source: {error}")
            continue
        if hashlib.sha256(raw).hexdigest() != expected:
            issues.append(f"{relative}: physical pin drifted")
    package = repo_root / "harness/authenticated_adr_lifecycle_contract"
    prefix = "harness/authenticated_adr_lifecycle_contract/"
    expected_names = {relative.removeprefix(prefix) for relative in CORE_SHA256
                      if relative.startswith(prefix)}
    try:
        package_mode = package.lstat()
        actual_names = {path.name for path in package.iterdir()}
    except OSError as error:
        issues.append(f"authenticated ADR lifecycle source closure unreadable: {error}")
    else:
        if not stat.S_ISDIR(package_mode.st_mode):
            issues.append("authenticated ADR lifecycle package must be a directory")
        if actual_names != expected_names:
            issues.append("authenticated ADR lifecycle package physical closure drifted")
    if len(CORE_SHA256) != 20:
        issues.append("authenticated ADR lifecycle source closure must remain exact20")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get("governance.authenticated_adr_lifecycle_v1_candidate")
    if not isinstance(detector, dict):
        return ["authenticated ADR lifecycle checker-only detector is missing"]
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    issues = []
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("authenticated ADR lifecycle detector argv drifted")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("authenticated ADR lifecycle detector must remain shadow")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"authenticated ADR lifecycle {polarity} test drifted")
    uses = [item for item in detectors.values() if CHECKER in
            (_mapping(item.get("implementation")).get("argv") or [])]
    if uses != [detector]:
        issues.append("authenticated ADR lifecycle requires one checker-only detector")
    encoded = json.dumps(detectors, sort_keys=True)
    others = json.dumps({key: value for key, value in detectors.items() if
                         key != "governance.authenticated_adr_lifecycle_v1_candidate"},
                        sort_keys=True)
    if any(token in encoded for token in GO_WIRING_TOKENS):
        issues.append("Go approval authority evidence cannot enter a detector")
    if any(token in others for token in LIFECYCLE_WIRING_TOKENS):
        issues.append("lifecycle candidate requires exactly one checker-only detector")
    return issues


def wiring_issues(agent_root):
    activation, a_error = load_yaml(agent_root / "engineering/activation.yml")
    routes, r_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, d_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if a_error or r_error or d_error:
        return ["authenticated ADR lifecycle Agent Engineering wiring is unreadable"]
    refs = _mapping(activation.get("canonical_extension_refs"))
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, expected in CANONICAL_REFS.items()
              if refs.get(field) != expected]
    by_id = {item.get("id"): item for item in disciplines.get("disciplines") or []}
    assets = _mapping(by_id.get("contract")).get("assets") or []
    for required in (SCHEMA, CHECKER):
        if required not in assets:
            issues.append(f"authenticated ADR lifecycle contract asset missing: {required}")
    routed = {item.get("ref") for route in routes.get("routes") or []
              for item in route.get("include") or []}
    issues.extend(f"authenticated ADR lifecycle or Go evidence cannot enter route: {field}"
                  for field, reference in CANONICAL_REFS.items() if reference in routed)
    encoded = json.dumps(routes, sort_keys=True)
    if any(token in encoded for token in GO_WIRING_TOKENS + LIFECYCLE_WIRING_TOKENS):
        issues.append("lifecycle or Go approval authority identifier cannot enter a route")
    return issues


def documentation_issues(repo_root):
    if not (repo_root / PROMOTION_SENTINEL).is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        try:
            text = read_bounded_file(repo_root / relative, label=relative).decode()
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: cannot validate lifecycle docs: {error}")
            continue
        issues.extend(f"{relative}: missing Proposed marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        roadmap_text = read_bounded_file(roadmap, label=str(roadmap)).decode()
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate lifecycle roadmap: {error}"]
    required = ("- [x] Authenticated ADR lifecycle v1 Proposed candidate evidence",
                "authority-bearing lifecycle promotion", "不关闭 G0")
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
        return [f"{path}: lifecycle governance Proposed ADR failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != physical:
        issues.append(f"{path}: physical pin drifted")
    expected = {"status": "proposed", "body_sha256": body, "self_sha256": self_digest}
    issues.extend(f"{path}: {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    issues.extend(f"{path}: missing boundary marker {marker!r}"
                  for marker in markers if marker not in normalized)
    return issues


def adr_issues(repo_root):
    issues = _one_adr_issues(
        repo_root, APPROVAL_AUTHORITY_DECISION, APPROVAL_AUTHORITY_DECISION_SHA256,
        APPROVAL_AUTHORITY_DECISION_BODY_SHA256,
        APPROVAL_AUTHORITY_DECISION_SELF_SHA256,
        ("StoredAuthorization", "does not accept", "no command, socket, route"),
    )
    issues.extend(_one_adr_issues(
        repo_root, SEMANTIC_DECISION, SEMANTIC_DECISION_SHA256,
        SEMANTIC_DECISION_BODY_SHA256, SEMANTIC_DECISION_SELF_SHA256,
        ("structural prerequisite", "never verifies Ed25519",
         "never mutates status", "ADR-0082 remains Proposed"),
    ))
    if not all((GOVERNANCE_DECISION_SHA256, GOVERNANCE_DECISION_BODY_SHA256,
                GOVERNANCE_DECISION_SELF_SHA256)):
        issues.append("ADR-0083 pins remain provisional pending independent review")
    else:
        issues.extend(_one_adr_issues(
            repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
            GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
            ("Registry v34", "checker-only", "must not copy",
             "Full authority-bearing lifecycle"),
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
