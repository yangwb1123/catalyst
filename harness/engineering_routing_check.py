#!/usr/bin/env python3
"""Validate deterministic context routes and monotonic workflow assurance."""
import math
import re
from engineering_check_support import (
    header_issues,
    load_yaml,
    mapping_issues,
    repo_path_issue,
    unique_id_issues,
    unknown_field_issues,
)
PROFILE_IDS = ["W0_direct", "W1_standard", "W2_assured", "W3_systemic"]
MATERIALITY = {"L0", "L1", "L2", "L3", "L4"}
RISK_FLOOR = {"L0": "W0_direct", "L1": "W1_standard", "L2": "W2_assured",
              "L3": "W3_systemic", "L4": "W3_systemic"}
PROFILE_FLOORS = {
    "W0_direct": {
        "required_workflows": {"build"}, "required_gates": {"lint", "build"},
        "required_reviewers": set(),
        "proof_obligations": {"changed_scope", "command_evidence", "governance_validation", "structural_gate"},
    },
    "W1_standard": {
        "required_workflows": {"build"}, "required_gates": {"lint", "test", "build", "complexity"},
        "required_reviewers": {"reviewer", "qa"},
        "proof_obligations": {"regression_evidence", "review_verdict"},
    },
    "W2_assured": {
        "required_workflows": {"discover", "design", "review", "build"},
        "required_gates": {"lint", "test", "build", "complexity", "arch", "security"},
        "required_reviewers": {"reviewer", "qa", "architect", "security-engineer"},
        "proof_obligations": {"impact_report", "rollback_plan"},
    },
    "W3_systemic": {
        "required_workflows": {"discover", "design", "review", "build", "deploy", "evolve"},
        "required_gates": {"lint", "test", "build", "complexity", "arch", "security"},
        "required_reviewers": {"reviewer", "qa", "architect", "security-engineer", "performance-engineer", "cto", "release-engineer"},
        "proof_obligations": {"architecture_decision", "release_boundary_evidence", "operator_evidence"},
    },
}
PROFILE_EXECUTION_MAX = {"W0_direct": 0.90, "W1_standard": 0.75,
                         "W2_assured": 0.45, "W3_systemic": 0.20}
PROFILE_REPAIR_MAX = {"W0_direct": 3, "W1_standard": 5, "W2_assured": 5, "W3_systemic": 5}
PROFILE_LEARNING_MAX = {"W0_direct": 0.10, "W1_standard": 0.15,
                        "W2_assured": 0.10, "W3_systemic": 0.05}
