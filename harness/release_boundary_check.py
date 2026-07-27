#!/usr/bin/env python3
"""Validate ForgeOS's declarative Deploy/Rollback trust boundary.

This check intentionally validates declarations, not runtime enforcement or
production state. It proves the shipped workflows stay human-gated, preserve
their immutable exact per-phase emit sets, and never request a remote tool. It
does not prove a target project uses a compatible forge-core binary or claim
that any external CI/operator action occurred.
"""
from collections import Counter
from pathlib import PurePosixPath

import yaml


RELEASE_PREFIX = "docs/release/"
RELEASE_AGENT = "release-engineer"
WORKFLOW_SPECS = {
    "deploy": {
        "next": "evolve",
        "expression": "external_apply_evidence_verified_by_human == true",
        "phases": [
            {
                "name": "release-planning",
                "feeds_forward": True,
                "emits": [
                    "docs/release/release-manifest.yml",
                    "docs/release/deployment-plan.md",
                    "docs/release/deployment-runbook.md",
                    "docs/release/go-no-go-checklist.md",
                ],
            },
            {
                "name": "release-plan-validation",
                "feeds_forward": False,
                "emits": ["docs/release/deployment-validation.md"],
                "on_fail": {
                    "action": "loop_back",
                    "target_phase": "release-planning",
                },
            },
        ],
    },
    "rollback": {
        "next": "",
        "expression": "external_rollback_evidence_verified_by_human == true",
        "phases": [
            {
                "name": "rollback-planning",
                "feeds_forward": True,
                "emits": [
                    "docs/release/rollback-plan.md",
                    "docs/release/rollback-runbook.md",
                    "docs/release/rollback-checklist.md",
                ],
            },
            {
                "name": "rollback-plan-validation",
                "feeds_forward": False,
                "emits": ["docs/release/rollback-validation.md"],
                "on_fail": {
                    "action": "loop_back",
                    "target_phase": "rollback-planning",
                },
            },
        ],
    },
}


def _load_yaml(path):
    try:
        with path.open(encoding="utf-8") as handle:
            return yaml.safe_load(handle), None
    except (yaml.YAMLError, OSError) as exc:
        return None, str(exc).replace("\n", " ")


def _iter_values(node, key):
    if isinstance(node, dict):
        for name, value in node.items():
            if name == key:
                yield value
            yield from _iter_values(value, key)
    elif isinstance(node, list):
        for value in node:
            yield from _iter_values(value, key)


def _safe_release_emit(value):
    if not isinstance(value, str) or not value.startswith(RELEASE_PREFIX):
        return False
    parts = PurePosixPath(value).parts
    return ".." not in parts and not PurePosixPath(value).is_absolute()


def _phase_issues(path, phases, spec):
    if not isinstance(phases, list) or not phases:
        return [f"{path}: release workflow must declare phases"]
    expected = spec["phases"]
    issues = []
    if len(phases) != len(expected):
        issues.append(
            f"{path}: release phase count {len(phases)} must equal {len(expected)}"
        )
    for index, phase in enumerate(phases):
        if not isinstance(phase, dict):
            issues.append(f"{path}: every release phase must be a mapping")
            continue
        if index >= len(expected):
            issues.append(f"{path}: unexpected extra phase '{phase.get('name', '<unnamed>')}'")
            continue
        issues.extend(_one_phase_issues(path, phase, expected[index], index))
    return issues


def _one_phase_issues(path, phase, expected, index):
    name = phase.get("name", "<unnamed>")
    issues = []
    identity = (
        name == expected["name"]
        and phase.get("agent") == RELEASE_AGENT
        and phase.get("readonly") is True
        and phase.get("model_tier") == "sonnet"
    )
    if not identity:
        issues.append(f"{path}: phase {index} identity/agent/model violates immutable contract")
    for key in ("required_gates", "requires_tools", "depends_on", "optional_for"):
        if phase.get(key):
            detail = "must not request remote tools" if key == "requires_tools" else f"must leave {key} empty"
            issues.append(f"{path}: phase '{name}' {detail}")
    for key in (
        "writes_adr", "required_when", "confidence_metric",
        "uses_template", "secondary_template",
    ):
        if phase.get(key):
            issues.append(f"{path}: phase '{name}' must not declare {key}")
    if phase.get("fresh_context") not in (None, False):
        issues.append(f"{path}: phase '{name}' must not enable fresh_context")
    emits = phase.get("emits")
    if not isinstance(emits, list) or Counter(emits) != Counter(expected["emits"]):
        issues.append(f"{path}: phase '{name}' emits violate immutable contract")
    if isinstance(emits, list):
        for emit in emits:
            if not _safe_release_emit(emit):
                issues.append(f"{path}: phase '{name}' emits outside {RELEASE_PREFIX}: {emit!r}")
    if bool(phase.get("feeds_forward")) != expected["feeds_forward"]:
        issues.append(f"{path}: phase '{name}' feeds_forward violates immutable contract")
    if phase.get("on_fail") != expected.get("on_fail"):
        issues.append(f"{path}: phase '{name}' on_fail violates immutable contract")
    return issues


