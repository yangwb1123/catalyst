"""ADR-0055 ContextPackage and ADR-0071 portable delivery governance."""

from __future__ import annotations

import json
import hashlib
import subprocess
import sys

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file
SCHEMA_RELATIVE = "docs/contracts/context-package-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/context-package-v1.json"
CHECKER_RELATIVE = "harness/context_package_contract_check.py"
SKILL_RELATIVE = ".agent/skills/context-engineering.md"
DECISION_RELATIVE = "docs/adr/0055-shadow-context-package-v1.md"
DELIVERY_DECISION_RELATIVE = "docs/adr/ADR-0071-portable-context-engineering-skill.md"
PORTABLE_SKILL_RELATIVE = "skills/context-engineering/SKILL.md"
PACKAGE_MANIFEST_RELATIVE = "skills/context-engineering/references/package-manifest.json"
PACKAGE_CHECKER_RELATIVE = "skills/context-engineering/scripts/check_package.py"
PACKAGE_MANIFEST_SHA256 = "7590df136eb828ba3ffe4892efffa2ab4a77fb87dff8a1bffccdde2d015852c5"
DELIVERY_DECISION_SHA256 = "455f097be6c6e8e658d7a92a60d9e50b08ef89300aa13accccac4bbf67098c84"
DELIVERY_DECISION_BODY_SHA256 = "92f2a415e51fac94f3ce61203b7eb3152efb4e18a0233f91e2fc00558cf4b84d"
DELIVERY_DECISION_SELF_SHA256 = "ed72467dddb730de425278d49c8c6bdb9e6f8a82904c8fa5a8eda6ce339fd101"
RESULT = (
    "ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, "
    "completion, persistence, or effect attestation)"
)
CONTEXT_PACKAGE = {
    "api_version": "forgeos.context-package/v1",
    "request_api_version": "forgeos.context-package-build-request/v1",
    "delivery": "shipped_pure_contract_and_closed_portable_skill",
    "mode": "authority_free_deterministic_context_projection",
    "source": {
        "input": "exact_canonical_caller_supplied_build_request",
        "repository_reads": "none",
        "hidden_wall_clock": "forbidden",
        "evaluation_time": "explicit_source_binding_as_of_unix_ms",
        "source_order": "source_id_utf8_byte_order",
    },
    "selection": {
        "required": "fail_closed_and_budget_reserved_first",
        "optional_order": "category_rank_then_priority_desc_then_source_id_bytes",
        "optional_overflow": "omit_with_unique_receipt",
        "ineligible_required": "fail_closed",
        "ineligible_optional": "omit_with_unique_receipt",
        "truncation": "optional_utf8_prefix_at_source_limit_only",
    },
    "lanes": {
        "values": ["instruction_candidates", "trusted_context", "untrusted_data"],
        "delimiter": "structured_json_lane_no_text_delimiter",
        "instruction_allowed": False,
        "untrusted_instruction_or_trusted_escalation": "reject",
        "quarantined_content": "omitted_from_package",
    },
    "redaction": {
        "replacement": "[REDACTED]",
        "order": "before_eligibility_truncation_selection_budget_digest_and_token_count",
        "plan": "caller_declared_ordered_non_overlapping_utf8_byte_ranges",
        "secret_detection_completeness": "not_attested",
        "receipt_contains_preimage": False,
    },
    "token_accounting": {
        "counter": "injected_digest_pinned_identity",
        "counted_bytes": "exact_canonical_structured_projection",
        "ambient_or_character_estimate_fallback": "forbidden",
        "reference_counter": "forgeos.token-counter.utf8-bytes/v1",
    },
    "identity": {
        "request_digest_domain": "forgeos.context-package-build-request.v1\0",
        "cache_key_digest_domain": "forgeos.context-package-cache-key.v1\0",
        "context_digest_domain": "forgeos.context-package.v1\0",
        "snippet_digest_domain": "forgeos.context-snippet.v1\0",
        "projected_content_digest_domain": "forgeos.context-content.v1\0",
        "projection_digest_domain": "forgeos.context-package-projection.v1\0",
        "cache_key_input": "exact_canonical_build_request",
        "context_self_digest_rule": "context_sha256_empty_while_hashing",
        "snippet_self_digest_rule": "snippet_sha256_empty_while_hashing",
    },
    "limits": {
        "max_request_bytes": 20_971_520,
        "max_package_bytes": 2_097_152,
        "max_sources": 64,
        "max_selected_snippets": 24,
        "max_content_bytes": 524_288,
        "max_tokens": 1_000_000,
        "max_source_bytes": 131_072,
        "max_redaction_ranges": 256,
    },
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
    "provider_prompt_integration": "unavailable",
    "grant_pdp_approval_integration": "unavailable",
    "portable_package": {
        "source_distributed": True,
        "closed_manifest_required": True,
        "assembler_argv": ["python3", "-I", "-B",
                           "skills/context-engineering/scripts/assemble.py"],
        "assembler_input": "exact_canonical_stdin_only",
        "assembler_arguments": "zero",
        "checker_argv": ["python3", "-I", "-B", PACKAGE_CHECKER_RELATIVE],
        "checker_package_root_argument": "zero_or_one",
        "fixture_counter_only": "forgeos.token-counter.utf8-bytes/v1",
        "copies_catalyst_go_or_rust_runtime": False,
        "installs_host_skill": False,
        "reads_ambient_sources": False,
        "invokes_provider_or_model": False,
        "grant_pdp_authority": "unavailable",
        "persistence": "none",
        "python_isolation_boundary":
            "excludes_script_current_directory_pythonpath_and_user_site_only",
        "system_site_stdlib_interpreter_startup_host_publisher":
            "not_disabled_authenticated_or_isolated",
        "package_integrity_nofollow_unavailable_result": "exit_1_fail_closed",
        "check_to_use_atomicity": "not_provided",
    },
}

