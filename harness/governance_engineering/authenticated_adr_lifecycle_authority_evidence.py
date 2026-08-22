"""ADR-0084/0085 Catalyst-only lifecycle authority evidence governance."""

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

from .evidence_claim_portable import EXPECTED_SCOPE


AUTHORITY_DECISION = (
    "docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-"
    "authority-service-v1.md"
)
GOVERNANCE_DECISION = (
    "docs/adr/ADR-0085-authenticated-architecture-decision-lifecycle-"
    "authority-evidence-and-source-distribution.md"
)
AUTHORITY_PACKAGE = "forge-core/internal/authenticatedadrlifecycleauthority"
AUTHORITY_CONTRACT = "forge-core/internal/authenticatedadrlifecyclecontract"
AUTHORITY_DECISION_SHA256 = (
    "5792739e70a6bdb6672ab5edbf9abe75a4c5ff16c4be770ac61e26a27e86dc48"
)
AUTHORITY_DECISION_BODY_SHA256 = (
    "ded7ddde8c384cb45583f8d909f3c9ceda8d1b8ae181ec196d1134ab3f4f371b"
)
AUTHORITY_DECISION_SELF_SHA256 = (
    "f170be3e165eaf0ba0c57a46ca1000d1d6a1577c0ef27a0524e87d7cfd9ce97b"
)
AUTHORITY_MANIFEST_SHA256 = (
    "1a85aa0aa90414039815e90c7be53d56d0222c8a742e37f33ef9681586a00778"
)
GOVERNANCE_DECISION_SHA256 = (
    "481cb05ec6b1b0a729d0bf928bdd9a31df039fb14e1f50741cc9efc7b773f728"
)
GOVERNANCE_DECISION_BODY_SHA256 = (
    "445f258a82446bb6aa436d15a7c93dbbaab40b142e102c8f8b287f922e396c56"
)
GOVERNANCE_DECISION_SELF_SHA256 = (
    "dbda4e571bb92f0bce4e6c1b7bae358dc56ccc0d793a82366c900c8e5f3cbdce"
)
SCOPE_SHA256 = "8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c"
DETECTOR_ID = "governance.authenticated_adr_lifecycle_v1_candidate"
DETECTOR = {
    "argv": ["python3", "harness/authenticated_adr_lifecycle_contract_check.py",
             "--golden", "repo_root"],
    "positive": (
        "test_registry_is_v39_scope_neutral_lifecycle_candidate_and_go_evidence"
    ),
    "negative": (
        "test_scope_route_authority_go_distribution_and_pin_drift_fail_closed"
    ),
}

AUTHORITY_FILES = (
    "authority_matrix_test.go", "build_receipts.go", "build_state.go", "bytes.go",
    "capability_binding_test.go", "config.go", "constants.go", "crypto.go",
    "digest.go", "doc.go", "errors.go", "fixture_authority.go", "input.go",
    "json.go", "map_values.go", "path_attack_test.go",
    "platform_supported_unix.go", "platform_unsupported_unix.go",
    "platform_unsupported_unix_test.go", "preflight_order_test.go",
    "prelock_unix.go", "service.go", "service_fault_test.go",
    "service_fixture_mutations_test.go", "service_fixture_test.go",
    "service_test.go", "signer.go", "state_bindings_unix.go",
    "state_close_unix.go", "state_commit_unix.go", "state_identity_unix.go",
    "state_layout.go", "state_lock_unix.go", "state_model.go",
    "state_open_unix.go", "state_other.go", "state_port_unix.go",
    "state_read_unix.go", "state_roots_unix.go", "state_session_unix.go",
    "state_syscall_fcntl_unix.go", "state_syscall_unix.go", "state_test.go",
    "types.go",
)
SEMANTIC_MARKERS = {
    "service.go": (
        "func TransitionAndStore", "*approvalauthority.StoredAuthorization",
        "func ReplayStored", "replayExact",
    ),
    "input.go": ("stored.AcceptancePrerequisite()",),
    "crypto.go": (
        "approval and lifecycle authority facts must be disjoint",
        "ed25519.Verify",
    ),
    "constants.go": (
        "acceptance-receipt.signature.v1", "supersession-receipt.signature.v1",
        "state.signature.v1", "maxTargets    = 64",
    ),
    "build_state.go": (
        'target["status"] = "superseded"', '"status": "accepted"',
    ),
    "state_commit_unix.go": (
        "s.port.rename", "s.port.syncDirectory", 'uncertain("reopen lifecycle state"',
    ),
    "state_model.go": ("verifyBundleSignatures",),
}

