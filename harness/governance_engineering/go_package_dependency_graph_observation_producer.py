"""ADR-0053 shipped local Go dependency observation governance freeze."""

import json

from governance_contract import ContractError, read_bounded_file


GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SUCCESS = (
    "OBSERVED_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH (all-regular-Go-file lexical "
    "import-header/source observation only; no selected build, dependency "
    "availability, compile success, architecture judgment, impact closure, "
    "completeness, truth, authority, claim, atom, persistence, or effect "
    "attestation)"
)
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SKILL_MARKERS = (
    "### Local Go package dependency graph observation producer 分支",
    "已交付",
    "selected-module-all-regular-go-files-union-v1",
    "go-parser-imports-only-no-partial-facts-v1",
    "OBSERVED_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH",
    "`go list|build|test|mod`",
    "GOOS/GOARCH",
    "source pre/post equality",
    "`go_commands=none`",
    "`network_access=none_by_profile`",
    "不得由该 package 自动生成 Evidence/Claim/Atom",
)
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_RUNTIME_PLATFORM = {
    "capture_default": "disabled",
    "supported_family": "unix",
    "non_unix_behavior": (
        "fail_closed_before_repository_process_or_file_observation"
    ),
    "parent_environment": "ignored_except_explicit_single_path_input",
    "git_child_environment": "fixed_home_lang_lc_all_path_and_git_flags_only",
    "git_commands": "fixed_read_only_rev_parse_and_ls_files_only",
    "git_binary_authentication": False,
    "go_commands": "none",
    "module_cache_access": "none",
    "network_access": "none_by_profile",
    "sandbox_egress_and_external_effect_containment": False,
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_NON_CAPABILITY = (
    "Local Go package dependency graph observation producer is explicit "
    "opt-in Unix lexical source observation only; it executes no Go toolchain "
    "command, resolves no actual GOOS/GOARCH/build tags/cgo/module cache/"
    "workspace/vendor/replace semantics, and attests no dependency availability, "
    "selected build, compile/test/runtime reachability, graph completeness, "
    "architecture judgment, impact closure, truth or authority; it does not "
    "authenticate Git, create Evidence/Claim/Atom/Context/Grant/Impact/Cost/Risk, "
    "append journal, write SQLite, persist data, or contain sandbox/egress/"
    "external effects"
)
LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH_OBSERVATION_PRODUCER = {
    "api_version": (
        "forgeos.governance.local-go-package-dependency-graph-observation-"
        "production/v1"
    ),
    "mode": (
        "explicit_opt_in_unix_local_go_package_dependency_graph_observation_"
        "production"
    ),
    "output_kind": (
        "forgeos.governance.local-go-package-dependency-graph-observation-"
        "production/v1"
    ),
    "observation_kind": "forgeos.go-package-dependency-graph-observation/v1",
    "exact_capture": {
        "capture_default": "disabled",
        "go_runtime_wiring": "explicit_opt_in_catalyst_go_api",
        "module_selection": "caller_supplied_regular_go_mod_in_shared_source",
        "file_selection_profile_id": (
            "selected-module-all-regular-go-files-union-v1"
        ),
        "parser_profile_id": "go-parser-imports-only-no-partial-facts-v1",
        "import_resolution_profile_id": (
            "selected-module-lexical-import-resolution-v1"
        ),
        "source_interval": "pre_and_post_source_exact_equality_required",
        "one_timestamp_per_graph": True,
        "non_unix_result": (
            "fail_closed_before_repository_process_or_file_observation"
        ),
    },
    "profiles": {
        "parameters": {
            "api_version": (
                "forgeos.go-package-dependency-capture.parameters/v1"
            ),
            "file_selection_profile_id": (
                "selected-module-all-regular-go-files-union-v1"
            ),
            "import_resolution_profile_id": (
                "selected-module-lexical-import-resolution-v1"
            ),
            "module_profile_id": "selected-go-mod-module-directive-v1",
            "parser_profile_id": "go-parser-imports-only-no-partial-facts-v1",
            "source_profile_id": "git-worktree-source-tree-v1",
        },
        "graph": {
            "api_version": (
                "forgeos.go-package-dependency-graph-observation/v1"
            ),
            "profile_id": "selected-go-module-lexical-dependency-graph-v1",
            "coverage": "selected_source_partition_and_stable_diagnostics",
        },
        "source": {
            "api_version": "forgeos.command-capture.source-tree/v1",
            "profile_id": "git-worktree-source-tree-v1",
            "coverage": "tracked_stage0_and_nonignored_untracked",
        },
    },
    "runtime_platform": GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_RUNTIME_PLATFORM,
    "identity": {
        "parameters_digest_domain": (
            "forgeos.governance.local-go-package-dependency-graph-"
            "parameters.v1\0"
        ),
        "graph_digest_domain": (
            "forgeos.governance.local-go-package-dependency-graph-"
            "observation.v1\0"
        ),
        "source_digest_domain": (
            "forgeos.governance.local-command-source-tree-profile.v1\0"
        ),
        "production_digest_domain": (
            "forgeos.governance.local-go-package-dependency-graph-"
            "observation-production.v1\0"
        ),
    },
    "constants": {
        "canonicalization": "forgeos.canonical-json/v1",
        "producer_id": "forgeos.local-go-package-dependency-graph-observer",
        "producer_type": "tool",
        "producer_version": "v1",
    },
    "limits": {
        "max_canonical_production_bytes": 16777216,
        "max_go_mod_bytes": 1048576,
        "max_go_file_parser_bytes": 4194304,
        "max_aggregate_parser_input_bytes": 67108864,
        "max_selected_regular_go_files": 16384,
        "max_nested_modules": 1024,
        "max_imports_per_file": 1024,
        "max_import_occurrences": 65536,
        "max_packages": 16384,
        "max_dependency_edges": 65536,
        "max_diagnostics": 16384,
        "max_source_entries": 65536,
        "max_individual_source_file_bytes": 1073741824,
        "max_source_content_bytes": 8589934592,
        "max_text_scalars": 4096,
        "max_text_bytes": 16384,
        "max_run_id_bytes": 160,
        "integer_domain": "signed_int64",
    },
    "positive_result": GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SUCCESS,
    "attests": [],
    "persistence": "none",
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_CANONICAL_REFS = {
    "local_go_package_dependency_graph_observation_producer_schema": (
        "docs/contracts/local-go-package-dependency-graph-observation-"
        "producer-v1.schema.json"
    ),
    "local_go_package_dependency_graph_observation_producer_golden_fixture": (
        "docs/contracts/fixtures/local-go-package-dependency-graph-"
        "observation-producer-v1.json"
    ),
    "local_go_package_dependency_graph_observation_producer_checker": (
        "harness/go_package_dependency_graph_observation_producer/check.py"
    ),
    "local_go_package_dependency_graph_observation_producer_decision": (
        "docs/adr/0053-local-go-package-dependency-graph-observation-"
        "producer-v1.md"
    ),
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_REFERENCE_IMPLEMENTATION = {
    "ref": "forge-core/internal/gopackagedependencyobservationproducer",
    "projection": "catalyst_repository_only",
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_CHECKER_REFERENCE_IMPLEMENTATION = {
    "ref": "harness/go_package_dependency_graph_observation_producer/check.py",
    "projection": "universal_scaffold",
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "parameters_digest_domain": (
        "forgeos.governance.local-go-package-dependency-graph-parameters.v1\0"
    ),
    "graph_digest_domain": (
        "forgeos.governance.local-go-package-dependency-graph-observation.v1\0"
    ),
    "source_digest_domain": (
        "forgeos.governance.local-command-source-tree-profile.v1\0"
    ),
    "production_digest_domain": (
        "forgeos.governance.local-go-package-dependency-graph-observation-"
        "production.v1\0"
    ),
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_LIMITS = (
    LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH_OBSERVATION_PRODUCER["limits"]
)
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_RUNTIME_PLATFORM = (
    GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_RUNTIME_PLATFORM
)
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_SEMANTIC_VALIDATION = {
    "schema_alone_sufficient": False,
    "python_checker_parses_go": False,
    "authoritative_checks": [
        "exact_compact_canonical_utf8_and_bounded_values",
        "fixed_parameter_profiles_and_domain_digest",
        "shared_source_profile_and_domain_digest",
        "selected_go_mod_regular_source_metadata_binding_and_canonical_"
        "observed_module_path",
        "source_derived_regular_and_symlink_nested_module_boundaries",
        "selected_regular_go_file_partition_and_coverage_accounting",
        "source_bound_success_files_and_one_diagnostic_per_failed_file",
        "file_derived_package_grouping_and_test_only_null_import_path",
        "lexical_import_occurrence_aggregation_and_candidate_resolution",
        "graph_source_parameters_producer_and_timestamp_binding",
    ],
}
GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_CAPABILITY_BOUNDARY = {
    "result": GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SUCCESS,
    "capture_default": "disabled",
    "go_runtime_wiring": "explicit_opt_in_catalyst_go_api",
    "governance_binding": "caller_supplied_only",
    "automatic_evidence_binding": False,
    "resolution_semantics": (
        "lexical_candidates_only_not_dependency_or_compile_resolution"
    ),
    "selected_build_semantics": "none",
    "compile_test_runtime_reachability": "not_evaluated",
    "goos_goarch_build_tags_cgo_semantics": "not_evaluated",
    "workspace_vendor_replace_module_graph_semantics": "not_resolved",
    "architecture_judgment": "none",
    "impact_analysis_or_closure": "none",
    "source_snapshot_semantics": (
        "pre_and_post_equal_bounded_interval_inventory_and_file_observation_"
        "not_atomic_"
        "filesystem_snapshot"
    ),
    "git_binary_authentication": False,
    "sandbox_egress_and_external_effect_containment": False,
    "persistence": "none",
    "journal_append": False,
    "sqlite_schema": 26,
    "attestations": [],
}


def go_package_dependency_graph_producer_skill_marker_issues(text, path):
    markers = GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SKILL_MARKERS
    if any(marker not in text for marker in markers):
        return [
            f"{path}: local Go dependency graph producer guidance or "
            "non-capability boundary is missing"
        ]
    return []


def go_package_dependency_graph_producer_registry_issues(data, path):
    key = "local_go_package_dependency_graph_observation_producer"
    producer = data.get(key) if isinstance(data, dict) else None
    issues = []
    if producer != LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH_OBSERVATION_PRODUCER:
        issues.append(f"{path}: {key} contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        expected_refs = GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_CANONICAL_REFS
        for field, expected in expected_refs.items():
            if refs.get(field) != expected:
                issues.append(
                    f"{path}: canonical_refs.{field} must remain {expected!r}"
                )
    scope = data.get("scope") if isinstance(data, dict) else None
    if (
        not isinstance(scope, dict)
        or scope.get("shipped_producers") != [
            "local_gate_command_observation_producer",
            "local_evolve_repo_locator_observation_producer",
            key,
            "local_project_source_snapshot_producer",
        ]
        or scope.get("staged_producers") != []
    ):
        issues.append(f"{path}: shipped/staged producer scope drifted")
    implementations = (
        data.get("reference_implementations") if isinstance(data, dict) else None
    )
    if (
        not isinstance(implementations, dict)
        or implementations.get(
            "local_go_package_dependency_graph_observation_producer_go"
        ) != GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_REFERENCE_IMPLEMENTATION
    ):
        issues.append(f"{path}: local Go dependency producer reference drifted")
    if (
        not isinstance(implementations, dict)
        or implementations.get(
            "local_go_package_dependency_graph_observation_producer_python_checker"
        ) != GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_CHECKER_REFERENCE_IMPLEMENTATION
    ):
        issues.append(f"{path}: local Go dependency checker reference drifted")
    non_capabilities = data.get("non_capabilities") if isinstance(data, dict) else None
    if (not isinstance(non_capabilities, list) or
            GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_NON_CAPABILITY not in non_capabilities):
        issues.append(f"{path}: local Go dependency non-capability drifted")
    return issues


def go_package_dependency_graph_producer_schema_issues(repo_root):
    relative = (
        "docs/contracts/local-go-package-dependency-graph-observation-"
        "producer-v1.schema.json"
    )
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate local Go dependency Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": (
            GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_CANONICALIZATION
        ),
        "x-forgeos-limits": GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_LIMITS,
        "x-forgeos-runtime-platform": (
            GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_RUNTIME_PLATFORM
        ),
        "x-forgeos-semantic-validation": (
            GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_SEMANTIC_VALIDATION
        ),
        "x-forgeos-capability-boundary": (
            GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_SCHEMA_CAPABILITY_BOUNDARY
        ),
    }
    return [
        f"{path}: {field} drifted"
        for field, value in expected.items()
        if schema.get(field) != value
    ]
