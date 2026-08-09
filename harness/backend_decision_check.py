#!/usr/bin/env python3
"""Validate Backend Engineering's shadow policy and decision packages.

Contract validity is not design correctness, approval, or completion authority.
"""
import hashlib
import re
import sys
from pathlib import Path

from backend_decision_contract import (
    DIMENSION_PROOF_TYPES,
    DIMENSION_OWNERS,
    EVIDENCE_CLASS_PRODUCERS,
    EVIDENCE_CLASS_PROOF_TYPES,
    EVIDENCE_CLASS_RESULTS,
    FORBIDDEN_DECISION_KEYS,
    IRREVERSIBLE_KINDS,
    MODEL_ROLE_CONDITIONS,
    MAX_EVIDENCE_BYTES,
    POLICY_REF,
    POLICY_SHA256,
    READINESS_DIMENSIONS,
    READINESS_DECISION_DEPENDENCIES,
    READINESS_PROOF_TYPES,
    SCHEMA_REF,
    SCHEMA_SHA256,
    SEQUENCE,
    STANDARD_REF,
    TOOL_PROOF_TYPES,
    TRIGGER_FLOORS,
    TRIGGER_IRREVERSIBLE_BINDINGS,
    TRIGGERS,
)
from backend_package_check import PACKAGE_FIELDS, classify_backend_package, validate_backend_package
from engineering_check_support import (
    header_issues,
    load_yaml,
    mapping_issues,
    repo_path_issue,
    unique_id_issues,
    unknown_field_issues,
)


SKILL_REFS = {
    "backend-engineering", "domain-modeling", "data-modeling-transactions",
    "architecture-tradeoff", "distributed-reliability-design",
    "performance-capacity", "data-migration-lifecycle", "api-contract-design",
    "observability-engineering", "secure-coding",
}
SKILL_SECTIONS = (
    "职责与触发", "输入契约", "执行 SOP", "输出契约",
    "规则、禁止与权限", "自动化与验收", "直接参考",
)
POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "applicability", "decision_sequence",
    "model_boundaries", "dimension_statuses", "dimensions",
    "irreversible_decision_controls", "production_readiness", "invariants",
    "evidence_contract", "primary_sources",
}
SCHEMA_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "artifact",
    "fields", "records", "semantic_rules",
}


def _strings(value, label, *, non_empty=False, unique=False):
    if not isinstance(value, list) or (non_empty and not value):
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list"]
    issues = []
    if not all(isinstance(item, str) and item.strip() for item in value):
        issues.append(f"{label}: values must be non-empty strings")
    if unique and all(isinstance(item, str) for item in value) and len(value) != len(set(value)):
        issues.append(f"{label}: values must be unique")
    return issues


def _shape(value, required, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, required, label)
    missing = set(required) - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _skill_issues(repo_root):
    issues = []
    for skill in sorted(SKILL_REFS):
        path = repo_root / ".agent" / "skills" / f"{skill}.md"
        if not path.is_file():
            issues.append(f"{path}: required backend Skill adapter missing")
            continue
        text = path.read_text(encoding="utf-8")
        for section in SKILL_SECTIONS:
            if not re.search(rf"^##\s+.*{re.escape(section)}", text, re.MULTILINE):
                issues.append(f"{path}: missing required section {section!r}")
    standard = repo_root / STANDARD_REF
    if not standard.is_file():
        issues.append(f"{standard}: backend decision reference missing")
    return issues


def _applicability_issues(data, label):
    applicability = data.get("applicability")
    fields = {"activation", "workflow_floor", "minimum_materiality", "triggers",
              "trigger_floors", "skip_requires", "unknown_load_bearing_decision"}
    issues = _shape(applicability, fields, f"{label}: applicability")
    if not isinstance(applicability, dict):
        return issues
    expected = {
        "activation": "any_trigger", "workflow_floor": "W1_standard",
        "minimum_materiality": "L1", "skip_requires": ["reason", "source_refs", "reviewer"],
        "unknown_load_bearing_decision": "block",
    }
    for field, value in expected.items():
        if applicability.get(field) != value:
            issues.append(f"{label}: applicability.{field} weakens the backend floor")
    triggers = applicability.get("triggers")
    if not isinstance(triggers, list) or not all(isinstance(item, str) for item in triggers) or set(triggers) != TRIGGERS:
        issues.append(f"{label}: applicability triggers must be the canonical set")
    floors = {key: {"materiality": value[0], "workflow": value[1]}
              for key, value in TRIGGER_FLOORS.items()}
    if applicability.get("trigger_floors") != floors:
        issues.append(f"{label}: trigger floors must be the canonical risk mapping")
    return issues


