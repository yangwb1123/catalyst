"""ADR-0058 narrow authenticated bootstrap repo-read execution governance."""

from __future__ import annotations

import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/bootstrap-repo-read-execution-v1.schema.json"
FIXTURE_RELATIVE = (
    "docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json"
)
CHECKER_RELATIVE = "harness/bootstrap_repo_read_execution_contract/check.py"
SKILL_RELATIVE = ".agent/skills/policy-authority.md"
DECISION_RELATIVE = (
    "docs/adr/0058-authenticated-bootstrap-repo-read-execution.md"
)
RUNTIME_PROFILE = "authenticated_bootstrap_repo_read_execution_v1"
ISSUANCE_PROFILE = "authenticated_bootstrap_repo_read_grant_issuance_v1"

BOOTSTRAP_REPO_READ_EXECUTION = {
    "runtime_profile_id": RUNTIME_PROFILE,
    "issuance_runtime_profile_id": ISSUANCE_PROFILE,
    "delivery": "catalyst_go_kernel_only",
    "status": "shipped_narrow_runtime_profile",
    "authority": {
        "executor": "non_agent_forgeos_kernel",
        "deployment_owner": "external_operator",
        "execution_trust_root": "independently_pinned_outside_repository",
        "binds_exact_issuance_root_and_epoch": True,
        "execution_keys_distinct_from_issuance_keys": True,
        "key_usages": [
            "execution_policy_sign", "execution_receipt_sign",
            "execution_request_auth",
        ],
        "key_ids_principals_and_public_keys_pairwise_distinct": True,
        "key_usage_artifacts": {
            "execution_policy_sign": ["execution_policy"],
            "execution_receipt_sign": [
                "usage_receipt", "complete_usage_ledger_snapshot",
            ],
            "execution_request_auth": ["invocation"],
        },
        "cross_usage_or_other_artifact_signing": "forbidden",
        "key_set_or_usage_expansion": "new_profile_and_external_pin_required",
        "signed_inputs": ["execution_policy", "invocation"],
        "signed_outputs": ["usage_ledger", "usage_receipt"],
        "policy_decisions": ["allow_activate_once", "deny_do_not_activate"],
        "deny_reserves_usage": False,
    },
    "scope": {
        "phase": "bootstrap_planning",
        "capabilities": ["repository-reader/v1"],
        "effects": ["repo.read"],
        "environments": ["development", "local", "test"],
        "resources": "one_to_sixteen_sorted_exact_manifest_paths",
        "manifest": "exact_regular_file_bytes_and_sha256",
        "aggregate_raw_content_max_bytes": 1_048_576,
        "per_file_raw_content_max_bytes": 1_048_576,
        "path_utf8_max_bytes": 4_096,
        "path_max_components": 256,
        "top_level_control_directories_case_insensitive": [".forge", ".git"],
        "unicode_and_spaces": "permitted_with_exact_utf8_byte_identity",
        "control_and_bidi_characters": "forbidden",
        "raw_encoding": "base64url_unpadded_first_delivery_only",
        "context_and_source_bindings": "opaque_exact_equality_only",
    },
    "usage_state": {
        "transitions": [
            "reserved_no_repo_io->effect_intent",
            "reserved_no_repo_io->quarantined",
            "effect_intent->completed",
            "effect_intent->failed_consumed",
            "effect_intent->quarantined",
        ],
        "single_use": True,
        "reservation_time": "inside_fresh_invocation_half_open_window",
        "intent_or_terminal_after_reservation": "may_cross_invocation_expiry",
        "reservation_precedes_repository_io": True,
        "reservation_precedes_bind_or_verify_repository_metadata_io": True,
        "effect_intent_precedes_first_read": True,
        "active_tail_resume_or_reread": "forbidden",
        "orphan_recovery": "durable_signed_quarantine_only",
        "failed_consumed_reasons": [
            "content_mismatch", "cooperative_timeout_exceeded",
            "repository_identity_changed", "repository_read_failed",
        ],
        "global_sequence_and_prior_receipt_chain": True,
        "grant_and_idempotency_unique_globally": True,
        "max_entries": 256,
        "max_snapshot_bytes": 16_777_216,
        "reservation_capacity_preflights_intent_and_terminal": True,
    },
    "persistence_and_replay": {
        "artifact": "signed_complete_usage_ledger_snapshot",
        "terminal_persistence_and_strict_reopen_before_delivery": True,
        "ledger_persists_raw_content": False,
        "first_completed_delivery": "receipt_metadata_and_raw_result",
        "completed_replay": "same_receipt_and_metadata_with_null_raw_result",
        "failed_or_quarantined_replay": (
            "same_receipt_with_null_metadata_and_raw_result"
        ),
        "replay_lookup_modes": [
            "exact_canonical_policy_and_invocation_pair",
            "both_raw_64_lower_hex_self_digests",
        ],
        "digest_lookup_scope": "terminal_groups_including_miss",
        "partial_or_conflicting_match": "fail_closed",
        "digest_only_miss_or_mixed_identity": "fail_closed_before_manifest",
        "replay_before": ["manifest", "repository", "clock", "receipt_seed"],
        "crash_during_raw_delivery": "raw_result_unrecoverable_no_reread",
        "output_failure": "terminal_persisted_delivery_uncertain_no_reread",
        "commit_fault_matrix": {
            "reservation_pre_publish": "fresh_retry_may_complete_with_at_most_one_read",
            "reservation_post_publish_uncertain": "quarantine_without_read",
            "intent_pre_publish": "quarantine_without_read",
            "intent_post_publish_uncertain": "quarantine_without_read",
            "terminal_pre_publish": "quarantine_after_one_read",
            "terminal_post_publish_uncertain": "completed_replay_after_one_read",
        },
        "second_repository_read_after_any_commit_fault": "forbidden",
    },
    "runtime_boundary": {
        "binary": "forge-kernel",
        "supported_platforms": ["linux_amd64", "linux_arm64"],
        "unsupported_platform_behavior": "fail_closed_before_reservation",
        "platform_check": "pure_build_tag_goos_arch_without_cwd_or_filesystem_probe",
        "bound_repository_preflight": (
            "visible_superblock_and_openat2_after_reservation_before_effect_intent"
        ),
        "bound_repository_preflight_failure": "signed_quarantine",
        "authority_root_and_state_outside_repository": "required",
        "authority_root_path": "absolute_canonical_all_ancestors_no_symlink",
        "authority_repository_overlap": (
            "forbidden_both_directions_by_resolved_ancestor_file_identity"
        ),
        "directory_mode": "0700",
        "local_file_mode": "0600",
        "effective_uid_check": "required_on_linux",
        "state_and_leaf_paths": "closed_canonical_relative",
        "authority_leaf_identity": "regular_single_link_no_special_mode_bits",
        "key_generation_or_provisioning": "forbidden",
        "repository_identity_bound_for_session": True,
        "reader": "linux_openat2_only_no_fallback",
        "probe_open_flags": ["O_PATH", "O_CLOEXEC"],
        "active_open_flags": [
            "O_RDONLY", "O_NONBLOCK", "O_CLOEXEC", "O_NOATIME", "O_NOCTTY",
        ],
        "noatime_scope": "initial_leaf_open_and_post_read_reopen",
        "noatime_unavailable_behavior": "fail_closed_without_atime_mutating_fallback",
        "openat2_resolve": [
            "RESOLVE_BENEATH", "RESOLVE_NO_XDEV", "RESOLVE_NO_SYMLINKS",
            "RESOLVE_NO_MAGICLINKS",
        ],
        "filesystem": "statfs_local_v1_magic_allowlist_only",
        "filesystem_allowlist": [
            "btrfs", "ext2_ext3_ext4", "overlayfs", "tmpfs", "xfs", "zfs",
        ],
        "rejected_filesystems": ["fuse", "nfs", "other_unlisted"],
        "fstatfs_attests": "directly_visible_superblock_magic_only",
        "overlay_lower_upper_or_physical_locality_attested": False,
        "local_backing_storage": "operator_deployment_precondition",
        "network_bytes_zero_semantics": "no_explicit_network_request_by_effect_only",
        "leaf_open_sequence": (
            "confined_probe_fstat_then_active_open_samefile_and_invariant_recheck"
        ),
        "leaf_invariants": "regular_single_link_exact_size_and_sha256",
        "static_fifo_or_device_active_open": "forbidden",
        "nonblock_noctty_purpose": "avoid_fifo_block_and_controlling_tty",
        "atomic_regular_only_open": "unavailable_on_linux",
        "raced_device_open_driver_side_effect_before_fstat_rejection": "possible",
        "repository_without_device_nodes": "operator_deployment_precondition",
        "untrusted_writer_without_cap_mknod": "operator_deployment_precondition",
        "no_untrusted_namespace_writer_during_execution": (
            "operator_deployment_precondition"
        ),
        "device_node_absence_or_driver_isolation_attested": False,
        "repository_write_network_process_secret_or_target_effect": False,
        "known_public_fixture_authority": (
            "forbidden_exact_root_and_any_fixture_public_key"
        ),
        "fixture_private_seed_or_production_bypass": "forbidden",
    },
    "timeout": {
        "semantics": "cooperative_not_hard_wall_clock",
        "starts": "after_durable_effect_intent_before_first_read",
        "content_reader_checks": (
            "before_between_and_after_statfs_openat2_stat_read_and_reopen"
        ),
        "grantstate_repository_revalidation_checks": "composite_before_and_after_only",
        "blocked_kernel_operation_may_overrun_budget": True,
        "post_return_timeout_precedence": "timeout_over_other_read_outcome",
        "late_kernel_return": "failed_consumed_cooperative_timeout_exceeded",
        "successful_elapsed_ms": "monotonic_duration_at_most_requested_timeout",
    },
    "transition_clock": {
        "sampling": "independent_wall_clock_sample_before_each_signed_transition",
        "transitions": [
            "reservation", "effect_intent", "quarantine", "terminal",
        ],
        "high_water": "advanced_by_each_successful_sample",
        "failure": (
            "no_fabricated_old_time_reservation_unwritten_existing_active_tail_unchanged"
        ),
    },
    "memory_hygiene": {
        "mutable_raw_buffers": "best_effort_clear_after_delivery_attempt",
        "secure_erasure_of_strings_gc_kernel_or_downstream_copies": False,
        "process_isolation_or_hsm_attestation": False,
    },
    "rollback": {
        "clock_high_water_scope": "relative_to_current_signed_snapshot_only",
        "administrator_complete_state_replacement_resistance": "unavailable",
        "tpm_remote_witness_or_external_counter": "unavailable",
    },
    "lifecycle": {
        "root_receipt_key_and_usage_namespace": "indivisible_deployment",
        "root_or_receipt_key_change_with_existing_ledger": "fail_closed",
        "fresh_root_and_state_inherit_spent_history": False,
        "fresh_namespace_consumes_prior_namespace_grants": False,
        "rotation_epoch_migration_or_state_clear_rebase": "unsupported",
        "continuous_rotation_requires": (
            "new_profile_and_adr_with_externally_witnessed_complete_single_use_history_migration"
        ),
    },
    "scaffold": {
        "inherits": [
            "adr", "schema", "fixture", "python_structural_checker",
            "governance_checker_and_tests",
        ],
        "installs_forge_kernel": False,
        "installs_execution_binary_keys_roots_or_state": False,
        "unavailable_runtime_result": "not_executed",
    },
    "structural_checker": {
        "detector": "shadow_structural_non_load_bearing",
        "validates": "canonical_shape_digest_and_declared_relations",
        "authenticates_ed25519_or_external_pin": False,
        "attests_openat2_durability_clock_or_effect": False,
    },
    "unavailable_runtime": {
        "approval": "unavailable",
        "revocation": "unavailable",
        "context_reassembly": "unavailable",
        "write_network_process_secret_target_effects": "unavailable",
        "staging_or_production": "unavailable",
        "generalized_kernel_pdp": "planned",
        "remote_ha_multitenant": "unavailable",
        "execution_root_or_signing_key_rotation": "unavailable",
        "trust_epoch_migration": "unavailable",
        "usage_state_clear_or_rebase": "unavailable",
    },
    "completion_authority_is_execution_authority": False,
}

