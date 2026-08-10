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
REPO_ROOT = AI_BATCH_DIR.parents[1]
SCRIPT = AI_BATCH_DIR / "pi-batch.py"
REQUIREMENT = Path(__file__).resolve().parent / "fixtures" / "requirement.md"
CROSSWALK = AI_BATCH_DIR / "afds-crosswalk.v1.yml"
TASK_KEYWORDS = AI_BATCH_DIR / "pbatch" / "task_keywords.yaml"
UI_RULES = AI_BATCH_DIR / "ui-specs" / "rules.yaml"


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
        result = json.loads(self.run_cli_output(*args).stdout)
        self.assert_legacy_output(result)
        return result

    def assert_legacy_output(self, result: dict) -> None:
        self.assertEqual(result.get("namespace"), "forgeos.legacy-ai-batch")
        self.assertEqual(result.get("version"), 1)
        self.assertIs(result.get("afds_direct_write"), False)

    def run_repo_cli_output(self, *args: str) -> subprocess.CompletedProcess:
        """Run the installed-tree CLI with optional packages enabled.

        The regular smoke helper deliberately uses ``-S`` and a temporary
        directory to prove the copied zero-dependency fallback.  This helper
        exercises the other public layout: the YAML registries loaded from a
        Forge repository whose public path base is the repository root.
        """
        env = os.environ.copy()
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        completed = subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=REPO_ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return completed

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

    def test_rules_check_resolves_bundled_files_from_repo_root(self) -> None:
        try:
            import yaml  # noqa: F401 - confirms the real registries load
        except ImportError:
            self.skipTest("PyYAML is required for the installed-tree contract")

        self.run_repo_cli_output("rules", "--check")
        result = json.loads(self.run_repo_cli_output(
            "rules", "企业订单支付审批页面", "--json").stdout)
        self.assert_legacy_output(result)
        self.assertEqual(Path(result["path_base"]), REPO_ROOT.resolve())
        sources = [source for rule in result["rules"]
                   for source in rule["files"]]
        self.assertTrue(sources)
        for source in sources:
            self.assertFalse(Path(source).is_absolute(), source)
            self.assertTrue((REPO_ROOT / source).is_file(), source)

    def test_react_native_beats_overlapping_react_platform(self) -> None:
        result = self.run_cli(
            "classify", "用 React Native 实现移动端表单页面", "--json")
        self.assertEqual(result["dominant"]["task_type"], "frontend_ui")
        self.assertEqual(result["dominant"]["platform"], "rn")
        self.assertEqual(result["dominant"]["profile"], "mobile")

    def test_frontend_business_profiles_cover_common_product_types(self) -> None:
        cases = {
            "CRM 客户跟进页面": "crm",
            "电商购物车页面": "commerce",
            "AI 对话 Agent 页面": "ai-agent",
        }
        for prompt, expected in cases.items():
            with self.subTest(prompt=prompt):
                result = self.run_cli("classify", prompt, "--json")
                self.assertEqual(result["dominant"]["task_type"], "frontend_ui")
                self.assertEqual(result["dominant"]["profile"], expected)

    def test_afds_crosswalk_requires_reclassification_for_ambiguity(self) -> None:
        crosswalk = json.loads(CROSSWALK.read_text(encoding="utf-8"))
        self.assertEqual(crosswalk["source"], {
            "namespace": "forgeos.legacy-ai-batch",
            "version": 1,
            "afds_direct_write": False,
        })
        self.assertFalse(crosswalk["policy"]["exact_mapping_direct_write"])
        self.assertEqual(crosswalk["policy"]["ambiguous_mapping_action"],
                         "second_classification_required")
        self.assertEqual(set(crosswalk["profiles"]) - {"unknown"}, {
            "erp", "cms", "oa", "crm", "commerce", "ai-agent",
            "dashboard", "immersive", "marketing", "mobile",
        })
        self.assertEqual(set(crosswalk["page_types"]), {
            "form", "table", "detail", "workbench", "wizard", "editor",
            "canvas", "chat", "master-detail", "settings", "timeline",
            "map", "immersive", "auth",
        })
        self.assertEqual(set(crosswalk["platforms"]) - {"unknown"},
                         {"tsx", "vue", "dart", "rn"})
        for group in ("profiles", "page_types", "platforms"):
            for legacy_id, entry in crosswalk[group].items():
                with self.subTest(group=group, legacy_id=legacy_id,
                                  contract="rationale"):
                    self.assertTrue(entry["rationale_required"])
                if entry["mapping"] == "ambiguous":
                    with self.subTest(group=group, legacy_id=legacy_id):
                        self.assertTrue(entry["second_classification_required"])
                        self.assertTrue(entry["rationale_required"])
                        self.assertGreaterEqual(len(entry["candidates"]), 2)

    def test_afds_crosswalk_candidates_exist_in_canonical_catalog(self) -> None:
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML is required to parse the canonical catalog")
        crosswalk = json.loads(CROSSWALK.read_text(encoding="utf-8"))
        keywords = yaml.safe_load(TASK_KEYWORDS.read_text(encoding="utf-8"))
        legacy_rules = yaml.safe_load(UI_RULES.read_text(encoding="utf-8"))
        self.assertEqual(set(crosswalk["profiles"]) - {"unknown"},
                         set(keywords["profiles"]))
        self.assertEqual(set(crosswalk["platforms"]) - {"unknown"},
                         set(keywords["platforms"]))
        self.assertEqual(set(crosswalk["page_types"]),
                         set(legacy_rules["page_types"]))
        catalog_path = REPO_ROOT / crosswalk["policy"]["canonical_catalog"]
        catalog = yaml.safe_load(catalog_path.read_text(encoding="utf-8"))
        canonical = {
            "profiles": {item["id"] for item in catalog["profiles"]},
            "page_types": {item["id"] for item in catalog["page_patterns"]},
            "platforms": set(catalog["platforms"]),
        }
        for group, allowed in canonical.items():
            for legacy_id, entry in crosswalk[group].items():
                with self.subTest(group=group, legacy_id=legacy_id):
                    self.assertTrue(set(entry["candidates"]) <= allowed)

    def test_rules_recognize_extended_page_patterns(self) -> None:
        cases = {
            "实现多步骤向导页面": "wizard",
            "实现 CMS 内容编辑器页面": "editor",
            "实现流程设计器画布页面": "canvas",
            "实现 AI 对话页面": "chat",
            "实现主从列表详情页面": "master-detail",
            "实现系统设置页面": "settings",
            "实现客户活动时间线页面": "timeline",
            "实现 GIS 地图页面": "map",
        }
        for prompt, expected in cases.items():
            with self.subTest(prompt=prompt):
                result = self.run_cli("rules", prompt, "--json")
                self.assertEqual(result["domain"], "frontend_ui")
                self.assertIn(expected, result["page_types"])

    def test_human_outputs_declare_canonical_path_base(self) -> None:
        commands = (
            ("classify", "用 Flutter 实现 ERP 排程列表页"),
            ("rules", "企业订单+支付审批,生产"),
            ("assess", "--file", str(REQUIREMENT)),
        )
        for command in commands:
            with self.subTest(command=command[0]):
                output = self.run_cli_output(*command).stdout
                self.assertIn("namespace=forgeos.legacy-ai-batch", output)
                self.assertIn("afds_direct_write=false", output)
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
                result = json.loads(completed.stdout)
                self.assert_legacy_output(result)
                return result

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