CANONICAL_REFS = {
    "context_package_schema": SCHEMA_RELATIVE,
    "context_package_golden_fixture": FIXTURE_RELATIVE,
    "context_package_checker": CHECKER_RELATIVE,
    "context_package_skill": SKILL_RELATIVE,
    "context_package_decision": DECISION_RELATIVE,
    "context_package_portable_skill": PORTABLE_SKILL_RELATIVE,
    "context_package_package_manifest": PACKAGE_MANIFEST_RELATIVE,
    "context_engineering_skill_decision": DELIVERY_DECISION_RELATIVE,
}

SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "request_digest_domain": "forgeos.context-package-build-request.v1\0",
    "cache_key_digest_domain": "forgeos.context-package-cache-key.v1\0",
    "context_digest_domain": "forgeos.context-package.v1\0",
    "snippet_digest_domain": "forgeos.context-snippet.v1\0",
    "projected_content_digest_domain": "forgeos.context-content.v1\0",
    "projection_digest_domain": "forgeos.context-package-projection.v1\0",
    "self_digest_rules": [
        "context_sha256 is empty while hashing the complete ContextPackage",
        "snippet_sha256 is empty while hashing the complete snippet",
    ],
}

SCHEMA_LIMITS = {
    "max_request_bytes": 20_971_520,
    "max_package_bytes": 2_097_152,
    "max_candidate_sources": 64,
    "max_selected_snippets": 24,
    "max_content_bytes": 524_288,
    "max_tokens": 1_000_000,
    "max_source_content_bytes": 131_072,
    "max_redaction_ranges": 256,
    "max_json_depth": 16,
    "max_object_fields": 32,
    "max_array_items": 256,
    "max_string_bytes": 131_072,
    "integer_domain": "signed_int64",
}

SCHEMA_SEMANTICS = {
    "mode": "authority_free_deterministic_context_projection",
    "lanes": ["instruction_candidates", "trusted_context", "untrusted_data"],
    "delimiter": "structured_json_lane_no_text_delimiter",
    "instruction_allowed": False,
    "redaction_before_budget": True,
    "required_fail_closed": True,
    "optional_omit_with_receipt": True,
    "token_counter_pinned": True,
    "source_order": "source_id_utf8_byte_ascending",
    "positive_result": RESULT,
    "attestations": [],
    "persistence": "none",
}

