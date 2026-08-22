#!/usr/bin/env python3
"""Governance regressions for the ADR-0057 narrow issuance profile."""

import copy
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.bootstrap_grant_issuance import (
    BOOTSTRAP_GRANT_ISSUANCE,
    CONTRACT_PROFILE,
    EXECUTION_PROFILE,
    FIXTURE_RELATIVE,
    RUNTIME_PROFILE,
    SCHEMA_RELATIVE,
    detector_issues,
    fixture_issues,
    registry_issues,
    schema_issues,
    skill_issues,
)


class BootstrapGrantIssuanceGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.policy_path = (
            self.root / ".agent" / "engineering" / "governance-contracts.yml"
        )
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_current_profile_is_narrow_and_structural_detector_is_shadow(self):
        self.assertEqual(
            self.policy["bootstrap_grant_issuance"], BOOTSTRAP_GRANT_ISSUANCE
        )
        self.assertEqual(
            self.policy["scope"]["shipped_runtime_profiles"],
            [RUNTIME_PROFILE, EXECUTION_PROFILE],
        )
        self.assertEqual(
            self.policy["scope"]["candidate_runtime_profiles"], []
        )
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(detector_issues(self.root / ".agent"), [])
        self.assertEqual(skill_issues(self.root), [])

    def test_runtime_profile_scope_escalation_is_rejected(self):
        mutations = []
        for field, value in (
            ("effects", ["repo.read", "repo.write"]),
            ("capabilities", ["repository-reader/v1", "repository-writer/v1"]),
            ("environments", ["development", "local", "production", "test"]),
            ("issuance_phase", "plan_finalization"),
        ):
            mutated = copy.deepcopy(self.policy)
            mutated["bootstrap_grant_issuance"]["scope"][field] = value
            mutations.append(mutated)
        for mutated in mutations:
            with self.subTest(scope=mutated["bootstrap_grant_issuance"]["scope"]):
                self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_complete_kernel_and_effect_execution_remain_unavailable(self):
        mutated = copy.deepcopy(self.policy)
        unavailable = mutated["bootstrap_grant_issuance"]["unavailable_runtime"]
        unavailable["complete_governance_kernel_pdp"] = "available"
        unavailable["effect_execution"] = "available"
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_result_byte_ceiling_is_frozen(self):
        mutated = copy.deepcopy(self.policy)
        mutated["bootstrap_grant_issuance"]["limits"]["max_result_bytes"] += 1
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_source_revision_byte_ceiling_is_frozen(self):
        mutated = copy.deepcopy(self.policy)
        mutated["bootstrap_grant_issuance"]["limits"][
            "max_source_revision_bytes"
        ] += 1
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_external_rollback_anchor_cannot_be_claimed(self):
        mutated = copy.deepcopy(self.policy)
        persistence = mutated["bootstrap_grant_issuance"]["persistence"]
        persistence["external_monotonic_anchor"] = "available"
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_scaffold_cannot_install_kernel_or_authority_material(self):
        mutated = copy.deepcopy(self.policy)
        scaffold = mutated["bootstrap_grant_issuance"]["scaffold"]
        scaffold["installs_forge_kernel"] = True
        scaffold["installs_trust_root_keys_or_state"] = True
        scaffold["unavailable_runtime_result"] = "issued"
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_known_public_fixture_authority_cannot_be_enabled(self):
        mutated = copy.deepcopy(self.policy)
        mutated["bootstrap_grant_issuance"]["runtime_boundary"][
            "known_public_fixture_authority"
        ] = "allowed"
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_unix_and_protected_filesystem_boundary_cannot_be_relaxed(self):
        mutations = {
            "supported_host_family": "any",
            "directory_mode": "0755",
            "leaf_identity": "regular_file_only",
            "repository_authority_overlap": "allowed",
        }
        for field, value in mutations.items():
            with self.subTest(field=field):
                mutated = copy.deepcopy(self.policy)
                mutated["bootstrap_grant_issuance"]["runtime_boundary"][field] = value
                self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_completion_authority_cannot_become_issuer(self):
        mutated = copy.deepcopy(self.policy)
        mutated["bootstrap_grant_issuance"]["completion_authority_is_issuer"] = True
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_capability_grant_kind_remains_contract_only(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_contract_only_kinds"] = []
        mutated["scope"]["shipped_kinds"].append("CapabilityGrant")
        self.assertTrue(registry_issues(mutated, self.policy_path))

    def test_schema_and_fixture_freeze_narrow_profile(self):
        self.assertEqual(schema_issues(self.root), [])
        self.assertEqual(fixture_issues(self.root), [])
        fixture = json.loads((self.root / FIXTURE_RELATIVE).read_text(encoding="utf-8"))
        self.assertIn(
            fixture["result"]["delivery_disposition"], ["stored", "exact_replay"]
        )
        encoded = json.dumps(fixture, sort_keys=True)
        self.assertIn(CONTRACT_PROFILE, encoded)

    def test_schema_open_root_is_rejected(self):
        temp_root = Path(tempfile.mkdtemp(prefix="bootstrap-issuance-schema-"))
        self.addCleanup(shutil.rmtree, temp_root, ignore_errors=True)
        destination = temp_root / SCHEMA_RELATIVE
        destination.parent.mkdir(parents=True)
        schema = json.loads((self.root / SCHEMA_RELATIVE).read_text(encoding="utf-8"))
        schema["additionalProperties"] = True
        destination.write_text(json.dumps(schema), encoding="utf-8")
        self.assertTrue(schema_issues(temp_root))

    def test_detector_cannot_become_load_bearing(self):
        temp_root = Path(tempfile.mkdtemp(prefix="bootstrap-issuance-detector-"))
        self.addCleanup(shutil.rmtree, temp_root, ignore_errors=True)
        destination = temp_root / "engineering" / "detectors.yml"
        destination.parent.mkdir(parents=True)
        registry = yaml.safe_load(
            (self.root / ".agent" / "engineering" / "detectors.yml").read_text(
                encoding="utf-8"
            )
        )
        detector = next(
            item for item in registry["detectors"]
            if item["id"] == "governance.bootstrap_grant_issuance_contract"
        )
        detector["state"] = "enforced"
        detector["invocation"]["owner"] = "forge_accept"
        detector["invocation"]["load_bearing"] = True
        destination.write_text(yaml.safe_dump(registry, sort_keys=False), encoding="utf-8")
        self.assertTrue(detector_issues(temp_root))


if __name__ == "__main__":
    unittest.main()
