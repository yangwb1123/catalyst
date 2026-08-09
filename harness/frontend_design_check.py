#!/usr/bin/env python3
"""Validate the shadow Frontend Design policy and optional package."""
import hashlib
import re
import sys
from pathlib import Path

from engineering_check_support import (
    header_issues,
    load_yaml,
    mapping_issues,
    unique_id_issues,
    unknown_field_issues,
)
from frontend_design.contract import (
    ASSUMPTION_BLOCK_THRESHOLD,
    CLASSIFICATION_FIELDS,
    DIMENSION_OWNERS,
    DIMENSION_PROOF_TYPES,
    EXECUTION_PROOF_TYPES,
    FORBIDDEN_KEYS,
    MAX_ARTIFACT_BYTES,
    PAGE_PATTERN_IDS,
    POLICY_REF,
    POLICY_SHA256,
    PROFILE_IDS,
    PROFILE_DEFAULTS,
    PROFILE_REF,
    PROFILE_SHA256,
    READINESS_DECISION_DEPENDENCIES,
    READINESS_DIMENSIONS,
    READINESS_PROOF_TYPES,
    RISK_FLOORS,
    REVIEW_PROOF_TYPES,
    SCHEMA_REF,
    SCHEMA_SHA256,
    SEQUENCE,
    STANDARD_REF,
    TRIGGER_FLOORS,
    TRIGGERS,
)
from frontend_design.package import PACKAGE_FIELDS, classify_frontend_package, validate_frontend_package

SKILL_REFS = {
    "information-interaction-design", "design-system-accessibility",
    "frontend-client-engineering",
}
SKILL_SECTIONS = (
    "职责与触发", "输入契约", "执行 SOP", "输出契约",
    "规则、禁止与权限", "自动化与验收", "直接参考",
)
POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "applicability", "decision_sequence", "rule_layers",
    "classification", "assumption_control", "dimension_statuses", "dimensions",
    "flow_contract", "verification", "readiness", "evidence_contract",
    "invariants", "primary_sources",
}
PROFILE_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "selection", "density_levels", "motion_levels", "platforms", "profiles",
    "page_patterns", "token_policy", "accessibility_policy", "verification_policy",
}
SCHEMA_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "artifact",
    "fields", "records", "semantic_rules",
}
SEMANTIC_RULE_IDS = {
    "exact_dimension_coverage", "no_completion_authority", "classification_provenance",
    "profile_and_pattern_catalog", "triggered_dimensions_cannot_be_na",
    "flow_path_completeness", "state_action_integrity", "high_risk_action_controls",
    "proof_claim_artifact_resolution", "evidence_class_separation",
    "verification_context_binding", "capture_reuse_prohibited",
    "screenshot_png_integrity",
    "visual_cannot_override_behavior", "independent_review",
    "readiness_dependency", "n_a_requires_evidence", "digest_is_not_identity",
}


def _shape(value, fields, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, fields, label)
    missing = fields - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _strings(value, label, *, non_empty=False, unique=False):
    if not isinstance(value, list) or (non_empty and not value):
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list"]
    issues = []
    if not all(isinstance(item, str) and item.strip() for item in value):
        issues.append(f"{label}: values must be non-empty strings")
    if unique and all(isinstance(item, str) for item in value) and len(value) != len(set(value)):
        issues.append(f"{label}: values must be unique")
    return issues


def _raw_digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _skill_issues(repo_root):
    issues = []
    for skill in sorted(SKILL_REFS):
        path = repo_root / ".agent" / "skills" / f"{skill}.md"
        if not path.is_file():
            issues.append(f"{path}: required frontend Skill adapter missing")
            continue
        text = path.read_text(encoding="utf-8")
        for section in SKILL_SECTIONS:
            if not re.search(rf"^##\s+.*{re.escape(section)}", text, re.MULTILINE):
                issues.append(f"{path}: missing required section {section!r}")
    standard = repo_root / STANDARD_REF
    if not standard.is_file():
        issues.append(f"{standard}: frontend design reference missing")
    return issues


