"""Strict workflow verdict-contract validation shared by forge-check/scaffold."""

QA_VERDICT_CONTRACT = "qa_v1"
REVIEWER_VERDICT_CONTRACT = "reviewer_v1"
BOUND_REVIEWER_VERDICT_CONTRACT = "reviewer_v2"
OUTPUT_BINDING_CONTRACT = "local_digest_v1"
REVIEWER_POLICY_REF = "../policies/modes.yml#workflow_depth.reviewer"
CANONICAL_WORKFLOW_FILES = frozenset({
    "discover.yml", "design.yml", "review.yml", "build.yml",
    "deploy.yml", "rollback.yml", "evolve.yml",
})


def _loop_shape_issues(path, contract, phase):
    issues = []
    on_fail = phase.get("on_fail")
    if not isinstance(on_fail, dict) or on_fail.get("action") != "loop_back":
        issues.append(f"{path}: {contract} requires on_fail.action: loop_back")
    target = on_fail.get("target_phase") if isinstance(on_fail, dict) else None
    if not isinstance(target, str) or not target.strip():
        issues.append(f"{path}: {contract} requires non-empty on_fail.target_phase")
    return issues


def _qa_shape_issues(path, stage, phase):
    issues = []
    if stage != "build":
        issues.append(f"{path}: {QA_VERDICT_CONTRACT} is restricted to stage 'build'")
    if phase.get("agent") != "qa":
        issues.append(f"{path}: {QA_VERDICT_CONTRACT} is restricted to agent 'qa'")
    issues.extend(_loop_shape_issues(path, QA_VERDICT_CONTRACT, phase))
    if str(phase.get("required_when") or "").strip() or phase.get("optional_for"):
        issues.append(f"{path}: {QA_VERDICT_CONTRACT} phase {phase.get('name')!r} must not be mode-skippable")
    gates = phase.get("required_gates")
    if not isinstance(gates, list) or "test" not in gates:
        issues.append(f"{path}: {QA_VERDICT_CONTRACT} phase {phase.get('name')!r} requires the independent test gate")
    return issues


def _reviewer_shape_issues(path, stage, phase):
    issues = []
    if stage != "build":
        issues.append(f"{path}: {phase.get('verdict_contract')} is restricted to stage 'build'")
    if phase.get("agent") != "reviewer":
        issues.append(f"{path}: {phase.get('verdict_contract')} is restricted to agent 'reviewer'")
    contract = phase.get("verdict_contract")
    issues.extend(_loop_shape_issues(path, contract, phase))
    required_when = str(phase.get("required_when") or "").strip()
    if phase.get("optional_for") or required_when not in ("", REVIEWER_POLICY_REF):
        issues.append(f"{path}: {contract} phase {phase.get('name')!r} has an unsafe mode-skip declaration")
    if phase.get("readonly") is not True or phase.get("fresh_context") is not True:
        issues.append(f"{path}: {contract} requires readonly: true and fresh_context: true")
    if phase.get("feeds_forward") is True or phase.get("writes_adr") is not None or phase.get("emits"):
        issues.append(f"{path}: {contract} must not emit, write ADRs, or feed output forward")
    return issues


def _phase_contract_issues(path, stage, phase):
    contract = phase.get("verdict_contract")
    if contract in (None, ""):
        if stage == "build" and phase.get("agent") == "qa":
            return [f"{path}: build QA phase {phase.get('name')!r} must declare verdict_contract: {QA_VERDICT_CONTRACT}"]
        return []
    if contract == QA_VERDICT_CONTRACT:
        return _qa_shape_issues(path, stage, phase)
    if contract in (REVIEWER_VERDICT_CONTRACT, BOUND_REVIEWER_VERDICT_CONTRACT):
        return _reviewer_shape_issues(path, stage, phase)
    return [f"{path}: verdict_contract {contract!r} is unsupported"]


