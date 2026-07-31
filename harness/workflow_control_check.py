"""Workflow control-flow integrity rules used by ``harness/check.py``."""

import posixpath

import yaml


VALID_MODEL_TIERS = {"haiku", "sonnet", "opus"}
EVOLVE_SCAN_CONTRACT = "evolve_scan_v1"
PHASE_REF_KEYS = {"target_phase", "loop_back_to"}
STAGE_REF_KEYS = {"next_stage"}
VALID_ACTIONS = {"loop_back", "loop_to_next_roadmap_item"}


def _load_yaml(path):
    try:
        with path.open(encoding="utf-8") as stream:
            return yaml.safe_load(stream), None
    except (yaml.YAMLError, OSError) as exc:
        return None, str(exc).replace("\n", " ")


def _workflow_phases(data):
    """Return the runnable phase list using the asset loader's hoisting rule."""
    if not isinstance(data, dict):
        return []
    phases = data.get("phases")
    if isinstance(phases, list) and phases:
        return phases
    loop = data.get("loop")
    loop_phases = loop.get("phases") if isinstance(loop, dict) else None
    if isinstance(loop_phases, list):
        return loop_phases
    return phases if isinstance(phases, list) else []


def _iter_key_values(node, keys):
    """Yield scalar values under any named key, at any nesting depth."""
    if isinstance(node, dict):
        for key, value in node.items():
            if key in keys and isinstance(value, (str, int, float)):
                yield value
            yield from _iter_key_values(value, keys)
    elif isinstance(node, list):
        for value in node:
            yield from _iter_key_values(value, keys)


def _workflow_phase_names(data):
    return {
        phase.get("name")
        for phase in _workflow_phases(data)
        if (
            isinstance(phase, dict)
            and isinstance(phase.get("name"), str)
            and phase.get("name").strip()
        )
    }


def _normalized_emit_identity(emit):
    """Match forge-core's portable slash + path-clean emit identity."""
    return posixpath.normpath(emit.replace("\\", "/"))


def _workflow_structure_issues(path, data):
    """Enforce the runtime's phase-name and per-phase emit identity contract."""
    issues = []
    seen_names = {}
    for index, phase in enumerate(_workflow_phases(data)):
        if not isinstance(phase, dict):
            continue
        name = phase.get("name")
        if not isinstance(name, str) or not name.strip():
            issues.append(f"{path}: phase[{index}] has an empty name")
            continue
        if name in seen_names:
            issues.append(
                f"{path}: phase[{index}] duplicates phase name {name!r} "
                f"first declared at phase[{seen_names[name]}]"
            )
        else:
            seen_names[name] = index

        # The map is deliberately per phase. A later phase may legally revise
        # the same artifact; only duplicate ownership inside one phase is an
        # ambiguous output contract.
        seen_emits = {}
        emits = phase.get("emits")
        if not isinstance(emits, list):
            continue
        for emit in emits:
            if not isinstance(emit, str):
                continue
            normalized = _normalized_emit_identity(emit)
            if normalized in seen_emits:
                issues.append(
                    f"{path}: phase {name!r} emit {emit!r} duplicates "
                    f"normalized target {normalized!r} already declared as "
                    f"{seen_emits[normalized]!r}"
                )
            else:
                seen_emits[normalized] = emit
    return issues


def _unsupported_transition_fields(path, data):
    """Reject declarative transition effects the runtime would silently drop."""
    if not isinstance(data, dict):
        return []
    stop = data.get("stop_condition")
    approved = stop.get("on_approved") if isinstance(stop, dict) else None
    if isinstance(approved, dict) and "emit" in approved:
        return [
            f"{path}: stop_condition.on_approved.emit is unsupported; "
            "declare produced files on a phase via emits/writes_adr "
            "(approval transitions route only)"
        ]
    return []


def _scan_contract_issues(path, data):
    """Mirror forge-core's explicit Evolve scan-contract shape."""
    if not isinstance(data, dict):
        return []
    issues = []
    declared = None
    phases = _workflow_phases(data)
    for index, phase in enumerate(phases):
        if not isinstance(phase, dict) or not phase.get("scan_contract"):
            continue
        name = phase.get("name")
        contract = phase.get("scan_contract")
        if contract != EVOLVE_SCAN_CONTRACT:
            issues.append(
                f"{path}: phase {name!r} has unsupported scan_contract {contract!r}"
            )
            continue
        if declared is not None:
            issues.append(
                f"{path}: scan_contract {EVOLVE_SCAN_CONTRACT!r} is declared "
                f"by both {declared!r} and {name!r}"
            )
        declared = name
        issues.extend(_scan_phase_shape_issues(path, data, phase, index))
    if declared is not None:
        issues.extend(_scan_dependency_issues(path, phases, declared))
    return issues


