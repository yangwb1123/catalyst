#!/usr/bin/env python3
"""Adversarial tests for legacy reviewer_v1 and bound reviewer_v2 governance."""

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent

sys.path.insert(0, str(HARNESS_DIR))
import check  # noqa: E402


VALID_REVIEWER = """id: custom
stage: build
phases:
  - {name: implementer, agent: implementer, readonly: false}
  - name: independent-review
    agent: reviewer
    readonly: true
    fresh_context: true
    verdict_contract: reviewer_v1
    required_when: ../policies/modes.yml#workflow_depth.reviewer
    on_fail: {action: loop_back, target_phase: implementer}
"""

CANONICAL_WORKFLOWS = (
    "discover.yml", "design.yml", "review.yml", "build.yml",
    "deploy.yml", "rollback.yml", "evolve.yml",
)


class ReviewerVerdictCheckerTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(tempfile.mkdtemp(prefix="reviewer-verdict-check-"))
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        shutil.copytree(REPO_ROOT / ".agent", self.repo / ".agent")
        self.agent_root = self.repo / ".agent"
        self.custom = self.agent_root / "workflows" / "custom.yml"

    def issues(self):
        return check.check_workflow_verdict_contracts(self.agent_root)

    def test_live_canonical_build_declares_one_strict_reviewer(self):
        self.assertEqual(self.issues(), [])
        for name in CANONICAL_WORKFLOWS:
            workflow, err = check._load_yaml(self.agent_root / "workflows" / name)
            self.assertIsNone(err)
            self.assertEqual(workflow.get("output_binding_contract"), "local_digest_v1")
        build, err = check._load_yaml(self.agent_root / "workflows" / "build.yml")
        self.assertIsNone(err)
        reviewers = [
            phase for phase in build["phases"]
            if phase.get("verdict_contract") == "reviewer_v2"
        ]
        self.assertEqual(len(reviewers), 1)

    def test_every_canonical_workflow_requires_selector(self):
        for name in CANONICAL_WORKFLOWS:
            with self.subTest(name=name):
                path = self.agent_root / "workflows" / name
                original = path.read_text(encoding="utf-8")
                path.write_text(
                    "\n".join(
                        line for line in original.splitlines()
                        if not line.startswith("output_binding_contract:")
                    ) + "\n",
                    encoding="utf-8",
                )
                issues = self.issues()
                self.assertTrue(
                    any(str(path) in issue and "canonical workflow" in issue for issue in issues),
                    issues,
                )
                path.write_text(original, encoding="utf-8")

    def test_unknown_output_binding_selector_is_rejected(self):
        path = self.agent_root / "workflows" / "discover.yml"
        original = path.read_text(encoding="utf-8")
        path.write_text(
            original.replace("output_binding_contract: local_digest_v1", "output_binding_contract: future_v9"),
            encoding="utf-8",
        )
        issues = self.issues()
        self.assertTrue(any("future_v9" in issue and "unsupported" in issue for issue in issues), issues)

    def test_canonical_build_requires_exactly_one_reviewer_v2(self):
        path = self.agent_root / "workflows" / "build.yml"
        original = path.read_text(encoding="utf-8")
        path.write_text(
            original.replace("    verdict_contract: reviewer_v2", ""),
            encoding="utf-8",
        )
        self.assertTrue(any("exactly one reviewer_v2" in issue for issue in self.issues()))

        duplicate = original.replace(
            "  # ── P5 QA / acceptance ──",
            "  - name: second-review\n"
            "    agent: reviewer\n"
            "    readonly: true\n"
            "    fresh_context: true\n"
            "    verdict_contract: reviewer_v2\n"
            "    on_fail: {action: loop_back, target_phase: implementer}\n\n"
            "  # ── P5 QA / acceptance ──",
        )
        path.write_text(duplicate, encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("found 2" in issue for issue in issues), issues)

    def test_reviewer_v2_requires_selector_and_bound_build_qa(self):
        path = self.agent_root / "workflows" / "build.yml"
        original = path.read_text(encoding="utf-8")
        path.write_text(
            "\n".join(
                line for line in original.splitlines()
                if not line.startswith("output_binding_contract:")
            ) + "\n",
            encoding="utf-8",
        )
        issues = self.issues()
        self.assertTrue(any("reviewer_v2 requires output_binding_contract" in issue for issue in issues), issues)

        path.write_text(
            original.replace("    verdict_contract: qa_v1", "    verdict_contract: ''"),
            encoding="utf-8",
        )
        issues = self.issues()
        self.assertTrue(any("at least one qa_v1" in issue for issue in issues), issues)

    def test_reviewer_v2_rejects_post_review_writer(self):
        path = self.agent_root / "workflows" / "build.yml"
        original = path.read_text(encoding="utf-8")
        path.write_text(
            original.replace("  - name: qa\n", "  - name: post-review-writer\n"
                             "    agent: implementer\n"
                             "    readonly: false\n\n"
                             "  - name: qa\n"),
            encoding="utf-8",
        )
        issues = self.issues()
        self.assertTrue(any("post-review-writer" in issue and "readonly" in issue for issue in issues), issues)

    def test_reviewer_v1_rejects_wrong_owner_and_mutable_shape(self):
        replacements = [
            ("stage: build", "stage: review", "stage 'build'"),
            ("agent: reviewer", "agent: qa", "agent 'reviewer'"),
            ("readonly: true", "readonly: false", "readonly: true"),
            ("fresh_context: true", "fresh_context: false", "fresh_context: true"),
            (
                "required_when: ../policies/modes.yml#workflow_depth.reviewer",
                "required_when: ../policies/modes.yml#workflow_depth.other",
                "unsafe mode-skip",
            ),
            (
                "    on_fail:",
                "    optional_for: [explorer]\n    on_fail:",
                "unsafe mode-skip",
            ),
            (
                "    on_fail:",
                "    feeds_forward: true\n    on_fail:",
                "must not emit",
            ),
            (
                "    on_fail:",
                "    emits: [review.md]\n    on_fail:",
                "must not emit",
            ),
        ]
        for old, new, want in replacements:
            with self.subTest(want=want, replacement=new):
                self.custom.write_text(VALID_REVIEWER.replace(old, new), encoding="utf-8")
                issues = self.issues()
                self.assertTrue(any(want in issue for issue in issues), issues)

    def test_reviewer_v1_rejects_loop_target_and_order_bypasses(self):
        cases = [
            ("target_phase: implementer", "target_phase: missing", "does not exist"),
            ("target_phase: implementer", "target_phase: independent-review", "earlier phase"),
            ("agent: implementer", "agent: reviewer", "agent 'implementer'"),
            ("readonly: false", "readonly: true", "must be writable"),
        ]
        for old, new, want in cases:
            with self.subTest(want=want):
                self.custom.write_text(VALID_REVIEWER.replace(old, new, 1), encoding="utf-8")
                issues = self.issues()
                self.assertTrue(any(want in issue for issue in issues), issues)

        self.custom.write_text(
            VALID_REVIEWER.replace(
                "  - name: independent-review",
                "  - {name: qa, agent: qa, verdict_contract: qa_v1, "
                "required_gates: [test], on_fail: {action: loop_back, "
                "target_phase: implementer}}\n  - name: independent-review",
            ),
            encoding="utf-8",
        )
        issues = self.issues()
        self.assertTrue(any("must precede Build QA" in issue for issue in issues), issues)

    def test_low_risk_custom_workflow_may_omit_reviewer_contract(self):
        self.custom.write_text(
            "id: low\nstage: build\nphases:\n"
            "  - {name: implementer, agent: implementer}\n"
            "  - {name: advisory, agent: reviewer}\n",
            encoding="utf-8",
        )
        self.assertEqual(self.issues(), [])


if __name__ == "__main__":
    unittest.main()