AUTHORITY_EVIDENCE = {
    "profile_id": "authenticated_architecture_decision_lifecycle_v1",
    "decision_status": "proposed",
    "delivery": (
        "catalyst_repository_only_go_lifecycle_authority_evidence_not_source_"
        "distributed"
    ),
    "semantic_authority": "ADR-0084_only",
    "implementation": {
        "contract": AUTHORITY_CONTRACT,
        "authority": AUTHORITY_PACKAGE,
        "language_and_dependencies": "go_standard_library_only",
        "availability": "catalyst_repository_internal_api_only_not_scaffolded",
    },
    "evidence": {
        "exact_frozen_v1_structural_parity": True,
        "opaque_stored_authorization_required_for_fresh_transition": True,
        "independent_approval_and_lifecycle_trust_facts": True,
        "ed25519_request_receipt_and_state_verification_and_signing": True,
        "atomic_multi_target_acceptance_and_supersession": True,
        "durable_locked_cas_sync_atomic_rename_and_authenticated_reopen": True,
        "exact_idempotent_replay_without_resigning_or_rewrite": True,
        "historical_existing_lock_only_replay_without_authority_writes": True,
    },
    "boundary": {
        "production_root_key_seed_state_clock_revocation_or_transport_supplied": False,
        "copies_go_contract_or_authority": False,
        "copies_production_keys_seed_state_receipt_or_ledger": False,
        "adds_command_socket_route_registry_scope_or_runtime_profile": False,
        "repository_source_mutation_or_accepted_document_generation": False,
        "grant_permission_generalized_effect_or_g0_closure": False,
        "architecture_compliance_or_administrator_rollback_resistance": False,
    },
    "positive_evidence": (
        "PROPOSED_CATALYST_ONLY_AUTHENTICATED_ADR_LIFECYCLE_V1_GO_AUTHORITY "
        "(authenticated only relative to explicit external trust and a real opaque "
        "StoredAuthorization; no production provisioning, repository source "
        "mutation, route, runtime registration, scaffolded Go, permission, "
        "generalized effect, architecture compliance, rollback resistance, or G0 "
        "attestation)"
    ),
    "attests": [],
    "persistence": (
        "repository_external_lifecycle_state_only_when_explicitly_provisioned_"
        "by_an_in_process_caller"
    ),
}

CANONICAL_REFS = {
    "authenticated_adr_lifecycle_v1_go_authority_decision": AUTHORITY_DECISION,
    "authenticated_adr_lifecycle_v1_go_authority_governance_decision": (
        GOVERNANCE_DECISION
    ),
}
PINS = {
    "authenticated_adr_lifecycle_v1_go_authority_decision_sha256": (
        AUTHORITY_DECISION_SHA256
    ),
    "authenticated_adr_lifecycle_v1_go_authority_manifest_sha256": (
        AUTHORITY_MANIFEST_SHA256
    ),
}
REFERENCE_IMPLEMENTATIONS = {
    "authenticated_adr_lifecycle_v1_go_contract": {
        "ref": AUTHORITY_CONTRACT,
        "projection": (
            "catalyst_repository_only_exact_frozen_structural_contract_not_"
            "scaffolded"
        ),
    },
    "authenticated_adr_lifecycle_v1_go_authority": {
        "ref": AUTHORITY_PACKAGE,
        "projection": (
            "catalyst_repository_only_stored_authorization_authenticated_"
            "lifecycle_authority_and_durable_store_not_scaffolded"
        ),
    },
}
NON_CAPABILITY = (
    "ADR-0084 is only Proposed Catalyst-repository lifecycle authority evidence "
    "relative to explicit external trust and a real opaque StoredAuthorization; "
    "Registry v35 and source-only distribution provision no production root, key, "
    "seed, state, clock, revocation publisher, caller or transport, copy no Go "
    "contract or authority, receipt or ledger, add no command, socket, route, "
    "Skill, evaluator, producer, service, registry scope or runtime profile, do not "
    "rewrite source or generate an Accepted ADR, grant permission, create a "
    "generalized Effect, attest architecture compliance or administrator rollback "
    "resistance, or close G0"
)
DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Registry v35 lifecycle authority evidence"],
    ".agent/ARCHITECTURE.md": ["ADR-0085 lifecycle authority evidence"],
    ".agent/ROADMAP.md": ["ADR-0084 authority evidence and source-only governance"],
    ".agent/CURRENT_SPRINT.md": ["Registry v35 lifecycle authority evidence"],
    ".agent/DECISIONS.md": ["D52 ADR-0084 lifecycle authority evidence"],
    ".agent/engineering/README.md": ["ADR-0085 adds Registry v35"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0085 Authenticated ADR Lifecycle Authority Evidence"
    ],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0085 Authenticated ADR Lifecycle Authority Evidence"
    ],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "Authenticated ADR lifecycle authority evidence and source-only governance"
    ],
}
PROMOTION_SENTINEL = "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("version") != 39:
        issues.append(f"{path}: lifecycle authority evidence requires Registry v39")
    if data.get("authenticated_adr_lifecycle_v1_go_authority_evidence") != (
            AUTHORITY_EVIDENCE):
        issues.append(f"{path}: lifecycle Go authority evidence drifted")
    if data.get("scope") != EXPECTED_SCOPE:
        issues.append(f"{path}: lifecycle authority evidence cannot expand scope")
    canonical = json.dumps(data.get("scope"), sort_keys=True,
                           separators=(",", ":")).encode()
    if hashlib.sha256(canonical).hexdigest() != SCOPE_SHA256:
        issues.append(f"{path}: complete scope digest drifted")
    for owner, expected in (("canonical_refs", CANONICAL_REFS),
                            ("contract_pins", PINS),
                            ("reference_implementations", REFERENCE_IMPLEMENTATIONS)):
        actual = _mapping(data.get(owner))
        issues.extend(f"{path}: {owner}.{field} drifted"
                      for field, value in expected.items()
                      if actual.get(field) != value)
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: lifecycle authority non-capability drifted")
    candidate = _mapping(data.get("authenticated_adr_lifecycle_v1_candidate_contract"))
    source = _mapping(candidate.get("source_distribution"))
    if source.get("copies_go_contract_or_authority") is not False:
        issues.append(f"{path}: structural candidate cannot distribute Go authority")
    return issues


