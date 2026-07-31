#!/usr/bin/env python3
"""Focused tests for workflow control-flow governance."""

import shutil
import sys
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
from test_check import make_temp_repo  # noqa: E402
import workflow_control_check as control  # noqa: E402


class WorkflowControlCheckTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def write_workflow(self, name, content):
        path = self.agent_root / "workflows" / name
        path.write_text(content, encoding="utf-8")
        return control.check_workflow_control_flow(self.agent_root)

    def test_dangling_target_phase_is_flagged(self):
        issues = self.write_workflow(
            "cf_phase.yml",
            "id: cf_phase\nstage: build\nphases:\n"
            "  - name: planner\n    agent: planner\n"
            "  - name: implementer\n    agent: implementer\n"
            "    on_fail:\n      action: loop_back\n"
            "      target_phase: nonexistent-phase\n",
        )
        self.assertTrue(
            any("nonexistent-phase" in issue and "target_phase" in issue
                for issue in issues),
            issues,
        )

    def test_empty_phase_name_is_flagged(self):
        issues = self.write_workflow(
            "cf_empty_name.yml",
            "id: cf_empty_name\nstage: build\nphases:\n"
            "  - name: ''\n    agent: planner\n",
        )
        self.assertTrue(
            any("phase[0]" in issue and "empty name" in issue for issue in issues),
            issues,
        )

    def test_duplicate_phase_name_is_flagged(self):
        issues = self.write_workflow(
            "cf_duplicate_name.yml",
            "id: cf_duplicate_name\nstage: build\nphases:\n"
            "  - name: implement\n    agent: implementer\n"
            "  - name: implement\n    agent: reviewer\n",
        )
        self.assertTrue(
            any("duplicates phase name 'implement'" in issue for issue in issues),
            issues,
        )

    def test_duplicate_normalized_emit_within_phase_is_flagged(self):
        issues = self.write_workflow(
            "cf_duplicate_emit.yml",
            "id: cf_duplicate_emit\nstage: build\nphases:\n"
            "  - name: implement\n    agent: implementer\n"
            "    emits: [b.md, a/../b.md]\n",
        )
        self.assertTrue(
            any(
                "duplicates normalized target 'b.md'" in issue
                and "a/../b.md" in issue
                for issue in issues
            ),
            issues,
        )

    def test_cross_phase_emit_reuse_is_allowed(self):
        issues = self.write_workflow(
            "cf_cross_phase_emit.yml",
            "id: cf_cross_phase_emit\nstage: build\nphases:\n"
            "  - name: draft\n    agent: planner\n"
            "    emits: [docs/review.md]\n"
            "  - name: revise\n    agent: reviewer\n"
            "    emits: [docs/review.md]\n",
        )
        self.assertEqual(issues, [])

    def test_dangling_loop_back_to_is_flagged(self):
        issues = self.write_workflow(
            "cf_loop.yml",
            "id: cf_loop\nstage: evolve\ntype: loop\n"
            "loop:\n  loop_back_to: ghost-phase\n  phases:\n"
            "    - name: scan\n      agent: explorer\n",
        )
        self.assertTrue(any("ghost-phase" in issue for issue in issues), issues)

    def test_hoisted_loop_phase_identity_is_checked(self):
        issues = self.write_workflow(
            "cf_loop_duplicate.yml",
            "id: cf_loop_duplicate\nstage: evolve\ntype: loop\n"
            "loop:\n  loop_back_to: scan\n  phases:\n"
            "    - name: scan\n      agent: explorer\n"
            "    - name: scan\n      agent: reviewer\n",
        )
        self.assertTrue(
            any("duplicates phase name 'scan'" in issue for issue in issues),
            issues,
        )

    def test_bad_model_tier_is_flagged(self):
        issues = self.write_workflow(
            "cf_tier.yml",
            "id: cf_tier\nstage: build\nphases:\n"
            "  - name: implementer\n    agent: implementer\n"
            "    model_tier: gpt5-turbo\n",
        )
        self.assertTrue(any("gpt5-turbo" in issue for issue in issues), issues)

    def test_bad_action_verb_is_flagged(self):
        issues = self.write_workflow(
            "cf_action.yml",
            "id: cf_action\nstage: build\nphases:\n"
            "  - name: planner\n    agent: planner\n"
            "  - name: implementer\n    agent: implementer\n"
            "    on_fail:\n      action: loop_bak\n"
            "      target_phase: planner\n",
        )
        self.assertTrue(
            any("loop_bak" in issue and "action" in issue for issue in issues),
            issues,
        )

    def test_valid_action_verbs_pass(self):
        issues = self.write_workflow(
            "cf_okaction.yml",
            "id: cf_okaction\nstage: build\nphases:\n"
            "  - name: implementer\n    agent: implementer\n"
            "    on_fail:\n      action: loop_back\n"
            "      target_phase: implementer\n"
            "stop_condition:\n  on_unmet:\n"
            "    action: loop_to_next_roadmap_item\n",
        )
        self.assertFalse(any("action" in issue for issue in issues), issues)

    def test_dangling_next_stage_is_flagged(self):
        issues = self.write_workflow(
            "cf_stage.yml",
            "id: cf_stage\nstage: build\nphases:\n"
            "  - name: planner\n    agent: planner\n"
            "stop_condition:\n  on_met:\n"
            "    next_stage: nonexistent-stage\n",
        )
        self.assertTrue(
            any("nonexistent-stage" in issue and "next_stage" in issue
                for issue in issues),
            issues,
        )

    def test_on_approved_emit_is_rejected_as_unsupported(self):
        issues = self.write_workflow(
            "cf_approval_emit.yml",
            "id: cf_approval_emit\nstage: build\nphases:\n"
            "  - name: planner\n    agent: planner\n"
            "stop_condition:\n  on_approved:\n"
            "    emit: [.agent/PROJECT.md]\n    next_stage: evolve\n",
        )
        self.assertTrue(
            any("on_approved.emit" in issue and "unsupported" in issue
                for issue in issues),
            issues,
        )

    def test_valid_model_tier_case_insensitive_passes(self):
        issues = self.write_workflow(
            "cf_ok.yml",
            "id: cf_ok\nstage: build\nphases:\n"
            "  - name: implementer\n    agent: implementer\n"
            "    model_tier: Opus\n",
        )
        self.assertEqual(issues, [])

    def test_valid_evolve_scan_contract_passes(self):
        issues = self.write_workflow(
            "cf_evolve_scan.yml",
            "id: cf_evolve_scan\nstage: evolve\ntype: loop\n"
            "loop:\n  loop_back_to: inventory\n  phases:\n"
            "    - name: inventory\n      agent: explorer\n"
            "      readonly: true\n      effect: observe\n"
            "      feeds_forward: true\n"
            "      scan_contract: evolve_scan_v1\n",
        )
        self.assertEqual(issues, [])

    def test_invalid_evolve_scan_contract_shapes_are_rejected(self):
        cases = {
            "unknown": (
                "stage: evolve\nreadonly: true\neffect: observe\n"
                "feeds_forward: true\nscan_contract: evolve_scan_v2\n",
                "unsupported",
            ),
            "writable": (
                "stage: evolve\nreadonly: false\neffect: observe\n"
                "feeds_forward: true\nscan_contract: evolve_scan_v1\n",
                "readonly=true",
            ),
            "not-forwarded": (
                "stage: evolve\nreadonly: true\neffect: observe\n"
                "feeds_forward: false\nscan_contract: evolve_scan_v1\n",
                "feeds_forward=true",
            ),
            "mode-skippable": (
                "stage: evolve\nreadonly: true\neffect: observe\n"
                "feeds_forward: true\noptional_for: [explorer]\n"
                "scan_contract: evolve_scan_v1\n",
                "mode-skippable",
            ),
            "emitting": (
                "stage: evolve\nreadonly: true\neffect: observe\n"
                "feeds_forward: true\nemits: [scan.md]\n"
                "scan_contract: evolve_scan_v1\n",
                "must not grant",
            ),
        }
        for name, (body, want) in cases.items():
            with self.subTest(name=name):
                lines = body.splitlines()
                phase = "\n".join(f"    {line}" for line in lines[1:])
                issues = self.write_workflow(
                    f"cf_scan_{name}.yml",
                    f"id: cf_scan_{name}\n{lines[0]}\nphases:\n"
                    f"  - name: inventory\n    agent: explorer\n{phase}\n",
                )
                self.assertTrue(any(want in issue for issue in issues), issues)

    def test_duplicate_evolve_scan_contract_is_rejected(self):
        issues = self.write_workflow(
            "cf_duplicate_scan.yml",
            "id: cf_duplicate_scan\nstage: evolve\nphases:\n"
            "  - &scan\n    name: scan-a\n    agent: explorer\n"
            "    readonly: true\n    effect: observe\n    feeds_forward: true\n"
            "    scan_contract: evolve_scan_v1\n"
            "  - <<: *scan\n    name: scan-b\n",
        )
        self.assertTrue(any("declared by both" in issue for issue in issues), issues)

    def test_late_or_gate_only_scan_contract_is_rejected(self):
        issues = self.write_workflow(
            "cf_late_scan.yml",
            "id: cf_late_scan\nstage: evolve\nphases:\n"
            "  - name: implement\n    agent: implementer\n    effect: mutate\n"
            "  - name: inventory\n    agent: harness\n"
            "    readonly: true\n    effect: observe\n"
            "    required_gates: [test]\n    depends_on: [implement]\n"
            "    feeds_forward: true\n    scan_contract: evolve_scan_v1\n",
        )
        for want in ("first phase", "non-harness", "depends_on=[]"):
            self.assertTrue(any(want in issue for issue in issues), issues)

    def test_parallel_dependency_graph_must_follow_scan(self):
        content = (
            "id: cf_scan_deps\nstage: evolve\nphases:\n"
            "  - name: inventory\n    agent: explorer\n"
            "    readonly: true\n    effect: observe\n"
            "    feeds_forward: true\n    scan_contract: evolve_scan_v1\n"
            "  - name: gap\n    agent: architect\n    depends_on: [inventory]\n"
            "  - name: implement\n    agent: implementer\n{implement_dep}"
        )
        issues = self.write_workflow(
            "cf_scan_deps.yml", content.format(implement_dep=""),
        )
        self.assertTrue(any("must transitively depend" in issue for issue in issues), issues)
        issues = self.write_workflow(
            "cf_scan_deps.yml",
            content.format(implement_dep="    depends_on: [gap]\n"),
        )
        self.assertFalse(any("must transitively depend" in issue for issue in issues), issues)

    def test_shipped_evolve_declares_scan_contract(self):
        path = self.agent_root / "workflows" / "evolve.yml"
        data = control.yaml.safe_load(path.read_text(encoding="utf-8"))
        phases = control._workflow_phases(data)
        self.assertEqual(
            [phase.get("scan_contract") for phase in phases],
            ["evolve_scan_v1"] + [None] * (len(phases) - 1),
        )

    def test_live_workflows_control_flow_is_clean(self):
        self.assertEqual(
            control.check_workflow_control_flow(self.agent_root), [],
            msg="live workflows must have clean control flow",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
