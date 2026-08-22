#!/usr/bin/env python3
"""Validate activation, canonical capability ownership and real detector wiring."""
import re

from engineering_check_support import (
    header_issues,
    load_yaml,
    mapping_issues,
    repo_path_issue,
    unique_id_issues,
    unknown_field_issues,
)


ENFORCED_BINDINGS = {
    "harness.governance": {
        "argv": ["python3", "harness/check.py"],
        "adapter": "acceptance.probeArch",
        "criterion": "arch_violations",
        "rule_refs": ["GOV-001"],
        "tests": {
            "positive": {"path": "harness/test_check.py", "contains": "test_real_repo_passes"},
            "negative": {"path": "harness/test_check.py", "contains": "test_detects_broken_workflow_ref"},
        },
    },
    "harness.structure": {
        "argv": ["node", "harness/gate.mjs"],
        "adapter": "acceptance.probeComplexity",
        "criterion": "complexity_violations",
        "rule_refs": ["QUAL-001"],
        "tests": {
            "positive": {"path": "harness/test_gate.mjs", "contains": "checkFileSizes passes"},
            "negative": {"path": "harness/test_gate.mjs", "contains": "checkFileSizes flags"},
        },
    },
    "harness.architecture": {
        "argv": ["node", "harness/arch/arch-check.mjs"],
        "adapter": "acceptance.probeArchitecture",
        "criterion": "architecture",
        "rule_refs": ["ARCH-001", "QUAL-001"],
        "tests": {
            "positive": {"path": "harness/arch/test_arch-check.mjs", "contains": "layering: a clean inward-pointing model has no violations"},
            "negative": {"path": "harness/arch/test_arch-check.mjs", "contains": "layering: domain importing infrastructure IS flagged"},
        },
    },
    "harness.secret_scan": {
        "argv": ["node", "harness/secret-scan.mjs"],
        "adapter": "acceptance.probeSecurity",
        "criterion": "security_findings",
        "rule_refs": ["SEC-001"],
        "tests": {
            "positive": {"path": "harness/test_secret-scan.mjs", "contains": "scanRepo over the real repo finds ZERO hardcoded secrets"},
            "negative": {"path": "harness/test_secret-scan.mjs", "contains": "a high-entropy secret assignment is detected"},
        },
    },
}


def check_engineering_activation(agent_root, relative, canonical_refs, extension_refs):
    path = agent_root / relative
    data, err = load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = mapping_issues(data, path, "engineering activation")
    if issues:
        return issues
    fields = {
        "api_version", "kind", "status", "owner", "version",
        "default_activation", "legacy_missing_project_binding",
        "enforce_supported", "completion_authority", "canonical_refs",
        "canonical_extension_refs",
    }
    issues.extend(unknown_field_issues(data, fields, path))
    issues.extend(header_issues(data, path, "EngineeringActivation"))
    expected = {
        "version": 1, "default_activation": "shadow",
        "legacy_missing_project_binding": "shadow", "enforce_supported": False,
        "completion_authority": "forge_accept", "canonical_refs": canonical_refs,
        "canonical_extension_refs": extension_refs,
    }
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{path}: {field} must remain {value!r} in v1")
    return issues


def check_engineering_project_binding(agent_root, canonical_refs):
    path = agent_root / "project.yml"
    data, err = load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    spec = data.get("engineering_spec") if isinstance(data, dict) else None
    if spec is None:
        return []  # legacy scaffold: activation.yml supplies the shadow default
    if not isinstance(spec, dict):
        return [f"{path}: engineering_spec must be a mapping when present"]
    issues = unknown_field_issues(
        spec, {"version", "activation", "refs", "completion_authority"},
        f"{path}: engineering_spec",
    )
    if spec.get("version") != 1:
        issues.append(f"{path}: engineering_spec.version must be 1")
    if spec.get("activation") not in {"off", "shadow", "enforce"}:
        issues.append(f"{path}: engineering_spec.activation is invalid")
    if spec.get("activation") == "enforce":
        issues.append(f"{path}: v1 is shadow-only until runtime adapters are implemented")
    if spec.get("refs") != canonical_refs:
        issues.append(f"{path}: engineering_spec.refs must match the canonical v1 registry")
    if spec.get("completion_authority") != "forge_accept":
        issues.append(f"{path}: completion_authority must remain forge_accept")
    return issues


def _acceptance_load_bearing(repo_root):
    path = repo_root / "harness" / "acceptance.mjs"
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return set(), "acceptance.mjs is unavailable"
    match = re.search(r"export const LOAD_BEARING\s*=\s*\[([^\]]+)\]", text)
    if not match:
        return set(), "LOAD_BEARING registry is unavailable"
    return set(re.findall(r"['\"]([a-z_]+)['\"]", match.group(1))), None


