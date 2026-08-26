#!/usr/bin/env python3
"""ForgeOS governance-layer integrity checker (`forge check`).

Host-independent validation of ForgeOS's own ``.agent/`` declarative
governance assets — the load-bearing wall applied to the governance layer
itself. Nothing else currently catches a broken agent reference or a
malformed workflow, which is exactly the kind of drift ForgeOS exists to
prevent.

Dependencies: ``python3`` + PyYAML (``pip install pyyaml``). PyYAML is the
sole third-party requirement; if it is missing the tool exits 2 with a clear
actionable message rather than crashing (see the import guard below).

CLI contract::

    python3 harness/check.py [repo_root]   # default: cwd

Exit 0 + ``forge-check: PASS (N checks)`` when clean.
Exit 1 + ``forge-check: FAIL - <k> issue(s):`` + indented list when not.
Exit 2 + ``forge-check: PyYAML is required ...`` when PyYAML is unavailable.

Design: one check == one function (each <=50 lines, data-driven, fault
tolerant); a tiny runner aggregates issues. Sibling to ``harness/gate.mjs``
(polyglot harness tools are allowed).
"""
import re
import sys
from pathlib import Path

# Standalone callers intentionally use the canonical ``python3 harness/check.py``
# argv. Disable bytecode before any third-party or repository-local imports so
# exact package closures cannot be polluted by this integrity observation.
sys.dont_write_bytecode = True

try:
    import yaml
except ImportError:  # pragma: no cover - clear actionable error, not a crash
    sys.stderr.write(
        "forge-check: PyYAML is required (pip install pyyaml). "
        "Could not import 'yaml'.\n"
    )
    sys.exit(2)

from mode_gating_check import check_workflow_mode_gating  # noqa: E402 — after yaml guard
from release_boundary_check import check_release_boundary  # noqa: E402 — after yaml guard
from workflow_control_check import check_workflow_control_flow  # noqa: E402 — after yaml guard
from workflow_verdict_check import (  # noqa: E402 — after yaml guard
    QA_VERDICT_CONTRACT,
    REVIEWER_VERDICT_CONTRACT,
    check_workflow_verdict_contracts as _check_workflow_verdict_contracts,
)
from agent_engineering_check import check_agent_engineering_spec  # noqa: E402 — after yaml guard
from engineering_check_support import read_bounded_spec  # noqa: E402 — after yaml guard
# --- domain constants (data-driven) ------------------------------------------

VALID_TIERS = {"Haiku", "Sonnet", "Opus"}  # v1: Claude only (DECISIONS D4)

# A workflow phase's `agent:` field must name a canonical role card directly
# (the phase's `name:` carries the descriptive role-stage label). There is no
# alias indirection: a name that is neither a card stem nor the `harness`
# pseudo-agent is a real broken reference. Keeping this a pure membership check
# means the checker can't silently freeze drift between workflows and cards.
# Non-LLM pseudo-agents legitimately appear as a workflow `agent:` but have no
# role card (they run the toolchain, not a model).
PSEUDO_AGENTS = {"harness"}

# Required agent-card sections, probed by heading keyword (tolerant of
# Chinese/English wording variance). Each tuple is a set of synonyms; the
# section is satisfied if ANY synonym appears in a heading/bold label.
REQUIRED_AGENT_SECTIONS = [
    ("role", ("role", "角色")),
    ("phase", ("phase", "阶段")),
    ("input", ("consumes", "input", "输入")),
    ("output", ("produces", "output", "输出")),
    ("boundaries", ("boundaries", "边界")),
    ("model", ("model", "模型", "档")),
    ("stop/handoff", ("handoff", "stop", "停止", "交接")),
]

REQUIRED_ACCEPTANCE_CRITERIA = ["test_pass", "lint", "build"]

# modes.yml `priorities:` is a per-mode ranking over these three trade-off axes.
# Each axis must be ranked 1..3 (1 = highest priority). The ranking is a WEAK
# order: ties are legal and intentional — cto declares {speed:3, quality:1,
# cost:3} ("quality only"; it produces no code, so speed and cost are equally
# irrelevant at the bottom). So the invariant is NOT a strict permutation (which
# would falsely flag cto); it is: exactly these three keys present, every value an
# int in this set. That still catches the real drift — a missing/typo'd axis or an
# out-of-range rank — without inventing a stricter shape than modes.yml declares.
PRIORITY_AXES = {"speed", "quality", "cost"}
PRIORITY_RANKS = {1, 2, 3}