CANONICAL_REFS = {
    "bootstrap_repo_read_execution_schema": SCHEMA_RELATIVE,
    "bootstrap_repo_read_execution_golden_fixture": FIXTURE_RELATIVE,
    "bootstrap_repo_read_execution_checker": CHECKER_RELATIVE,
    "bootstrap_repo_read_execution_skill": SKILL_RELATIVE,
    "bootstrap_repo_read_execution_decision": DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "bootstrap_repo_read_execution_python_checker": {
        "ref": CHECKER_RELATIVE,
        "projection": "universal_scaffold_structural_only",
    },
    "bootstrap_repo_read_execution_authority_go": {
        "ref": "forge-core/internal/bootstraprepoexecutionauthority",
        "projection": "catalyst_repository_only_not_scaffolded",
    },
    "bootstrap_repo_read_execution_runtime_go": {
        "ref": "forge-core/internal/bootstrapreporeadexecution",
        "projection": "catalyst_repository_only_not_scaffolded",
    },
    "bootstrap_repo_read_execution_reader_go": {
        "ref": "forge-core/internal/pinnedreporead",
        "projection": "catalyst_repository_only_not_scaffolded",
    },
    "bootstrap_repo_read_execution_state_go": {
        "ref": "forge-core/internal/grantstate",
        "projection": "catalyst_repository_only_wire_neutral_not_scaffolded",
    },
    "bootstrap_repo_read_execution_kernel_cli": {
        "ref": "forge-core/cmd/forge-kernel",
        "projection": "catalyst_repository_only_not_scaffolded",
    },
}