def _read_regular(path, label, maximum=2 * 1024 * 1024):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise OSError("O_NOFOLLOW is unavailable")
    descriptor = os.open(path, flags | nofollow)
    try:
        info = os.fstat(descriptor)
        if not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o644:
            raise OSError(f"{label} must be regular mode 0644")
        if info.st_nlink != 1:
            raise OSError(f"{label} must have link count one")
        if info.st_size > maximum:
            raise ContractError(f"{label} exceeds {maximum} bytes")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(maximum + 1)
        if len(raw) > maximum:
            raise ContractError(f"{label} exceeds {maximum} bytes")
        return raw
    finally:
        os.close(descriptor)


def _authority_inventory_issues(package):
    try:
        package_info = package.lstat()
        entries = {entry.name: entry.lstat() for entry in package.iterdir()}
    except OSError as error:
        return [f"{package}: authority inventory unavailable: {error}"], {}
    issues = []
    if not stat.S_ISDIR(package_info.st_mode):
        issues.append(f"{package}: authority package must be a directory")
    if set(entries) != set(AUTHORITY_FILES):
        issues.append(f"{package}: exact 44-file physical closure drifted")
    for name, info in entries.items():
        if name in AUTHORITY_FILES and not stat.S_ISREG(info.st_mode):
            issues.append(f"{package / name}: authority entry must be regular")
    return issues, entries


def authority_implementation_issues(repo_root):
    package = repo_root / AUTHORITY_PACKAGE
    issues, entries = _authority_inventory_issues(package)
    if issues or set(entries) != set(AUTHORITY_FILES):
        return issues
    rows = []
    for name in AUTHORITY_FILES:
        path = package / name
        try:
            raw = _read_regular(path, str(path))
            text = raw.decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: authority evidence cannot be read: {error}")
            continue
        digest = hashlib.sha256(raw).hexdigest()
        rows.append(f"{digest}  {AUTHORITY_PACKAGE}/{name}\n")
        issues.extend(f"{path}: missing authority semantic marker {marker!r}"
                      for marker in SEMANTIC_MARKERS.get(name, ())
                      if marker not in text)
    aggregate = hashlib.sha256("".join(rows).encode()).hexdigest()
    if len(rows) != 44 or aggregate != AUTHORITY_MANIFEST_SHA256:
        issues.append(f"{package}: frozen exact44 aggregate pin drifted")
    return issues


