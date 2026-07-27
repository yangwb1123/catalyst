#!/usr/bin/env python3
"""Tests for the ForgeOS governance checker (`harness/check.py`).

Uses only Python's built-in ``unittest`` — NO external test dependencies.
The integration tests drive the real CLI via ``subprocess`` (decoupled from
internal APIs); unit-style tests import functions directly. All tests are
real and deterministic: fixtures copy the live ``.agent/`` tree into a temp
dir, then inject a single defect and assert the checker catches it.

Run::

    python3 -m unittest harness.test_check
    python3 harness/test_check.py
"""
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
CHECK_PY = HARNESS_DIR / "check.py"
AGENT_SRC = REPO_ROOT / ".agent"


def run_cli(repo_root):
    """Invoke `python3 harness/check.py <repo_root>`; return CompletedProcess."""
    return subprocess.run(
        [sys.executable, str(CHECK_PY), str(repo_root)],
        capture_output=True,
        text=True,
        check=False,
    )


def make_temp_repo():
    """Copy the live .agent/ tree into a fresh temp repo; return its path."""
    tmp = Path(tempfile.mkdtemp(prefix="forge-check-"))
    shutil.copytree(AGENT_SRC, tmp / ".agent")
    return tmp


class RealRepoTest(unittest.TestCase):
    """The checker must pass on ForgeOS's own clean governance layer."""

    def test_real_repo_passes(self):
        result = run_cli(REPO_ROOT)
        self.assertEqual(
            result.returncode, 0,
            msg=f"expected PASS, got rc={result.returncode}\n{result.stdout}\n{result.stderr}",
        )
        self.assertIn("forge-check: PASS", result.stdout)


class BrokenWorkflowRefTest(unittest.TestCase):
    """A workflow naming a non-existent agent must FAIL and be named."""

    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def test_detects_broken_workflow_ref(self):
        wf = self.repo / ".agent" / "workflows" / "broken.yml"
        wf.write_text(
            "id: broken\n"
            "phases:\n"
            "  - name: ghost\n"
            "    agent: nonexistent-agent\n",
            encoding="utf-8",
        )
        result = run_cli(self.repo)
        self.assertEqual(result.returncode, 1, msg=result.stdout)
        self.assertIn("forge-check: FAIL", result.stdout)
        # The offending agent name must be called out by the report.
        self.assertIn("nonexistent-agent", result.stdout)

    def test_loop_phase_ref_is_checked(self):
        """Agents declared under a loop: body are validated too."""
        wf = self.repo / ".agent" / "workflows" / "looped.yml"
        wf.write_text(
            "id: looped\n"
            "type: loop\n"
            "loop:\n"
            "  phases:\n"
            "    - name: deep\n"
            "      agent: bogus-loop-agent\n",
            encoding="utf-8",
        )
        result = run_cli(self.repo)
        self.assertEqual(result.returncode, 1, msg=result.stdout)
        self.assertIn("bogus-loop-agent", result.stdout)

    def test_former_alias_is_now_rejected(self):
        """Regression: the old alias bridge is gone — a logical role-stage
        name like `scanner` is no longer accepted; only canonical card stems
        (or `harness`) resolve. Guards against re-freezing workflow/card drift.
        """
        wf = self.repo / ".agent" / "workflows" / "aliased.yml"
        wf.write_text(
            "id: aliased\n"
            "phases:\n"
            "  - name: scan\n"
            "    agent: scanner\n",
            encoding="utf-8",
        )
        result = run_cli(self.repo)
        self.assertEqual(result.returncode, 1, msg=result.stdout)
        self.assertIn("scanner", result.stdout)

class MissingSectionTest(unittest.TestCase):
    """An agent card missing a required section must be caught."""

    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def test_detects_missing_section(self):
        card = self.repo / ".agent" / "agents" / "stub.md"
        # Has Role/Phase/inputs/outputs/model/handoff but NO boundaries section.
        card.write_text(
            "# Agent: stub\n\n"
            "**Role** — does a thing.\n"
            "**Phase** — Build\n"
            "**Default model** — Sonnet\n\n"
            "## inputs (consumes)\n- something\n\n"
            "## outputs (produces)\n- something\n\n"
            "## handoff / stop\n- done\n",
            encoding="utf-8",
        )
        result = run_cli(self.repo)
        self.assertEqual(result.returncode, 1, msg=result.stdout)
        self.assertIn("forge-check: FAIL", result.stdout)
        self.assertIn("stub.md", result.stdout)
        self.assertIn("boundaries", result.stdout)


