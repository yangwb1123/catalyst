"""ADR-0067 proposed-only ADR v2 governance integration."""

from __future__ import annotations

import json

from architecture_decision_record_v2 import ContractError, validate_golden
from governance_contract import read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/architecture-decision-record-v2.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/ADR-9001-proposed-boundary.md"
CHECKER_RELATIVE = "harness/architecture_decision_record_v2_check.py"
SKILL_RELATIVE = ".agent/skills/adr-governance.md"
DECISION_RELATIVE = "docs/adr/0067-proposed-only-adr-v2-frontmatter.md"
SUCCESS = (
    "STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document "
    "bytes only; no identity, ownership, approver, evidence, claim, graph, "
    "acceptance, compliance, persistence, transition, execution, or effect "
    "attestation)"
)

ARCHITECTURE_DECISION_RECORD_V2 = {
    "api_version": "forgeos.architecture-decision-record/v2",
    "kind": "ArchitectureDecisionRecord",
    "delivery": "strict_proposed_only_document_contract",
    "mode": "exact_explicit_document_validation",
    "input_kind": "caller_supplied_single_new_adr_v2_markdown_document",
    "output_kind": "structurally_valid_proposed_adr_v2",
    "document_contract": {
        "framing": "lf_only_canonical_json_frontmatter_and_exact_markdown_body",
        "status": "proposed",
        "accepted_at_unix_ms": None,
        "acceptance_id": None,
        "superseded_by": [],
        "legacy_v2_parse_retro_validation_or_migration": "forbidden",
        "ambient_reference_resolution": "none",
    },
    "declared_relations": {
        "owner_refs": "caller_or_author_declared_responsibility_only",
        "approver_refs": "caller_or_author_declared_required_review_only",
        "claim_and_evidence_refs": "shape_checked_without_resolution_or_truth",
        "affected_node_ids": "shape_checked_without_graph_resolution_or_coverage",
        "implementation_refs": "normalized_locator_only_without_file_read",
    },
    "local_execution": {
        "universal_checker": "explicit_file_or_pinned_golden_only",
        "go_runtime_binding": "current_writes_adr_candidate_only",
        "universal_repository_scan": False,
        "go_legacy_baseline_byte_scan": True,
        "go_legacy_v2_parse_or_rewrite": False,
        "clock_access": False,
        "credential_access": False,
        "provider_access": False,
        "network_access": False,
        "database_access": False,
        "persistence": "none",
    },
    "unavailable_runtime": {
        "owner_or_approver_authentication": "unavailable",
        "approval_record_resolution": "unavailable",
        "separation_of_duty": "unavailable",
        "proposed_to_terminal_state_machine": "unavailable",
        "accepted_document_immutability": "unavailable",
        "claim_evidence_or_graph_resolution": "unavailable",
        "architecture_compliance": "unavailable",
        "lifecycle_or_supersession_transition": "unavailable",
        "legacy_migration_or_retro_validation": "unavailable",
        "graph_edge_or_coverage_production": "unavailable",
        "persistence_or_effect": "unavailable",
    },
    "authority_semantics": {
        "owner_or_approver_identity_attestation": False,
        "approval_or_acceptance_attestation": False,
        "approval_record_attestation": False,
        "claim_evidence_or_graph_attestation": False,
        "architecture_compliance_attestation": False,
        "legacy_migration_attestation": False,
        "persistence_attestation": False,
        "transition_attestation": False,
        "execution_attestation": False,
        "effect_attestation": False,
        "positive_result": SUCCESS,
        "attestations": [],
    },
    "semantic_validation": {
        "schema_alone_sufficient": False,
        "exact_document_and_physical_basename_required": True,
        "canonical_frontmatter_and_strict_unicode_required": True,
        "body_heading_section_and_digest_binding_recomputed": True,
        "sorted_unique_and_cross_reference_relations_recomputed": True,
        "complete_document_bounds_fail_closed": True,
        "current_writes_adr_attempt_only": True,
        "existing_adr_documents_not_retro_validated": True,
    },
    "limits": {
        "max_document_bytes": 262144,
        "max_frontmatter_json_bytes": 65536,
        "max_body_bytes": 196608,
        "max_json_depth": 16,
        "max_object_fields": 64,
        "max_array_items": 64,
        "max_document_name_utf8_bytes": 255,
        "integer_domain": "signed_int64",
        "runtime_string_length_unit": "utf8_bytes",
    },
    "positive_result": SUCCESS,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "architecture_decision_record_v2_schema": SCHEMA_RELATIVE,
    "architecture_decision_record_v2_golden_fixture": FIXTURE_RELATIVE,
    "architecture_decision_record_v2_checker": CHECKER_RELATIVE,
    "architecture_decision_record_v2_skill": SKILL_RELATIVE,
    "architecture_decision_record_v2_decision": DECISION_RELATIVE,
}
REFERENCE_IMPLEMENTATIONS = {
    "architecture_decision_record_v2_python": {
        "ref": "harness/architecture_decision_record_v2",
        "projection": "universal_scaffold_strict_proposed_document_checker",
    },
    "architecture_decision_record_v2_go": {
        "ref": "forge-core/internal/adrv2",
        "projection": "catalyst_repository_only_current_writes_adr_candidate_validator",
    },
}
NON_CAPABILITY = (
    "ArchitectureDecisionRecord v2 validates only one explicitly supplied new "
    "Proposed document and its declared reference shapes; writes_adr retains its "
    "existing legacy-baseline integrity snapshot but does not parse retro-"
    "validate or migrate legacy ADRs, authenticate owners or approvers, resolve "
    "ApprovalRecord Claim Evidence or Graph nodes, produce graph coverage or "
    "edges, accept or transition decisions, enforce immutability or compliance, "
    "persist lifecycle state, execute lifecycle actions, or attest authority "
    "completion or effect"
)
DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "--golden", "repo_root"],
    "positive": "test_registry_classifies_proposed_only_evaluator_without_authority",
    "negative": "test_scope_authority_and_non_capability_drift_fail_closed",
}
SKILL_MARKERS = [
    "ADR-0067", "forgeos.architecture-decision-record/v2", "Proposed",
    "caller/author", "ApprovalRecord", "writes_adr", "不做 v2 解析",
    "baseline integrity snapshot", "graph-node-", "Schema-only", "forge accept",
]
DOC_MARKERS = {
    ".agent/AGENTS.md": "ADR-0067",
    ".agent/ARCHITECTURE.md": "Proposed-only ADR v2",
    ".agent/ROADMAP.md": "Wave 3–A1 Proposed-only ADR v2",
    ".agent/CURRENT_SPRINT.md": "Sprint 114（✅ DONE；Proposed-only ADR v2）",
    ".agent/DECISIONS.md": "D39 Proposed-only ADR v2",
    ".agent/engineering/README.md": "ADR-0067 Proposed-only ADR v2",
    "docs/design/ai-engineering-os/README.md": "ADR-0067 Proposed-only ADR v2",
    "docs/design/ai-engineering-os/governance-contracts.md":
        "ADR-0067 Proposed-only ADR v2",
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md":
        "Proposed-only ArchitectureDecisionRecord v2",
}
PROMOTION_SENTINEL = "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("architecture_decision_record_v2") != ARCHITECTURE_DECISION_RECORD_V2:
        issues.append(f"{path}: proposed-only ADR v2 evaluator contract drifted")
    scope = _mapping(data.get("scope"))
    evaluators = scope.get("shipped_evaluators") or []
    if evaluators != [
            "local_go_package_impact_prescan", "graph_snapshot",
            "graph_snapshot_test_source", "architecture_decision_record_v2",
            "capability_registry", "planning_capability_ownership",
            "project_source_snapshot"]:
        issues.append(f"{path}: shipped pure evaluator scope drifted")
    forbidden = sum((scope.get(name) or [] for name in (
        "shipped_kinds", "shipped_contract_only_kinds", "shipped_producers",
        "shipped_projectors", "shipped_runtime_profiles")), [])
    if "architecture_decision_record_v2" in forbidden or (
            "ArchitectureDecisionRecord" in forbidden):
        issues.append(f"{path}: ADR v2 cannot be a kind, producer, projector, or authority")
    issues.extend(_registry_ref_issues(data, path))
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: ADR v2 non-capability boundary drifted")
    if (_mapping(data.get("legacy")).get("adr_import") !=
            "explicit_supplied_bytes_unverified_read_only_no_parse_projection_v1"):
        issues.append(f"{path}: legacy ADR import must remain explicit and no-parse")
    return issues