def _acceptance_wiring_issues(repo_root, detector_id, expected):
    path = repo_root / "harness" / "acceptance.mjs"
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"{path}: cannot verify detector wiring ({exc})"]
    function_name = expected["adapter"].split(".")[-1]
    definition = re.search(rf"export function {re.escape(function_name)}\s*\(.*?\)\s*{{", text)
    collect = re.search(r"export function collect\(\).*?return \[(.*?)\]\.map", text, re.S)
    issues = []
    if not definition or not collect or not re.search(rf"\b{re.escape(function_name)}\(\)", collect.group(1)):
        issues.append(f"{path}: {detector_id!r} is not invoked by forge_accept collect()")
    start = definition.start() if definition else 0
    next_function = text.find("export function ", start + 1)
    body = text[start:next_function if next_function >= 0 else len(text)]
    if expected["criterion"] not in body:
        issues.append(f"{path}: {function_name} is not bound to {expected['criterion']!r}")
    actual_argv = _probe_run_argv(body)
    if actual_argv != expected["argv"]:
        issues.append(
            f"{path}: {function_name} executes {actual_argv!r}, "
            f"expected exact detector argv {expected['argv']!r}"
        )
    verdict = re.search(
        rf"return\s+result\(\s*['\"]{re.escape(expected['criterion'])}['\"]\s*,"
        r"\s*r\.ok\s*\?\s*PASS\s*:\s*FAIL\s*,",
        body,
    )
    if not verdict:
        issues.append(f"{path}: {function_name} does not derive PASS/FAIL from the detector exit")
    return issues


def _probe_run_argv(body):
    """Extract the sole `const r = run(...join(HARNESS_DIR...))` argv."""
    if len(re.findall(r"\brun\s*\(", body)) != 1:
        return None
    match = re.search(
        r"\bconst\s+r\s*=\s*run\(\s*(['\"])(?P<exe>[^'\"]+)\1\s*,\s*"
        r"\[\s*join\(\s*HARNESS_DIR(?P<parts>(?:\s*,\s*['\"][^'\"]+['\"])+)"
        r"\s*\)\s*\]\s*\)\s*;",
        body,
    )
    if not match:
        return None
    parts = re.findall(r"['\"]([^'\"]+)['\"]", match.group("parts"))
    return [match.group("exe"), "/".join(["harness", *parts])]


def _test_binding_issues(repo_root, detector_id, kind, binding, path):
    label = f"{path}: detector {detector_id!r} {kind} test"
    if not isinstance(binding, dict) or set(binding) != {"path", "contains"}:
        return [f"{label} requires exactly path and contains"]
    issue = repo_path_issue(repo_root, binding.get("path"), label)
    if issue:
        return [issue]
    target = repo_root / binding["path"]
    if not target.is_file():
        return [f"{label}: referenced test is not a regular file"]
    token = binding.get("contains")
    if not isinstance(token, str) or not token.strip():
        return [f"{label}: contains must be non-empty"]
    if token not in target.read_text(encoding="utf-8"):
        return [f"{label}: case marker {token!r} is not present"]
    return []


def _enforced_binding_issues(item, detector_id, argv, invocation, load_bearing, label):
    expected = ENFORCED_BINDINGS.get(detector_id)
    actual = {"argv": argv, "adapter": invocation.get("adapter"),
              "criterion": invocation.get("acceptance_criterion"),
              "rule_refs": item.get("rule_refs"), "tests": item.get("tests")}
    issues = []
    if expected != actual:
        issues.append(f"{label}: enforced binding is not a registered forge_accept detector")
    if invocation.get("owner") != "forge_accept" or invocation.get("load_bearing") is not True:
        issues.append(f"{label}: enforced detector must be load-bearing under forge_accept")
    if invocation.get("acceptance_criterion") not in load_bearing:
        issues.append(f"{label}: criterion is not load-bearing in acceptance.mjs")
    return issues


def _entrypoint_argument(argv):
    if argv[:3] == ["python3", "-I", "-B"]:
        return argv[3] if len(argv) > 3 else None
    return argv[1] if len(argv) > 1 else None


