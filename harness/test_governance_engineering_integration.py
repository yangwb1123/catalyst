#!/usr/bin/env python3
"""Integration tests for Evidence/Claim wiring in Agent Engineering."""
import json
import shutil
import unittest

import yaml

from test_agent_engineering_check import engineering, make_temp_repo, replace_once
import governance_engineering_check as governance


class GovernanceEngineeringIntegrationTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def test_extension_cannot_leave_registry(self):
        path = self.agent_root / "engineering" / "activation.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        del data["canonical_extension_refs"]["governance_contract_schema"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical_extension_refs" in issue for issue in self.issues()))

    def test_policy_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "persistence: local_append_only_exact_evidence_claim_journal",
                     "persistence: authoritative_truth_ledger")
        self.assertTrue(any("protected governance contract policy" in issue for issue in self.issues()))

    def test_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "governance-evidence-claim-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("schema_sha256" in issue for issue in self.issues()))

    def test_journal_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("journal_schema_sha256" in issue for issue in self.issues()))

    def test_cognitive_atom_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "cognitive-atom-projection-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("cognitive_atom_schema_sha256" in issue
                            for issue in self.issues()))

    def test_cognitive_atom_golden_pin_is_enforced(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "cognitive-atom-projection-v1.json")
        replace_once(path, '"task_id": "fixture-task-001"',
                     '"task_id": "fixture-task-drifted"')
        self.assertTrue(any("cognitive_atom_golden_fixture_sha256" in issue
                            for issue in self.issues()))

    def test_artifact_evidence_adapter_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "artifact-evidence-adapter-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("artifact_evidence_adapter_schema_sha256" in issue
                            for issue in self.issues()))

    def test_artifact_evidence_adapter_golden_pin_is_enforced(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "artifact-evidence-adapter-v1.json")
        replace_once(path, '"run_id": "run-artifact-0048"',
                     '"run_id": "run-artifact-drifted"')
        self.assertTrue(any("artifact_evidence_adapter_golden_fixture_sha256" in issue
                            for issue in self.issues()))

    def test_command_observation_adapter_schema_pin_is_enforced(self):
        path = (self.repo / "docs" / "contracts" /
                "command-observation-evidence-adapter-v1.schema.json")
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("command_observation_evidence_adapter_schema_sha256" in issue
                            for issue in self.issues()))

    def test_command_observation_adapter_golden_pin_is_enforced(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "command-observation-evidence-adapter-v1.json")
        replace_once(path, '"run_id": "run-command-0049"',
                     '"run_id": "run-command-drifted"')
        self.assertTrue(any(
            "command_observation_evidence_adapter_golden_fixture_sha256" in issue
            for issue in self.issues()
        ))

    def test_missing_journal_schema_is_rejected(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        path.unlink()
        self.assertTrue(any("required pin target missing" in issue for issue in self.issues()))

    def test_missing_cognitive_atom_schema_is_rejected(self):
        path = self.repo / "docs" / "contracts" / "cognitive-atom-projection-v1.schema.json"
        path.unlink()
        issues = self.issues()
        self.assertTrue(any("required pin target missing" in issue and
                            "cognitive-atom-projection-v1.schema.json" in issue
                            for issue in issues), issues)

    def test_missing_artifact_evidence_adapter_schema_is_rejected(self):
        path = self.repo / "docs" / "contracts" / "artifact-evidence-adapter-v1.schema.json"
        path.unlink()
        issues = self.issues()
        self.assertTrue(any("required pin target missing" in issue and
                            "artifact-evidence-adapter-v1.schema.json" in issue
                            for issue in issues), issues)

    def test_missing_command_observation_adapter_schema_is_rejected(self):
        path = (self.repo / "docs" / "contracts" /
                "command-observation-evidence-adapter-v1.schema.json")
        path.unlink()
        issues = self.issues()
        self.assertTrue(any("required pin target missing" in issue and
                            "command-observation-evidence-adapter-v1.schema.json" in issue
                            for issue in issues), issues)

    def test_journal_schema_registers_the_bounded_list_envelope(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        self.assertIn(
            {"$ref": "#/$defs/record_inspection_list"},
            schema["oneOf"],
        )
        record_list = schema["$defs"]["record_inspection_list"]
        self.assertEqual(record_list["required"], ["api_version", "kind", "records"])
        self.assertEqual(record_list["properties"]["records"]["maxItems"], 100)
        self.assertEqual(
            record_list["properties"]["records"]["items"],
            {"$ref": "#/$defs/record_inspection"},
        )

    def test_journal_schema_freezes_reference_closure_admissibility_limits(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(
            schema["x-forgeos-reference-closure-limits"],
            governance.SCHEMA_CLOSURE_LIMITS,
        )

    def test_registry_freezes_closure_limits_and_runtime_delivery(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        limits = data["journal"]["limits"]
        for field, expected in governance.REFERENCE_CLOSURE_LIMITS.items():
            self.assertEqual(limits[field], expected)
        self.assertEqual(data["journal"]["runtime_delivery"], governance.RUNTIME_DELIVERY)

    def test_registry_freezes_exact_cognitive_atom_projection(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 10)
        self.assertEqual(data["cognitive_atom_projection"],
                         governance.COGNITIVE_ATOM_PROJECTION)

    def test_registry_freezes_exact_artifact_evidence_adapter(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 10)
        self.assertEqual(data["artifact_evidence_adapter"],
                         governance.ARTIFACT_EVIDENCE_ADAPTER)

    def test_registry_freezes_exact_command_observation_adapter(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 10)
        self.assertEqual(data["command_observation_evidence_adapter"],
                         governance.COMMAND_OBSERVATION_EVIDENCE_ADAPTER)

    def test_cognitive_atom_projection_registry_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "    max_atoms: 256", "    max_atoms: 255")
        issues = self.issues()
        self.assertTrue(any("cognitive_atom_projection contract drifted" in issue
                            for issue in issues), issues)

    def test_cognitive_atom_schema_extension_drift_is_rejected(self):
        path = self.repo / "docs" / "contracts" / "cognitive-atom-projection-v1.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-limits"]["max_atoms"] = 255
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("x-forgeos-limits drifted" in issue for issue in issues),
                        issues)

    def test_artifact_evidence_adapter_registry_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "    max_request_bytes: 131072",
                     "    max_request_bytes: 131071")
        issues = self.issues()
        self.assertTrue(any("artifact_evidence_adapter contract drifted" in issue
                            for issue in issues), issues)

    def test_artifact_evidence_adapter_schema_extension_drift_is_rejected(self):
        path = self.repo / "docs" / "contracts" / "artifact-evidence-adapter-v1.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-mapping"]["source_trust"] = "authoritative"
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("x-forgeos-mapping drifted" in issue for issue in issues),
                        issues)

    def test_command_observation_adapter_registry_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "    projectable_termination: exited",
                     "    projectable_termination: timed_out")
        issues = self.issues()
        self.assertTrue(any("command_observation_evidence_adapter contract drifted" in issue
                            for issue in issues), issues)

    def test_command_observation_schema_extension_drift_is_rejected(self):
        path = (self.repo / "docs" / "contracts" /
                "command-observation-evidence-adapter-v1.schema.json")
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-mapping"]["source_trust"] = "authoritative"
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("x-forgeos-mapping drifted" in issue for issue in issues),
                        issues)

    def test_cognitive_atom_golden_validator_is_integrated(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "cognitive-atom-projection-v1.json")
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["atom_id"] = "atom-" + "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("golden.expected.atom_id: golden value mismatch" in issue
                            for issue in issues), issues)

    def test_artifact_evidence_adapter_golden_validator_is_integrated(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "artifact-evidence-adapter-v1.json")
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["request_sha256"] = "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("golden.expected.request_sha256" in issue
                            for issue in issues), issues)

    def test_command_observation_golden_validator_is_integrated(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "command-observation-evidence-adapter-v1.json")
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["request_sha256"] = "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("golden.expected.request_sha256" in issue
                            for issue in issues), issues)

    def test_reference_closure_limit_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "max_dependency_records: 1024", "max_dependency_records: 1025")
        self.assertTrue(any("max_dependency_records must remain 1024" in issue
                            for issue in self.issues()))

    def test_skill_requires_real_runtime_and_honest_scaffold_boundary(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "forge-runtime governance journal show",
                     "forge governance journal show")
        self.assertTrue(any("compatible forge-runtime CLI" in issue for issue in self.issues()))

    def test_skill_requires_artifact_adapter_branch(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "### Artifact provenance adapter 分支",
                     "### Removed artifact provenance branch")
        issues = self.issues()
        self.assertTrue(any("artifact Evidence adapter guidance" in issue
                            for issue in issues), issues)

    def test_skill_requires_artifact_adapter_non_capability_boundary(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(
            path,
            "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)",
            "ADAPTED_SHADOW",
        )
        issues = self.issues()
        self.assertTrue(any("artifact Evidence adapter guidance" in issue
                            for issue in issues), issues)

    def test_skill_cannot_claim_artifact_adapter_persistence(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "不会创建 Claim/CognitiveAtom，不会 append journal",
                     "不会创建 Claim/CognitiveAtom，会 append journal")
        issues = self.issues()
        self.assertTrue(any("artifact Evidence adapter guidance" in issue
                            for issue in issues), issues)

    def test_skill_requires_command_observation_adapter_branch(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "### Command observation adapter 分支",
                     "### Removed command observation branch")
        issues = self.issues()
        self.assertTrue(any("command observation Evidence adapter guidance" in issue
                            for issue in issues), issues)

    def test_skill_requires_command_observation_non_capability_boundary(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, governance.COMMAND_SUCCESS, "ADAPTED_SHADOW")
        issues = self.issues()
        self.assertTrue(any("command observation Evidence adapter guidance" in issue
                            for issue in issues), issues)

    def test_oversized_contract_json_is_rejected_before_hashing(self):
        path = self.repo / "docs" / "contracts" / "governance-evidence-claim-v1.schema.json"
        with path.open("wb") as stream:
            stream.truncate(1_048_577)
        issues = self.issues()
        self.assertTrue(any("schema.json exceeds 1048576 bytes" in issue
                            for issue in issues), issues)

    def test_oversized_registry_is_rejected_before_yaml_parsing(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        with path.open("wb") as stream:
            stream.truncate(524_289)
        issues = self.issues()
        self.assertTrue(any("file exceeds 524288 bytes" in issue for issue in issues), issues)

    def test_skill_floor_is_required(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "## 自动化与验收", "## Removed verification")
        self.assertTrue(any("missing required section '自动化与验收'" in issue for issue in self.issues()))

    def test_detector_requires_record_set_argument(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.evidence_claim_contract")
        detector["implementation"]["argv"].pop()
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("requires exact record-set arguments" in issue
                            for issue in self.issues()))

    def test_cognitive_atom_detector_requires_exact_projection_arguments(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "aadm.cognitive_atom_projection")
        detector["implementation"]["argv"].pop()
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("requires exact projection arguments" in issue
                            for issue in issues), issues)

    def test_artifact_evidence_detector_requires_exact_adapter_arguments(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.artifact_evidence_adapter")
        detector["implementation"]["argv"].pop()
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("requires exact adapter arguments" in issue
                            for issue in issues), issues)

    def test_artifact_evidence_detector_must_remain_shadow(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.artifact_evidence_adapter")
        detector["state"] = "planned"
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("requires the exact shadow binding" in issue
                            for issue in issues), issues)

    def test_artifact_evidence_detector_cannot_become_load_bearing(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.artifact_evidence_adapter")
        detector["invocation"]["owner"] = "forge_accept"
        detector["invocation"]["adapter"] = "acceptance.probeArchitecture"
        detector["invocation"]["acceptance_criterion"] = "architecture"
        detector["invocation"]["load_bearing"] = True
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("requires the exact shadow binding" in issue
                            for issue in issues), issues)

    def test_command_observation_detector_requires_exact_adapter_arguments(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.command_observation_evidence_adapter")
        detector["implementation"]["argv"].pop()
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("requires exact adapter arguments" in issue
                            for issue in issues), issues)

    def test_command_observation_detector_must_remain_shadow_non_load_bearing(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.command_observation_evidence_adapter")
        detector["invocation"]["owner"] = "forge_accept"
        detector["invocation"]["load_bearing"] = True
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("requires the exact shadow binding" in issue
                            for issue in issues), issues)


if __name__ == "__main__":
    unittest.main()
