"""ADR-0069 planning-only Capability ownership governance integration."""

from __future__ import annotations

import hashlib
import json

from architecture_decision_record_v2 import validate_document_file
from governance_contract import ContractError, read_bounded_file
from engineering_check_support import load_yaml
from planning_capability_ownership_projection import load_golden


SCHEMA_RELATIVE = "docs/contracts/planning-capability-ownership-projection-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/planning-capability-ownership-projection-v1.json"
CHECKER_RELATIVE = "harness/planning_capability_ownership_projection/check.py"
SKILL_RELATIVE = ".agent/skills/capability-ownership-projection.md"
DECISION_RELATIVE = "docs/adr/ADR-0069-planning-capability-ownership-projection-v1.md"
CATALOG_RELATIVE = "docs/design/ai-engineering-os/capability-catalog.v1.yml"
MAPPING_RELATIVE = "docs/design/ai-engineering-os/capability-skill-map.v1.yml"

CATALOG_BYTES = 33_000
CATALOG_SHA256 = "bc6efe535539c5f129af51486d8e81b9844b5ee6448fae2bce649fc159658d74"
MAPPING_BYTES = 5_924
MAPPING_SHA256 = "bfb2277fe66cd9f0c609b5be10ad77ad0969603edd19e5a6ccbe38b8e3409462"
FIXTURE_BYTES = 172_733
FIXTURE_SHA256 = "3d0a877bef0939cff5752fc5d602e0d3a90e19639308801008f9d2d9ff139f36"
SCHEMA_SHA256 = "a2ed6eb754c07478eeaaf2ae73a889ba985553c4220a7b6771be9e6a36078083"
REQUEST_SHA256 = "3639c4d3ad21db93db254b7da2643d492ca39c4dda5438de426379cd70718cfa"
PROJECTION_SHA256 = "53754ded32379d6520f3bd2b9d2956238731ad40c11124be457b724b4c150fa2"

RESULT = (
    "PROJECTED_PLANNING_CAPABILITY_OWNERSHIP_ONLY (complete declared primary-"
    "owner coverage and logical adapter references for the supplied planning "
    "sources only; no file existence, Skill availability, Registry mutation, "
    "authentication, authorization, permission, invocation, routing, execution, "
    "persistence, transition, or effect attestation)"
)

