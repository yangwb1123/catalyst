"""Offline entry-point smoke tests for the four documented subcommands."""

from __future__ import annotations

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest


AI_BATCH_DIR = Path(__file__).resolve().parents[1]
SCRIPT = AI_BATCH_DIR / "pi-batch.py"
REQUIREMENT = Path(__file__).resolve().parent / "fixtures" / "requirement.md"


class CLISmokeTest(unittest.TestCase):
    def run_cli_output(self, *args: str) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        with tempfile.TemporaryDirectory() as cwd:
            completed = subprocess.run(
                [sys.executable, "-S", str(SCRIPT), *args],
                cwd=cwd,
                env=env,
                capture_output=True,
                text=True,
                timeout=15,
                check=False,
            )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return completed

    def run_cli(self, *args: str) -> dict:
        return json.loads(self.run_cli_output(*args).stdout)

    def assert_output_path(self, result: dict, source: str) -> Path:
        base = Path(result["path_base"])
        self.assertTrue(base.is_absolute(), base)
        self.assertEqual(base, base.resolve())
        self.assertFalse(Path(source).is_absolute(), source)
        target = (base / source).resolve()
        self.assertTrue(target.is_file(), target)
        return target

    def test_classify_runs_without_optional_packages(self) -> None:
        result = self.run_cli(
            "classify", "用 Flutter 实现 ERP 排程列表页", "--json")
        self.assertEqual(result["dominant"]["task_type"], "frontend_ui")
        self.assertEqual(result["dominant"]["system_type"], "optimization")
        self.assert_output_path(result, result["frontend_pipeline"])

    def test_rules_routes_to_bundled_methodology(self) -> None:
        result = self.run_cli("rules", "企业订单+支付审批,生产", "--json")
        rule = next(item for item in result["rules"]
                    if item["id"] == "system-type-state-machine")
        self.assert_output_path(result, rule["files"][0])
        for item in result["rules"]:
            for source in item["files"]:
                self.assert_output_path(result, source)

    def test_assess_reads_an_offline_file(self) -> None:
        result = self.run_cli("assess", "--file", str(REQUIREMENT), "--json")
        self.assertEqual(result["classification"]["task_type"], "backend")
        self.assertEqual(result["classification"]["system_type"], "state-machine")
        self.assertGreater(result["completeness"]["score"], 0)
        for source in result["product"]["specs"]:
            self.assert_output_path(result, source)
        workflow_paths = re.findall(
            r"\.agent/workflows/[\w-]+\.yml", result["workflow"]["suggestion"])
        self.assertTrue(workflow_paths)
        for source in workflow_paths:
            self.assert_output_path(result, source)
        frontend = self.run_cli(
            "assess", "实现 Flutter 企业订单支付审批表单页面", "--json")
        for item in frontend["prescription"]:
            for source in item["files"]:
                self.assert_output_path(frontend, source)

    def test_eval_loads_bundled_fixtures_from_any_cwd(self) -> None:
        result = self.run_cli("eval", "--json")
        self.assertGreater(result["total"], 0)
        self.assertEqual(result["failed"], 0, result["failures"])

    def test_rules_check_validates_effective_bundled_registries(self) -> None:
        self.run_cli_output("rules", "--check")

    def test_human_outputs_declare_canonical_path_base(self) -> None:
        commands = (
            ("classify", "用 Flutter 实现 ERP 排程列表页"),
            ("rules", "企业订单+支付审批,生产"),
            ("assess", "--file", str(REQUIREMENT)),
        )
        for command in commands:
            with self.subTest(command=command[0]):
                output = self.run_cli_output(*command).stdout
                match = re.search(r"(?m)^Path base: (.+)$", output)
                self.assertIsNotNone(match, output)
                base = Path(match.group(1))
                self.assertTrue(base.is_absolute() and base.is_dir(), base)
                self.assertEqual(base, base.resolve())

    def test_eval_quick_coverage_reports_only_executed_suites(self) -> None:
        result = self.run_cli("eval", "--quick", "--coverage", "--json")
        expected_total = sum(
            len(json.loads((AI_BATCH_DIR / "evals" / name).read_text())["cases"])
            for name in ("classifier.yaml", "rules.yaml")
        )
        self.assertEqual(result["total"], expected_total)
        self.assertEqual({item["suite"] for item in result["coverage"]},
                         {"classifier", "rules"})
        self.assertEqual(sum(item["cases"] for item in result["coverage"]),
                         result["total"])

    def test_eval_filter_coverage_reports_only_executed_case(self) -> None:
        result = self.run_cli(
            "eval", "--filter", "backend_workflow", "--coverage", "--json")
        self.assertEqual(result["total"], 1)
        self.assertEqual(result["coverage"], [{
            "suite": "classifier", "cases": 1,
            "domains": ["backend", "state-machine"],
        }])

    def test_copied_bundle_is_self_contained_without_forge_project(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            copied = Path(root) / "ai-batch"
            shutil.copytree(AI_BATCH_DIR, copied)
            script = copied / "pi-batch.py"
            env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}

            def copied_cli(*args: str) -> dict:
                completed = subprocess.run(
                    [sys.executable, "-S", str(script), *args],
                    cwd=root, env=env, capture_output=True, text=True,
                    timeout=15, check=False,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)
                return json.loads(completed.stdout)

            classified = copied_cli("classify", "实现 Flutter 表单页面", "--json")
            self.assertEqual(Path(classified["path_base"]), copied.resolve())
            self.assert_output_path(classified, classified["frontend_pipeline"])
            matched = copied_cli("rules", "企业订单支付审批页面", "--json")
            for rule in matched["rules"]:
                for source in rule["files"]:
                    self.assert_output_path(matched, source)
            checked = subprocess.run(
                [sys.executable, "-S", str(script), "rules", "--check"],
                cwd=root, env=env, capture_output=True, text=True,
                timeout=15, check=False,
            )
            self.assertEqual(checked.returncode, 0, checked.stderr)
            assessed = copied_cli("assess", "企业订单支付审批平台", "--json")
            for source in assessed["product"]["specs"]:
                self.assert_output_path(assessed, source)
            routes = re.findall(r"methodologies/[\w.-]+\.md",
                                assessed["workflow"]["suggestion"])
            self.assertTrue(routes, assessed["workflow"])
            for source in routes:
                self.assert_output_path(assessed, source)
            evaluated = copied_cli("eval", "--json")
            self.assertEqual(evaluated["failed"], 0, evaluated["failures"])


if __name__ == "__main__":
    unittest.main()