def _boundary_issues(data, label):
    boundaries = data.get("model_boundaries")
    fields = {"semantic_roles", "default", "role_activation",
              "direct_orm_exposure_to_public_contract", "reuse_exception_requires"}
    issues = _shape(boundaries, fields, f"{label}: model_boundaries")
    if not isinstance(boundaries, dict):
        return issues
    roles = boundaries.get("semantic_roles")
    if not isinstance(roles, list) or not all(isinstance(item, str) for item in roles) or set(roles) != set(MODEL_ROLE_CONDITIONS):
        issues.append(f"{label}: model semantic roles are incomplete")
    if boundaries.get("default") != "separate_when_owner_or_change_reason_differs":
        issues.append(f"{label}: model separation default was weakened")
    if boundaries.get("role_activation") != MODEL_ROLE_CONDITIONS:
        issues.append(f"{label}: model role activation conditions changed")
    if boundaries.get("direct_orm_exposure_to_public_contract") != "prohibited":
        issues.append(f"{label}: direct ORM exposure must remain prohibited")
    reuse = {"identical_owner", "identical_change_reason", "identical_security_classification",
             "no_public_or_persistence_coupling", "reviewer_rationale"}
    requirements = boundaries.get("reuse_exception_requires")
    if not isinstance(requirements, list) or not all(isinstance(item, str) for item in requirements) or set(requirements) != reuse:
        issues.append(f"{label}: model reuse exception controls are incomplete")
    return issues


def _dimension_issues(data, label):
    dimensions = data.get("dimensions")
    id_issues, ids = unique_id_issues(dimensions, label, "backend dimension")
    issues = list(id_issues)
    if ids != set(DIMENSION_OWNERS):
        issues.append(f"{label}: decision dimensions must be exactly {sorted(DIMENSION_OWNERS)}")
    for item in dimensions if isinstance(dimensions, list) else []:
        if not isinstance(item, dict):
            continue
        dim_id = item.get("id")
        issues.extend(_shape(item, {"id", "owner_skill", "questions", "proof_types"},
                             f"{label}: dimension {dim_id!r}"))
        if not isinstance(dim_id, str) or dim_id not in DIMENSION_OWNERS:
            issues.append(f"{label}: dimension id must be a canonical string")
            expected_owner, expected_proofs = None, set()
        else:
            expected_owner = DIMENSION_OWNERS[dim_id]
            expected_proofs = DIMENSION_PROOF_TYPES[dim_id]
        if item.get("owner_skill") != expected_owner:
            issues.append(f"{label}: dimension {dim_id!r} has wrong owner Skill")
        issues.extend(_strings(item.get("questions"), f"{label}: dimension {dim_id!r} questions",
                               non_empty=True))
        issues.extend(_strings(item.get("proof_types"),
                               f"{label}: dimension {dim_id!r} proof_types",
                               non_empty=True, unique=True))
        proof_types = item.get("proof_types")
        if not isinstance(proof_types, list) or not all(isinstance(value, str) for value in proof_types) \
                or set(proof_types) != expected_proofs:
            issues.append(f"{label}: dimension {dim_id!r} proof types changed")
    return issues