def _applicability_issues(data, label):
    record = data.get("applicability")
    fields = {"activation", "workflow_floor", "minimum_materiality", "triggers",
              "trigger_floors", "skip_requires", "unknown_load_bearing_decision"}
    issues = _shape(record, fields, f"{label}: applicability")
    if not isinstance(record, dict):
        return issues
    expected = {
        "activation": "any_trigger", "workflow_floor": "W1_standard",
        "minimum_materiality": "L1",
        "skip_requires": ["reason", "source_refs", "independent_reviewer"],
        "unknown_load_bearing_decision": "block",
    }
    for field, value in expected.items():
        if record.get(field) != value:
            issues.append(f"{label}: applicability.{field} weakens the frontend floor")
    values = record.get("triggers")
    if not isinstance(values, list) or not all(isinstance(item, str) for item in values) or set(values) != TRIGGERS:
        issues.append(f"{label}: applicability triggers must be the canonical set")
    floors = {key: {"materiality": value[0], "workflow": value[1]}
              for key, value in TRIGGER_FLOORS.items()}
    if record.get("trigger_floors") != floors:
        issues.append(f"{label}: trigger floors must be the canonical risk mapping")
    return issues


def _dimension_issues(data, label):
    records = data.get("dimensions")
    id_issues, ids = unique_id_issues(records, label, "frontend dimension")
    issues = list(id_issues)
    if ids != set(DIMENSION_OWNERS):
        issues.append(f"{label}: dimensions must be exactly {sorted(DIMENSION_OWNERS)}")
    for record in records if isinstance(records, list) else []:
        if not isinstance(record, dict):
            continue
        dimension = record.get("id")
        item_label = f"{label}: dimension {dimension!r}"
        issues.extend(_shape(record, {"id", "owner_skill", "questions", "proof_types"}, item_label))
        if record.get("owner_skill") != DIMENSION_OWNERS.get(dimension):
            issues.append(f"{item_label}: wrong owner Skill")
        issues.extend(_strings(record.get("questions"), f"{item_label}.questions", non_empty=True))
        issues.extend(_strings(record.get("proof_types"), f"{item_label}.proof_types",
                               non_empty=True, unique=True))
        proof_types = record.get("proof_types")
        if not isinstance(proof_types, list) or not all(isinstance(item, str) for item in proof_types) \
                or set(proof_types) != DIMENSION_PROOF_TYPES.get(dimension, set()):
            issues.append(f"{item_label}: proof types changed")
    return issues


def _readiness_issues(data, label):
    record = data.get("readiness")
    fields = {"dimensions", "allowed_results", "proof_types", "decision_dependencies",
              "not_applicable_requires", "final_decision_owner"}
    issues = _shape(record, fields, f"{label}: readiness")
    if not isinstance(record, dict):
        return issues
    if record.get("dimensions") != READINESS_DIMENSIONS:
        issues.append(f"{label}: readiness dimensions changed")
    if record.get("allowed_results") != ["ready", "not_ready", "not_applicable"]:
        issues.append(f"{label}: readiness results changed")
    proofs = record.get("proof_types")
    normalized = {key: set(value) for key, value in proofs.items()
                  if isinstance(key, str) and isinstance(value, list)
                  and all(isinstance(item, str) for item in value)} if isinstance(proofs, dict) else {}
    if normalized != READINESS_PROOF_TYPES:
        issues.append(f"{label}: readiness proof types changed")
    dependencies = record.get("decision_dependencies")
    normalized = {key: set(value) for key, value in dependencies.items()
                  if isinstance(key, str) and isinstance(value, list)
                  and all(isinstance(item, str) for item in value)} if isinstance(dependencies, dict) else {}
    if normalized != READINESS_DECISION_DEPENDENCIES:
        issues.append(f"{label}: readiness dependencies changed")
    if record.get("not_applicable_requires") != ["reason", "proof_refs"]:
        issues.append(f"{label}: readiness N/A floor changed")
    if record.get("final_decision_owner") != "forge_accept":
        issues.append(f"{label}: readiness cannot self-authorize")
    return issues


