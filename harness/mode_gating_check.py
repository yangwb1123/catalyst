#!/usr/bin/env python3
"""Drift-guard: workflow `mode_gating:` blocks vs modes.yml's canonical values.

Several ``.agent/workflows/*.yml`` files restate ``modes.yml``'s per-mode
``workflow_depth`` values as a human-readable ``mode_gating:`` cross-reference
(e.g. ``discover.yml``: ``engineering: full``) alongside an ``authority:``
pointer naming the canonical fragment it restates
(``../policies/modes.yml#workflow_depth.<dimension>``). Nothing previously
checked that the restatement stayed honest — editing ``modes.yml`` without
updating the matching workflow (or vice versa) drifted silently, exactly the
class of cross-file drift ``harness/check.py``'s ``check_modes_router_tiers``
and ``check_mode_priorities`` already guard against for other fields.

Kept in its own module — not because the check itself is logically separate
from ``harness/check.py``, but because ``check.py`` was already sitting at
this repo's ``max_file_lines`` gate ceiling with zero headroom (see skill:
``refactor-large-file``). ``harness/check.py`` imports and registers
``check_workflow_mode_gating`` exactly like every other ``check_*`` function.

Not every workflow declares this shape: one with no ``mode_gating:`` block,
or whose ``authority:`` doesn't resolve to a known ``workflow_depth``
dimension (``build.yml`` restates ``harness.gates``/``reviewer_required``
under different key names, with no ``authority:`` key at all), is skipped
rather than treated as an error.
"""
import yaml

# See module docstring for the dimension-resolution rule this marker anchors.
AUTHORITY_WORKFLOW_DEPTH_MARKER = "#workflow_depth."


def _load_yaml(path):
    """Parse one YAML file; return (data, error_message_or_None)."""
    try:
        with path.open(encoding="utf-8") as fh:
            return yaml.safe_load(fh), None
    except (yaml.YAMLError, OSError) as exc:
        return None, str(exc).replace("\n", " ")


def _authority_dimension(authority):
    """Extract the `workflow_depth.<dimension>` name from an `authority:` ref.

    e.g. '../policies/modes.yml#workflow_depth.design' -> 'design'. Returns
    None when `authority` isn't a string or doesn't point at workflow_depth
    (a fragment pointing elsewhere, like build.yml's would-be
    '#workflow_depth.reviewer' under a differently-named key, is simply not
    this shape).
    """
    if not isinstance(authority, str):
        return None
    idx = authority.find(AUTHORITY_WORKFLOW_DEPTH_MARKER)
    if idx == -1:
        return None
    dimension = authority[idx + len(AUTHORITY_WORKFLOW_DEPTH_MARKER):].strip()
    return dimension or None


def _canonical_workflow_depth(modes, mode_name, dimension):
    """modes.yml's `modes.<mode_name>.workflow_depth.<dimension>` value, or None."""
    mode = modes.get(mode_name)
    depth = mode.get("workflow_depth") if isinstance(mode, dict) else None
    return depth.get(dimension) if isinstance(depth, dict) else None


def _known_workflow_depth_dimensions(modes):
    """Every `workflow_depth.<dimension>` key declared by ANY mode in modes.yml.

    Used to decide whether a workflow's `authority:` fragment "resolves to a
    known dimension" — a fragment naming a dimension nothing in modes.yml
    actually declares (typo, or a future/removed dimension) is unresolved,
    not a mismatch to report.
    """
    dimensions = set()
    for mode in modes.values():
        depth = mode.get("workflow_depth") if isinstance(mode, dict) else None
        if isinstance(depth, dict):
            dimensions.update(depth)
    return dimensions


def _gating_contracts(gating):
    """Yield flat authority contracts, supporting one or several dimensions."""
    if not isinstance(gating, dict):
        return
    if "authority" in gating:
        yield gating
        return
    for value in gating.values():
        if isinstance(value, dict) and "authority" in value:
            yield value


def check_workflow_mode_gating(agent_root):
    """workflow mode_gating values must agree with modes.yml's canonical values.

    For each workflows/*.yml's `mode_gating:` block, resolve which
    workflow_depth dimension its `authority:` names, then compare every
    declared per-mode value against modes.yml's actual
    `modes.<mode>.workflow_depth.<dimension>` for that SAME dimension and
    SAME mode. Mirrors check_modes_router_tiers/check_mode_priorities's
    issues-list-of-strings convention; see module docstring for scope.
    """
    modes_path = agent_root / "policies" / "modes.yml"
    modes_data, err = _load_yaml(modes_path)
    if err or not isinstance(modes_data, dict):
        return [] if err else [f"{modes_path}: expected a YAML mapping"]
    modes = modes_data.get("modes") or {}
    if not isinstance(modes, dict):
        modes = {}
    known_dimensions = _known_workflow_depth_dimensions(modes)

    issues = []
    for path in sorted((agent_root / "workflows").glob("*.yml")):
        data, err = _load_yaml(path)
        if err or not isinstance(data, dict):
            continue  # YAML errors reported by check.check_yaml_parses
        gating = data.get("mode_gating")
        if not isinstance(gating, dict):
            continue  # no mode_gating block — not every workflow needs one
        for contract in _gating_contracts(gating):
            dimension = _authority_dimension(contract.get("authority"))
            if dimension is None or dimension not in known_dimensions:
                continue  # authority doesn't resolve to a known workflow_depth dimension
            # Only compare keys that are BOTH declared here AND real modes in
            # modes.yml — key-agnostic, so a future mode needs no code change here.
            for mode_name in sorted(set(contract) & set(modes)):
                declared = contract[mode_name]
                canonical = _canonical_workflow_depth(modes, mode_name, dimension)
                if declared != canonical:
                    issues.append(
                        f"{path}: mode_gating.{mode_name}={declared!r} disagrees with "
                        f"{modes_path} modes.{mode_name}.workflow_depth.{dimension}="
                        f"{canonical!r}"
                    )
    return issues
