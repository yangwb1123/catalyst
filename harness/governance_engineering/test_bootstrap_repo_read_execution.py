#!/usr/bin/env python3
"""Governance regressions for the ADR-0058 narrow execution profile."""

import copy
import sys
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.bootstrap_repo_read_execution import (
    BOOTSTRAP_REPO_READ_EXECUTION,
    ISSUANCE_PROFILE,
    RUNTIME_PROFILE,
    artifact_issues,
    detector_issues,
    registry_issues,
    skill_issues,
)
from engineering_routing.contract import ROUTE_INCLUDE_FLOORS


class BootstrapRepoReadExecutionGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.policy_path = (
            self.root / ".agent" / "engineering" / "governance-contracts.yml"
        )
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_shipped_profile_and_shadow_detector_are_frozen(self):
        self.assertEqual(
            self.policy["bootstrap_repo_read_execution"],
            BOOTSTRAP_REPO_READ_EXECUTION,
        )
        self.assertEqual(
            self.policy["scope"]["shipped_runtime_profiles"],
            [ISSUANCE_PROFILE, RUNTIME_PROFILE],
        )
        self.assertEqual(
            self.policy["scope"]["candidate_runtime_profiles"], []
        )
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(artifact_issues(self.root), [])
        self.assertEqual(detector_issues(self.root / ".agent"), [])
        self.assertEqual(skill_issues(self.root), [])

    def test_governance_route_requires_the_execution_schema(self):
        path = self.root / ".agent" / "engineering" / "context-routes.yml"
        registry = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in registry["routes"]
                     if item["id"] == "governance")
        by_ref = {item["ref"]: item for item in route["include"]}
        schema = "docs/contracts/bootstrap-repo-read-execution-v1.schema.json"
        self.assertEqual(
            ROUTE_INCLUDE_FLOORS["governance"][schema], "trusted_context"
        )
        self.assertEqual(by_ref[schema]["lane"], "trusted_context")
        self.assertIs(by_ref[schema]["required"], True)

    def assert_mutation_rejected(self, section, field, value):
        mutated = copy.deepcopy(self.policy)
        mutated["bootstrap_repo_read_execution"][section][field] = value
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_independent_execution_authority_cannot_merge_with_issuance(self):
        self.assert_mutation_rejected(
            "authority", "execution_keys_distinct_from_issuance_keys", False
        )
        self.assert_mutation_rejected(
            "authority", "execution_trust_root", "reuse_issuance_root"
        )
        self.assert_mutation_rejected(
            "authority", "key_usage_artifacts", {
                "execution_receipt_sign": ["usage_receipt", "execution_policy"],
            },
        )

    def test_single_use_state_order_cannot_be_relaxed(self):
        self.assert_mutation_rejected(
            "usage_state", "effect_intent_precedes_first_read", False
        )
        self.assert_mutation_rejected(
            "usage_state", "active_tail_resume_or_reread", "allowed"
        )

    def test_replay_cannot_recover_or_persist_raw_content(self):
        self.assert_mutation_rejected(
            "persistence_and_replay", "ledger_persists_raw_content", True
        )
        self.assert_mutation_rejected(
            "persistence_and_replay", "completed_replay", "reread_raw_result"
        )
        self.assert_mutation_rejected(
            "persistence_and_replay", "digest_lookup_scope", "active_or_terminal"
        )

    def test_openat2_linux_boundary_cannot_be_widened(self):
        self.assert_mutation_rejected(
            "runtime_boundary", "supported_platforms", ["any"]
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "platform_check", "probe_current_working_directory"
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "reader", "portable_path_walker"
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "noatime_unavailable_behavior", "fallback_without_noatime"
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "device_node_absence_or_driver_isolation_attested", True
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "leaf_open_sequence", "active_open_then_fstat"
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "filesystem", "any_filesystem"
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "rejected_filesystems", []
        )
        self.assert_mutation_rejected(
            "runtime_boundary", "overlay_lower_upper_or_physical_locality_attested", True
        )

    def test_timeout_and_administrator_rollback_caveats_remain_explicit(self):
        self.assert_mutation_rejected("timeout", "semantics", "hard_deadline")
        self.assert_mutation_rejected(
            "timeout", "post_return_timeout_precedence", "read_outcome_first"
        )
        self.assert_mutation_rejected(
            "rollback", "administrator_complete_state_replacement_resistance",
            "available",
        )

    def test_each_signed_transition_requires_a_fresh_clock_sample(self):
        self.assert_mutation_rejected(
            "transition_clock", "sampling", "reuse_previous_timestamp"
        )
        self.assert_mutation_rejected(
            "transition_clock", "failure", "sign_with_clock_high_water"
        )

    def test_root_key_and_usage_namespace_cannot_be_rotated_or_rebased(self):
        self.assert_mutation_rejected(
            "lifecycle", "root_or_receipt_key_change_with_existing_ledger",
            "migrate_implicitly",
        )
        self.assert_mutation_rejected(
            "lifecycle", "fresh_root_and_state_inherit_spent_history", True
        )

    def test_fixture_authority_and_scaffold_runtime_remain_forbidden(self):
        self.assert_mutation_rejected(
            "runtime_boundary", "known_public_fixture_authority", "allowed"
        )
        self.assert_mutation_rejected(
            "scaffold", "installs_execution_binary_keys_roots_or_state", True
        )
        self.assert_mutation_rejected(
            "scaffold", "unavailable_runtime_result", "completed"
        )

    def test_capability_grant_remains_contract_only(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_contract_only_kinds"] = []
        mutated["scope"]["shipped_kinds"].append("CapabilityGrant")
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_shipped_profile_cannot_regress_to_candidate(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["candidate_runtime_profiles"] = [RUNTIME_PROFILE]
        mutated["scope"]["shipped_runtime_profiles"] = [ISSUANCE_PROFILE]
        self.assertTrue(registry_issues(mutated, self.policy_path))


if __name__ == "__main__":
    unittest.main()