PROFILE_STOP_FLOORS = {
    "W0_direct": {
        "success": {"acceptance_satisfied", "required_proofs_present"},
        "blocked": {"hard_constraint_conflict", "authority_missing"},
    },
    "W1_standard": {
        "success": {"acceptance_satisfied", "required_proofs_present", "findings_closed"},
        "blocked": {"same_failure_repeated", "hard_constraint_conflict", "authority_missing"},
    },
    "W2_assured": {
        "success": {"acceptance_satisfied", "required_proofs_present", "findings_closed", "human_approval"},
        "blocked": {"same_failure_repeated", "hard_constraint_conflict", "authority_missing", "rollback_missing"},
    },
    "W3_systemic": {
        "success": {
            "acceptance_satisfied", "required_proofs_present", "findings_closed",
            "human_approval", "operator_evidence",
        },
        "blocked": {
            "same_failure_repeated", "hard_constraint_conflict", "authority_missing",
            "rollback_missing", "external_effect_unknown",
        },
    },
}
HUMAN_APPROVAL_PROFILES = {"W2_assured", "W3_systemic"}
CONTEXT_FIELDS = {"changed_path", "task_type", "materiality", "workflow_profile", "capability_id"}
CONTEXT_LANES = {"instruction", "trusted_context", "untrusted_data"}
CONTEXT_BUDGET_MAX = {"max_files": 24, "max_total_bytes": 524288}
REQUIRED_DENY_GLOBS = {"**/.env", "**/.env.*", "**/*private-key*", "**/.ssh/**"}
BASE_REQUIRED_FLOORS = {
    ".agent/AGENTS.md": {"lane": "instruction", "max_bytes": 65536},
    ".agent/PROJECT.md": {"lane": "trusted_context", "max_bytes": 131072},
    ".agent/project.yml": {"lane": "trusted_context", "max_bytes": 65536},
}
REQUIRED_ROUTE_IDS = {
    "governance", "architecture-boundary", "implementation", "security-change",
    "user-experience", "release-boundary", "data-and-contract", "backend-runtime",
}
ROUTE_INCLUDE_FLOORS = {
    "governance": {".agent/engineering/rules.yml": "instruction", ".agent/policies/modes.yml": "trusted_context"},
    "architecture-boundary": {".agent/skills/clean-architecture.md": "instruction", ".agent/skills/domain-modeling.md": "instruction", ".agent/skills/architecture-tradeoff.md": "instruction", ".agent/engineering/backend-decision-gates.yml": "instruction", ".agent/eval/backend-decision-package.schema.yml": "trusted_context", ".arch/rules.yaml": "trusted_context"},
    "implementation": {".agent/skills/testing.md": "instruction", ".agent/workflows/build.yml": "trusted_context"},
    "security-change": {".agent/skills/security-review.md": "instruction", ".agent/skills/secure-coding.md": "instruction", ".agent/engineering/backend-decision-gates.yml": "instruction", ".agent/eval/backend-decision-package.schema.yml": "trusted_context", "harness/secret-scan.mjs": "trusted_context"},
    "user-experience": {".agent/skills/code-review.md": "instruction", ".agent/skills/information-interaction-design.md": "instruction", ".agent/skills/design-system-accessibility.md": "instruction", ".agent/skills/ui-geometry.md": "instruction", ".agent/skills/frontend-client-engineering.md": "instruction", ".agent/skills/frontend-code-architecture.md": "instruction", ".agent/engineering/frontend-design-gates.yml": "instruction", ".agent/engineering/frontend-code-architecture.yml": "instruction", ".agent/engineering/frontend-profiles.yml": "trusted_context", ".agent/eval/frontend-design-package.schema.yml": "trusted_context", ".arch/frontend-architecture.v1.json": "trusted_context", ".arch/frontend-architecture-baseline.v1.json": "trusted_context", ".arch/frontend-architecture-waivers.v1.json": "trusted_context", "docs/design/ai-engineering-os/frontend-design-standard.md": "trusted_context", "docs/design/ai-engineering-os/frontend-code-architecture-standard.md": "trusted_context"},
    "release-boundary": {".agent/workflows/deploy.yml": "instruction", ".agent/workflows/rollback.yml": "instruction"},
    "data-and-contract": {
        ".agent/skills/testing.md": "instruction", ".agent/skills/data-modeling-transactions.md": "instruction",
        ".agent/skills/data-migration-lifecycle.md": "instruction", ".agent/skills/api-contract-design.md": "instruction",
        ".agent/engineering/backend-decision-gates.yml": "instruction",
        ".agent/eval/backend-decision-package.schema.yml": "trusted_context",
        ".agent/eval/completion-evidence.schema.yml": "trusted_context"},
    "backend-runtime": {".agent/skills/backend-engineering.md": "instruction", ".agent/skills/distributed-reliability-design.md": "instruction", ".agent/skills/performance-capacity.md": "instruction", ".agent/skills/observability-engineering.md": "instruction", ".agent/engineering/backend-decision-gates.yml": "instruction", ".agent/eval/backend-decision-package.schema.yml": "trusted_context"},
}
ROUTE_MATCH_FLOORS = {
    "governance": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": [".agent/**", "harness/**"]},
        {"field": "task_type", "operator": "in", "values": ["governance", "agent_engineering"]},
    ]},
    "architecture-boundary": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/domain/**", "**/application/**", "**/infrastructure/**", "**/interfaces/**"]},
        {"field": "capability_id", "operator": "in", "values": ["modular-architecture", "architecture-conformance", "change-impact-analysis"]},
    ]},
    "implementation": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["src/**", "lib/**", "app/**", "packages/**"]},
        {"field": "task_type", "operator": "in", "values": ["feature", "bug_fix", "refactor"]},
    ]},
    "security-change": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/auth/**", "**/security/**", "**/identity/**"]},
        {"field": "task_type", "operator": "in", "values": ["security", "authentication", "authorization"]},
        {"field": "materiality", "operator": "in", "values": ["L4"]},
    ]},
    "user-experience": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/ui/**", "**/components/**", "**/pages/**", "**/screens/**", "**/views/**", "**/routes/**", "**/widgets/**", "**/features/**", "**/entities/**", "**/shared/ui/**", "**/shared/api/**", "**/styles/**", "**/theme/**", "**/tokens/**", "**/*.tsx", "**/*.jsx", "**/*.vue", "**/*.dart", "**/*.css", "**/*.scss", "**/*.sass", "**/*.less"]},
        {"field": "capability_id", "operator": "in", "values": ["information-architecture", "interaction-design", "content-design", "visual-design", "design-system", "accessibility", "usability-testing", "frontend-engineering", "client-engineering"]},
    ]},
    "release-boundary": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any",
         "values": ["docs/release/**", ".agent/workflows/deploy.yml", ".agent/workflows/rollback.yml"]},
        {"field": "task_type", "operator": "in",
         "values": ["deploy", "release", "rollback", "production_change"]},
    ]},
    "data-and-contract": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": [
            "**/migrations/**", "**/schema/**", "**/database/**", "**/db/**", "**/sql/**",
            "**/dao/**", "**/persistence/**", "**/repositories/**", "**/entities/**",
            "**/models/**", "**/api/**", "**/contracts/**", "**/proto/**", "**/events/**"]},
        {"field": "capability_id", "operator": "in", "values": [
            "data-modeling", "schema-review", "query-index-analysis", "transaction-design",
            "migration-engineering", "data-quality", "api-design", "event-contract", "compatibility",
            "idempotency", "error-modeling", "contract-testing"]},
    ]},
    "backend-runtime": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": [
            "**/internal/**", "**/services/**", "**/controllers/**", "**/handlers/**",
            "**/models/**", "**/repositories/**", "**/persistence/**", "**/clients/**",
            "**/jobs/**", "**/workers/**", "**/queues/**", "**/cache/**"]},
        {"field": "capability_id", "operator": "in", "values": [
            "backend-engineering", "distributed-systems", "concurrency", "observability", "benchmarking"]},
    ]},
}
SELECTION_FIELDS = {
    "strategy", "route_order", "match_snapshot", "path_semantics", "max_files",
    "max_total_bytes", "on_required_missing", "on_required_overflow",
    "on_optional_overflow", "required_metadata", "omit_log_required",
    "untrusted_content_behavior", "secret_redaction_required", "merge",
    "deny_globs", "base_required",
}


def _safe_glob(raw, label):
    if not isinstance(raw, str) or not raw.strip():
        return f"{label}: glob must be a non-empty string"
    if raw.startswith(("/", "!")) or "\\" in raw or ".." in raw.split("/"):
        return f"{label}: unsafe repository-relative POSIX glob {raw!r}"
    if any(token in raw for token in ("{", "}", "$(", "`")):
        return f"{label}: unsupported glob expansion {raw!r}"
    return None


def _catalog_capabilities(agent_root):
    path = agent_root.parent / "docs/design/ai-engineering-os/capability-catalog.v1.yml"
    data, err = load_yaml(path)
    if err or not isinstance(data, dict):
        return set()
    return {
        value for node in data.get("nodes", []) if isinstance(node, dict)
        for value in node.get("capabilities", []) if isinstance(value, str)
    }


def _include_issues(item, label, repo_root):
    if not isinstance(item, dict) or set(item) != {"ref", "lane", "required", "max_bytes"}:
        return [f"{label}: include requires exactly ref/lane/required/max_bytes"]
    issues = []
    issue = repo_path_issue(repo_root, item.get("ref"), label)
    if issue:
        issues.append(issue)
    elif not (repo_root / item["ref"]).is_file():
        issues.append(f"{label}: context ref must be a regular file")
    if item.get("lane") not in CONTEXT_LANES:
        issues.append(f"{label}: invalid trust lane")
    ref = str(item.get("ref", ""))
    if ref.startswith(".ai/prompts/") and item.get("lane") != "untrusted_data":
        issues.append(f"{label}: .ai/prompts sources must remain untrusted_data")
    instruction_allowed = ref == ".agent/AGENTS.md" or ref.startswith(
        (".agent/skills/", ".agent/workflows/", ".agent/engineering/")
    )
    if item.get("lane") == "instruction" and not instruction_allowed:
        issues.append(f"{label}: instruction lane is restricted to .agent/ governance")
    if not isinstance(item.get("required"), bool):
        issues.append(f"{label}: required must be boolean")
    size = item.get("max_bytes")
    if not isinstance(size, int) or isinstance(size, bool) or size <= 0 or size > 524288:
        issues.append(f"{label}: max_bytes must be within 1..524288")
    return issues


def _predicate_issues(predicate, label, capabilities):
    if not isinstance(predicate, dict) or set(predicate) != {"field", "operator", "values"}:
        return [f"{label}: predicate requires exactly field/operator/values"]
    field, operator, values = predicate.get("field"), predicate.get("operator"), predicate.get("values")
    issues = []
    if field not in CONTEXT_FIELDS:
        issues.append(f"{label}: unknown field {field!r}")
    expected_operator = "glob_any" if field == "changed_path" else "in"
    if operator != expected_operator:
        issues.append(f"{label}: {field!r} requires operator {expected_operator!r}")
    if not isinstance(values, list) or not values or not all(isinstance(v, str) and v for v in values):
        issues.append(f"{label}: values must be a non-empty string list")
        return issues
    if field == "changed_path":
        issues.extend(issue for index, value in enumerate(values)
                      if (issue := _safe_glob(value, f"{label}.values[{index}]")))
    elif field == "materiality" and set(values) - MATERIALITY:
        issues.append(f"{label}: unknown materiality value")
    elif field == "workflow_profile" and set(values) - set(PROFILE_IDS):
        issues.append(f"{label}: unknown workflow profile")
    elif field == "capability_id" and set(values) - capabilities:
        issues.append(f"{label}: unknown capability id(s) {sorted(set(values) - capabilities)}")
    elif field == "task_type" and any(not re.fullmatch(r"[a-z][a-z0-9_]*", value) for value in values):
        issues.append(f"{label}: task_type values must be normalized identifiers")
    return issues


def _base_required_issues(base, path, repo_root):
    if not isinstance(base, list) or not base:
        return [f"{path}: selection.base_required must be non-empty"]
    issues = []
    for index, item in enumerate(base):
        issues.extend(_include_issues(item, f"{path}: base_required[{index}]", repo_root))
    by_ref = {item.get("ref"): item for item in base if isinstance(item, dict)}
    if len(by_ref) != len(base):
        issues.append(f"{path}: selection.base_required contains duplicate refs")
    for ref, floor in BASE_REQUIRED_FLOORS.items():
        item = by_ref.get(ref)
        if not item or item.get("required") is not True or item.get("lane") != floor["lane"]:
            issues.append(f"{path}: selection.base_required weakens canonical entry {ref!r}")
        elif item.get("max_bytes", floor["max_bytes"] + 1) > floor["max_bytes"]:
            issues.append(f"{path}: selection.base_required amplifies budget for {ref!r}")
    return issues


def _selection_issues(selection, path, repo_root):
    issues = unknown_field_issues(selection, SELECTION_FIELDS, f"{path}: selection")
    expected = {
        "strategy": "deterministic_bounded_union", "route_order": "priority_desc_then_id_asc",
        "match_snapshot": "frozen_task_profile", "path_semantics": "repo_relative_posix_case_sensitive",
        "on_required_missing": "block", "on_required_overflow": "block",
        "on_optional_overflow": "omit_with_receipt",
        "untrusted_content_behavior": "data_only_never_instruction",
    }
    for field, value in expected.items():
        if selection.get(field) != value:
            issues.append(f"{path}: selection.{field} must be {value!r}")
    for field, maximum in CONTEXT_BUDGET_MAX.items():
        value = selection.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            issues.append(f"{path}: selection.{field} must be a positive integer")
        elif value > maximum:
            issues.append(f"{path}: selection.{field} exceeds the v1 budget ceiling {maximum}")
    metadata = {"source_path", "source_revision", "selected_reason", "freshness", "content_sha256", "trust_lane"}
    if set(selection.get("required_metadata") or []) != metadata:
        issues.append(f"{path}: selection.required_metadata vocabulary is incomplete")
    if selection.get("omit_log_required") is not True or selection.get("secret_redaction_required") is not True:
        issues.append(f"{path}: omission logging and secret redaction must be required")
    expected_merge = {"deny_precedence": "absolute", "required_precedence": "required_wins",
                      "trust_precedence": "least_trusted_wins", "byte_limit": "minimum_wins"}
    if selection.get("merge") != expected_merge:
        issues.append(f"{path}: selection.merge must preserve the v1 fail-closed algebra")
    deny_globs = selection.get("deny_globs")
    if not isinstance(deny_globs, list) or not deny_globs:
        issues.append(f"{path}: selection.deny_globs must be a non-empty list")
    else:
        for index, raw in enumerate(deny_globs):
            issue = _safe_glob(raw, f"{path}: deny_globs[{index}]")
            if issue:
                issues.append(issue)
        if REQUIRED_DENY_GLOBS - set(deny_globs):
            issues.append(f"{path}: selection.deny_globs omits a v1 secret boundary")
    issues.extend(_base_required_issues(selection.get("base_required"), path, repo_root))
    return issues


def _route_issues(route, path, repo_root, capabilities):
    if not isinstance(route, dict):
        return []
    issues = []
    route_id = route.get("id")
    issues.extend(unknown_field_issues(route, {"id", "priority", "match", "include"}, f"{path}: context route"))
    priority = route.get("priority")
    if not isinstance(priority, int) or isinstance(priority, bool) or not 0 <= priority <= 1000:
        issues.append(f"{path}: route {route_id!r} requires priority within 0..1000")
    match = route.get("match")
    if not isinstance(match, dict) or set(match) != {"op", "predicates"}:
        issues.append(f"{path}: route {route_id!r} requires typed match op/predicates")
    else:
        if match.get("op") not in {"any", "all"}:
            issues.append(f"{path}: route {route_id!r} has invalid match op")
        predicates = match.get("predicates")
        if not isinstance(predicates, list) or not predicates:
            issues.append(f"{path}: route {route_id!r} requires non-empty predicates")
        for index, predicate in enumerate(predicates or []):
            issues.extend(_predicate_issues(predicate, f"{path}: route {route_id!r} predicate[{index}]", capabilities))
    if route_id in ROUTE_MATCH_FLOORS and match != ROUTE_MATCH_FLOORS[route_id]:
        issues.append(f"{path}: route {route_id!r} weakens its canonical v1 match trigger")
    includes = route.get("include")
    if not isinstance(includes, list) or not includes:
        issues.append(f"{path}: route {route_id!r} requires non-empty include")
    else:
        for index, item in enumerate(includes):
            issues.extend(_include_issues(item, f"{path}: route {route_id!r} include[{index}]", repo_root))
        refs = [item.get("ref") for item in includes if isinstance(item, dict)]
        if len(refs) != len(set(refs)):
            issues.append(f"{path}: route {route_id!r} contains duplicate include refs")
        by_ref = {item.get("ref"): item for item in includes if isinstance(item, dict)}
        for ref, lane in ROUTE_INCLUDE_FLOORS.get(route_id, {}).items():
            item = by_ref.get(ref)
            if not item or item.get("required") is not True or item.get("lane") != lane:
                issues.append(f"{path}: route {route_id!r} weakens required context {ref!r}")
    return issues


def check_engineering_context_routes(agent_root, relative):
    path = agent_root / relative
    data, err = load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = mapping_issues(data, path, "context route registry")
    if issues:
        return issues
    allowed = {"api_version", "kind", "status", "runtime_binding", "owner", "selection", "routes"}
    issues.extend(unknown_field_issues(data, allowed, path))
    issues.extend(header_issues(data, path, "ContextRouteRegistry"))
    selection = data.get("selection")
    if not isinstance(selection, dict):
        return [f"{path}: missing selection mapping"]
    repo_root = agent_root.parent
    issues.extend(_selection_issues(selection, path, repo_root))
    routes = data.get("routes")
    if not isinstance(routes, list) or not routes:
        return issues + [f"{path}: routes must be a non-empty list"]
    id_issues, route_ids = unique_id_issues(routes, path, "context route")
    issues.extend(id_issues)
    if route_ids != REQUIRED_ROUTE_IDS:
        issues.append(f"{path}: context routes must be exactly {sorted(REQUIRED_ROUTE_IDS)} in v1")
    capabilities = _catalog_capabilities(agent_root)
    for route in routes:
        issues.extend(_route_issues(route, path, repo_root, capabilities))
    return issues


def _known_workflows(agent_root):
    ids = set()
    for path in (agent_root / "workflows").glob("*.yml"):
        data, err = load_yaml(path)
        if not err and isinstance(data, dict) and isinstance(data.get("id"), str):
            ids.add(data["id"])
    return ids


def _known_gates(agent_root):
    data, err = load_yaml(agent_root / "policies" / "modes.yml")
    if err or not isinstance(data, dict) or not isinstance(data.get("gate_catalog"), dict):
        return set()
    return set(data["gate_catalog"])


def _stop_condition_issues(stop, profile_id, path):
    label = f"{path}: profile {profile_id!r}"
    if not isinstance(stop, dict) or set(stop) != {"success", "blocked", "max_repair_attempts"}:
        return [f"{label} requires exactly success/blocked/max_repair_attempts stop conditions"]
    issues = []
    for field in ("success", "blocked"):
        values = stop.get(field)
        if not isinstance(values, list) or not values or not all(isinstance(v, str) and v for v in values):
            issues.append(f"{label} requires non-empty string-list {field} stop conditions")
            continue
        if len(values) != len(set(values)):
            issues.append(f"{label} has duplicate {field} stop conditions")
        missing = PROFILE_STOP_FLOORS.get(profile_id, {}).get(field, set()) - set(values)
        if missing:
            issues.append(f"{label} is below its {field} stop-condition floor: {sorted(missing)}")
    attempts = stop.get("max_repair_attempts")
    if isinstance(attempts, bool) or not isinstance(attempts, int) or attempts <= 0:
        issues.append(f"{label} requires positive integer max_repair_attempts")
    elif attempts > PROFILE_REPAIR_MAX.get(profile_id, 0):
        issues.append(f"{label} exceeds v1 repair-attempt ceiling {PROFILE_REPAIR_MAX[profile_id]}")
    return issues


def _profile_issues(profile, path, workflows, agents, gates):
    profile_id = profile.get("id", "<unknown>")
    issues = []
    for field, known in (("required_workflows", workflows), ("required_reviewers", agents),
                         ("required_gates", gates)):
        values = profile.get(field)
        if not isinstance(values, list):
            issues.append(f"{path}: profile {profile_id!r} requires list {field}")
        elif set(values) - known:
            issues.append(f"{path}: profile {profile_id!r} has unknown {field}: {sorted(set(values) - known)}")
    autonomy = profile.get("autonomy")
    axes = {"information", "planning", "execution", "learning"}
    if not isinstance(autonomy, dict) or set(autonomy) != axes:
        issues.append(f"{path}: profile {profile_id!r} requires exactly {sorted(axes)} autonomy axes")
    elif any(isinstance(value, bool) or not isinstance(value, (int, float))
             or not math.isfinite(value) or value < 0 or value > 1
             for value in autonomy.values()):
        issues.append(f"{path}: profile {profile_id!r} autonomy values must be within 0..1")
    elif autonomy["execution"] > PROFILE_EXECUTION_MAX.get(profile_id, 0):
        ceiling = PROFILE_EXECUTION_MAX[profile_id]
        issues.append(f"{path}: profile {profile_id!r} execution autonomy exceeds v1 ceiling {ceiling}")
    if (isinstance(autonomy, dict) and isinstance(autonomy.get("learning"), (int, float))
            and not isinstance(autonomy["learning"], bool)
            and autonomy["learning"] > PROFILE_LEARNING_MAX.get(profile_id, 0)):
        ceiling = PROFILE_LEARNING_MAX[profile_id]
        issues.append(f"{path}: profile {profile_id!r} learning autonomy exceeds v1 ceiling {ceiling}")
    stop = profile.get("stop_conditions")
    success_conditions = stop.get("success", []) if isinstance(stop, dict) else []
    issues.extend(_stop_condition_issues(stop, profile_id, path))
    if not isinstance(profile.get("human_gate_required"), bool):
        issues.append(f"{path}: profile {profile_id!r} human_gate_required must be boolean")
    if profile_id in HUMAN_APPROVAL_PROFILES:
        if profile.get("human_gate_required") is not True:
            issues.append(f"{path}: profile {profile_id!r} requires a human gate")
        if "human_approval" not in success_conditions:
            issues.append(f"{path}: profile {profile_id!r} requires human_approval before success")
    if not isinstance(profile.get("proof_obligations"), list) or not profile.get("proof_obligations"):
        issues.append(f"{path}: profile {profile_id!r} requires proof obligations")
    floor = PROFILE_FLOORS.get(profile_id, {})
    for field, required in floor.items():
        missing = required - set(profile.get(field) or [])
        if missing:
            issues.append(f"{path}: profile {profile_id!r} is below its {field} floor: {sorted(missing)}")
    return issues


def _monotonic_profile_issues(profiles, path):
    issues = []
    fields = ("required_workflows", "required_gates", "required_reviewers", "proof_obligations")
    for previous, current in zip(profiles, profiles[1:]):
        for field in fields:
            missing = set(previous.get(field) or []) - set(current.get(field) or [])
            if missing:
                issues.append(f"{path}: {current.get('id')!r} weakens {field}: {sorted(missing)}")
        previous_execution = (previous.get("autonomy") or {}).get("execution", -1)
        current_execution = (current.get("autonomy") or {}).get("execution", 2)
        if current_execution > previous_execution:
            issues.append(f"{path}: execution autonomy must not increase with materiality")
    return issues


def check_engineering_workflow_profiles(agent_root, relative):
    path = agent_root / relative
    data, err = load_yaml(path)
    if err:
        return [f"{path}: invalid YAML ({err})"]
    issues = mapping_issues(data, path, "workflow profile registry")
    if issues:
        return issues
    allowed = {
        "api_version", "kind", "status", "runtime_binding", "owner",
        "materiality_levels", "gate_catalog", "risk_floor", "profiles",
    }
    issues.extend(unknown_field_issues(data, allowed, path))
    issues.extend(header_issues(data, path, "GovernanceProfileRegistry"))
    if set(data.get("materiality_levels") or []) != MATERIALITY:
        issues.append(f"{path}: materiality_levels vocabulary is invalid")
    profiles = data.get("profiles")
    id_issues, ids = unique_id_issues(profiles, path, "profile")
    issues.extend(id_issues)
    if ids != set(PROFILE_IDS):
        issues.append(f"{path}: profiles must be exactly {PROFILE_IDS}")
    by_id = {p.get("id"): p for p in profiles if isinstance(p, dict)} if isinstance(profiles, list) else {}
    ordered = [by_id[name] for name in PROFILE_IDS if name in by_id]
    if [item.get("rank") for item in ordered] != list(range(len(PROFILE_IDS))):
        issues.append(f"{path}: profile ranks must be 0..{len(PROFILE_IDS) - 1}")
    workflows = _known_workflows(agent_root)
    agents = {item.stem for item in (agent_root / "agents").glob("*.md")}
    gates = _known_gates(agent_root)
    if set(data.get("gate_catalog") or []) != gates:
        issues.append(f"{path}: gate_catalog must reuse policies/modes.yml exactly")
    profile_fields = {
        "id", "rank", "required_workflows", "required_gates", "required_reviewers",
        "proof_obligations", "human_gate_required", "autonomy", "stop_conditions",
    }
    for profile in ordered:
        issues.extend(unknown_field_issues(profile, profile_fields, f"{path}: profile {profile.get('id')!r}"))
        issues.extend(_profile_issues(profile, path, workflows, agents, gates))
    risk_floor = data.get("risk_floor")
    if risk_floor != RISK_FLOOR:
        issues.append(f"{path}: risk_floor must remain the canonical L0-W0/L1-W1/L2-W2/L3+-W3 mapping")
    issues.extend(_monotonic_profile_issues(ordered, path))
    return issues