SNIPPET_LANE_RULES = [
    {"oneOf": [
        {"properties": {"declared_lane": {"const": "untrusted_data"},
                        "lane": {"const": "untrusted_data"}}},
        {"properties": {"declared_lane": {"const": "trusted_context"},
                        "lane": {"const": "trusted_context"}}},
        {"properties": {"declared_lane": {"const": "instruction"},
                        "source_class": {"enum": ["system_policy", "user_instruction"]},
                        "lane": {"const": "instruction_candidates"}}},
        {"properties": {"declared_lane": {"const": "instruction"},
                        "source_class": {"const": "governance_record"},
                        "lane": {"const": "trusted_context"}}},
    ]},
    {"if": {"properties": {"source_class": {"enum": [
        "repository", "web", "log", "issue", "tool_output", "artifact", "other",
    ]}}}, "then": {"properties": {
        "declared_lane": {"const": "untrusted_data"},
        "declared_trust": {"const": "untrusted"},
        "lane": {"const": "untrusted_data"},
    }}},
    {"if": {"properties": {"required": {"const": True}}}, "then": {"properties": {
        "selection_reason": {"const": "required_source"},
        "truncation": {"type": "null"},
    }}, "else": {"properties": {
        "selection_reason": {"const": "priority_selection"},
    }}},
]

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "repo_root",
             "context_package_build_request", "context_package"],
    "positive": "test_golden_fixture_is_assembled_shadow",
    "negative": "test_untrusted_instruction_lane_is_rejected",
}

PORTABLE_DETECTOR = {
    "argv": ["python3", "-I", "-B", PACKAGE_CHECKER_RELATIVE],
    "positive": "test_valid_closed_package",
    "negative": "test_unavailable_descriptor_primitives_fail_closed",
}

REFERENCE_IMPLEMENTATIONS = {
    "context_package_python": {
        "ref": CHECKER_RELATIVE, "projection": "universal_scaffold"},
    "context_package_go": {
        "ref": "forge-core/internal/contextpackagecontract",
        "projection": "catalyst_repository_only"},
    "context_package_rust": {
        "ref": "forge-runtime/crates/domain/src/context_package_contract",
        "projection": "catalyst_repository_only"},
    "context_package_portable_skill": {
        "ref": "skills/context-engineering", "projection":
        "source_distributed_closed_pure_stdin_adapter_without_provider_or_runtime_authority"},
}

NON_CAPABILITY = (
    "Portable Context Engineering assembles only caller-supplied exact bytes "
    "with the frozen fixture counter; it discovers no sources, invokes no provider "
    "or model, installs no prompt or host Skill, authenticates no publisher, "
    "provides no check-to-use atomicity, Grant, PDP, Approval, truth, instruction, "
    "completion, persistence, runtime routing, execution or effect authority"
)

SKILL_MARKERS = ["forgeos.context-package/v1", "instruction_allowed=false",
                 "[REDACTED]", "required source", "optional source", "TokenCounter",
                 "cache hit", "ASSEMBLED_SHADOW", "forge accept"]

PORTABLE_SKILL_MARKERS = ["python3 -I -B scripts/check_package.py",
                          "python3 -I -B scripts/assemble.py",
                          "caller supplied and unauthenticated",
                          "does not atomically bind a later assembler process",
                          "does not disable, authenticate, or isolate system site",
                          "not live model context"]