def _registry_ref_issues(data, path):
    issues = []
    refs = _mapping(data.get("canonical_refs"))
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(path, label=SCHEMA_RELATIVE, max_bytes=1_048_576)
        schema = json.loads(raw.decode("utf-8"))
    except (OSError, ContractError, UnicodeDecodeError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate ADR v2 Schema: {error}"]
    issues = []
    authority = _mapping(schema.get("x-forgeos-authority-semantics"))
    semantics = _mapping(schema.get("x-forgeos-semantic-validation"))
    limits = _mapping(schema.get("x-forgeos-limits"))
    false_fields = (
        "acceptance_or_approval_record",
        "authenticated_identity_or_separation_of_duty",
        "claim_evidence_or_graph_resolution",
        "architecture_compliance_attestation",
        "legacy_adr_migration_or_retro_validation",
    )
    if any(authority.get(field) is not False for field in false_fields):
        issues.append(f"{path}: ADR v2 authority boundary drifted")
    if authority.get("positive_result") != SUCCESS:
        issues.append(f"{path}: ADR v2 positive result drifted")
    if semantics.get("schema_alone_sufficient") is not False or (
            semantics.get("ambient_reads") != "none"):
        issues.append(f"{path}: ADR v2 strict semantic validation drifted")
    expected_limits = {
        "max_document_bytes": 262144, "max_frontmatter_json_bytes": 65536,
        "max_body_bytes": 196608, "max_json_depth": 16,
        "max_object_fields": 64, "max_array_items": 64,
        "max_document_name_utf8_bytes": 255,
    }
    if any(limits.get(field) != value for field, value in expected_limits.items()):
        issues.append(f"{path}: ADR v2 resource limits drifted")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.architecture_decision_record_v2")
    if not isinstance(detector, dict):
        return ["ADR v2 shadow detector is missing"]
    issues = []
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("ADR v2 detector requires the exact pinned-golden arguments")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("ADR v2 detector must remain shadow and non-load-bearing")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"ADR v2 detector {polarity} test drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate ADR v2 Skill: {error}"]
    return [f"{path}: missing ADR v2 marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def documentation_issues(repo_root):
    if not (repo_root / PROMOTION_SENTINEL).is_file():
        return []
    issues = []
    for relative, marker in DOC_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ADR v2 promotion: {error}")
            continue
        if marker not in text:
            issues.append(f"{path}: missing ADR-0067 promotion marker {marker!r}")
    issues.extend(_roadmap_state_issues(repo_root))
    return issues


def _roadmap_state_issues(repo_root):
    path = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(path, label=str(path)).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate ADR v2 roadmap state: {error}"]
    delivered = "- [x] 为新 ADR 增加 v2 frontmatter"
    lifecycle = (
        "- [ ] 实现 Accepted ADR immutable + supersede 状态机和 "
        "Architecture Compliance；"
    )
    issues = []
    if delivered not in text:
        issues.append(f"{path}: proposed-only ADR v2 roadmap item must be checked")
    if lifecycle not in text:
        issues.append(f"{path}: Accepted ADR lifecycle item must remain unchecked")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(documentation_issues(repo_root))
    try:
        validate_golden(repo_root)
    except (OSError, ContractError) as error:
        issues.append(f"{repo_root / FIXTURE_RELATIVE}: invalid ADR v2 golden: {error}")
    return issues