PLANNING_CAPABILITY_OWNERSHIP = {
    "api_version": "forgeos.planning-capability-ownership-projection/v1",
    "request_api_version": (
        "forgeos.planning-capability-ownership-projection-request/v1"
    ),
    "delivery": "shipped_go_python_pure_projectors_golden_checker_cli_and_scaffold",
    "adr_status": "proposed",
    "status": "planning_only",
    "mode": "planning_only_declared_ownership_and_logical_adapter_refs",
    "source_boundary": {
        "input": "two_caller_supplied_exact_yaml_byte_strings_only",
        "catalog_document": "capability-catalog.v1.yml",
        "mapping_document": "capability-skill-map.v1.yml",
        "catalog_bytes": CATALOG_BYTES,
        "catalog_sha256": CATALOG_SHA256,
        "mapping_bytes": MAPPING_BYTES,
        "mapping_sha256": MAPPING_SHA256,
        "ambient_repository_search_or_fallback": False,
        "source_authentication_or_currentness": "not_evaluated",
    },
    "coverage": {
        "catalog_node_count": 17,
        "capability_occurrence_count": 145,
        "unique_capability_count": 140,
        "mapping_package_count": 38,
        "mapped_capability_count": 140,
        "binding_count": 140,
        "primary_owner_cardinality": "exactly_one_per_unique_capability",
        "logical_adapter_derivation": ".agent/skills/ + owner_skill + .md",
        "physical_resolution": "not_performed",
        "skill_availability": "not_evaluated",
    },
    "identity": {
        "canonicalization": "forgeos.canonical-json/v1",
        "digest_chain": ["exact_source_bytes", "request", "binding", "projection"],
        "request_sha256": REQUEST_SHA256,
        "projection_sha256": PROJECTION_SHA256,
        "fixture_bytes": FIXTURE_BYTES,
        "fixture_sha256": FIXTURE_SHA256,
    },
    "product_cli": {
        "surface": "forge capability-ownership project --catalog FILE|- --mapping FILE|-",
        "option_order": "interchangeable",
        "stdin_sources": "exactly_one",
        "usage_exit_code": 2,
        "input_or_semantic_exit_code": 1,
        "pre_output_failure_classes": ["argument", "input", "semantic"],
        "pre_output_failure_stdout_bytes": 0,
        "success_output": "exact_compact_canonical_projection_plus_one_lf",
        "stdout_write_failure_exit_code": 1,
        "stdout_write_is_transactional": False,
        "partial_write_is_valid_artifact": False,
        "python_validate_and_golden_surfaces": "universal_internal_checker_only",
    },
    "local_execution": {
        "pure_projector_repository_access": False,
        "logical_adapter_stat_open_parse_or_generation": False,
        "registry_input_or_mutation": False,
        "environment_clock_credential_process_provider_network_database_access": False,
        "implementation_selection_or_execution": False,
        "persistence": "none",
    },
    "scaffold_boundary": {
        "copies_exact_catalog_and_mapping_sources": True,
        "copies_adr_schema_golden_python_projector_checker_and_tests": True,
        "copies_catalyst_go_runtime": False,
        "generates_declared_owner_skill_or_host_adapter_files": False,
        "existing_same_name_markdown_counts_as_resolution": False,
    },
    "authority_semantics": {
        "source_owner_or_repository_authentication": False,
        "adapter_file_existence_or_skill_availability_attestation": False,
        "registry_mutation": False,
        "grant_or_pdp_activation": False,
        "capability_invocation": False,
        "implementation_selection": False,
        "authorization_decision": "none",
        "permission_attestation": False,
        "runtime_routing_attestation": False,
        "persistence_transition_execution_or_effect_attestation": False,
        "positive_result": RESULT,
        "attestations": [],
    },
    "semantic_validation": {
        "schema_alone_sufficient": False,
        "strict_frozen_yaml_profile_and_closed_shapes": True,
        "complete_unique_primary_owner_coverage_recomputed": True,
        "all_occurrences_node_ids_bindings_logical_refs_and_digests_recomputed": True,
        "complete_canonical_projection_byte_comparison": True,
        "cross_language_exact_golden_required": True,
    },
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "planning_capability_ownership_schema": SCHEMA_RELATIVE,
    "planning_capability_ownership_golden_fixture": FIXTURE_RELATIVE,
    "planning_capability_ownership_checker": CHECKER_RELATIVE,
    "planning_capability_ownership_skill": SKILL_RELATIVE,
    "planning_capability_ownership_decision": DECISION_RELATIVE,
    "planning_capability_ownership_catalog_source": CATALOG_RELATIVE,
    "planning_capability_ownership_mapping_source": MAPPING_RELATIVE,
}
ACTIVATION_REFS = {
    "planning_capability_ownership_schema": SCHEMA_RELATIVE,
    "planning_capability_ownership_fixture": FIXTURE_RELATIVE,
    "planning_capability_ownership_checker": CHECKER_RELATIVE,
    "planning_capability_ownership_skill": SKILL_RELATIVE,
    "planning_capability_ownership_decision": DECISION_RELATIVE,
    "planning_capability_ownership_catalog_source": CATALOG_RELATIVE,
    "planning_capability_ownership_mapping_source": MAPPING_RELATIVE,
}
REFERENCE_IMPLEMENTATIONS = {
    "planning_capability_ownership_go": {
        "ref": "forge-core/internal/planningownership",
        "projection": "catalyst_repository_only_pure_projector_and_product_cli",
    },
    "planning_capability_ownership_python": {
        "ref": "harness/planning_capability_ownership_projection",
        "projection": "universal_scaffold_pure_projector_and_internal_checker",
    },
}
NON_CAPABILITY = (
    "Planning Capability Ownership Projection v1 proves only complete unique "
    "declared primary-owner coverage and derives unresolved logical adapter refs "
    "from two caller-supplied exact planning sources; it does not authenticate "
    "sources or owners, resolve or generate files, Skill packages or host adapters, "
    "mutate the ADR-0068 Registry, activate Grant/PDP, construct CapabilityInvocation, "
    "select or execute implementations, load plugins, route runtime work, persist "
    "state, authorize permission, advance a transition, or attest execution or effect"
)

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "--golden", "repo_root"],
    "positive": "test_registry_classifies_planning_projector_without_authority",
    "negative": "test_scope_physical_authority_and_roadmap_drift_fail_closed",
}
SKILL_MARKERS = [
    "ADR-0069", "forge capability-ownership project --catalog FILE|- --mapping FILE|-",
    "exactly one", "PROJECTED_PLANNING_CAPABILITY_OWNERSHIP_ONLY",
    "physical_resolution:not_performed", "skill_availability:not_evaluated",
    "TO_BE_REPLACED_BY_SCHEMA_PIN", "forge accept",
]
DOC_MARKERS = {
    ".agent/AGENTS.md": "ADR-0069",
    ".agent/ARCHITECTURE.md": "Planning Capability Ownership Projection v1",
    ".agent/ROADMAP.md": "Planning Capability Ownership Projection v1",
    ".agent/CURRENT_SPRINT.md": "ADR 0069",
    ".agent/DECISIONS.md": "Planning Capability Ownership Projection v1",
    ".agent/engineering/README.md": "ADR-0069",
    "docs/design/ai-engineering-os/README.md": "ADR-0069",
    "docs/design/ai-engineering-os/governance-contracts.md": "ADR-0069",
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": "Planning Capability Ownership Projection v1",
}
IGNORED_SOURCE_FIELD_GROUPS = {
    "catalog_mapping_fields": [
        "authority_semantics", "canonical_vocabulary", "control_plane_joins",
        "gates", "risk_levels", "universal_node_contract",
    ],
    "catalog_string_fields": ["decision_ref", "runtime_note"],
    "catalog_string_array_fields": ["extension_decision_refs"],
    "catalog_node_string_fields": ["name", "owner_lens", "purpose"],
    "catalog_node_generic_array_fields": [
        "activities", "authority", "entry_criteria", "escalation", "exit_criteria",
        "forbidden", "handoff", "inputs", "memory_updates", "outputs",
        "quality_gates", "rules",
    ],
    "mapping_string_fields": ["skill_specification"],
    "mapping_string_array_fields": ["mapping_rules"],
}
SEMANTIC_SOURCE_BOUNDS = {
    "catalog_nodes": "1_to_64_with_unique_two_digit_node_ids",
    "capability_identifier": "1_to_160_UTF8_bytes_matching_[a-z0-9][a-z0-9._:/-]*",
    "catalog_capability_coverage": "at_most_512_unique_and_at_most_4096_total_occurrences; each_node_capabilities_is_1_to_512_unique_identifiers",
    "mapping_packages": "1_to_64_with_unique_skill_names",
    "skill_name": "1_to_160_UTF8_bytes_matching_[a-z0-9][a-z0-9._-]*_and_derived_logical_adapter_ref_at_most_192_UTF8_bytes",
    "implementation_wave": "integer_1_to_6",
    "package_includes": "1_to_512_unique_capability_identifiers",
    "mapping_capability_coverage": "at_most_512_globally_unique_primary_owned_capabilities",
    "coverage_equality": "catalog_unique_capability_set_equals_mapping_includes_set",
}
YAML_GRAMMAR_MARKERS = {
    "block_node_placement": ("document root", "block mapping or block sequence", "only inline"),
    "blank_line_handling": ("document start", "next nonblank", "terminates folded"),
    "typed_scalar_precedence": ("before plain-string fallback", "-1", "MinInt64"),
    "block_colon_nesting_counters": ("never fall below zero", "unmatched ] or }", "later colon is top-level"),
    "document_marker_and_directive_context": ("indentation-stripped physical", "inline mapping or sequence scalar ...", "accepted as a string"),
    "flow_entry_separators": ("nonfinal", "trailing comma", "forbidden"),
    "compact_sequence_mapping_continuation": ("skipped blank lines", "exactly two spaces", "sequence line"),
    "depth_accounting": ("depth 1", "mapping keys consume no depth", "depth 17 is rejected before parsing"),
}


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("planning_capability_ownership") != PLANNING_CAPABILITY_OWNERSHIP:
        issues.append(f"{path}: planning Capability ownership contract drifted")
    scope = _mapping(data.get("scope"))
    evaluators = scope.get("shipped_evaluators") or []
    expected = [
        "local_go_package_impact_prescan", "graph_snapshot",
        "graph_snapshot_test_source", "architecture_decision_record_v2",
        "capability_registry", "planning_capability_ownership",
        "project_source_snapshot",
    ]
    if evaluators != expected:
        issues.append(f"{path}: shipped pure evaluator scope drifted")
    forbidden = sum((scope.get(name) or [] for name in (
        "shipped_kinds", "shipped_contract_only_kinds", "shipped_producers",
        "shipped_projectors", "shipped_runtime_profiles", "planned_kinds")), [])
    if any(value in forbidden for value in (
            "planning_capability_ownership", "PlanningCapabilityOwnershipProjection")):
        issues.append(f"{path}: planning projection cannot be runtime/kind authority")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: planning ownership non-capability boundary drifted")
    refs, implementations = _mapping(data.get("canonical_refs")), _mapping(
        data.get("reference_implementations"))
    for field, expected_value in CANONICAL_REFS.items():
        if refs.get(field) != expected_value:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected_value in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected_value:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def _schema_boundary_issues(schema, path):
    issues = []
    delivery = _mapping(schema.get("x-forgeos-delivery"))
    if delivery.get("scaffold_does_not_copy") != ["catalyst_go_runtime"]:
        issues.append(f"{path}: scaffold copy boundary drifted")
    if delivery.get("scaffold_does_not_generate") != [
            "declared_owner_skill_packages", "projection_derived_host_adapters"]:
        issues.append(f"{path}: scaffold generation boundary drifted")
    if delivery.get(
            "existing_same_name_markdown_is_physical_resolution_or_availability_evidence"
    ) is not False:
        issues.append(f"{path}: existing same-name non-evidence boundary drifted")
    named_input = delivery.get("named_file_input")
    if not isinstance(named_input, str) or any(marker not in named_input for marker in (
            "nonempty regular leaf", "leaf symlink directory and special file",
            "identity type mode size and modification-time")):
        issues.append(f"{path}: named file input boundary drifted")
    if delivery.get("named_file_input_nonclaims") != [
            "parent_component_symlinks_are_not_prohibited", "no_directory_confinement",
            "no_current_repository_source_attestation", "no_atomic_repository_snapshot",
            "no_stability_outside_the_individual_read"]:
        issues.append(f"{path}: named file input nonclaims drifted")
    platform = delivery.get("named_file_input_platform")
    if not isinstance(platform, str) or any(marker not in platform for marker in (
            "Unix additionally compares change-time", "Windows relies on reparse-point",
            "does not separately compare change-time", "Go platforms without")):
        issues.append(f"{path}: named file input platform boundary drifted")
    yaml_profile = _mapping(schema.get("x-forgeos-yaml-source-profile"))
    for field, markers in YAML_GRAMMAR_MARKERS.items():
        value = yaml_profile.get(field)
        if not isinstance(value, str) or any(marker not in value for marker in markers):
            issues.append(f"{path}: YAML source profile {field} drifted")
    source_semantics = _mapping(schema.get("x-forgeos-source-semantics"))
    ignored_shapes = _mapping(source_semantics.get("ignored_source_field_shapes"))
    for group, fields in IGNORED_SOURCE_FIELD_GROUPS.items():
        shape = _mapping(ignored_shapes.get(group))
        if shape.get("fields") != fields or not isinstance(shape.get("shape"), str):
            issues.append(f"{path}: ignored source shape {group} drifted")
    if source_semantics.get("ignored_field_interpretation") != "none":
        issues.append(f"{path}: ignored source interpretation drifted")
    if source_semantics.get("semantic_source_bounds") != SEMANTIC_SOURCE_BOUNDS:
        issues.append(f"{path}: semantic source bounds drifted")
    return issues


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(path, label=SCHEMA_RELATIVE, max_bytes=1_048_576)
        schema = json.loads(raw.decode("utf-8"))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate planning ownership Schema: {error}"]
    issues = _schema_boundary_issues(schema, path)
    if hashlib.sha256(raw).hexdigest() != SCHEMA_SHA256:
        issues.append(f"{path}: planning ownership Schema physical pin drifted")
    authority = _mapping(schema.get("x-forgeos-authority-semantics"))
    if authority.get("positive_result") != RESULT or authority.get("attestations") != []:
        issues.append(f"{path}: planning-only authority result drifted")
    if authority.get("registry_input_or_mutation") is not False:
        issues.append(f"{path}: Registry boundary drifted")
    observation = _mapping(_mapping(schema.get("x-forgeos-projection")).get(
        "current_source_observation"))
    expected_observation = {
        "catalog_bytes": CATALOG_BYTES, "catalog_nodes": 17,
        "catalog_occurrences": 145, "catalog_sha256": CATALOG_SHA256,
        "mapping_bytes": MAPPING_BYTES, "mapping_packages": 38,
        "mapping_sha256": MAPPING_SHA256, "unique_capabilities_and_bindings": 140,
    }
    if observation != expected_observation:
        issues.append(f"{path}: exact planning source observation drifted")
    return issues


