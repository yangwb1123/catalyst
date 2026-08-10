"""ADR-0052 local Evolve locator observation producer governance freeze."""

import json

from governance_contract import ContractError, read_bounded_file


EVOLVE_LOCATOR_PRODUCER_SUCCESS = (
    "CAPTURED_LOCAL_EVOLVE_LOCATOR_SET (local report/source capture only; "
    "locator set may be empty; no scan judgment, completion, truth, authority, "
    "claim, atom, persistence, or effect attestation)"
)
EVOLVE_LOCATOR_PRODUCER_SKILL_MARKERS = (
    "### Local Evolve locator observation producer 分支",
    "CAPTURED_LOCAL_EVOLVE_LOCATOR_SET",
    "跨 relation/path 不去重",
    "bounded-interval",
    "不自动调用 ADR-0050",
    "TMPDIR",
)
LOCAL_EVOLVE_LOCATOR_OBSERVATION_PRODUCER = {
    "api_version": (
        "forgeos.governance.local-evolve-repo-locator-observation-production/v1"
    ),
    "mode": "explicit_opt_in_unix_local_evolve_locator_observation_production",
    "output_kind": (
        "forgeos.governance.local-evolve-repo-locator-observation-production/v1"
    ),
    "observation_kind": "forgeos.evolve-repo-locator/v1",
    "exact_capture": {
        "capture_default": "disabled",
        "go_runtime_wiring": "explicit_opt_in_catalyst_go_api",
        "report_input": "already_returned_complete_evolve_scan_output",
        "locator_set": "zero_or_more_preserving_cross_relation_duplicates",
        "one_timestamp_per_set": True,
        "non_unix_result": "fail_closed_before_repository_or_process_observation",
    },
    "profiles": {
        "parameters": {
            "api_version": "forgeos.evolve-capture.parameters/v1",
            "contract": "evolve_scan_v1",
            "report_profile_id": "evolve-scan-canonical-marker-v1",
            "source_profile_id": "git-worktree-source-tree-v1",
        },
        "report": {
            "api_version": "forgeos.evolve-capture.report/v1",
            "profile_id": "evolve-scan-canonical-marker-v1",
            "coverage": "complete_canonical_marker_line_preimage",
        },
        "source": {
            "api_version": "forgeos.command-capture.source-tree/v1",
            "profile_id": "git-worktree-source-tree-v1",
            "coverage": "tracked_stage0_and_nonignored_untracked",
        },
    },
    "local_execution": {
        "parent_environment": "ignore_all_except_exactly_one_path",
        "tmpdir_forwarded": False,
        "git_child_environment": "fixed_home_lang_lc_all_path_and_git_flags_only",
        "git_commands": "fixed_read_only_rev_parse_and_ls_files_only",
        "git_binary_authenticated": False,
        "sandbox_egress_effect_containment": False,
    },
    "identity": {
        "parameters_digest_domain": (
            "forgeos.governance.local-evolve-repo-locator-parameters.v1\0"
        ),
        "report_digest_profile": "raw_sha256_of_complete_canonical_marker_line",
        "source_digest_domain": (
            "forgeos.governance.local-command-source-tree-profile.v1\0"
        ),
        "production_digest_domain": (
            "forgeos.governance.local-evolve-repo-locator-observation-production.v1\0"
        ),
    },
    "constants": {
        "canonicalization": "forgeos.canonical-json/v1",
        "producer_id": "forgeos.local-evolve-repo-locator-observer",
        "producer_type": "tool",
        "producer_version": "v1",
    },
    "limits": {
        "max_canonical_production_bytes": 16777216,
        "max_observations": 240,
        "max_report_payload_bytes": 65536,
        "max_report_bytes": 65552,
        "max_report_opportunities": 24,
        "max_evidence_per_relation": 8,
        "max_evidence_file_bytes": 1048576,
        "max_source_entries": 65536,
        "max_individual_source_file_bytes": 1073741824,
        "max_source_content_bytes": 8589934592,
        "max_text_scalars": 4096,
        "max_text_bytes": 16384,
        "max_run_id_bytes": 160,
        "integer_domain": "signed_int64",
    },
    "positive_result": EVOLVE_LOCATOR_PRODUCER_SUCCESS,
    "attests": [],
    "persistence": "none",
}
EVOLVE_LOCATOR_PRODUCER_CANONICAL_REFS = {
    "local_evolve_repo_locator_observation_producer_schema": (
        "docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json"
    ),
    "local_evolve_repo_locator_observation_producer_golden_fixture": (
        "docs/contracts/fixtures/"
        "local-evolve-repo-locator-observation-producer-v1.json"
    ),
    "local_evolve_repo_locator_observation_producer_checker": (
        "harness/evolve_locator_observation_producer/check.py"
    ),
    "local_evolve_repo_locator_observation_producer_decision": (
        "docs/adr/0052-local-evolve-repo-locator-observation-producer-v1.md"
    ),
}
EVOLVE_LOCATOR_PRODUCER_REFERENCE_IMPLEMENTATION = {
    "ref": "forge-core/internal/evolvelocatorobservationproducer",
    "projection": "catalyst_repository_only",
}
EVOLVE_LOCATOR_PRODUCER_CHECKER_REFERENCE_IMPLEMENTATION = {
    "ref": "harness/evolve_locator_observation_producer/check.py",
    "projection": "universal_scaffold",
}
EVOLVE_LOCATOR_PRODUCER_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "parameters_digest_domain": (
        "forgeos.governance.local-evolve-repo-locator-parameters.v1\0"
    ),
    "report_digest_profile": "raw_sha256_of_complete_canonical_marker_line",
    "source_digest_domain": (
        "forgeos.governance.local-command-source-tree-profile.v1\0"
    ),
    "production_digest_domain": (
        "forgeos.governance.local-evolve-repo-locator-observation-production.v1\0"
    ),
}
EVOLVE_LOCATOR_PRODUCER_SCHEMA_LIMITS = (
    LOCAL_EVOLVE_LOCATOR_OBSERVATION_PRODUCER["limits"]
)
EVOLVE_LOCATOR_PRODUCER_SCHEMA_SEMANTIC_VALIDATION = {
    "schema_alone_sufficient": False,
    "observation_ref": "#EvolveRepoLocatorV1",
    "authoritative_checks": [
        "exact_compact_canonical_utf8",
        "forbidden_unicode_float_bool_integer_and_bounded_text",
        "parameters_fixed_profile_and_domain_digest",
        "complete_canonical_report_marker_preimage_bytes_and_digest",
        "report_shape_depth_dimension_relation_and_opportunity_matrix",
        "report_dimensions_and_opportunities_canonical_order",
        "shared_source_revision_entries_profile_and_domain_digest",
        "source_entries_sorted_unique_and_complete_for_enumerated_inventory",
        "report_locator_maps_to_bounded_regular_source_entry",
        "observations_exact_report_order_and_cross_relation_multiplicity",
        "observation_content_source_report_parameters_and_capture_binding",
    ],
}
EVOLVE_LOCATOR_PRODUCER_SCHEMA_CAPABILITY_BOUNDARY = {
    "result": EVOLVE_LOCATOR_PRODUCER_SUCCESS,
    "capture_default": "disabled",
    "go_runtime_wiring": "explicit_opt_in_catalyst_go_api",
    "governance_binding": "caller_supplied_only",
    "automatic_adr0050_binding": False,
    "locator_set_cardinality": (
        "zero_or_more_preserving_cross_relation_duplicates"
    ),
    "source_snapshot_semantics": (
        "bounded_interval_inventory_and_entry_observation_not_atomic_"
        "filesystem_snapshot_or_execution_time_source_pinning"
    ),
    "git_binary_authentication": False,
    "sandbox_egress_and_external_effect_containment": False,
    "persistence": "none",
    "journal_append": False,
    "sqlite_schema": 25,
    "attestations": [],
}


