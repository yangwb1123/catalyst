"""Frozen governance wiring for the Evolve repository locator adapter."""

import json

from governance_contract import ContractError, read_bounded_file


EVOLVE_LOCATOR_SUCCESS = (
    "ADAPTED_SHADOW (locator mapping only; no file/report verification, scan "
    "judgment, completion, truth, authority, claim, atom, persistence, or effect "
    "attestation)"
)
EVOLVE_LOCATOR_SKILL_MARKERS = (
    "### Evolve repository locator adapter 分支",
    ("python3 -B harness/evolve_repo_locator_evidence_adapter_check.py --golden "
     "<repo-root>"),
    ("python3 -B harness/evolve_repo_locator_evidence_adapter_check.py <repo-root> "
     "<request.json> <evidence-record.json>"),
    EVOLVE_LOCATOR_SUCCESS,
    "不会读取当前 repository path",
    "不会验证 file/report/tree/parameters digest preimage",
    "不会确认 finding、clear 或 opportunity",
    "不会创建 Claim/Atom、不会 append journal",
)
EVOLVE_REPO_LOCATOR_EVIDENCE_ADAPTER = {
    "api_version": "forgeos.governance.evolve-repo-locator-evidence-adapter/v1",
    "mode": "deterministic_evolve_repo_locator_to_evidence_shadow",
    "source_kind": "forgeos.evolve-repo-locator/v1",
    "output_kind": "EvidenceRecord",
    "source_fields": [
        "api_version", "canonicalization", "content", "locator",
        "observed_at_unix_ms", "producer", "scan_context", "source",
    ],
    "binding_fields": [
        "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope",
        "sensitivity", "sequence", "subjects", "supersedes_record_ids",
    ],
    "exact_request": {
        "canonical_required": True,
        "compact_utf8_required": True,
        "observation_anchor": "#EvolveRepoLocatorV1",
        "reads_current_repository_file": False,
        "verifies_content_or_report_preimage": False,
        "relation_values": ["clear", "finding", "opportunity"],
        "opportunity_id_profile": "evolve_scan_v1_ascii_64",
        "unavailable_projectable": False,
    },
    "identity": {
        "locator_digest_domain":
            "forgeos.governance.evolve-repo-locator.locator.v1\0",
        "source_digest_domain":
            "forgeos.governance.evolve-repo-locator-source.v1\0",
        "request_digest_domain":
            "forgeos.governance.evolve-repo-locator-evidence-adapter.request.v1\0",
        "evidence_record_digest_domain": "forgeos.governance.evidence-record.v1\0",
        "record_id_prefix": "evolve-locator-evidence-",
        "snapshot_id_prefix": "evolve-locator-",
        "principal_run_id_prefix": "evolve-locator-adaptation-",
    },
    "constants": {
        "principal_id": "forgeos.evolve-repo-locator-evidence-adapter",
        "principal_type": "tool",
        "authority_domain": "shadow",
        "role": "evidence-adapter",
        "collector_from": "producer_declaration",
        "evidence_type": "repo_locator",
        "locator_type": "repo",
        "line_projection": "zero_to_null_pair_positive_to_equal_pair",
        "directness": "direct",
        "source_trust": "observed",
        "content_role": "untrusted_data",
        "source_snapshot_type": "repository",
        "artifact_sha256": "observation.content.sha256",
        "status": "valid",
    },
    "limits": {
        "max_request_bytes": 131072,
        "max_depth": 16,
        "max_object_fields": 64,
        "max_array_items": 256,
        "max_string_bytes": 16384,
        "max_detail_bytes": 512,
        "max_path_scalars": 4096,
        "max_opportunity_id_bytes": 64,
        "max_content_bytes": 1048576,
        "integer_domain": "signed_int64",
    },
    "positive_result": EVOLVE_LOCATOR_SUCCESS,
    "attests": [],
    "persistence": "none",
}
EVOLVE_LOCATOR_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "locator_digest_domain":
        "forgeos.governance.evolve-repo-locator.locator.v1\0",
    "source_digest_domain":
        "forgeos.governance.evolve-repo-locator-source.v1\0",
    "request_digest_domain":
        "forgeos.governance.evolve-repo-locator-evidence-adapter.request.v1\0",
    "evidence_record_digest_domain": "forgeos.governance.evidence-record.v1\0",
}
EVOLVE_LOCATOR_SCHEMA_LIMITS = EVOLVE_REPO_LOCATOR_EVIDENCE_ADAPTER["limits"]
EVOLVE_LOCATOR_SCHEMA_SEMANTIC_VALIDATION = {
    "schema_alone_sufficient": False,
    "observation_ref": "#EvolveRepoLocatorV1",
    "authoritative_checks": [
        "exact_compact_canonical_utf8", "bounded_unicode_and_utf8_bytes",
        "sorted_unique_identifier_arrays",
        "canonical_ascii_casefold_protected_repository_path",
        "evolve_locator_control_and_identifier_vocabulary",
        "relation_opportunity_id_matrix",
    ],
    "reads_current_repository_file": False,
    "verifies_content_or_report_preimage": False,
}
EVOLVE_LOCATOR_SCHEMA_MAPPING = {
    "result": EVOLVE_LOCATOR_SUCCESS,
    "input_kind": "forgeos.evolve-repo-locator/v1",
    "output_kind": "EvidenceRecord",
    "record_id_prefix": "evolve-locator-evidence-",
    "snapshot_id_prefix": "evolve-locator-",
    "principal_id": "forgeos.evolve-repo-locator-evidence-adapter",
    "principal_type": "tool",
    "authority_domain": "shadow",
    "role": "evidence-adapter",
    "principal_run_id": "evolve-locator-adaptation-<request_sha256>",
    "collector_from": "producer_declaration",
    "evidence_type": "repo_locator",
    "locator_type": "repo",
    "line_projection": "zero_to_null_pair_positive_to_equal_pair",
    "directness": "direct",
    "source_trust": "observed",
    "content_role": "untrusted_data",
    "source_snapshot_type": "repository",
    "artifact_sha256": "observation.content.sha256",
    "status": "valid",
    "persistence": "none",
    "attestations": [],
}
EVOLVE_LOCATOR_CANONICAL_REFS = {
    "evolve_repo_locator_evidence_adapter_schema":
        "docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json",
    "evolve_repo_locator_evidence_adapter_golden_fixture":
        "docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json",
    "evolve_repo_locator_evidence_adapter_checker":
        "harness/evolve_repo_locator_evidence_adapter_check.py",
    "evolve_repo_locator_evidence_adapter_decision":
        "docs/adr/0050-evolve-repo-locator-evidence-adapter-v1.md",
}
EVOLVE_LOCATOR_EVIDENCE_DETECTOR = {
    "id": "governance.evolve_repo_locator_evidence_adapter",
    "version": "1.0.0",
    "state": "shadow",
    "rule_refs": ["TRUTH-002"],
    "implementation": {
        "argv": [
            "python3", "harness/evolve_repo_locator_evidence_adapter_check.py",
            "repo_root", "evolve_repo_locator_evidence_request", "evidence_record",
        ],
        "cwd": "repo_root",
        "shell": False,
    },
    "invocation": {
        "owner": "operator",
        "adapter": "standalone.evolveRepoLocatorEvidenceAdapter",
        "acceptance_criterion": None,
        "load_bearing": False,
    },
    "fail_closed": True,
    "tests": {
        "positive": {
            "path": "harness/test_evolve_repo_locator_evidence_adapter_check.py",
            "contains": "test_golden_fixture_is_adapted_shadow",
        },
        "negative": {
            "path": "harness/test_evolve_repo_locator_evidence_adapter_check.py",
            "contains": "test_projection_drift_is_rejected",
        },
    },
}