def fixture_and_source_issues(repo_root):
    issues = []
    for relative, size, digest in (
        (CATALOG_RELATIVE, CATALOG_BYTES, CATALOG_SHA256),
        (MAPPING_RELATIVE, MAPPING_BYTES, MAPPING_SHA256),
        (FIXTURE_RELATIVE, FIXTURE_BYTES, FIXTURE_SHA256),
    ):
        path = repo_root / relative
        try:
            raw = read_bounded_file(path, label=relative, max_bytes=1_048_576)
        except (OSError, ContractError) as error:
            issues.append(f"{path}: cannot validate physical pin: {error}")
            continue
        if len(raw) != size or hashlib.sha256(raw).hexdigest() != digest:
            issues.append(f"{path}: physical byte/hash pin drifted")
    try:
        golden = load_golden(repo_root)
        projection = _mapping(golden.get("projection"))
        request = _mapping(golden.get("request"))
        if request.get("request_sha256") != REQUEST_SHA256:
            issues.append(f"{repo_root / FIXTURE_RELATIVE}: request digest drifted")
        if projection.get("projection_sha256") != PROJECTION_SHA256:
            issues.append(f"{repo_root / FIXTURE_RELATIVE}: projection digest drifted")
    except (OSError, ContractError, ValueError) as error:
        issues.append(f"{repo_root / FIXTURE_RELATIVE}: golden rejected: {error}")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.planning_capability_ownership")
    if not isinstance(detector, dict):
        return ["planning Capability ownership shadow detector is missing"]
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    issues = []
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("planning ownership detector requires exact golden arguments")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("planning ownership detector must remain shadow/non-load-bearing")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"planning ownership detector {polarity} test drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate planning ownership Skill: {error}"]
    markers = [marker for marker in SKILL_MARKERS if marker != "TO_BE_REPLACED_BY_SCHEMA_PIN"]
    return [f"{path}: missing planning ownership marker {marker!r}"
            for marker in markers if marker not in text]