DELIVERY_DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Portable Context Engineering"],
    ".agent/ARCHITECTURE.md": ["Portable Context Engineering Skill"],
    ".agent/ROADMAP.md": ["`context-engineering` narrow package slice"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 118", "context-engineering"],
    ".agent/DECISIONS.md": ["D43 Portable Context Engineering Skill"],
    ".agent/engineering/README.md": ["ADR-0071 adds"],
    "docs/design/ai-engineering-os/README.md": ["ADR-0071 Portable Context Engineering Skill"],
    "docs/design/ai-engineering-os/governance-contracts.md": ["ADR-0071 Portable Context Engineering Skill"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": ["`context-engineering` narrow package slice"],
}

def _pairs(pairs):
    value = {}
    for key, child in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key {key!r}")
        value[key] = child
    return value

def _load_schema(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(path, label=SCHEMA_RELATIVE, max_bytes=1_048_576)
        return path, json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs), None
    except (OSError, ContractError, UnicodeDecodeError, ValueError, json.JSONDecodeError) as error:
        return path, None, error

def registry_issues(data, path):
    issues = []
    if data.get("context_package") != CONTEXT_PACKAGE:
        issues.append(f"{path}: context_package contract drifted")
    refs = data.get("canonical_refs") if isinstance(data.get("canonical_refs"), dict) else {}
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    implementations = data.get("reference_implementations")
    implementations = implementations if isinstance(implementations, dict) else {}
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    pins = data.get("contract_pins")
    pins = pins if isinstance(pins, dict) else {}
    if pins.get("context_package_package_manifest_sha256") != PACKAGE_MANIFEST_SHA256:
        issues.append(f"{path}: portable ContextPackage manifest pin drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: portable ContextPackage non-capability drifted")
    scope = data.get("scope") if isinstance(data.get("scope"), dict) else {}
    if "ContextPackage" not in (scope.get("shipped_kinds") or []):
        issues.append(f"{path}: ContextPackage must be a shipped authority-free kind")
    if "ContextPackage" in (scope.get("planned_kinds") or []):
        issues.append(f"{path}: ContextPackage cannot remain planned after ADR-0055")
    return issues

def schema_issues(repo_root):
    path, schema, error = _load_schema(repo_root)
    if error:
        return [f"{path}: cannot validate ContextPackage Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": SCHEMA_LIMITS,
        "x-forgeos-context-semantics": SCHEMA_SEMANTICS,
    }
    issues = [f"{path}: {field} drifted" for field, value in expected.items()
              if schema.get(field) != value]
    definitions = schema.get("$defs") if isinstance(schema.get("$defs"), dict) else {}
    snippet = definitions.get("snippet") if isinstance(definitions.get("snippet"), dict) else {}
    if snippet.get("allOf") != SNIPPET_LANE_RULES:
        issues.append(f"{path}: snippet trust-lane enforcement drifted")
    hash_shape = definitions.get("hash") if isinstance(definitions.get("hash"), dict) else {}
    if (hash_shape.get("minLength"), hash_shape.get("maxLength")) != (64, 64):
        issues.append(f"{path}: hash exact-length enforcement drifted")
    for name in ("text", "content"):
        shape = definitions.get(name) if isinstance(definitions.get(name), dict) else {}
        if not isinstance(shape.get("not"), dict) or "pattern" not in shape["not"]:
            issues.append(f"{path}: {name} forbidden-scalar enforcement drifted")
    return issues

def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detectors = detector_index(agent_root, "engineering/detectors.yml")
    detector = detectors.get("governance.context_package_contract")
    if not isinstance(detector, dict):
        return ["ContextPackage shadow detector is missing"]
    implementation = detector.get("implementation")
    tests = detector.get("tests")
    issues = []
    if not isinstance(implementation, dict) or implementation.get("argv") != DETECTOR["argv"]:
        issues.append("ContextPackage detector requires exact request/package arguments")
    if not isinstance(tests, dict):
        issues.append("ContextPackage detector tests are missing")
    else:
        for polarity in ("positive", "negative"):
            test = tests.get(polarity)
            if not isinstance(test, dict) or test.get("contains") != DETECTOR[polarity]:
                issues.append(f"ContextPackage detector {polarity} test drifted")
    portable = detectors.get("governance.context_engineering_portable_package")
    if not isinstance(portable, dict):
        return issues + ["Context Engineering package detector is missing"]
    implementation = portable.get("implementation")
    invocation = portable.get("invocation")
    tests = portable.get("tests")
    if not isinstance(implementation, dict) or (
            implementation.get("argv") != PORTABLE_DETECTOR["argv"]):
        issues.append("Context Engineering package detector argv drifted")
    if portable.get("state") != "shadow" or not isinstance(invocation, dict) or (
            invocation.get("load_bearing") is not False):
        issues.append("Context Engineering package detector must remain shadow")
    for polarity in ("positive", "negative"):
        test = tests.get(polarity) if isinstance(tests, dict) else None
        if not isinstance(test, dict) or (
                test.get("contains") != PORTABLE_DETECTOR[polarity]):
            issues.append(f"Context Engineering package {polarity} test drifted")
    return issues

def skill_issues(repo_root):
    issues = []
    for relative, markers in {
            SKILL_RELATIVE: SKILL_MARKERS,
            PORTABLE_SKILL_RELATIVE: PORTABLE_SKILL_MARKERS,
    }.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ContextPackage Skill: {error}")
            continue
        issues.extend(f"{path}: missing ContextPackage marker {marker!r}"
                      for marker in markers if marker not in text)
    return issues

def package_issues(repo_root):
    manifest_path = repo_root / PACKAGE_MANIFEST_RELATIVE
    try:
        raw = read_bounded_file(manifest_path, label=PACKAGE_MANIFEST_RELATIVE,
                                max_bytes=1_048_576)
    except (OSError, ContractError) as error:
        return [f"{manifest_path}: cannot validate portable package pin: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != PACKAGE_MANIFEST_SHA256:
        issues.append(f"{manifest_path}: portable package manifest pin drifted")
    checker_path = repo_root / PACKAGE_CHECKER_RELATIVE
    try:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(checker_path),
             str(repo_root / "skills/context-engineering")],
            cwd=repo_root, capture_output=True, check=False, timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        issues.append(f"{checker_path}: portable package rejected: {error}")
    else:
        if result.returncode != 0:
            detail = result.stderr.decode("utf-8", "replace").strip()
            issues.append(f"{checker_path}: portable package rejected: {detail}")
    return issues

def wiring_issues(repo_root, agent_root):
    from engineering_check_support import load_yaml
    activation, error = load_yaml(agent_root / "engineering/activation.yml")
    extension = activation.get("canonical_extension_refs") if not error else {}
    extension = extension if isinstance(extension, dict) else {}
    expected = {
        "context_package_portable_skill": PORTABLE_SKILL_RELATIVE,
        "context_package_package_manifest": PACKAGE_MANIFEST_RELATIVE,
        "context_engineering_skill_decision": DELIVERY_DECISION_RELATIVE,
    }
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, value in expected.items() if extension.get(field) != value]
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if route_error or discipline_error:
        return issues + ["Context Engineering route/discipline registry unreadable"]
    governance = next((item for item in routes["routes"]
                       if item.get("id") == "governance"), {})
    refs = {item.get("ref") for item in governance.get("include") or []}
    if not {SKILL_RELATIVE, SCHEMA_RELATIVE}.issubset(refs):
        issues.append("Context Engineering governance route is incomplete")
    all_refs = {item.get("ref") for route in routes["routes"]
                for item in route.get("include") or []}
    if PORTABLE_SKILL_RELATIVE in all_refs:
        issues.append("portable Skill cannot become a routed instruction source")
    by_id = {item.get("id"): item for item in disciplines["disciplines"]}
    required = {"context": "skills/context-engineering",
                "tool": "skills/context-engineering",
                "contract": PORTABLE_SKILL_RELATIVE}
    for discipline, asset in required.items():
        if asset not in (by_id.get(discipline, {}).get("assets") or []):
            issues.append(f"Context Engineering {discipline} assets incomplete")
    return issues