# --- helpers -----------------------------------------------------------------

def _load_yaml(path):
    """Parse bounded YAML while preserving this checker's safe-load semantics."""
    try:
        raw = read_bounded_spec(path)
        return yaml.safe_load(raw.decode("utf-8")), None
    except (yaml.YAMLError, OSError, UnicodeDecodeError, ValueError, RecursionError) as exc:
        return None, str(exc).replace("\n", " ")


def _agent_card_names(agent_dir):
    """Set of declared agent role-card stems (e.g. {'architect', ...})."""
    return {p.stem for p in agent_dir.glob("*.md")}


def _iter_strings(node):
    """Yield every string scalar in a nested dict/list YAML structure."""
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for value in node.values():
            yield from _iter_strings(value)
    elif isinstance(node, list):
        for value in node:
            yield from _iter_strings(value)


def _skill_refs(text):
    """Extract skill names from `skill: <name>` references in text.

    Skill names are ``[a-z0-9-]``; the name ends at the first character
    outside that set (whitespace, punctuation, or CJK text running straight
    into a closing paren, as in the prose `(skill: foo)`).
    """
    refs = []
    for chunk in text.split("skill:")[1:]:
        token = ""
        for ch in chunk.strip():
            if ch in "abcdefghijklmnopqrstuvwxyz0123456789-":
                token += ch
            else:
                break
        if token:
            refs.append(token)
    return refs

# --- checks (one check == one function, each returns a list of issues) -------

def check_yaml_parses(agent_root):
    """Every .agent/**/*.yml must parse as valid YAML."""
    issues = []
    for path in sorted(agent_root.rglob("*.yml")):
        _, err = _load_yaml(path)
        if err:
            issues.append(f"{path}: invalid YAML ({err})")
    return issues


def _structural_text(body):
    """Lowercased text of STRUCTURAL lines only (headings + bold labels).

    A required section must be declared as a heading (`## 输入`) or a bold
    label (`**Role** — ...`), not merely mentioned in prose. Searching the
    whole body let an incidental word like "model" in a sentence satisfy a
    section it never actually declares; this restricts the haystack so the
    check measures real structure.
    """
    structural = []
    for line in body.splitlines():
        if re.match(r"^\s*#{1,6}\s", line) or re.search(r"\*\*.+?\*\*", line):
            structural.append(line)
    return "\n".join(structural).lower()


def check_agent_sections(agent_root):
    """Every agents/*.md must contain all required sections (by keyword)."""
    issues = []
    for path in sorted((agent_root / "agents").glob("*.md")):
        structural = _structural_text(path.read_text(encoding="utf-8"))
        for label, synonyms in REQUIRED_AGENT_SECTIONS:
            if not any(syn in structural for syn in synonyms):
                issues.append(
                    f"{path}: missing required section '{label}' "
                    f"(looked for {'/'.join(synonyms)})"
                )
    return issues


def check_workflow_agent_refs(agent_root):
    """Every workflow `agent:` ref must be a role-card stem or pseudo-agent."""
    issues = []
    valid = _agent_card_names(agent_root / "agents") | PSEUDO_AGENTS
    for path in sorted((agent_root / "workflows").glob("*.yml")):
        data, err = _load_yaml(path)
        if err:
            continue  # YAML errors reported by check_yaml_parses
        for phase in _collect_phases(data):
            agent = phase.get("agent") if isinstance(phase, dict) else None
            # Normalize a list-valued agent (`agent: [a, b]`) to a list before
            # the membership test — a bare `name not in valid` on a list value
            # used to CRASH with TypeError. A scalar becomes a one-element list.
            names = agent if isinstance(agent, list) else [agent] if agent else []
            for name in names:
                if name not in valid:
                    issues.append(
                        f"{path}: workflow references agent '{name}' "
                        f"with no matching .agent/agents/{name}.md"
                    )
    return issues


