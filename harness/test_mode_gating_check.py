#!/usr/bin/env python3
"""Tests for `harness/mode_gating_check.py` (workflow mode_gating drift-guard).

Split from harness/test_check.py purely to keep both files under this repo's
max_file_lines gate (check.py/test_check.py were already at that ceiling —
see mode_gating_check.py's module docstring). Reuses test_check's fixture
helpers (`make_temp_repo`) so the "copy the live .agent/ tree, inject one
defect" discipline stays identical across both files.

The core behavior (agreement passes / mismatch is flagged / the real repo is
clean) is covered directly in harness/test_check.py, mirroring
check_mode_priorities' test style, per the assignment. This file covers the
"not an error" SKIP branches: no mode_gating block, no authority key (the
build.yml shape), and an authority fragment that doesn't resolve to a known
workflow_depth dimension.

Run::

    python3 -m unittest harness.test_mode_gating_check
    python3 harness/test_mode_gating_check.py
"""
import shutil
import sys
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
from test_check import make_temp_repo  # noqa: E402 — after sys.path tweak
import mode_gating_check  # noqa: E402


class SkipShapesTest(unittest.TestCase):
    """Shapes that must NOT be reported as drift (see module docstring)."""

    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def test_no_mode_gating_block_is_skipped(self):
        # A workflow with no mode_gating block at all is not an error.
        wf = self.agent_root / "workflows" / "mg_none.yml"
        wf.write_text(
            "id: mg_none\nstage: build\nphases:\n  - name: p\n    agent: implementer\n",
            encoding="utf-8",
        )
        issues = mode_gating_check.check_workflow_mode_gating(self.agent_root)
        self.assertFalse(any("mg_none.yml" in i for i in issues), issues)

    def test_no_authority_key_is_skipped(self):
        # build.yml's mode_gating restates DIFFERENT fragments under different
        # key names (gate_set / reviewer_required), with no `authority:` key
        # at all — that shape is not an error, just not this check's business.
        wf = self.agent_root / "workflows" / "mg_shapeless.yml"
        wf.write_text(
            "id: mg_shapeless\nstage: build\nmode_gating:\n"
            "  gate_set: ../policies/modes.yml#harness.gates\n"
            "  reviewer_required: ../policies/modes.yml#workflow_depth.reviewer\n"
            "phases:\n  - name: p\n    agent: implementer\n",
            encoding="utf-8",
        )
        issues = mode_gating_check.check_workflow_mode_gating(self.agent_root)
        self.assertFalse(any("mg_shapeless.yml" in i for i in issues), issues)

    def test_unknown_dimension_is_skipped(self):
        # An authority fragment naming a dimension modes.yml never actually
        # declares (typo / removed dimension) is unresolved, not a mismatch.
        wf = self.agent_root / "workflows" / "mg_unknown.yml"
        wf.write_text(
            "id: mg_unknown\nstage: design\nmode_gating:\n"
            "  explorer: nonsense\n"
            "  authority: ../policies/modes.yml#workflow_depth.not_a_real_dimension\n"
            "phases:\n  - name: p\n    agent: architect\n",
            encoding="utf-8",
        )
        issues = mode_gating_check.check_workflow_mode_gating(self.agent_root)
        self.assertFalse(any("mg_unknown.yml" in i for i in issues), issues)

    def test_live_build_yml_shape_is_skipped(self):
        # Belt-and-suspenders on the REAL build.yml: its differently-shaped
        # mode_gating block must not be flagged (back-compat).
        issues = mode_gating_check.check_workflow_mode_gating(self.agent_root)
        self.assertFalse(any("build.yml" in i for i in issues), issues)

    def test_nested_multi_dimension_mismatch_is_flagged(self):
        wf = self.agent_root / "workflows" / "mg_multi.yml"
        wf.write_text(
            "id: mg_multi\nstage: evolve\nmode_gating:\n"
            "  depth:\n"
            "    explorer: opportunistic\n"
            "    authority: ../policies/modes.yml#workflow_depth.evolve\n"
            "  mutation_authority:\n"
            "    explorer: auto-act\n"
            "    authority: ../policies/modes.yml#workflow_depth.evolve_authority\n"
            "phases:\n  - name: scan\n    agent: explorer\n",
            encoding="utf-8",
        )
        issues = mode_gating_check.check_workflow_mode_gating(self.agent_root)
        self.assertTrue(
            any("evolve_authority" in issue and "explorer" in issue for issue in issues),
            issues,
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
