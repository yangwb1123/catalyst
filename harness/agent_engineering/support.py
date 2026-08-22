#!/usr/bin/env python3
"""Shared source-tree fixture support for Agent Engineering tests."""

import shutil
import stat
import sys
import tempfile
from pathlib import Path

import yaml

HARNESS_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = HARNESS_DIR.parent
CHECKER = HARNESS_DIR / "agent_engineering_check.py"
sys.path.insert(0, str(HARNESS_DIR))
from agent_engineering.contract import PROJECT_REFS  # noqa: E402
import agent_engineering_check as engineering  # noqa: E402


def _copy_tree(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, target)


def _copy_optional_source(source, target):
    try:
        mode = source.lstat().st_mode
    except FileNotFoundError:
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    if stat.S_ISDIR(mode):
        shutil.copytree(source, target)
    elif stat.S_ISREG(mode):
        shutil.copy2(source, target)
    else:
        raise ValueError(f"optional fixture source has an unsafe type: {source}")


def _seed_fixture_binding(root):
    """Give mutation tests a binding without changing a legacy source tree."""
    path = root / ".agent" / "project.yml"
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    if "engineering_spec" in data:
        return
    data["engineering_spec"] = {
        "version": 1,
        "activation": "shadow",
        "refs": dict(PROJECT_REFS),
        "completion_authority": "forge_accept",
    }
    path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")


def make_temp_repo():
    root = Path(tempfile.mkdtemp(prefix="forge-engineering-"))
    for relative in (
        ".agent", ".ai", ".arch", "harness", "docs/release",
        "docs/design/ai-engineering-os", "docs/adr", "docs/contracts",
        "skills/project-snapshot", "skills/context-engineering",
        "skills/evidence-claim-management", "skills/policy-authority",
        "skills/adr-governance", "skills/knowledge-graph-curation",
        "skills/change-impact-cost-risk",
    ):
        _copy_tree(REPO_ROOT / relative, root / relative)
    for relative in (
        "forge-core/internal/kerneloperationalcontract",
        "forge-core/internal/kerneldecisioncontract",
        "forge-core/internal/decisioncapsulecontract",
        "forge-runtime/crates/domain/src/kernel_operational_contract",
        "forge-runtime/crates/domain/src/kernel_decision_contract",
        "forge-runtime/crates/domain/src/decision_capsule_contract",
        "forge-runtime/crates/domain/src/lib.rs",
    ):
        _copy_optional_source(REPO_ROOT / relative, root / relative)
    audit = REPO_ROOT / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
    if audit.exists():
        shutil.copy2(audit, root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md")
    _seed_fixture_binding(root)
    return root


def replace_once(path, old, new):
    text = path.read_text(encoding="utf-8")
    if text.count(old) < 1:
        raise AssertionError(f"fixture token not found: {old}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