def check_skill_refs(agent_root):
    """Every `skill:` ref in workflows/agent cards must resolve to a file."""
    issues = []
    skills = {p.stem for p in (agent_root / "skills").glob("*.md")}
    sources = sorted((agent_root / "workflows").glob("*.yml")) + sorted(
        (agent_root / "agents").glob("*.md")
    )
    for path in sources:
        text = path.read_text(encoding="utf-8")
        for ref in _skill_refs(text):
            if ref not in skills:
                issues.append(
                    f"{path}: references skill '{ref}' "
                    f"with no matching .agent/skills/{ref}.md"
                )
    return issues


def _tier_tokens(tiers):
    """Every model token declared under tiers.{models,by_score,by_task_type}.

    `models` may be a LIST (`[haiku, sonnet]`) or a MAPPING (validate its
    VALUES, not its keys); `by_score` is a list of `{model: ...}`; and
    `by_task_type` is a task->tier mapping whose VALUES are the tiers. A bad
    tier in ANY of these slots is a real defect, so collect them all.
    """
    tokens = []
    models = tiers.get("models")
    if isinstance(models, dict):
        tokens.extend(models.values())
    elif isinstance(models, list):
        tokens.extend(models)
    for entry in tiers.get("by_score") or []:
        if isinstance(entry, dict) and "model" in entry:
            tokens.append(entry["model"])
    by_task = tiers.get("by_task_type")
    if isinstance(by_task, dict):
        tokens.extend(by_task.values())
    return tokens


def check_routing_tiers(agent_root):
    """routing/policy.yml tier names must be a subset of {Haiku,Sonnet,Opus}."""
    path = agent_root / "routing" / "policy.yml"
    data, err = _load_yaml(path)
    if err or not isinstance(data, dict):
        return [] if err else [f"{path}: expected a YAML mapping"]
    issues = []
    tiers = data.get("tiers") or {}
    for model in _tier_tokens(tiers if isinstance(tiers, dict) else {}):
        if str(model).capitalize() not in VALID_TIERS:
            issues.append(
                f"{path}: tier '{model}' not in {sorted(VALID_TIERS)} "
                f"(v1 is Claude-only)"
            )
    return issues


def check_modes_router_tiers(agent_root):
    """modes.yml router_default_tier values must be valid Claude tiers."""
    path = agent_root / "policies" / "modes.yml"
    data, err = _load_yaml(path)
    if err or not isinstance(data, dict):
        return [] if err else [f"{path}: expected a YAML mapping"]
    issues = []
    for mode_name, mode in (data.get("modes") or {}).items():
        if not isinstance(mode, dict):
            continue
        tier = mode.get("router_default_tier")
        if tier is not None and str(tier).capitalize() not in VALID_TIERS:
            issues.append(
                f"{path}: mode '{mode_name}' router_default_tier "
                f"'{tier}' not in {sorted(VALID_TIERS)}"
            )
    return issues


def _priority_issues(mode_name, prio, path):
    """Validate one mode's `priorities` block; return a list of issues.

    The honest invariant (see PRIORITY_AXES/PRIORITY_RANKS): exactly the three
    axes are present, and every rank is an int in {1,2,3}. Ties are allowed
    (cto's "quality only" ranks speed=cost=3 on purpose), so this is NOT a
    permutation check — only a missing/extra axis or an out-of-range rank fails.
    Fault tolerant: a non-mapping `priorities` (or a missing one) is reported as
    one issue, never a crash.
    """
    if prio is None:
        return [f"{path}: mode '{mode_name}' is missing a 'priorities' block"]
    if not isinstance(prio, dict):
        return [f"{path}: mode '{mode_name}' priorities must be a mapping of "
                f"{sorted(PRIORITY_AXES)} -> rank 1..3"]
    if set(prio) != PRIORITY_AXES:
        return [f"{path}: mode '{mode_name}' priorities keys {sorted(prio)} "
                f"must be exactly {sorted(PRIORITY_AXES)}"]
    return [
        f"{path}: mode '{mode_name}' priority '{axis}'={rank!r} must be an "
        f"int in {sorted(PRIORITY_RANKS)}"
        # bool is an int subclass in Python; reject True/False explicitly so a
        # stray `speed: true` is caught rather than silently read as rank 1.
        for axis, rank in prio.items()
        if isinstance(rank, bool) or not isinstance(rank, int) or rank not in PRIORITY_RANKS
    ]


