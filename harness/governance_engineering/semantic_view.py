#!/usr/bin/env python3
"""ADR-0054 registry, Schema, golden and Skill integration checker."""
import json
import sys
from pathlib import Path

HARNESS_ROOT = Path(__file__).resolve().parents[1]
if str(HARNESS_ROOT) not in sys.path:
    sys.path.insert(0, str(HARNESS_ROOT))

from engineering_check_support import load_yaml
from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/governance-semantic-view-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/governance-semantic-view-v1.json"
SOURCE_RECORD_REF = (
    "docs/contracts/fixtures/governance-evidence-claim-v1.json#/records/1/record"
)
POLICY_RELATIVE = ".agent/engineering/governance-contracts.yml"
SKILL_RELATIVE = ".agent/skills/evidence-claim-management.md"
SEMANTIC_VIEW = {
    "api_version": "forgeos.governance-semantic-view/v1",
    "mode": "deterministic_rebuildable_local_journal_projection",
    "interpretation": "semantic_projection_only_no_truth_or_authority",
    "source": {
        "record_kinds": ["EvidenceRecord", "KnowledgeClaim"],
        "current_record": "always_current_structural_aggregate_tail",
        "as_of_record_selection": "never_selects_historical_tail",
        "evaluation_time": "explicit_caller_supplied_as_of_unix_ms",
        "hidden_wall_clock": "forbidden",
    },
    "lifecycle": {
        "mode": "durable_shadow_subset_only",
        "stable_fields": [
            "record_kind", "aggregate_id", "project_id", "scope", "claim_type",
            "subject", "predicate", "object_type", "object_value", "owner",
        ],
        "sequence_one_state_policy":
            "any_claim_type_admissible_authority_free_shadow_state",
        "sequence_one_allowed_states": {
            "fact": ["candidate", "contested"],
            "constraint": ["candidate"],
            "decision": ["proposed"],
            "inference": ["candidate"],
            "assumption": ["open", "testing"],
            "hypothesis": ["open", "testing"],
            "lesson": ["candidate"],
            "proposal": ["draft", "submitted"],
            "unknown": ["open", "investigating"],
        },
        "transitions": {
            "fact": [
                "candidate_to_candidate", "candidate_to_contested",
                "contested_to_candidate", "contested_to_contested",
            ],
            "constraint": ["candidate_to_candidate"],
            "decision": ["proposed_to_proposed"],
            "inference": ["candidate_to_candidate"],
            "assumption": ["open_to_open", "open_to_testing", "testing_to_testing"],
            "hypothesis": ["open_to_open", "open_to_testing", "testing_to_testing"],
            "lesson": ["candidate_to_candidate"],
            "proposal": [
                "draft_to_draft", "draft_to_submitted", "submitted_to_submitted",
            ],
            "unknown": [
                "open_to_open", "open_to_investigating",
                "investigating_to_investigating",
            ],
        },
        "authoritative_state_result": "reject_authority_unavailable",
    },
    "temporal": {
        "states": [
            "fresh", "not_yet_valid", "review_overdue", "validation_overdue",
            "validity_expired",
        ],
        "precedence": [
            "not_yet_valid", "validity_expired", "validation_overdue",
            "review_overdue", "fresh",
        ],
        "semantics": "declared_time_comparison_only",
    },
    "conflicts": {
        "key_fields": ["claim_type", "project_id", "scope", "subject", "predicate"],
        "candidates": "active_claim_tails_with_distinct_object_digests",
        "deterministic_order": "conflict_key_then_aggregate_id_bytes",
        "winner": "none",
    },
    "validation_jobs": {
        "claim_types": ["assumption", "hypothesis"],
        "materialization": "one_per_current_complete_validation_plan",
        "due": "explicit_as_of_reaches_due_while_active",
        "execution": "none",
        "verdict": "none",
    },
    "identity": {
        "projection_digest_domain": "forgeos.governance.semantic-projection.v1\0",
        "conflict_key_digest_domain": "forgeos.governance.claim-conflict-key.v1\0",
        "object_digest_domain": "forgeos.governance.claim-object.v1\0",
        "validation_plan_digest_domain": "forgeos.governance.validation-plan.v1\0",
        "validation_job_digest_domain": "forgeos.governance.validation-job.v1\0",
        "validation_job_id_prefix": "governance-validation-job-",
    },
    "storage": {
        "schema": "sqlite_hub_v27_additive_projection",
        "append_transaction":
            "exact_records_structural_heads_semantic_heads_and_jobs_atomic",
        "v26_backfill": "exact_journal_records_and_reference_relations",
        "corruption": "fail_closed_no_implicit_repair",
        "public_read_migration": "forbidden",
        "public_read_connection": "exact_v27_live_mode_ro_query_only",
        "public_read_snapshot": "single_deferred_transaction",
        "public_read_logical_hub_writes": "none",
        "sqlite_sidecar_effects":
            "may_create_or_remove_empty_wal_shm_and_coordinate_shm_locks",
        "fully_read_only_filesystem_failure": "unavailable",
    },
    "limits": {
        "max_list_results": 100,
        "max_conflict_members": 100,
        "max_claim_head_scan": 10000,
        "per_view_integrity_union":
            "complete_history_reference_closure_and_full_owning_batches",
        "max_view_unique_records": 1024,
        "max_view_unique_canonical_bytes": 16777216,
        "scan_integrity_union":
            "all_unique_owning_batch_records_and_verification_work",
        "max_scan_unique_records": 65536,
        "max_scan_unique_canonical_bytes": 268435456,
        "max_scan_work_units": 1000000,
    },
    "positive_result": "PROJECTED_DECLARED_SEMANTICS",
    "attests": [],
}
SCHEMA_SEMANTICS = {
    "interpretation": "semantic_projection_only_no_truth_or_authority",
    "evaluation_time": "explicit_caller_supplied_unix_ms",
    "record_selection": "always_current_structural_aggregate_tail",
    "as_of_record_selection": "never_selects_historical_tail",
    "conflicts": "candidates_only_no_winner",
    "validation_jobs": "deterministic_schedule_only_no_execution_or_verdict",
}
STATE_REFS = {
    "#/$defs/fact_shadow_state",
    "#/$defs/candidate_shadow_state",
    "#/$defs/decision_shadow_state",
    "#/$defs/validation_shadow_state",
    "#/$defs/proposal_shadow_state",
    "#/$defs/unknown_shadow_state",
}
DECLARED_STATE_REFS = STATE_REFS | {
    "#/$defs/evidence_state",
    "#/$defs/shadow_claim_state",
}
SHADOW_STATE_DEFINITIONS = {
    "fact_shadow_state": {"enum": ["candidate", "contested"]},
    "candidate_shadow_state": {"const": "candidate"},
    "decision_shadow_state": {"const": "proposed"},
    "validation_shadow_state": {"enum": ["open", "testing"]},
    "proposal_shadow_state": {"enum": ["draft", "submitted"]},
    "unknown_shadow_state": {"enum": ["open", "investigating"]},
}
SEQUENCE_ONE_STATE_DEFINITIONS = {
    "fact": "fact_shadow_state",
    "constraint": "candidate_shadow_state",
    "decision": "decision_shadow_state",
    "inference": "candidate_shadow_state",
    "assumption": "validation_shadow_state",
    "hypothesis": "validation_shadow_state",
    "lesson": "candidate_shadow_state",
    "proposal": "proposal_shadow_state",
    "unknown": "unknown_shadow_state",
}
CANONICAL_REFS = {
    "semantic_view_schema": SCHEMA_RELATIVE,
    "semantic_view_golden_fixture": FIXTURE_RELATIVE,
    "semantic_view_checker": "harness/governance_engineering/semantic_view.py",
    "semantic_view_decision": "docs/adr/0054-local-governance-semantic-view-v1.md",
}
SKILL_MARKER_GROUPS = {
    "always-current/no-history": (
        "always-current structural aggregate tail",
        "as_of_unix_ms never selects a historical tail",
    ),
    "sequence-one lifecycle": (
        "sequence=1 accepts any type-admissible authority-free shadow state",
        "successor sequences require strict lifecycle transitions",
    ),
    "exact-v27 live-read sidecar boundary": (
        "exact-v27 live read", "mode=ro", "query_only",
        "transient empty WAL/SHM sidecar", "SHM read-lock bytes",
        "read-only filesystem may return unavailable",
    ),
    "per-view owning-batch integrity union": (
        "complete history + reference closure + full owning-batch union",
        "1,024 unique records", "16,777,216 canonical bytes",
    ),
    "shared scan budgets": (
        "shared multi-head scan budget", "65,536 unique records",
        "268,435,456 canonical bytes", "1,000,000 work units",
    ),
    "v26 relation validation": (
        "v26 backfill validates exact journal records and reference relations",
    ),
}


