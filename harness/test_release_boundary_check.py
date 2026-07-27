#!/usr/bin/env python3
"""Tests for the declarative production-delivery boundary check."""
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml


HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
sys.path.insert(0, str(HARNESS_DIR))
from release_boundary_check import check_release_boundary  # noqa: E402


def make_repo():
    root = Path(tempfile.mkdtemp(prefix="forge-release-check-"))
    shutil.copytree(REPO_ROOT / ".agent", root / ".agent")
    return root


def mutate_yaml(path, mutate):
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    mutate(data)
    path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")


class ReleaseBoundaryTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def test_live_assets_are_clean(self):
        self.assertEqual(check_release_boundary(self.agent_root), [])

    def test_build_must_handoff_to_deploy(self):
        path = self.agent_root / "workflows" / "build.yml"
        mutate_yaml(path, lambda data: data["stop_condition"]["on_met"].update(
            {"next_stage": "evolve"}))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("Build must hand off exactly to deploy" in i for i in issues), issues)

    def test_emit_cannot_escape_release_directory(self):
        path = self.agent_root / "workflows" / "deploy.yml"
        mutate_yaml(path, lambda data: data["phases"][0]["emits"].append(
            "../production-secret.yml"))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("emits outside docs/release/" in i for i in issues), issues)

    def test_remote_tool_requirement_is_forbidden(self):
        path = self.agent_root / "workflows" / "deploy.yml"
        mutate_yaml(path, lambda data: data["phases"][0].update(
            {"requires_tools": ["kubectl"]}))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("must not request remote tools" in i for i in issues), issues)

    def test_workflow_top_level_must_be_readonly(self):
        path = self.agent_root / "workflows" / "deploy.yml"
        mutate_yaml(path, lambda data: data.update({"readonly": False}))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("top-level id/stage/readonly/loop shape" in i for i in issues), issues)

    def test_rollback_cannot_join_the_spine(self):
        path = self.agent_root / "workflows" / "deploy.yml"
        mutate_yaml(path, lambda data: data["stop_condition"]["on_approved"].update(
            {"next_stage": "rollback"}))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("rollback is standalone" in i for i in issues), issues)

    def test_human_rejection_must_loop_to_planning(self):
        path = self.agent_root / "workflows" / "rollback.yml"
        mutate_yaml(path, lambda data: data["stop_condition"].pop("on_rejected"))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("human rejection must loop back" in i for i in issues), issues)

    def test_phase_order_and_count_are_immutable(self):
        path = self.agent_root / "workflows" / "deploy.yml"
        mutate_yaml(path, lambda data: data["phases"].reverse())
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("phase 0 identity/agent/model" in i for i in issues), issues)
        mutate_yaml(path, lambda data: data["phases"].append(dict(data["phases"][0])))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("release phase count" in i for i in issues), issues)

    def test_model_and_feedback_flow_are_immutable(self):
        path = self.agent_root / "workflows" / "rollback.yml"
        mutate_yaml(path, lambda data: data["phases"][0].update(
            {"model_tier": "haiku", "feeds_forward": False}))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("identity/agent/model" in i for i in issues), issues)
        self.assertTrue(any("feeds_forward" in i for i in issues), issues)

    def test_gates_adr_and_extra_runtime_controls_are_forbidden(self):
        path = self.agent_root / "workflows" / "deploy.yml"
        mutate_yaml(path, lambda data: data["phases"][0].update({
            "required_gates": ["tests"],
            "writes_adr": {"condition": "always", "target": "docs/adr/"},
            "required_when": "always",
        }))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("required_gates" in i for i in issues), issues)
        self.assertTrue(any("writes_adr" in i for i in issues), issues)
        self.assertTrue(any("required_when" in i for i in issues), issues)

    def test_validation_loop_and_stop_expression_are_immutable(self):
        path = self.agent_root / "workflows" / "rollback.yml"
        mutate_yaml(path, lambda data: (
            data["phases"][-1]["on_fail"].update({"target_phase": "rollback-plan-validation"}),
            data["stop_condition"].update({"expression": "true"}),
        ))
        issues = check_release_boundary(self.agent_root)
        self.assertTrue(any("on_fail violates immutable contract" in i for i in issues), issues)
        self.assertTrue(any("stop expression violates immutable contract" in i for i in issues), issues)


if __name__ == "__main__":
    unittest.main(verbosity=2)