def wiring_issues(repo_root, agent_root):
    activation, error = load_yaml(agent_root / "engineering/activation.yml")
    extension = _mapping(activation.get("canonical_extension_refs")) if not error else {}
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, expected in ACTIVATION_REFS.items()
              if extension.get(field) != expected]
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if route_error or discipline_error:
        return issues + ["planning ownership route/discipline registry unreadable"]
    required = {SKILL_RELATIVE, SCHEMA_RELATIVE}
    for route_id in ("governance", "architecture-boundary"):
        route = next((item for item in routes["routes"] if item.get("id") == route_id), {})
        if not required.issubset({item.get("ref") for item in route.get("include") or []}):
            issues.append(f"planning ownership {route_id} route is incomplete")
    contract = next((item for item in disciplines["disciplines"]
                     if item.get("id") == "contract"), {})
    if not required.issubset(set(contract.get("assets") or [])):
        issues.append("planning ownership contract discipline assets incomplete")
    return issues


def documentation_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    issues = []
    for relative, marker in DOC_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ADR-0069 delivery: {error}")
            continue
        if marker not in text:
            issues.append(f"{path}: missing ADR-0069 delivery marker {marker!r}")
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(roadmap, label=str(roadmap)).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate ownership roadmap: {error}"]
    required = [
        "- [x] 校验 catalog fine capability → package primary owner 全覆盖且唯一，并从 mapping 生成 adapter 引用；",
        "- [ ] 按 `implementation_wave` 逐 package 实现 Skill",
        "- [ ] `harness/check.py` 校验 capability↔role↔workflow↔artifact↔gate↔permission",
        "- [ ] 定义 CapabilityPluginManifest",
    ]
    issues.extend(f"{roadmap}: roadmap boundary {marker!r} missing"
                  for marker in required if marker not in text)
    return issues


def adr_issues(repo_root):
    path = repo_root / DECISION_RELATIVE
    try:
        metadata = validate_document_file(path)
        text = read_bounded_file(path, label=DECISION_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: ADR-0069 v2 validation failed: {error}"]
    issues = []
    if metadata.get("status") != "proposed":
        issues.append(f"{path}: ADR-0069 must remain proposed")
    for marker in (
        "forge capability-ownership project --catalog FILE|- --mapping FILE|-",
        "This delivered proposed slice", "does not generate an adapter file",
        "The document root is a block map or block sequence",
        "never fall below zero", "inline mapping or sequence scalar `...`",
        "Specifically, catalog `authority_semantics`",
        "Every nonfinal flow-map or flow-sequence entry",
        "There are 1–64 packages",
    ):
        if marker not in text:
            issues.append(f"{path}: missing delivered-boundary marker {marker!r}")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(fixture_and_source_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(wiring_issues(repo_root, agent_root))
    issues.extend(documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    return issues
