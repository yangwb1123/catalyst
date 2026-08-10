"""ADR-0051 local command-observation producer governance freeze."""
import json

from governance_contract import ContractError, read_bounded_file


LOCAL_COMMAND_PRODUCER_SUCCESS = (
    "OBSERVED_LOCAL_PROCESS (local process capture only; no pass, criterion, "
    "completion, truth, authority, identity, persistence, or external-effect "
    "attestation)"
)
LOCAL_COMMAND_PRODUCER_SKILL_MARKERS = (
    "### Local gate command observation producer 分支",
    "GateObservedWith",
    "ProbeAllObservedWith",
    "PURE_CONTRACT_FIXTURE",
    "capture disabled",
    "不会把 exit=0 当作 PASS",
)
LOCAL_GATE_COMMAND_OBSERVATION_PRODUCER = {
    "api_version": (
        "forgeos.governance.local-gate-command-observation-production/v1"
    ),
    "mode": "explicit_opt_in_unix_local_process_observation_production",
    "output_kind": (
        "forgeos.governance.local-gate-command-observation-production/v1"
    ),
    "observation_kind": "forgeos.command-observation/v1",
    "command_classes": {
        "gate": ["node", "harness/gate.mjs"],
        "check": ["python3", "harness/check.py", "."],
        "accept": ["node", "harness/acceptance.mjs"],
        "probe_all": ["node", "harness/acceptance.mjs", "--json"],
    },
    "exact_execution": {
        "logical_cwd": ".",
        "stdin": "empty",
        "evidence_type": "gate_result",
        "capture_default": "disabled",
        "go_runtime_wiring": "explicit_opt_in_in_memory_gate_api",
        "one_observation_per_spawn": True,
        "non_unix_result": "fail_closed_before_spawn",
    },
    "profiles": {
        "environment": {
            "api_version": "forgeos.command-capture.environment/v1",
            "profile_id": "scrubbed-parent-environment-v1",
            "actual_child_environment_matches_manifest": True,
        },
        "tool": {
            "api_version": "forgeos.command-capture.tool/v1",
            "profile_id": "resolved-top-level-executable-v1",
            "coverage": "top_level_executable_only",
        },
        "source": {
            "api_version": "forgeos.command-capture.source-tree/v1",
            "profile_id": "git-worktree-source-tree-v1",
            "coverage": "tracked_stage0_and_nonignored_untracked",
        },
    },
    "identity": {
        "environment_digest_domain": (
            "forgeos.governance.local-command-environment-profile.v1\0"
        ),
        "tool_digest_domain": (
            "forgeos.governance.local-command-tool-profile.v1\0"
        ),
        "source_digest_domain": (
            "forgeos.governance.local-command-source-tree-profile.v1\0"
        ),
        "production_digest_domain": (
            "forgeos.governance.local-command-observation-production.v1\0"
        ),
    },
    "constants": {
        "producer_id": "forgeos.local-gate-command-observer",
        "producer_type": "tool",
        "producer_version": "v1",
        "canonicalization": "forgeos.canonical-json/v1",
    },
    "limits": {
        "max_canonical_manifest_bytes": 16777216,
        "max_observation_bytes": 131072,
        "max_environment_variables": 256,
        "max_source_entries": 65536,
        "max_individual_file_bytes": 1073741824,
        "max_source_content_bytes": 8589934592,
        "max_symlink_hops": 32,
        "max_text_scalars": 4096,
        "max_text_bytes": 16384,
        "max_timeout_ms": 86400000,
        "integer_domain": "signed_int64",
        "exit_code_domain": "portable_nonnegative_signed_int32",
    },
    "positive_result": LOCAL_COMMAND_PRODUCER_SUCCESS,
    "attests": [],
    "persistence": "none",
}
LOCAL_COMMAND_PRODUCER_CANONICAL_REFS = {
    "local_gate_command_observation_producer_schema": (
        "docs/contracts/local-gate-command-observation-producer-v1.schema.json"
    ),
    "local_gate_command_observation_producer_golden_fixture": (
        "docs/contracts/fixtures/local-gate-command-observation-producer-v1.json"
    ),
    "local_gate_command_observation_producer_checker": (
        "harness/local_command_observation_producer_check.py"
    ),
    "local_gate_command_observation_producer_decision": (
        "docs/adr/0051-local-gate-command-observation-producer-v1.md"
    ),
}
LOCAL_COMMAND_PRODUCER_REFERENCE_IMPLEMENTATION = {
    "ref": "forge-core/internal/localcommandobservationproducer",
    "projection": "catalyst_repository_only",
}
LOCAL_COMMAND_PRODUCER_CHECKER_REFERENCE_IMPLEMENTATION = {
    "ref": "harness/local_command_observation_producer_check.py",
    "projection": "universal_scaffold",
}
LOCAL_COMMAND_PRODUCER_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "environment_digest_domain": (
        "forgeos.governance.local-command-environment-profile.v1\0"
    ),
    "tool_digest_domain": "forgeos.governance.local-command-tool-profile.v1\0",
    "source_digest_domain": (
        "forgeos.governance.local-command-source-tree-profile.v1\0"
    ),
    "production_digest_domain": (
        "forgeos.governance.local-command-observation-production.v1\0"
    ),
}
LOCAL_COMMAND_PRODUCER_SCHEMA_LIMITS = (
    LOCAL_GATE_COMMAND_OBSERVATION_PRODUCER["limits"]
)
LOCAL_COMMAND_PRODUCER_SCHEMA_SEMANTIC_VALIDATION = {
    "schema_alone_sufficient": False,
    "observation_ref": "#CommandObservationV1",
    "authoritative_checks": [
        "exact_compact_canonical_utf8",
        "forbidden_unicode_and_bounded_text",
        "local_command_class_exact_argv",
        "environment_names_valid_sorted_unique_and_path_present",
        "secret_cloud_auth_and_proxy_names_absent",
        "environment_manifest_digest_matches_command",
        "tool_paths_symlink_chain_mode_bytes_and_digest",
        "tool_manifest_digest_matches_command",
        "source_revision_object_format",
        "source_entries_sorted_unique_and_complete_for_enumerated_inventory",
        "tracked_index_and_worktree_kind_compatible",
        "tracked_forge_and_gitlink_rejected_untracked_forge_excluded_release_included",
        "source_manifest_digest_and_revision_match_observation",
        "nondecreasing_time",
        "stream_count_and_digest_relations",
        "empty_stdin_and_stream_digests",
        "execution_started_drain_complete_and_projectable_termination",
    ],
}
LOCAL_COMMAND_PRODUCER_SCHEMA_CAPABILITY_BOUNDARY = {
    "result": LOCAL_COMMAND_PRODUCER_SUCCESS,
    "capture_default": "disabled",
    "go_runtime_wiring": "explicit_opt_in_in_memory_gate_api",
    "governance_binding": "caller_supplied_only",
    "persistence": "none",
    "source_snapshot_semantics": (
        "bounded_interval_inventory_and_entry_observation_not_atomic_"
        "filesystem_snapshot_or_execution_time_source_pinning"
    ),
    "journal_append": False,
    "sqlite_schema": 25,
    "attestations": [],
}