class OtherChecksTest(unittest.TestCase):
    """Direct-function tests for the remaining checks (decoupled from CLI)."""

    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        sys.path.insert(0, str(HARNESS_DIR))
        import check  # noqa: E402 — imported after sys.path tweak
        self.check = check
        self.agent_root = self.repo / ".agent"

    def tearDown(self):
        if str(HARNESS_DIR) in sys.path:
            sys.path.remove(str(HARNESS_DIR))

    def test_detects_invalid_yaml(self):
        bad = self.agent_root / "workflows" / "malformed.yml"
        bad.write_text("id: x\nphases: [unclosed\n", encoding="utf-8")
        issues = self.check.check_yaml_parses(self.agent_root)
        self.assertTrue(any("malformed.yml" in i for i in issues), issues)

    def test_detects_bad_skill_ref(self):
        card = self.agent_root / "agents" / "skillref.md"
        card.write_text("uses (skill: not-a-real-skill) here\n", encoding="utf-8")
        issues = self.check.check_skill_refs(self.agent_root)
        self.assertTrue(
            any("not-a-real-skill" in i for i in issues), issues
        )

    def test_detects_bad_routing_tier(self):
        path = self.agent_root / "routing" / "policy.yml"
        path.write_text(
            "version: 1\ntiers:\n  models: [haiku, gpt4, opus]\n",
            encoding="utf-8",
        )
        issues = self.check.check_routing_tiers(self.agent_root)
        self.assertTrue(any("gpt4" in i for i in issues), issues)

    def test_detects_bad_tier_in_by_score_and_by_task_type(self):
        # P16: a bad tier hiding in by_score[].model or by_task_type.* values
        # (not in `models`) must still be caught.
        path = self.agent_root / "routing" / "policy.yml"
        path.write_text(
            "version: 1\n"
            "tiers:\n"
            "  models: [haiku, sonnet, opus]\n"
            "  by_score:\n"
            "    - { when: 'total > 0.9', model: gpt4 }\n"
            "  by_task_type:\n"
            "    docs: gemini\n",
            encoding="utf-8",
        )
        issues = self.check.check_routing_tiers(self.agent_root)
        self.assertTrue(any("gpt4" in i for i in issues), issues)
        self.assertTrue(any("gemini" in i for i in issues), issues)

    def test_models_as_mapping_validates_values(self):
        # P16: a mapping-shaped `models` must validate VALUES, not keys.
        path = self.agent_root / "routing" / "policy.yml"
        path.write_text(
            "version: 1\ntiers:\n  models:\n    cheap: haiku\n    smart: gpt4\n",
            encoding="utf-8",
        )
        issues = self.check.check_routing_tiers(self.agent_root)
        self.assertTrue(any("gpt4" in i for i in issues), issues)
        # The key 'smart' is not a tier and must NOT be flagged.
        self.assertFalse(any("smart" in i for i in issues), issues)

    def test_list_valued_agent_does_not_crash(self):
        # P17: a list-valued `agent:` previously crashed with TypeError. It must
        # be normalized and each member validated.
        wf = self.agent_root / "workflows" / "listagent.yml"
        wf.write_text(
            "id: la\nphases:\n  - name: p\n    agent: [architect, ghost-agent]\n",
            encoding="utf-8",
        )
        issues = self.check.check_workflow_agent_refs(self.agent_root)
        self.assertTrue(any("ghost-agent" in i for i in issues), issues)
        # The valid member must NOT be flagged.
        self.assertFalse(any("architect" in i for i in issues), issues)

    def test_detects_bad_modes_tier(self):
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  weird:\n    router_default_tier: gemini\n",
            encoding="utf-8",
        )
        issues = self.check.check_modes_router_tiers(self.agent_root)
        self.assertTrue(any("gemini" in i for i in issues), issues)

    def test_mode_priorities_valid_permutation_passes(self):
        # A clean ranking (a strict permutation) is valid.
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  explorer:\n"
            "    priorities: { speed: 1, quality: 3, cost: 2 }\n",
            encoding="utf-8",
        )
        self.assertEqual(self.check.check_mode_priorities(self.agent_root), [])

    def test_mode_priorities_ties_are_allowed(self):
        # cto's real {speed:3, quality:1, cost:3} ("quality only") is a legal
        # weak order — ties must NOT be flagged (this is the honest invariant,
        # not a strict permutation).
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  cto:\n"
            "    priorities: { speed: 3, quality: 1, cost: 3 }\n",
            encoding="utf-8",
        )
        self.assertEqual(self.check.check_mode_priorities(self.agent_root), [])

    def test_live_modes_priorities_are_well_formed(self):
        # Belt-and-suspenders on the REAL modes.yml: every shipped mode's
        # priorities passes (back-compat — this repo must stay green).
        self.assertEqual(self.check.check_mode_priorities(self.agent_root), [])

    def test_mode_priorities_missing_axis_is_flagged(self):
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  broken:\n"
            "    priorities: { speed: 1, quality: 2 }\n",  # cost missing
            encoding="utf-8",
        )
        issues = self.check.check_mode_priorities(self.agent_root)
        self.assertTrue(any("broken" in i and "cost" in i for i in issues), issues)

    def test_mode_priorities_out_of_range_rank_is_flagged(self):
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  broken:\n"
            "    priorities: { speed: 0, quality: 1, cost: 2 }\n",  # 0 invalid
            encoding="utf-8",
        )
        issues = self.check.check_mode_priorities(self.agent_root)
        self.assertTrue(any("broken" in i and "speed" in i for i in issues), issues)

    def test_mode_priorities_non_int_rank_is_flagged(self):
        # A non-int rank (and a bool, which is an int subclass) must be caught.
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  broken:\n"
            "    priorities: { speed: high, quality: 1, cost: 2 }\n",
            encoding="utf-8",
        )
        issues = self.check.check_mode_priorities(self.agent_root)
        self.assertTrue(any("broken" in i and "speed" in i for i in issues), issues)

    def test_mode_priorities_extra_axis_is_flagged(self):
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  broken:\n"
            "    priorities: { speed: 1, quality: 2, cost: 3, latency: 1 }\n",
            encoding="utf-8",
        )
        issues = self.check.check_mode_priorities(self.agent_root)
        self.assertTrue(any("broken" in i for i in issues), issues)

    def test_mode_priorities_missing_block_is_flagged_not_crash(self):
        # Fault tolerance: a mode with NO priorities block reports an issue and
        # does not crash the check.
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  broken:\n    summary: no priorities here\n",
            encoding="utf-8",
        )
        issues = self.check.check_mode_priorities(self.agent_root)
        self.assertTrue(any("broken" in i and "priorities" in i for i in issues), issues)

    def test_mode_priorities_non_mapping_is_flagged_not_crash(self):
        # A scalar/list `priorities` previously risked a crash on set()/items();
        # it must be a clean issue instead.
        path = self.agent_root / "policies" / "modes.yml"
        path.write_text(
            "id: modes\nmodes:\n  broken:\n    priorities: fast\n",
            encoding="utf-8",
        )
        issues = self.check.check_mode_priorities(self.agent_root)
        self.assertTrue(any("broken" in i for i in issues), issues)

    # mode_gating vs modes.yml drift-guard — more fixtures in test_mode_gating_check.py.
    def test_live_workflow_mode_gating_is_clean(self):
        self.assertEqual(self.check.check_workflow_mode_gating(self.agent_root), [])

    def test_workflow_mode_gating_agreement_passes(self):
        (self.agent_root / "workflows" / "mg_ok.yml").write_text(
            "id: mg\nstage: design\nmode_gating:\n  explorer: light\n"
            "  authority: ../policies/modes.yml#workflow_depth.design\n"
            "phases:\n  - name: p\n    agent: architect\n", encoding="utf-8",
        )
        self.assertEqual(self.check.check_workflow_mode_gating(self.agent_root), [])

    def test_workflow_mode_gating_mismatch_is_flagged(self):
        (self.agent_root / "workflows" / "mg_bad.yml").write_text(
            "id: mg\nstage: design\nmode_gating:\n  explorer: full\n"  # really 'light'
            "  authority: ../policies/modes.yml#workflow_depth.design\n"
            "phases:\n  - name: p\n    agent: architect\n", encoding="utf-8",
        )
        issues = self.check.check_workflow_mode_gating(self.agent_root)
        self.assertTrue(any("mg_bad.yml" in i and "explorer" in i for i in issues), issues)

    def test_detects_missing_acceptance_criteria(self):
        path = self.agent_root / "eval" / "acceptance.schema.yml"
        path.write_text(
            "schema: feature-acceptance\ncriteria:\n  lint: {expect: pass}\n",
            encoding="utf-8",
        )
        issues = self.check.check_acceptance_schema(self.agent_root)
        # test_pass and build are required but absent.
        self.assertTrue(any("test_pass" in i for i in issues), issues)
        self.assertTrue(any("build" in i for i in issues), issues)

    def test_live_workflow_agent_refs_are_canonical(self):
        # No alias indirection: every live workflow `agent:` must be a real
        # role-card stem (or the `harness` pseudo-agent), so the checker passes
        # and zero drift remains between workflows and cards.
        issues = self.check.check_workflow_agent_refs(self.agent_root)
        self.assertEqual(issues, [], msg=f"unexpected: {issues}")
        # Belt-and-suspenders: assert the invariant directly, independent of
        # the checker, so this test fails if drift is reintroduced.
        valid = self.check._agent_card_names(
            self.agent_root / "agents"
        ) | self.check.PSEUDO_AGENTS
        for wf in sorted((self.agent_root / "workflows").glob("*.yml")):
            data, _ = self.check._load_yaml(wf)
            for phase in self.check._collect_phases(data):
                name = phase.get("agent") if isinstance(phase, dict) else None
                if name:
                    self.assertIn(name, valid, msg=f"{wf}: stray agent {name}")

    def test_skill_ref_handles_cjk_after_paren(self):
        # `(skill: refactor-large-file),复检` — name must stop at ')'.
        refs = self.check._skill_refs("先重构(skill: refactor-large-file),复检通过")
        self.assertEqual(refs, ["refactor-large-file"])

if __name__ == "__main__":
    unittest.main(verbosity=2)