def evolve_locator_producer_skill_marker_issues(text, path):
    if any(marker not in text for marker in EVOLVE_LOCATOR_PRODUCER_SKILL_MARKERS):
        return [
            f"{path}: local Evolve locator observation producer guidance or "
            "non-capability boundary is missing"
        ]
    return []


def evolve_locator_producer_registry_issues(data, path):
    key = "local_evolve_repo_locator_observation_producer"
    producer = data.get(key) if isinstance(data, dict) else None
    issues = []
    if producer != LOCAL_EVOLVE_LOCATOR_OBSERVATION_PRODUCER:
        issues.append(f"{path}: {key} contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in EVOLVE_LOCATOR_PRODUCER_CANONICAL_REFS.items():
            if refs.get(field) != expected:
                issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    scope = data.get("scope") if isinstance(data, dict) else None
    if (not isinstance(scope, dict) or
            scope.get("staged_producers") != [] or
            scope.get("shipped_producers") != [
                "local_gate_command_observation_producer", key,
                "local_go_package_dependency_graph_observation_producer",
            ]):
        issues.append(f"{path}: shipped/staged producer scope drifted")
    implementations = (data.get("reference_implementations")
                       if isinstance(data, dict) else None)
    if (not isinstance(implementations, dict) or
            implementations.get("local_evolve_locator_observation_producer_go") !=
            EVOLVE_LOCATOR_PRODUCER_REFERENCE_IMPLEMENTATION):
        issues.append(f"{path}: local Evolve producer Go reference drifted")
    if (not isinstance(implementations, dict) or
            implementations.get(
                "local_evolve_locator_observation_producer_python_checker"
            ) != EVOLVE_LOCATOR_PRODUCER_CHECKER_REFERENCE_IMPLEMENTATION):
        issues.append(f"{path}: local Evolve producer Python checker drifted")
    return issues


def evolve_locator_producer_schema_issues(repo_root):
    relative = (
        "docs/contracts/local-evolve-repo-locator-observation-producer-v1."
        "schema.json"
    )
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate local Evolve producer Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": EVOLVE_LOCATOR_PRODUCER_SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": EVOLVE_LOCATOR_PRODUCER_SCHEMA_LIMITS,
        "x-forgeos-semantic-validation": EVOLVE_LOCATOR_PRODUCER_SCHEMA_SEMANTIC_VALIDATION,
        "x-forgeos-capability-boundary": EVOLVE_LOCATOR_PRODUCER_SCHEMA_CAPABILITY_BOUNDARY,
    }
    return [f"{path}: {field} drifted" for field, value in expected.items()
            if schema.get(field) != value]