def local_command_producer_skill_marker_issues(text, path):
    if any(marker not in text for marker in LOCAL_COMMAND_PRODUCER_SKILL_MARKERS):
        return [
            f"{path}: local command observation producer guidance or "
            "non-capability boundary is missing"
        ]
    return []


def local_command_producer_registry_issues(data, path):
    key = "local_gate_command_observation_producer"
    producer = data.get(key) if isinstance(data, dict) else None
    issues = []
    if producer != LOCAL_GATE_COMMAND_OBSERVATION_PRODUCER:
        issues.append(f"{path}: {key} contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in LOCAL_COMMAND_PRODUCER_CANONICAL_REFS.items():
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
            "local_go_package_dependency_graph_observation_producer",
        ]
        or scope.get("staged_producers") != []
    ):
        issues.append(
            f"{path}: shipped/staged producer scope drifted"
        )
    implementations = (
        data.get("reference_implementations") if isinstance(data, dict) else None
    )
    if (
        not isinstance(implementations, dict)
        or implementations.get("local_gate_command_observation_producer_go")
        != LOCAL_COMMAND_PRODUCER_REFERENCE_IMPLEMENTATION
    ):
        issues.append(
            f"{path}: local producer Go reference implementation drifted"
        )
    if (
        not isinstance(implementations, dict)
        or implementations.get(
            "local_gate_command_observation_producer_python_checker"
        ) != LOCAL_COMMAND_PRODUCER_CHECKER_REFERENCE_IMPLEMENTATION
    ):
        issues.append(
            f"{path}: local producer Python checker reference drifted"
        )
    return issues


def local_command_producer_schema_issues(repo_root):
    relative = (
        "docs/contracts/local-gate-command-observation-producer-v1.schema.json"
    )
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate local command producer Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": (
            LOCAL_COMMAND_PRODUCER_SCHEMA_CANONICALIZATION
        ),
        "x-forgeos-limits": LOCAL_COMMAND_PRODUCER_SCHEMA_LIMITS,
        "x-forgeos-semantic-validation": (
            LOCAL_COMMAND_PRODUCER_SCHEMA_SEMANTIC_VALIDATION
        ),
        "x-forgeos-capability-boundary": (
            LOCAL_COMMAND_PRODUCER_SCHEMA_CAPABILITY_BOUNDARY
        ),
    }
    return [
        f"{path}: {field} drifted"
        for field, value in expected.items()
        if schema.get(field) != value
    ]