SKILL_MARKERS = [
    "ADR-0058", RUNTIME_PROFILE, "独立 execution trust root", "openat2",
    "reserved_no_repo_io", "effect_intent", "failed_consumed", "quarantined",
    "receipt-only replay", "不持久化 raw", "cooperative timeout",
    "管理员", "fixture", "not_executed", "forge accept",
]


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("bootstrap_repo_read_execution") != BOOTSTRAP_REPO_READ_EXECUTION:
        issues.append(f"{path}: bootstrap repo-read execution profile drifted")
    scope = _mapping(data.get("scope"))
    if scope.get("shipped_runtime_profiles") != [ISSUANCE_PROFILE, RUNTIME_PROFILE]:
        issues.append(f"{path}: ADR-0057/0058 shipped runtime profiles drifted")
    if scope.get("candidate_runtime_profiles") != []:
        issues.append(f"{path}: accepted ADR-0058 cannot remain a candidate profile")
    if scope.get("shipped_contract_only_kinds") != [
            "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
            "TransitionReceipt"]:
        issues.append(
            f"{path}: ApprovalRecord, CapabilityGrant, KnowledgeUpdateProposal, and TransitionReceipt must remain contract-only")
    for field, expected in CANONICAL_REFS.items():
        if _mapping(data.get("canonical_refs")).get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def _load_json(path, label, max_bytes):
    raw = read_bounded_file(path, label=label, max_bytes=max_bytes)
    return json.loads(raw.decode("utf-8"))