def check_mode_priorities(agent_root):
    """modes.yml each mode's `priorities` must rank {speed,quality,cost} 1..3.

    A governance-completeness check (the trade-off declaration must be well
    formed), NOT a routing rule: priorities express a mode's intent and their
    EFFECT is already carried by router_default_tier + gates + evolve depth.
    This only keeps the declaration honest (no missing/extra axis, no rank
    outside 1..3); ties are intentional and permitted. See _priority_issues.
    """
    path = agent_root / "policies" / "modes.yml"
    data, err = _load_yaml(path)
    if err or not isinstance(data, dict):
        return [] if err else [f"{path}: expected a YAML mapping"]
    issues = []
    for mode_name, mode in (data.get("modes") or {}).items():
        if not isinstance(mode, dict):
            continue
        issues.extend(_priority_issues(mode_name, mode.get("priorities"), path))
    return issues


def check_acceptance_schema(agent_root):
    """eval/acceptance.schema.yml must parse and carry required criteria."""
    path = agent_root / "eval" / "acceptance.schema.yml"
    data, err = _load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    if not isinstance(data, dict):
        return [f"{path}: expected a YAML mapping"]
    criteria = data.get("criteria")
    if not isinstance(criteria, dict):
        return [f"{path}: missing 'criteria' mapping"]
    return [
        f"{path}: acceptance criteria missing required key '{key}'"
        for key in REQUIRED_ACCEPTANCE_CRITERIA
        if key not in criteria
    ]


def check_workflow_verdict_contracts(agent_root):
    """Validate the two declared Build verdict handshakes."""
    return _check_workflow_verdict_contracts(agent_root, _load_yaml, _collect_phases)


# --- runner ------------------------------------------------------------------

CHECKS = [
    check_yaml_parses,
    check_agent_sections,
    check_workflow_agent_refs,
    check_skill_refs,
    check_routing_tiers,
    check_modes_router_tiers,
    check_mode_priorities,
    check_workflow_mode_gating,
    check_acceptance_schema,
    check_workflow_verdict_contracts,
    check_workflow_control_flow,
    check_release_boundary,
    check_agent_engineering_spec,
]

def _collect_phases(node):
    """Yield every mapping carrying an `agent:` key, anywhere in the workflow.

    Recurses generically rather than hard-coding `phases:` / `loop.phases:`,
    so an `agent:` nested under any new structural shape (sub-phases, on_fail
    handlers, future keys) is still validated instead of silently skipped.
    """
    if isinstance(node, dict):
        if "agent" in node:
            yield node
        for value in node.values():
            yield from _collect_phases(value)
    elif isinstance(node, list):
        for value in node:
            yield from _collect_phases(value)


def run_checks(repo_root):
    """Run every check against ``<repo_root>/.agent``; return issue list."""
    agent_root = Path(repo_root) / ".agent"
    if not agent_root.is_dir():
        return [f"{agent_root}: governance directory not found"]
    issues = []
    for check in CHECKS:
        # Per-check isolation: a malformed asset that makes one check raise must
        # not crash the whole tool (which would forfeit the 0/1/2 exit contract).
        # Surface it as an issue (FAIL), not an uncaught traceback.
        try:
            issues.extend(check(agent_root))
        except Exception as exc:  # noqa: BLE001 — deliberately broad: fault tolerance
            issues.append(f"{check.__name__}: crashed ({type(exc).__name__}: {exc})")
    return issues


def main(argv):
    repo_root = argv[1] if len(argv) > 1 else "."
    issues = run_checks(repo_root)
    if not issues:
        print(f"forge-check: PASS ({len(CHECKS)} checks)")
        return 0
    print(f"forge-check: FAIL - {len(issues)} issue(s):")
    for issue in issues:
        print(f"  {issue}")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