def documentation_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    issues = []
    for relative, markers in DELIVERY_DOCUMENT_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate delivery marker: {error}")
            continue
        issues.extend(f"{path}: missing Context Engineering marker {marker!r}"
                      for marker in markers if marker not in text)
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(roadmap, label=str(roadmap)).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate roadmap: {error}"]
    if "- [ ] 按 `implementation_wave` 逐 package 实现 Skill" not in text:
        issues.append(f"{roadmap}: 38-package parent must remain unchecked")
    if "  - [x] `context-engineering` 窄切片" not in text:
        issues.append(f"{roadmap}: context-engineering nested item must be checked")
    return issues

def adr_issues(repo_root):
    path = repo_root / DELIVERY_DECISION_RELATIVE
    try:
        raw = read_bounded_file(path, label=DELIVERY_DECISION_RELATIVE)
        metadata = validate_document_file(path)
        text = raw.decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: ADR-0071 v2 validation failed: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != DELIVERY_DECISION_SHA256:
        issues.append(f"{path}: ADR-0071 physical pin drifted")
    expected = {"status": "proposed",
                "body_sha256": DELIVERY_DECISION_BODY_SHA256,
                "self_sha256": DELIVERY_DECISION_SELF_SHA256}
    issues.extend(f"{path}: ADR-0071 {field} drifted"
                  for field, value in expected.items() if metadata.get(field) != value)
    normalized = " ".join(text.split())
    for marker in ("zero-argument stdin adapter", "system site", "check-to-use",
                   "parent 38-package item", "compile a provider prompt",
                   "supply provider credentials"):
        if marker not in normalized:
            issues.append(f"{path}: missing delivery marker {marker!r}")
    return issues

def integration_issues(data, path, repo_root, agent_root):
    from context_package_contract_check import validate_golden_fixture
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(package_issues(repo_root))
    issues.extend(wiring_issues(repo_root, agent_root))
    issues.extend(documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