def evolve_locator_skill_marker_issues(text, path):
    if any(marker not in text for marker in EVOLVE_LOCATOR_SKILL_MARKERS):
        return [
            f"{path}: Evolve locator Evidence adapter guidance or "
            "non-capability boundary is missing"
        ]
    return []


def evolve_locator_adapter_registry_issues(data, path):
    key = "evolve_repo_locator_evidence_adapter"
    adapter = data.get(key) if isinstance(data, dict) else None
    issues = []
    if adapter != EVOLVE_REPO_LOCATOR_EVIDENCE_ADAPTER:
        issues.append(f"{path}: {key} contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in EVOLVE_LOCATOR_CANONICAL_REFS.items():
            if refs.get(field) != expected:
                issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    return issues


def evolve_locator_adapter_schema_issues(repo_root):
    relative = "docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json"
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate Evolve locator adapter Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": EVOLVE_LOCATOR_SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": EVOLVE_LOCATOR_SCHEMA_LIMITS,
        "x-forgeos-semantic-validation": EVOLVE_LOCATOR_SCHEMA_SEMANTIC_VALIDATION,
        "x-forgeos-mapping": EVOLVE_LOCATOR_SCHEMA_MAPPING,
    }
    return [f"{path}: {field} drifted" for field, value in expected.items()
            if schema.get(field) != value]


def evolve_locator_detector_issues(detectors):
    detector = detectors.get("governance.evolve_repo_locator_evidence_adapter")
    argv = [
        "python3", "harness/evolve_repo_locator_evidence_adapter_check.py",
        "repo_root", "evolve_repo_locator_evidence_request", "evidence_record",
    ]
    if not isinstance(detector, dict):
        return ["Evolve locator Evidence adapter detector is missing"]
    issues = []
    implementation = detector.get("implementation")
    if not isinstance(implementation, dict) or implementation.get("argv") != argv:
        issues.append("Evolve locator Evidence adapter detector requires exact arguments")
    if detector != EVOLVE_LOCATOR_EVIDENCE_DETECTOR:
        issues.append("Evolve locator Evidence adapter detector requires exact shadow binding")
    return issues