def _readiness_policy_issues(data, label):
    readiness = data.get("production_readiness")
    fields = {"dimensions", "allowed_results", "proof_types", "decision_dependencies",
              "not_applicable_requires", "final_decision_owner"}
    issues = _shape(readiness, fields, f"{label}: production_readiness")
    if isinstance(readiness, dict):
        if readiness.get("dimensions") != READINESS_DIMENSIONS:
            issues.append(f"{label}: production readiness dimensions changed")
        if readiness.get("allowed_results") != ["ready", "not_ready", "not_applicable"]:
            issues.append(f"{label}: readiness result vocabulary changed")
        expected_proofs = {key: sorted(value) for key, value in READINESS_PROOF_TYPES.items()}
        actual_proofs = readiness.get("proof_types")
        normalized = {key: sorted(value) for key, value in actual_proofs.items()
                      if isinstance(key, str) and isinstance(value, list)
                      and all(isinstance(item, str) for item in value)} if isinstance(actual_proofs, dict) else {}
        if normalized != expected_proofs:
            issues.append(f"{label}: readiness proof types changed")
        expected_dependencies = {key: sorted(value) for key, value in READINESS_DECISION_DEPENDENCIES.items()}
        dependencies = readiness.get("decision_dependencies")
        normalized_dependencies = {key: sorted(value) for key, value in dependencies.items()
                                   if isinstance(key, str) and isinstance(value, list)
                                   and all(isinstance(item, str) for item in value)} \
            if isinstance(dependencies, dict) else {}
        if normalized_dependencies != expected_dependencies:
            issues.append(f"{label}: readiness decision dependencies changed")
        if readiness.get("not_applicable_requires") != ["reason", "evidence_refs"]:
            issues.append(f"{label}: readiness N/A evidence floor changed")
        if readiness.get("final_decision_owner") != "forge_accept":
            issues.append(f"{label}: readiness cannot self-authorize")
    return issues


def _irreversible_policy_issues(data, label):
    irreversible = data.get("irreversible_decision_controls")
    issues = _shape(irreversible, {"applies_to", "package_fields", "trigger_bindings", "requires"},
                    f"{label}: irreversible_decision_controls")
    if isinstance(irreversible, dict):
        applies = irreversible.get("applies_to")
        if not isinstance(applies, list) or not all(isinstance(item, str) for item in applies) or set(applies) != IRREVERSIBLE_KINDS:
            issues.append(f"{label}: irreversible decision kinds changed")
        package_fields = ["decision_kinds", "reversibility", "migration_cost", "blast_radius",
                          "adr_ref", "reviewer_id", "revisit_trigger"]
        if irreversible.get("package_fields") != package_fields:
            issues.append(f"{label}: irreversible package fields changed")
        bindings = {key: {"dimension": value[0], "decision_kind": value[1]}
                    for key, value in TRIGGER_IRREVERSIBLE_BINDINGS.items()}
        if irreversible.get("trigger_bindings") != bindings:
            issues.append(f"{label}: irreversible trigger bindings changed")
        required = {"two_alternatives", "migration_cost", "blast_radius", "adr_ref",
                    "independent_reviewer", "revisit_trigger"}
        required_values = irreversible.get("requires")
        if not isinstance(required_values, list) or not all(isinstance(item, str) for item in required_values) \
                or set(required_values) != required:
            issues.append(f"{label}: irreversible decision controls changed")
    return issues


def _source_issues(data, label):
    sources, issues = data.get("primary_sources"), []
    source_issues, _ = unique_id_issues(sources, label, "primary source")
    issues.extend(source_issues)
    if not isinstance(sources, list) or not sources:
        issues.append(f"{label}: primary sources cannot be empty")
    for item in sources if isinstance(sources, list) else []:
        if not isinstance(item, dict) or set(item) != {"id", "url"}:
            issues.append(f"{label}: primary source requires exactly id/url")
            continue
        source_id, url = item["id"], item["url"]
        if not isinstance(source_id, str) or not source_id.strip():
            issues.append(f"{label}: primary source id must be a non-empty string")
        if not isinstance(url, str) or not url.startswith("https://"):
            issues.append(f"{label}: primary source URL must be an https string")
    return issues