def artifact_issues(repo_root):
    issues = []
    try:
        schema = _load_json(repo_root / SCHEMA_RELATIVE, SCHEMA_RELATIVE, 2_097_152)
        fixture = _load_json(repo_root / FIXTURE_RELATIVE, FIXTURE_RELATIVE, 41_943_040)
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"bootstrap repo-read execution artifact cannot be read: {error}"]
    if schema.get("additionalProperties") is not False:
        issues.append(f"{SCHEMA_RELATIVE}: root must remain closed")
    expected = {
        "completed_receipt", "effect_intent_receipt", "execution_policy",
        "execution_result", "execution_trust_root", "expected_manifest",
        "first_delivery", "grant", "grant_issuance_receipt", "invocation",
        "issuance_ledger", "issuance_policy", "issuance_request",
        "issuance_trust_root", "reserved_receipt", "result_metadata",
        "signature_profile", "usage_ledger",
    }
    if set(fixture) != expected:
        issues.append(f"{FIXTURE_RELATIVE}: exact golden top-level fields drifted")
    encoded_ledger = json.dumps(fixture.get("usage_ledger", {}), sort_keys=True)
    if "content_base64url" in encoded_ledger:
        issues.append(f"{FIXTURE_RELATIVE}: usage ledger must not persist raw content")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.bootstrap_repo_read_execution_contract"
    )
    if not isinstance(detector, dict):
        return ["bootstrap repo-read execution structural detector is missing"]
    issues = []
    invocation = _mapping(detector.get("invocation"))
    if detector.get("state") != "shadow" or detector.get("fail_closed") is not True:
        issues.append("execution detector must remain shadow and fail closed")
    if invocation.get("owner") != "operator":
        issues.append("execution detector must remain operator-owned")
    if invocation.get("load_bearing") is not False:
        issues.append("execution detector must remain non-load-bearing")
    expected = ["python3", CHECKER_RELATIVE, "--golden", "repo_root"]
    if _mapping(detector.get("implementation")).get("argv") != expected:
        issues.append("execution detector argv drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate Policy/Authority Skill: {error}"]
    return [f"{path}: missing ADR-0058 boundary marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def integration_issues(data, path, repo_root, agent_root):
    from bootstrap_repo_read_execution_contract.check import validate_golden_fixture
    issues = registry_issues(data, path)
    issues.extend(artifact_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