def _source_issues(data, label):
    records, issues = data.get("primary_sources"), []
    id_issues, _ = unique_id_issues(records, label, "primary source")
    issues.extend(id_issues)
    if not isinstance(records, list) or not records:
        return issues + [f"{label}: primary_sources must be non-empty list"]
    for record in records:
        if not isinstance(record, dict) or set(record) != {"id", "url"}:
            issues.append(f"{label}: primary source requires exactly id/url")
        elif not isinstance(record.get("url"), str) or not record["url"].startswith("https://"):
            issues.append(f"{label}: primary source URL must use https")
    return issues


def validate_frontend_policy(data, repo_root, label=POLICY_REF):
    issues = mapping_issues(data, label, "frontend design policy")
    if issues:
        return issues
    issues.extend(unknown_field_issues(data, POLICY_FIELDS, label))
    issues.extend(header_issues(data, label, "FrontendDesignPolicy"))
    expected = {"runtime_binding": "pre_code_shadow_review_only",
                "owner": "product-design-engineering", "version": 1,
                "completion_authority": "forge_accept", "decision_sequence": SEQUENCE,
                "dimension_statuses": ["addressed", "not_applicable", "blocked"]}
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{label}: {field} changed")
    issues.extend(_applicability_issues(data, label))
    classification = data.get("classification")
    if not isinstance(classification, dict) or classification.get("required_fields") != CLASSIFICATION_FIELDS:
        issues.append(f"{label}: classification fields changed")
    expected_risk_floors = {key: {"materiality": value[0], "workflow": value[1]}
                            for key, value in RISK_FLOORS.items()}
    if not isinstance(classification, dict) or classification.get("risk_floors") != expected_risk_floors:
        issues.append(f"{label}: classification risk floors changed")
    control = data.get("assumption_control")
    if not isinstance(control, dict) or control.get("load_bearing_threshold") != ASSUMPTION_BLOCK_THRESHOLD:
        issues.append(f"{label}: assumption blocking threshold changed")
    issues.extend(_dimension_issues(data, label))
    issues.extend(_readiness_issues(data, label))
    evidence = data.get("evidence_contract")
    if not isinstance(evidence, dict) or evidence.get("max_artifact_bytes") != MAX_ARTIFACT_BYTES \
            or evidence.get("result_classification") != "structurally_valid_only" \
            or evidence.get("provenance_trust") != "declarative_only":
        issues.append(f"{label}: evidence honesty contract changed")
    issues.extend(_source_issues(data, label))
    issues.extend(_skill_issues(repo_root))
    return issues


def validate_frontend_profiles(data, label=PROFILE_REF):
    issues = mapping_issues(data, label, "frontend profile catalog")
    if issues:
        return issues
    issues.extend(unknown_field_issues(data, PROFILE_FIELDS, label))
    issues.extend(header_issues(data, label, "FrontendProfileCatalog"))
    expected = {"runtime_binding": "selection_policy_only",
                "owner": "product-design-engineering", "version": 1}
    for field, value in expected.items():
        if data.get(field) != value:
            issues.append(f"{label}: {field} changed")
    profile_issues, profile_ids = unique_id_issues(data.get("profiles"), label, "profile")
    pattern_issues, pattern_ids = unique_id_issues(data.get("page_patterns"), label, "page pattern")
    issues.extend(profile_issues + pattern_issues)
    if profile_ids != PROFILE_IDS:
        issues.append(f"{label}: profile ids changed")
    if pattern_ids != PAGE_PATTERN_IDS:
        issues.append(f"{label}: page pattern ids changed")
    selection = data.get("selection")
    if not isinstance(selection, dict) or selection.get("standards_precedence") != "absolute" \
            or selection.get("unknown_profile") != "block":
        issues.append(f"{label}: profile precedence or unknown behavior changed")
    expected_override = ["field", "default", "selected", "reason", "scope", "risk",
                         "compensating_proof_claim_ids", "reviewer_id", "expires_at"]
    if not isinstance(selection, dict) or selection.get("heuristic_override_requires") != expected_override:
        issues.append(f"{label}: profile override controls changed")
    defaults = {item.get("id"): (item.get("density"), item.get("motion_level"))
                for item in data.get("profiles", []) if isinstance(item, dict)
                and isinstance(item.get("id"), str)} \
        if isinstance(data.get("profiles"), list) else {}
    if defaults != PROFILE_DEFAULTS:
        issues.append(f"{label}: profile density/motion defaults changed")
    accessibility = data.get("accessibility_policy")
    if not isinstance(accessibility, dict) or accessibility.get("fixed_minimum_font_size") != "none" \
            or accessibility.get("automated_scan_claim") != "partial_only":
        issues.append(f"{label}: accessibility truth boundary changed")
    return issues