def _scan_phase_shape_issues(path, data, phase, index):
    name, contract = phase.get("name"), phase.get("scan_contract")
    issues = []
    if index != 0:
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} must be "
            "the first phase so every later phase observes its validated report"
        )
    if (data.get("stage") != "evolve" or phase.get("readonly") is not True
            or phase.get("effect") != "observe"):
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} requires "
            "stage evolve, readonly=true, and effect=observe"
        )
    if phase.get("agent") == "harness" or phase.get("required_gates"):
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} must execute "
            "a non-harness Agent with required_gates=[]"
        )
    if phase.get("depends_on"):
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} is the root "
            "producer and requires depends_on=[]"
        )
    if phase.get("emits") or phase.get("writes_adr") is not None:
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} must not grant "
            "emits or writes_adr"
        )
    if phase.get("feeds_forward") is not True:
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} requires "
            "feeds_forward=true"
        )
    if phase.get("required_when") or phase.get("optional_for"):
        issues.append(
            f"{path}: phase {name!r} scan_contract {contract!r} "
            "must not be mode-skippable"
        )
    return issues


def _scan_dependency_issues(path, phases, scan_name):
    if not any(p.get("depends_on") for p in phases if isinstance(p, dict)):
        return []
    by_name = {
        phase.get("name"): phase for phase in phases
        if isinstance(phase, dict) and phase.get("name")
    }
    memo = {}

    def reaches_scan(name, active):
        if name in memo:
            return memo[name]
        if name in active:
            return False
        phase = by_name.get(name, {})
        active = active | {name}
        result = any(
            dep == scan_name or reaches_scan(dep, active)
            for dep in phase.get("depends_on", [])
            if dep in by_name
        )
        memo[name] = result
        return result

    return [
        f"{path}: phase {phase.get('name')!r} must transitively depend on "
        f"contracted scan phase {scan_name!r} when depends_on enables parallel mode"
        for phase in phases[1:]
        if isinstance(phase, dict) and not reaches_scan(phase.get("name"), set())
    ]


def check_workflow_control_flow(agent_root):
    """Validate phase, tier, stage, action and transition-effect references."""
    workflows = sorted((agent_root / "workflows").glob("*.yml"))
    parsed = []
    stages = set()
    for path in workflows:
        data, err = _load_yaml(path)
        if err:
            continue
        parsed.append((path, data))
        if isinstance(data, dict) and isinstance(data.get("stage"), str):
            stages.add(data["stage"])
    issues = []
    for path, data in parsed:
        issues.extend(_workflow_structure_issues(path, data))
        issues.extend(_scan_contract_issues(path, data))
        phases = _workflow_phase_names(data)
        for ref in _iter_key_values(data, PHASE_REF_KEYS):
            if ref not in phases:
                issues.append(
                    f"{path}: control-flow target_phase '{ref}' is not a phase in "
                    f"this workflow (have: {sorted(phases)})"
                )
        for tier in _iter_key_values(data, {"model_tier"}):
            if str(tier).lower() not in VALID_MODEL_TIERS:
                issues.append(
                    f"{path}: model_tier '{tier}' not in "
                    f"{sorted(VALID_MODEL_TIERS)} (v1 is Claude-only)"
                )
        for stage in _iter_key_values(data, STAGE_REF_KEYS):
            if stage not in stages:
                issues.append(
                    f"{path}: next_stage '{stage}' is not a known spine stage "
                    f"(have: {sorted(stages)})"
                )
        for action in _iter_key_values(data, {"action"}):
            if action not in VALID_ACTIONS:
                issues.append(
                    f"{path}: control-flow action '{action}' not in "
                    f"{sorted(VALID_ACTIONS)} (a typo silently degrades the "
                    "declared loop-back/next-item action to legacy abort/replay)"
                )
        issues.extend(_unsupported_transition_fields(path, data))
    return issues
