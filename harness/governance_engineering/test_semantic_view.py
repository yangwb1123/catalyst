#!/usr/bin/env python3
"""Adversarial tests for the ADR-0054 semantic-view public Schema."""

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.semantic_view import (
    FIXTURE_RELATIVE,
    SCHEMA_RELATIVE,
    SKILL_MARKER_GROUPS,
    SOURCE_RECORD_REF,
    fixture_issues,
    schema_issues,
    skill_marker_issues,
)


class SemanticViewSchemaTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        self.schema_path = self.repo / SCHEMA_RELATIVE
        self.schema_path.parent.mkdir(parents=True)
        source = Path(__file__).resolve().parents[2] / SCHEMA_RELATIVE
        shutil.copy2(source, self.schema_path)
        self.source_root = Path(__file__).resolve().parents[2]
        self.fixture_path = self.repo / FIXTURE_RELATIVE
        self.fixture_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(self.source_root / FIXTURE_RELATIVE, self.fixture_path)
        source_relative = SOURCE_RECORD_REF.split("#", 1)[0]
        source_fixture = self.repo / source_relative
        source_fixture.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(self.source_root / source_relative, source_fixture)

    def mutate(self, operation):
        schema = json.loads(self.schema_path.read_text(encoding="utf-8"))
        operation(schema)
        self.schema_path.write_text(json.dumps(schema), encoding="utf-8")
        return schema_issues(self.repo)

    def test_current_tail_and_as_of_semantics_are_frozen(self):
        issues = self.mutate(lambda schema: schema["x-forgeos-semantics"].update({
            "as_of_record_selection": "select_latest_tail_at_or_before_as_of",
        }))
        self.assertTrue(any("x-forgeos-semantics drifted" in issue for issue in issues), issues)

    def test_current_schema_has_no_structural_contract_drift(self):
        self.assertEqual(schema_issues(self.repo), [])

    def test_golden_source_reference_resolves_and_binds_exact_record(self):
        self.assertEqual(fixture_issues(self.repo), [])
        fixture = json.loads(self.fixture_path.read_text(encoding="utf-8"))
        fixture["source_record_ref"] = SOURCE_RECORD_REF.replace("/records/1/", "/records/0/")
        self.fixture_path.write_text(json.dumps(fixture), encoding="utf-8")
        issues = fixture_issues(self.repo)
        self.assertTrue(any("source_record" in issue for issue in issues), issues)

    def test_legal_sequence_one_fact_contested_state_is_represented(self):
        schema = json.loads(self.schema_path.read_text(encoding="utf-8"))
        self.assertEqual(
            schema["$defs"]["fact_shadow_state"],
            {"enum": ["candidate", "contested"]},
        )

    def test_removing_legal_sequence_one_state_is_rejected(self):
        issues = self.mutate(
            lambda schema: schema["$defs"]["fact_shadow_state"].update({
                "enum": ["candidate"],
            })
        )
        self.assertTrue(any("sequence-one fact state policy drifted" in issue
                            for issue in issues), issues)

    def test_authority_state_cannot_enter_shadow_state_definitions(self):
        issues = self.mutate(
            lambda schema: schema["$defs"]["fact_shadow_state"]["enum"].append("confirmed")
        )
        self.assertTrue(any("fact_shadow_state drifted" in issue for issue in issues), issues)

    def test_projection_conflict_and_job_state_conditions_are_required(self):
        for definition, marker in [
            ("projection", "projection claim-type state conditions drifted"),
            ("conflict_group", "conflict member claim-type state conditions drifted"),
            ("validation_job", "validation job claim-type state condition drifted"),
        ]:
            with self.subTest(definition=definition):
                issues = self.mutate(
                    lambda schema, name=definition: schema["$defs"][name].pop("allOf")
                )
                self.assertTrue(any(marker in issue for issue in issues), issues)
                shutil.copy2(
                    Path(__file__).resolve().parents[2] / SCHEMA_RELATIVE,
                    self.schema_path,
                )

    def test_conflict_member_minimum_is_not_optional(self):
        issues = self.mutate(
            lambda schema: schema["$defs"]["conflict_group"]["properties"]
            ["members"].update({"minItems": 1})
        )
        self.assertTrue(any("conflict member bounds drifted" in issue
                            for issue in issues), issues)

    def test_validation_jobs_remain_assumption_or_hypothesis_only(self):
        issues = self.mutate(
            lambda schema: schema["$defs"]["validation_job"]["properties"]
            ["claim_type"].update({"enum": ["assumption", "hypothesis", "fact"]})
        )
        self.assertTrue(any("validation job claim types drifted" in issue
                            for issue in issues), issues)

    def test_partial_validation_plan_shape_is_rejected_by_governance_check(self):
        issues = self.mutate(
            lambda schema: schema["$defs"]["claim_fields"].pop("oneOf")
        )
        self.assertTrue(any(
            "validation plan presence condition drifted" in issue for issue in issues
        ), issues)

    def test_validation_plan_must_remain_bound_to_validation_claim_types(self):
        issues = self.mutate(
            lambda schema: schema["$defs"]["claim_fields"].pop("allOf")
        )
        self.assertTrue(any(
            "validation plan claim-type condition drifted" in issue for issue in issues
        ), issues)

    def test_skill_semantic_contract_markers_are_frozen(self):
        skill_path = self.source_root / ".agent" / "skills" / "evidence-claim-management.md"
        text = skill_path.read_text(encoding="utf-8")
        self.assertEqual(skill_marker_issues(text, skill_path), [])
        for label, markers in SKILL_MARKER_GROUPS.items():
            with self.subTest(label=label):
                mutated = text.replace(markers[0], f"drifted-{label}", 1)
                issues = skill_marker_issues(mutated, skill_path)
                self.assertTrue(any(label in issue for issue in issues), issues)

    def test_standalone_checker_accepts_current_repository(self):
        checker = self.source_root / "harness" / "governance_engineering" / "semantic_view.py"
        result = subprocess.run(
            [sys.executable, "-B", str(checker), str(self.source_root)],
            cwd=self.source_root,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")
        self.assertIn("semantic-view-check: PASS", result.stdout)


if __name__ == "__main__":
    unittest.main()