def validate_frontend_schema(data, label=SCHEMA_REF):
    issues = mapping_issues(data, label, "frontend package schema")
    if issues:
        return issues
    issues.extend(unknown_field_issues(data, SCHEMA_FIELDS, label))
    issues.extend(header_issues(data, label, "FrontendDesignPackageContract"))
    artifact = data.get("artifact")
    if not isinstance(artifact, dict):
        return issues + [f"{label}: artifact must be mapping"]
    required = artifact.get("required_fields")
    required = set(required) if isinstance(required, list) \
        and all(isinstance(item, str) for item in required) else None
    forbidden = artifact.get("forbidden_fields")
    forbidden = set(forbidden) if isinstance(forbidden, list) \
        and all(isinstance(item, str) for item in forbidden) else None
    if required != PACKAGE_FIELDS:
        issues.append(f"{label}: artifact required fields changed")
    if forbidden != FORBIDDEN_KEYS:
        issues.append(f"{label}: forbidden completion fields changed")
    if artifact.get("result_classification") != "structurally_valid_only":
        issues.append(f"{label}: result classification overclaims authority")
    rules = data.get("semantic_rules")
    id_issues, ids = unique_id_issues(rules, label, "semantic rule")
    issues.extend(id_issues)
    if ids != SEMANTIC_RULE_IDS:
        issues.append(f"{label}: semantic rules must be exactly the canonical set")
    return issues


def _load_contract(path, validator, repo_root=None):
    data, error = load_yaml(path)
    if error:
        return [f"{path}: invalid YAML ({error})"]
    try:
        return validator(data, repo_root, str(path)) if repo_root else validator(data, str(path))
    except (RecursionError, TypeError, ValueError, OverflowError) as exc:
        return [f"{path}: malformed contract value ({type(exc).__name__})"]


def check_frontend_design_contract(repo_root):
    paths = {"policy": repo_root / POLICY_REF, "profile": repo_root / PROFILE_REF,
             "schema": repo_root / SCHEMA_REF}
    issues = _load_contract(paths["policy"], validate_frontend_policy, repo_root)
    issues.extend(_load_contract(paths["profile"], validate_frontend_profiles))
    issues.extend(_load_contract(paths["schema"], validate_frontend_schema))
    for name, expected in (("policy", POLICY_SHA256), ("profile", PROFILE_SHA256),
                           ("schema", SCHEMA_SHA256)):
        try:
            actual = _raw_digest(paths[name])
        except OSError as exc:
            issues.append(f"{paths[name]}: cannot read canonical bytes ({exc})")
            continue
        if actual != expected:
            issues.append(f"{paths[name]}: canonical {name} bytes changed without a v1 governance update")
    return issues


def main(argv):
    repo_root = Path(argv[1] if len(argv) > 1 else ".").resolve()
    issues = check_frontend_design_contract(repo_root)
    package = None
    if len(argv) > 2 and not issues:
        path = Path(argv[2])
        try:
            package_size = path.stat().st_size
        except OSError as exc:
            issues.append(f"{path}: cannot read package ({exc})")
            package_size = 0
        if package_size > 393216:
            issues.append(f"{path}: package exceeds 393216 bytes")
        elif not issues:
            package, error = load_yaml(path)
            if error:
                issues.append(f"{path}: invalid YAML ({error})")
            else:
                issues.extend(validate_frontend_package(package, repo_root))
    if issues:
        print(f"frontend-design-check: INVALID - {len(issues)} issue(s):")
        for issue in issues:
            print(f"  {issue}")
        return 1
    classification = classify_frontend_package(package, issues) if package else "STRUCTURALLY_VALID"
    print(f"frontend-design-check: {classification} (shadow; no completion authority)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