def _detector_shape_issues(item, path, repo_root, load_bearing):
    detector_id = item.get("id", "<unknown>")
    label = f"{path}: detector {detector_id!r}"
    fields = {"id", "version", "state", "rule_refs", "implementation", "invocation", "fail_closed", "tests"}
    issues = unknown_field_issues(item, fields, label)
    if not re.fullmatch(r"[a-z][a-z0-9_.]+", str(detector_id)):
        issues.append(f"{label}: invalid id")
    if not re.fullmatch(r"\d+\.\d+\.\d+", str(item.get("version", ""))):
        issues.append(f"{label}: version must be semantic")
    implementation, invocation = item.get("implementation"), item.get("invocation")
    if not isinstance(implementation, dict) or set(implementation) != {"argv", "cwd", "shell"}:
        issues.append(f"{label}: implementation shape is invalid")
        implementation = {}
    if implementation.get("cwd") != "repo_root" or implementation.get("shell") is not False:
        issues.append(f"{label}: detector must use repo_root with shell=false")
    argv = implementation.get("argv")
    if not isinstance(argv, list) or not argv or not all(isinstance(v, str) and v for v in argv):
        issues.append(f"{label}: argv must be a non-empty string list")
    elif len(argv) > 1:
        entrypoint = _entrypoint_argument(argv)
        issue = repo_path_issue(repo_root, entrypoint, f"{label} entrypoint")
        if issue:
            issues.append(issue)
        elif not (repo_root / entrypoint).is_file():
            issues.append(f"{label}: entrypoint is not a regular file")
    invocation_fields = {"owner", "adapter", "acceptance_criterion", "load_bearing"}
    if not isinstance(invocation, dict) or set(invocation) != invocation_fields:
        issues.append(f"{label}: invocation shape is invalid")
        invocation = {}
    if item.get("state") == "enforced":
        issues.extend(_enforced_binding_issues(
            item, detector_id, argv, invocation, load_bearing, label,
        ))
    if item.get("fail_closed") is not True:
        issues.append(f"{label}: fail_closed must be true")
    rule_refs = item.get("rule_refs")
    if not isinstance(rule_refs, list) or not rule_refs or not all(isinstance(v, str) for v in rule_refs):
        issues.append(f"{label}: rule_refs must be a non-empty string list")
    tests = item.get("tests")
    if not isinstance(tests, dict) or set(tests) != {"positive", "negative"}:
        issues.append(f"{label}: tests must contain positive and negative bindings")
    else:
        for kind in ("positive", "negative"):
            issues.extend(_test_binding_issues(repo_root, detector_id, kind, tests[kind], path))
    return issues


def check_engineering_detectors(agent_root, relative):
    path = agent_root / relative
    data, err = load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = mapping_issues(data, path, "detector registry")
    if issues:
        return issues
    fields = {"api_version", "kind", "status", "owner", "states", "result_contract", "detectors"}
    issues.extend(unknown_field_issues(data, fields, path))
    issues.extend(header_issues(data, path, "DetectorRegistry"))
    if set(data.get("states") or []) != {"enforced", "shadow", "planned", "retired"}:
        issues.append(f"{path}: detector states vocabulary is invalid")
    if data.get("result_contract") != "acceptance-criterion/v1":
        issues.append(f"{path}: result_contract must remain acceptance-criterion/v1")
    detectors = data.get("detectors")
    id_issues, ids = unique_id_issues(detectors, path, "detector")
    issues.extend(id_issues)
    repo_root = agent_root.parent
    load_bearing, error = _acceptance_load_bearing(repo_root)
    if error:
        issues.append(f"{path}: {error}")
    for item in detectors if isinstance(detectors, list) else []:
        if isinstance(item, dict):
            if item.get("state") not in set(data.get("states") or []):
                issues.append(f"{path}: detector {item.get('id')!r} has invalid state")
            issues.extend(_detector_shape_issues(item, path, repo_root, load_bearing))
    rules, rules_err = load_yaml(agent_root / "engineering" / "rules.yml")
    rule_ids = {item.get("id") for item in (rules or {}).get("rules", []) if isinstance(item, dict)} if not rules_err else set()
    for item in detectors if isinstance(detectors, list) else []:
        if isinstance(item, dict) and set(item.get("rule_refs") or []) - rule_ids:
            issues.append(f"{path}: detector {item.get('id')!r} has dangling rule_refs")
    missing = set(ENFORCED_BINDINGS) - ids
    if missing:
        issues.append(f"{path}: required enforced detectors missing: {sorted(missing)}")
    for detector_id, expected in ENFORCED_BINDINGS.items():
        issues.extend(_acceptance_wiring_issues(repo_root, detector_id, expected))
    return issues


def detector_index(agent_root, relative):
    data, err = load_yaml(agent_root / relative)
    if err or not isinstance(data, dict):
        return {}
    return {item.get("id"): item for item in data.get("detectors", []) if isinstance(item, dict)}


def check_capability_ownership(agent_root, catalog_ref, map_ref):
    repo_root = agent_root.parent
    catalog, catalog_err = load_yaml(repo_root / catalog_ref)
    ownership, ownership_err = load_yaml(repo_root / map_ref)
    if catalog_err or ownership_err:
        return [f"capability catalogs invalid: {catalog_err or ownership_err}"]
    issues = []
    if catalog.get("status") != "planning_only" or catalog.get("executable") is not False:
        issues.append(f"{catalog_ref}: catalog must remain planning_only and non-executable")
    capabilities = []
    for node in catalog.get("nodes", []):
        if isinstance(node, dict):
            capabilities.extend(node.get("capabilities") or [])
    packages = ownership.get("packages") if isinstance(ownership, dict) else None
    owned = [cap for item in packages or [] if isinstance(item, dict) for cap in item.get("includes", [])]
    if len(owned) != len(set(owned)):
        issues.append(f"{map_ref}: a capability has multiple primary Skill owners")
    if set(owned) != set(capabilities):
        missing, extra = set(capabilities) - set(owned), set(owned) - set(capabilities)
        issues.append(f"{map_ref}: ownership coverage mismatch missing={sorted(missing)} extra={sorted(extra)}")
    if ownership.get("status") != "planning_only" or ownership.get("executable") is not False:
        issues.append(f"{map_ref}: ownership map must remain planning_only and non-executable")
    return issues