def _adr_one(repo_root, relative, physical, body, self_digest, markers):
    path = repo_root / relative
    try:
        raw = _read_regular(path, relative, 256 * 1024)
        metadata = validate_document_bytes(raw, path.name)
        normalized = " ".join(raw.decode().split())
    except (OSError, ADRContractError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: lifecycle authority Proposed ADR failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != physical:
        issues.append(f"{path}: physical pin drifted")
    expected = {"status": "proposed", "body_sha256": body,
                "self_sha256": self_digest}
    issues.extend(f"{path}: {field} drifted" for field, value in expected.items()
                  if metadata.get(field) != value)
    issues.extend(f"{path}: missing boundary marker {marker!r}"
                  for marker in markers if marker not in normalized)
    return issues


def adr_issues(repo_root):
    issues = _adr_one(
        repo_root, AUTHORITY_DECISION, AUTHORITY_DECISION_SHA256,
        AUTHORITY_DECISION_BODY_SHA256, AUTHORITY_DECISION_SELF_SHA256,
        ("StoredAuthorization", "does not accept", "command, route, socket"),
    )
    issues.extend(_adr_one(
        repo_root, GOVERNANCE_DECISION, GOVERNANCE_DECISION_SHA256,
        GOVERNANCE_DECISION_BODY_SHA256, GOVERNANCE_DECISION_SELF_SHA256,
        ("Registry v35", "checker-only", "must not copy", "source-only"),
    ))
    return issues


def detector_issues(agent_root):
    path = agent_root / "engineering/detectors.yml"
    data, error = load_yaml(path)
    if error or not isinstance(data, dict):
        return [f"{path}: lifecycle detector registry unavailable: {error}"]
    detectors = data.get("detectors") if isinstance(data.get("detectors"), list) else []
    lifecycle = [item for item in detectors if isinstance(item, dict) and
                 "authenticated_adr_lifecycle" in str(item.get("id", ""))]
    issues = []
    if len(lifecycle) != 1 or lifecycle[0].get("id") != DETECTOR_ID:
        issues.append("lifecycle governance requires exactly one checker-only shadow")
        return issues
    item = lifecycle[0]
    implementation = _mapping(item.get("implementation"))
    invocation, tests = _mapping(item.get("invocation")), _mapping(item.get("tests"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("lifecycle checker-only detector argv drifted")
    if item.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("lifecycle detector must remain a non-load-bearing shadow")
    for field in ("positive", "negative"):
        if _mapping(tests.get(field)).get("contains") != DETECTOR[field]:
            issues.append(f"lifecycle detector {field} sentinel drifted")
    encoded = json.dumps(detectors, sort_keys=True)
    forbidden = (AUTHORITY_PACKAGE, AUTHORITY_CONTRACT,
                 "authenticated_adr_lifecycle_v1_go_authority",
                 "authenticated_adr_lifecycle_v1_service", "lifecycle-projector")
    if any(token in encoded for token in forbidden):
        issues.append("Go lifecycle authority or service cannot enter a detector")
    return issues


def wiring_issues(agent_root):
    from agent_engineering.contract import EXTENSION_REFS
    issues = []
    activation, activation_error = load_yaml(agent_root / "engineering/activation.yml")
    extensions = _mapping(activation).get("canonical_extension_refs")
    for field, expected in CANONICAL_REFS.items():
        if _mapping(extensions).get(field) != expected or EXTENSION_REFS.get(field) != expected:
            issues.append(f"lifecycle authority activation ref {field} drifted")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    contract = next((item for item in _mapping(disciplines).get("disciplines", [])
                     if isinstance(item, dict) and item.get("id") == "contract"), {})
    assets = contract.get("assets") if isinstance(contract.get("assets"), list) else []
    if not all(value in assets for value in CANONICAL_REFS.values()):
        issues.append("lifecycle authority decisions are absent from contract discipline")
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    encoded = json.dumps(routes, sort_keys=True) if route_error is None else ""
    if activation_error or discipline_error or route_error:
        issues.append("lifecycle authority Agent Engineering wiring is unreadable")
    forbidden = tuple(CANONICAL_REFS.values()) + (
        AUTHORITY_PACKAGE, AUTHORITY_CONTRACT,
        "authenticated_adr_lifecycle_v1_go_authority", "lifecycle-authority-service",
    )
    if any(token in encoded for token in forbidden):
        issues.append("lifecycle authority evidence cannot enter a context route")
    if any("authenticated_adr_lifecycle" in key and key.endswith("_skill")
           for key in EXTENSION_REFS):
        issues.append("lifecycle authority evidence cannot install a Skill")
    return issues


def documentation_issues(repo_root):
    if not (repo_root / PROMOTION_SENTINEL).is_file():
        return []
    issues = []
    for relative, markers in DOCUMENT_MARKERS.items():
        try:
            text = (repo_root / relative).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            issues.append(f"{relative}: cannot validate v35 docs: {error}")
            continue
        issues.extend(f"{relative}: missing v35 marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = roadmap.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate v35 roadmap: {error}"]
    markers = ("ADR-0084 authority evidence and source-only governance",
               "Go authority remains Catalyst-only", "不关闭 G0")
    issues.extend(f"{roadmap}: missing v35 boundary {marker!r}"
                  for marker in markers if marker not in text)
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(detector_issues(agent_root))
    issues.extend(wiring_issues(agent_root))
    issues.extend(adr_issues(repo_root))
    issues.extend(documentation_issues(repo_root))
    return issues