def registry_issues(data, path):
    projection = data.get("semantic_view") if isinstance(data, dict) else None
    issues = []
    if projection != SEMANTIC_VIEW:
        issues.append(f"{path}: semantic_view contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in CANONICAL_REFS.items():
            if refs.get(field) != expected:
                issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    return issues


def skill_marker_issues(text, path):
    issues = []
    for label, markers in SKILL_MARKER_GROUPS.items():
        if not all(marker in text for marker in markers):
            issues.append(f"{path}: ADR-0054 Skill {label} contract drifted")
    return issues


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        schema = json.loads(read_bounded_file(path, label=SCHEMA_RELATIVE))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate GovernanceSemanticView Schema: {error}"]
    issues = []
    if schema.get("x-forgeos-semantics") != SCHEMA_SEMANTICS:
        issues.append(f"{path}: x-forgeos-semantics drifted")
    if schema.get("oneOf") != [
        {"$ref": "#/$defs/assessment"},
        {"$ref": "#/$defs/conflict_list"},
        {"$ref": "#/$defs/validation_job_list"},
    ]:
        issues.append(f"{path}: public semantic read envelopes drifted")
    definitions = schema.get("$defs") if isinstance(schema.get("$defs"), dict) else {}
    try:
        bounds = (
            definitions["conflict_group"]["properties"]["members"]["maxItems"],
            definitions["conflict_list"]["properties"]["conflicts"]["maxItems"],
            definitions["validation_job_list"]["properties"]["jobs"]["maxItems"],
        )
    except (KeyError, TypeError):
        bounds = None
    if bounds != (100, 100, 100):
        issues.append(f"{path}: semantic public list bounds drifted")
    for name, expected in SHADOW_STATE_DEFINITIONS.items():
        if definitions.get(name) != expected:
            issues.append(f"{path}: {name} drifted")
    issues.extend(_sequence_one_state_issues(definitions, path))
    issues.extend(_public_read_shape_issues(definitions, path))
    expected_projection_refs = STATE_REFS | {"#/$defs/evidence_state"}
    if _state_refs(definitions.get("projection")) != expected_projection_refs:
        issues.append(f"{path}: projection claim-type state conditions drifted")
    if _state_refs(definitions.get("conflict_group")) != STATE_REFS:
        issues.append(f"{path}: conflict member claim-type state conditions drifted")
    if _state_refs(definitions.get("validation_job")) != {
            "#/$defs/shadow_claim_state", "#/$defs/validation_shadow_state"}:
        issues.append(f"{path}: validation job claim-type state condition drifted")
    if not _validation_plan_is_all_present_or_all_absent(definitions):
        issues.append(f"{path}: validation plan presence condition drifted")
    if not _validation_plan_is_bound_to_claim_type(definitions):
        issues.append(f"{path}: validation plan claim-type condition drifted")
    return issues


def _declared_states(definition):
    if not isinstance(definition, dict):
        return None
    if isinstance(definition.get("enum"), list):
        return definition["enum"]
    if isinstance(definition.get("const"), str):
        return [definition["const"]]
    return None


def _sequence_one_state_issues(definitions, path):
    allowed = SEMANTIC_VIEW["lifecycle"]["sequence_one_allowed_states"]
    issues = []
    for claim_type, definition_name in SEQUENCE_ONE_STATE_DEFINITIONS.items():
        actual = _declared_states(definitions.get(definition_name))
        if actual != allowed[claim_type]:
            issues.append(f"{path}: sequence-one {claim_type} state policy drifted")
    return issues


def _public_read_shape_issues(definitions, path):
    conflict_group = definitions.get("conflict_group", {})
    conflict_list = definitions.get("conflict_list", {})
    job = definitions.get("validation_job", {})
    job_list = definitions.get("validation_job_list", {})
    conflict_members = conflict_group.get("properties", {}).get("members")
    conflicts = conflict_list.get("properties", {}).get("conflicts")
    job_properties = job.get("properties", {})
    jobs = job_list.get("properties", {}).get("jobs")
    expected = {
        "conflict member bounds": (conflict_members or {}).get("minItems") == 2,
        "conflict list kind": conflict_list.get("properties", {}).get("kind") ==
            {"const": "GovernanceClaimConflictList"},
        "validation job id": job_properties.get("job_id") == {
            "type": "string", "pattern": "^governance-validation-job-[a-f0-9]{64}$",
        },
        "validation job claim types": job_properties.get("claim_type") ==
            {"enum": ["assumption", "hypothesis"]},
        "validation job evidence bounds": job_properties.get("required_evidence_types", {}).get(
            "minItems") == 1,
        "validation job list kind": job_list.get("properties", {}).get("kind") ==
            {"const": "GovernanceValidationJobList"},
        "public list item refs": (
            (conflicts or {}).get("items") == {"$ref": "#/$defs/conflict_group"}
            and (jobs or {}).get("items") == {"$ref": "#/$defs/validation_job"}
        ),
    }
    return [f"{path}: {label} drifted" for label, valid in expected.items() if not valid]


def _state_refs(value):
    refs = set()
    if isinstance(value, dict):
        for key, nested in value.items():
            if key == "$ref" and nested in DECLARED_STATE_REFS:
                refs.add(nested)
            else:
                refs.update(_state_refs(nested))
    elif isinstance(value, list):
        for nested in value:
            refs.update(_state_refs(nested))
    return refs


def _validation_plan_is_all_present_or_all_absent(definitions):
    claim = definitions.get("claim_fields")
    alternatives = claim.get("oneOf") if isinstance(claim, dict) else None
    if not isinstance(alternatives, list) or len(alternatives) != 2:
        return False
    properties = [item.get("properties") for item in alternatives
                  if isinstance(item, dict)]
    if len(properties) != 2 or not all(isinstance(item, dict) for item in properties):
        return False
    absent, present = properties
    return (
        absent.get("validation_due_unix_ms") == {"type": "null"}
        and absent.get("validation_owner_id") == {"type": "null"}
        and absent.get("validation_plan_sha256") == {"type": "null"}
        and absent.get("required_evidence_types") == {"maxItems": 0}
        and present.get("validation_due_unix_ms") == {"$ref": "#/$defs/unix_ms"}
        and present.get("validation_owner_id") == {"$ref": "#/$defs/identifier"}
        and present.get("validation_plan_sha256") == {"$ref": "#/$defs/hash"}
        and present.get("required_evidence_types") == {"minItems": 1}
    )


def _validation_plan_is_bound_to_claim_type(definitions):
    claim = definitions.get("claim_fields")
    bindings = claim.get("allOf") if isinstance(claim, dict) else None
    if not isinstance(bindings, list) or len(bindings) != 1:
        return False
    binding = bindings[0] if isinstance(bindings[0], dict) else {}
    claim_type = (binding.get("if", {}).get("properties", {})
                  .get("claim_type"))
    then = binding.get("then", {}).get("properties")
    otherwise = binding.get("else", {}).get("properties")
    return (
        claim_type == {"enum": ["assumption", "hypothesis"]}
        and isinstance(then, dict)
        and isinstance(otherwise, dict)
        and then.get("validation_due_unix_ms") == {"$ref": "#/$defs/unix_ms"}
        and then.get("required_evidence_types") == {"minItems": 1}
        and otherwise.get("validation_due_unix_ms") == {"type": "null"}
        and otherwise.get("required_evidence_types") == {"maxItems": 0}
    )


def _resolve_source_record(repo_root, reference):
    if reference != SOURCE_RECORD_REF:
        return None
    relative, pointer = reference.split("#", 1)
    try:
        current = json.loads(read_bounded_file(repo_root / relative, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    for encoded in pointer.removeprefix("/").split("/"):
        token = encoded.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and token in current:
            current = current[token]
        elif isinstance(current, list) and token.isdigit():
            index = int(token)
            if index >= len(current):
                return None
            current = current[index]
        else:
            return None
    return current if isinstance(current, dict) else None


def fixture_issues(repo_root):
    path = repo_root / FIXTURE_RELATIVE
    try:
        fixture = json.loads(read_bounded_file(path, label=FIXTURE_RELATIVE))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate GovernanceSemanticView golden: {error}"]
    expected = fixture.get("expected_assessment")
    projection = expected.get("projection") if isinstance(expected, dict) else None
    claim = projection.get("claim") if isinstance(projection, dict) else None
    head = projection.get("head") if isinstance(projection, dict) else None
    source = _resolve_source_record(repo_root, fixture.get("source_record_ref"))
    metadata = source.get("metadata") if isinstance(source, dict) else None
    integrity = source.get("integrity") if isinstance(source, dict) else None
    checks = {
        "api_version": fixture.get("api_version") ==
            "forgeos.governance-semantic-view-golden/v1",
        "interpretation": fixture.get("interpretation") ==
            "semantic_projection_only_no_truth_or_authority",
        "source_record_ref": fixture.get("source_record_ref") == SOURCE_RECORD_REF,
        "source_record_binding": isinstance(metadata, dict)
            and isinstance(integrity, dict) and isinstance(head, dict)
            and metadata.get("record_id") == head.get("record_id")
            and metadata.get("aggregate_id") == head.get("aggregate_id")
            and metadata.get("sequence") == head.get("sequence")
            and integrity.get("canonical_sha256") == head.get("canonical_sha256"),
        "schema_ref": fixture.get("schema_ref") == SCHEMA_RELATIVE,
        "projection_input": fixture.get("projection_input") == {
            "updated_at_ms": 77, "as_of_unix_ms": 1700000002000,
        },
        "expected_assessment": isinstance(expected, dict)
            and expected.get("api_version") == "forgeos.governance-semantic-view/v1"
            and expected.get("kind") == "GovernanceSemanticAssessment"
            and expected.get("interpretation") ==
                "semantic_projection_only_no_truth_or_authority"
            and expected.get("semantic_view_version") == 1
            and expected.get("evaluated_at_unix_ms") == 1700000002000,
        "expected_projection": isinstance(projection, dict)
            and projection.get("v") == 1
            and projection.get("projection_sha256") ==
                "9754f24cb1c6f33d72492e1391c9bc70e44d5893020def288610b84f17e88fea",
        "expected_head": isinstance(head, dict)
            and head.get("record_id") == "kcr-0001"
            and head.get("updated_at_ms") == 77,
        "expected_claim": isinstance(claim, dict)
            and claim.get("object_sha256") ==
                "1ac7d6dc7edb8d8a2e1fc46bbeddeac4a29dfef46189a9d00cf460fe8522d9e7"
            and claim.get("conflict_key_sha256") ==
                "18c4cde5df45ae97c4a1fed20c635898a2fd1aa647086773afb3efbac73d8ae0",
    }
    return [f"{path}: {field} drifted" for field, valid in checks.items() if not valid]


def repository_issues(repo_root):
    root = Path(repo_root).resolve()
    policy_path = root / POLICY_RELATIVE
    data, error = load_yaml(policy_path)
    if error:
        issues = [f"{policy_path}: invalid YAML ({error})"]
    else:
        issues = registry_issues(data, policy_path)
    issues.extend(schema_issues(root))
    issues.extend(fixture_issues(root))
    skill_path = root / SKILL_RELATIVE
    try:
        text = read_bounded_file(skill_path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        issues.append(f"{skill_path}: cannot validate ADR-0054 Skill: {error}")
    else:
        issues.extend(skill_marker_issues(text, skill_path))
    return issues


def main(argv=None):
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        print("usage: semantic_view.py <repo-root>", file=sys.stderr)
        return 2
    issues = repository_issues(args[0])
    if issues:
        for issue in issues:
            print(f"semantic-view-check: {issue}", file=sys.stderr)
        print("semantic-view-check: FAIL", file=sys.stderr)
        return 1
    print("semantic-view-check: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