def _stop_issues(path, stop, spec):
    if not isinstance(stop, dict):
        return [f"{path}: release workflow must declare stop_condition"]
    issues = []
    if stop.get("type") != "human_gate" or stop.get("human_approval") != "required":
        issues.append(f"{path}: external application must end at a required human_gate")
    if stop.get("durable_wait") is not True:
        issues.append(f"{path}: human gate must declare durable_wait: true")
    if stop.get("expression") != spec["expression"]:
        issues.append(f"{path}: stop expression violates immutable contract")
    for key in ("all_of", "anti_pattern", "on_met", "on_unmet"):
        if stop.get(key):
            issues.append(f"{path}: stop_condition must not declare {key}")
    first_name = spec["phases"][0]["name"]
    rejected = {"action": "loop_back", "target_phase": first_name}
    if stop.get("on_rejected") != rejected:
        issues.append(f"{path}: human rejection must loop back to '{first_name}'")
    approved = stop.get("on_approved") or {}
    next_stage = approved.get("next_stage") if isinstance(approved, dict) else None
    if (next_stage or "") != spec["next"]:
        label = spec["next"] or "no next_stage (standalone)"
        issues.append(f"{path}: approved handoff must be {label!r}")
    return issues


def _workflow_issues(agent_root, name, spec):
    path = agent_root / "workflows" / f"{name}.yml"
    data, err = _load_yaml(path)
    if err or not isinstance(data, dict):
        return [f"{path}: missing or invalid release workflow ({err or 'expected mapping'})"]
    issues = []
    if (
        data.get("id") != name
        or data.get("stage") != name
        or data.get("readonly") is not True
        or data.get("loop") is not None
    ):
        issues.append(f"{path}: immutable top-level id/stage/readonly/loop shape")
    phases = data.get("phases")
    issues.extend(_phase_issues(path, phases, spec))
    issues.extend(_stop_issues(path, data.get("stop_condition"), spec))
    return issues


def _spine_issues(agent_root):
    build_path = agent_root / "workflows" / "build.yml"
    build, err = _load_yaml(build_path)
    if err or not isinstance(build, dict):
        return []  # parse/missing errors are owned by check_yaml_parses
    next_stages = list(_iter_values(build.get("stop_condition"), "next_stage"))
    issues = [] if next_stages == ["deploy"] else [
        f"{build_path}: converged Build must hand off exactly to deploy"
    ]
    for path in sorted((agent_root / "workflows").glob("*.yml")):
        data, err = _load_yaml(path)
        if err or not isinstance(data, dict) or path.name == "rollback.yml":
            continue
        if "rollback" in _iter_values(data, "next_stage"):
            issues.append(f"{path}: rollback is standalone and must not be a next_stage")
    return issues


def _card_issues(agent_root):
    path = agent_root / "agents" / "release-engineer.md"
    try:
        text = path.read_text(encoding="utf-8").lower()
    except OSError as exc:
        return [f"{path}: missing release-engineer card ({exc})"]
    required = ["docs/release/**", "不执行 `kubectl`", "外部 ci/operator", "approval marker"]
    return [
        f"{path}: missing production-boundary statement {fragment!r}"
        for fragment in required if fragment not in text
    ]


def check_release_boundary(agent_root):
    """Deploy/rollback assets must stay local, human-gated and auditable."""
    issues = []
    for name, spec in WORKFLOW_SPECS.items():
        issues.extend(_workflow_issues(agent_root, name, spec))
    issues.extend(_spine_issues(agent_root))
    issues.extend(_card_issues(agent_root))
    return issues