def _evidence_contract_issues(data, label):
    contract = data.get("evidence_contract")
    fields = {"max_file_bytes", "subject_binding", "cross_class_locator_reuse", "identity_trust", "classes"}
    issues = _shape(contract, fields, f"{label}: evidence_contract")
    if not isinstance(contract, dict):
        return issues
    expected = {"max_file_bytes": MAX_EVIDENCE_BYTES, "subject_binding": "exact",
                "cross_class_locator_reuse": "prohibited",
                "identity_trust": "digest_bound_declaration_only"}
    for field, value in expected.items():
        if contract.get(field) != value:
            issues.append(f"{label}: evidence_contract.{field} changed")
    classes = contract.get("classes")
    if not isinstance(classes, dict) or set(classes) != set(EVIDENCE_CLASS_PROOF_TYPES):
        return issues + [f"{label}: evidence classes changed"]
    for class_name, record in classes.items():
        if not isinstance(record, dict) or set(record) != {"producers", "allowed_results", "proof_types"}:
            issues.append(f"{label}: evidence class {class_name!r} has invalid shape")
            continue
        expected_sets = (EVIDENCE_CLASS_PRODUCERS[class_name], EVIDENCE_CLASS_RESULTS[class_name],
                         EVIDENCE_CLASS_PROOF_TYPES[class_name])
        for field, values in zip(("producers", "allowed_results", "proof_types"), expected_sets):
            actual = record.get(field)
            if not isinstance(actual, list) or not all(isinstance(item, str) for item in actual) \
                    or set(actual) != values:
                issues.append(f"{label}: evidence class {class_name!r} {field} changed")
    return issues


def validate_backend_policy(data, repo_root, label=POLICY_REF):
    issues = mapping_issues(data, label, "backend decision policy")
    if issues:
        return issues
    issues.extend(unknown_field_issues(data, POLICY_FIELDS, label))
    issues.extend(header_issues(data, label, "BackendDecisionPolicy"))
    expected = {
        "runtime_binding": "pre_code_shadow_review_only", "owner": "architecture",
        "version": 1, "completion_authority": "forge_accept", "decision_sequence": SEQUENCE,
        "dimension_statuses": ["addressed", "not_applicable", "blocked"],
    }
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{label}: {field} must remain {value!r}")
    for check in (_applicability_issues, _boundary_issues, _dimension_issues,
                  _readiness_policy_issues, _irreversible_policy_issues,
                  _evidence_contract_issues, _source_issues):
        issues.extend(check(data, label))
    invariants = data.get("invariants")
    invariant_issues, _ = unique_id_issues(invariants, label, "invariant")
    issues.extend(invariant_issues)
    if not isinstance(invariants, list) or not invariants:
        issues.append(f"{label}: invariants cannot be empty")
    for item in invariants if isinstance(invariants, list) else []:
        if not isinstance(item, dict) or set(item) != {"id", "rule"}:
            issues.append(f"{label}: invariant requires exactly id/rule")
        elif not all(isinstance(item[field], str) and item[field].strip() for field in ("id", "rule")):
            issues.append(f"{label}: invariant id/rule must be non-empty strings")
    issues.extend(_skill_issues(repo_root))
    return issues