def _target_issues(path, contract, phases, phase_index, target):
    if not isinstance(target, str) or not target.strip():
        return []
    matches = [(index, phase) for index, phase in enumerate(phases) if phase.get("name") == target]
    if not matches:
        return [f"{path}: {contract} target {target!r} does not exist"]
    if len(matches) != 1:
        return [f"{path}: {contract} target {target!r} is ambiguous"]
    target_index, target_phase = matches[0]
    if target_index >= phase_index:
        return [f"{path}: {contract} target {target!r} must be an earlier phase"]
    if target_phase.get("agent") != "implementer":
        return [f"{path}: {contract} target {target!r} must use agent 'implementer'"]
    if target_phase.get("readonly") is True:
        return [f"{path}: {contract} target {target!r} must be writable"]
    if str(target_phase.get("required_when") or "").strip() or target_phase.get("optional_for"):
        return [f"{path}: {contract} target {target!r} must not be mode-skippable"]
    return []


def _topology_issues(path, data, phases):
    issues = []
    selector = data.get("output_binding_contract")
    bound = selector == OUTPUT_BINDING_CONTRACT
    if selector not in (None, "", OUTPUT_BINDING_CONTRACT):
        issues.append(f"{path}: output_binding_contract {selector!r} is unsupported")
    v2_reviewers = [
        (i, p) for i, p in enumerate(phases)
        if p.get("verdict_contract") == BOUND_REVIEWER_VERDICT_CONTRACT
    ]
    if v2_reviewers and not bound:
        issues.append(
            f"{path}: {BOUND_REVIEWER_VERDICT_CONTRACT} requires "
            f"output_binding_contract: {OUTPUT_BINDING_CONTRACT}"
        )
    wanted = BOUND_REVIEWER_VERDICT_CONTRACT if bound else REVIEWER_VERDICT_CONTRACT
    reviewers = [(i, p) for i, p in enumerate(phases) if p.get("verdict_contract") == wanted]
    if path.name in CANONICAL_WORKFLOW_FILES and not bound:
        issues.append(
            f"{path}: canonical workflow must declare "
            f"output_binding_contract: {OUTPUT_BINDING_CONTRACT}"
        )
    qa_indexes = [i for i, phase in enumerate(phases) if phase.get("verdict_contract") == QA_VERDICT_CONTRACT]
    if bound and data.get("stage") == "build":
        if len(v2_reviewers) != 1:
            issues.append(
                f"{path}: bound Build requires exactly one {BOUND_REVIEWER_VERDICT_CONTRACT} "
                f"phase (found {len(v2_reviewers)})"
            )
        if not qa_indexes:
            issues.append(f"{path}: bound Build requires at least one {QA_VERDICT_CONTRACT} phase")
    if qa_indexes:
        for index, phase in reviewers:
            if index >= min(qa_indexes):
                issues.append(f"{path}: {wanted} phase {phase.get('name')!r} must precede Build QA")
    for index, phase in reviewers:
        for later in phases[index + 1:]:
            if bound and later.get("readonly") is not True:
                issues.append(f"{path}: phase {later.get('name')!r} after reviewer_v2 must be readonly")
    return issues


def check_workflow_verdict_contracts(agent_root, load_yaml, collect_phases):
    """Validate qa_v1 plus legacy reviewer_v1 and bound reviewer_v2 shapes."""
    issues = []
    for path in sorted((agent_root / "workflows").glob("*.yml")):
        data, err = load_yaml(path)
        if err or not isinstance(data, dict):
            continue
        phases = list(collect_phases(data))
        issues.extend(_topology_issues(path, data, phases))
        for index, phase in enumerate(phases):
            contract = phase.get("verdict_contract")
            issues.extend(_phase_contract_issues(path, data.get("stage"), phase))
            if contract in (QA_VERDICT_CONTRACT, REVIEWER_VERDICT_CONTRACT, BOUND_REVIEWER_VERDICT_CONTRACT):
                on_fail = phase.get("on_fail")
                target = on_fail.get("target_phase") if isinstance(on_fail, dict) else None
                issues.extend(_target_issues(path, contract, phases, index, target))
    return issues