def validate_backend_schema(data, label=SCHEMA_REF):
    issues = mapping_issues(data, label, "backend package schema")
    if issues:
        return issues
    issues.extend(unknown_field_issues(data, SCHEMA_FIELDS, label))
    issues.extend(header_issues(data, label, "BackendDecisionPackageContract"))
    if data.get("runtime_binding") != "artifact_validation_only" or data.get("owner") != "architecture":
        issues.append(f"{label}: schema must remain architecture-owned artifact validation only")
    artifact = data.get("artifact")
    fields = {"name", "version", "max_bytes", "evidence_file_max_bytes",
              "required_fields", "forbidden_fields"}
    issues.extend(_shape(artifact, fields, f"{label}: artifact"))
    if isinstance(artifact, dict):
        identity = (artifact.get("name"), artifact.get("version"), artifact.get("max_bytes"),
                    artifact.get("evidence_file_max_bytes"))
        if identity != ("BackendDecisionPackage", 1, 262144, MAX_EVIDENCE_BYTES):
            issues.append(f"{label}: artifact identity or size ceiling changed")
        required_fields = artifact.get("required_fields")
        if not isinstance(required_fields, list) or not all(isinstance(item, str) for item in required_fields) \
                or set(required_fields) != PACKAGE_FIELDS:
            issues.append(f"{label}: artifact required fields are incomplete")
        forbidden_fields = artifact.get("forbidden_fields")
        if not isinstance(forbidden_fields, list) or not all(isinstance(item, str) for item in forbidden_fields) \
                or set(forbidden_fields) != FORBIDDEN_DECISION_KEYS:
            issues.append(f"{label}: completion-authority forbidden fields changed")
    if not isinstance(data.get("fields"), dict) or set(data["fields"]) != PACKAGE_FIELDS:
        issues.append(f"{label}: field contracts must exactly match required fields")
    records = {"principal_record", "applicability_record", "review_record", "decision_record", "sourced_claim",
               "assumption_record", "evidence_record", "risk_record", "readiness_record"}
    if not isinstance(data.get("records"), dict) or set(data["records"]) != records:
        issues.append(f"{label}: record contracts are incomplete")
    expected_rules = {
        "exact_dimension_coverage", "addressed_requires_verified_evidence",
        "not_applicable_is_evidenced", "blocked_is_honest", "evidence_is_resolved",
        "proof_type_matches_subject",
        "assumptions_are_not_facts", "risky_assumptions_block", "trigger_floors_apply",
        "low_reversibility_is_controlled", "reviewer_is_independent", "readiness_is_not_completion",
        "readiness_depends_on_decisions",
        "policy_and_schema_are_pinned", "critical_risk_is_not_ready", "no_completion_authority",
    }
    rule_issues, rule_ids = unique_id_issues(data.get("semantic_rules"), label, "semantic rule")
    issues.extend(rule_issues)
    if rule_ids != expected_rules:
        issues.append(f"{label}: semantic rules must be exactly {sorted(expected_rules)}")
    return issues


def check_backend_decision_contract(repo_root):
    policy_path, schema_path = repo_root / POLICY_REF, repo_root / SCHEMA_REF
    issues = []
    for path, kind, expected in ((policy_path, "policy", POLICY_SHA256),
                                 (schema_path, "schema", SCHEMA_SHA256)):
        issue = repo_path_issue(repo_root, str(path.relative_to(repo_root)), f"backend {kind}")
        if issue:
            issues.append(issue)
        elif expected != hashlib.sha256(path.read_bytes()).hexdigest():
            issues.append(f"{path}: canonical {kind} bytes changed without a v1 governance update")
    policy, error = load_yaml(policy_path)
    if error:
        issues.append(f"{policy_path}: invalid YAML ({error})")
    else:
        issues.extend(validate_backend_policy(policy, repo_root, str(policy_path)))
    schema, error = load_yaml(schema_path)
    if error:
        issues.append(f"{schema_path}: invalid YAML ({error})")
    else:
        issues.extend(validate_backend_schema(schema, str(schema_path)))
    return issues


def _run(argv):
    repo_root = Path(argv[1] if len(argv) > 1 else ".").resolve()
    issues = check_backend_decision_contract(repo_root)
    classification = None
    if len(argv) > 2 and not issues:
        package_path = Path(argv[2])
        if not package_path.is_absolute():
            package_path = repo_root / package_path
        try:
            size = package_path.stat().st_size
        except OSError as exc:
            issues.append(f"{package_path}: cannot read package ({exc})")
        else:
            if size > 262144:
                issues.append(f"{package_path}: package exceeds 262144 bytes")
            package, error = load_yaml(package_path)
            if error:
                issues.append(f"{package_path}: invalid YAML ({error})")
            else:
                package_issues = validate_backend_package(package, repo_root, str(package_path))
                issues.extend(package_issues)
                if not package_issues:
                    classification = classify_backend_package(package)
    if not issues:
        print(f"backend-decision-check: {classification or 'PASS'}")
        return 0
    print(f"backend-decision-check: FAIL - {len(issues)} issue(s):")
    for issue in issues:
        print(f"  {issue}")
    return 1


def main(argv):
    try:
        return _run(argv)
    except Exception as exc:  # A validator must reject hostile structure, never expose a traceback.
        print(f"backend-decision-check: FAIL - malformed input ({type(exc).__name__})")
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
