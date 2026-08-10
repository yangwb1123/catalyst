"""Frozen governance wiring for shipped shadow source adapters."""
import json

from governance_contract import ContractError, read_bounded_file


ARTIFACT_SKILL_MARKERS = (
    "### Artifact provenance adapter 分支",
    "python3 -B harness/artifact_evidence_adapter_check.py --golden <repo-root>",
    ("python3 -B harness/artifact_evidence_adapter_check.py <repo-root> "
     "<request.json> <evidence-record.json>"),
    ("ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect "
     "attestation)"),
    "不会认证 manifest/agent/model/collector",
    "不会读取当前文件",
    "不要读取 artifact path 的当前文件",
    "不会创建 Claim/CognitiveAtom，不会 append journal、写 SQLite 或产生 effect",
)
ARTIFACT_EVIDENCE_ADAPTER = {
    "api_version": "forgeos.governance.artifact-evidence-adapter/v1",
    "mode": "deterministic_artifact_provenance_to_evidence_shadow",
    "source_kind": "forgeos.artifact.v1",
    "output_kind": "EvidenceRecord",
    "source_fields": [
        "_format", "agent", "created_at", "model", "path", "phase",
        "prompt_sha256", "run_id", "sha256", "size", "workflow",
    ],
    "binding_fields": [
        "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope",
        "sensitivity", "sequence", "source_revision", "source_tree_sha256",
        "subjects", "supersedes_record_ids",
    ],
    "exact_request": {
        "canonical_required": True,
        "compact_utf8_required": True,
        "artifact_key_exception": "request.artifact._format",
        "legacy_empty_format": "reject",
        "reads_current_artifact": False,
    },
    "identity": {
        "source_digest_domain": "forgeos.governance.artifact-provenance-source.v1\0",
        "request_digest_domain":
            "forgeos.governance.artifact-evidence-adapter.request.v1\0",
        "evidence_record_digest_domain": "forgeos.governance.evidence-record.v1\0",
        "record_id_prefix": "artifact-evidence-",
        "snapshot_id_prefix": "artifact-snapshot-",
        "timestamp_projection": "floor_nonnegative_instant_to_unix_milliseconds",
    },
    "constants": {
        "principal_id": "forgeos.artifact-evidence-adapter",
        "principal_type": "tool",
        "authority_domain": "shadow",
        "role": "evidence-adapter",
        "collector_version": "v1",
        "evidence_type": "artifact",
        "directness": "direct",
        "source_trust": "observed",
        "content_role": "untrusted_data",
        "status": "valid",
    },
    "limits": {
        "max_request_bytes": 131072,
        "max_depth": 16,
        "max_object_fields": 64,
        "max_array_items": 256,
        "max_string_bytes": 16384,
        "integer_domain": "signed_int64",
    },
    "positive_result": (
        "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect "
        "attestation)"
    ),
    "attests": [],
    "persistence": "none",
}
ARTIFACT_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "artifact_key_exception": "request.artifact._format",
    "source_digest_domain": "forgeos.governance.artifact-provenance-source.v1\0",
    "request_digest_domain":
        "forgeos.governance.artifact-evidence-adapter.request.v1\0",
    "evidence_record_digest_domain": "forgeos.governance.evidence-record.v1\0",
    "timestamp_projection": "floor_nonnegative_instant_to_unix_milliseconds",
}
ARTIFACT_SCHEMA_LIMITS = ARTIFACT_EVIDENCE_ADAPTER["limits"]
ARTIFACT_SCHEMA_MAPPING = {
    "result": (
        "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect "
        "attestation)"
    ),
    "input_kind": "forgeos.artifact.v1",
    "output_kind": "EvidenceRecord",
    "record_id_prefix": "artifact-evidence-",
    "snapshot_id_prefix": "artifact-snapshot-",
    "principal_id": "forgeos.artifact-evidence-adapter",
    "principal_type": "tool",
    "authority_domain": "shadow",
    "role": "evidence-adapter",
    "collector_version": "v1",
    "evidence_type": "artifact",
    "directness": "direct",
    "source_trust": "observed",
    "content_role": "untrusted_data",
    "status": "valid",
    "persistence": "none",
    "attestations": [],
}
ARTIFACT_CANONICAL_REFS = {
    "artifact_evidence_adapter_schema":
        "docs/contracts/artifact-evidence-adapter-v1.schema.json",
    "artifact_evidence_adapter_golden_fixture":
        "docs/contracts/fixtures/artifact-evidence-adapter-v1.json",
    "artifact_evidence_adapter_checker": "harness/artifact_evidence_adapter_check.py",
    "artifact_evidence_adapter_decision":
        "docs/adr/0048-artifact-provenance-evidence-adapter-v1.md",
}
ARTIFACT_EVIDENCE_DETECTOR = {
    "id": "governance.artifact_evidence_adapter",
    "version": "1.0.0",
    "state": "shadow",
    "rule_refs": ["TRUTH-002"],
    "implementation": {
        "argv": [
            "python3", "harness/artifact_evidence_adapter_check.py", "repo_root",
            "artifact_evidence_request", "evidence_record",
        ],
        "cwd": "repo_root",
        "shell": False,
    },
    "invocation": {
        "owner": "operator",
        "adapter": "standalone.artifactEvidenceAdapter",
        "acceptance_criterion": None,
        "load_bearing": False,
    },
    "fail_closed": True,
    "tests": {
        "positive": {
            "path": "harness/test_artifact_evidence_adapter_check.py",
            "contains": "test_golden_fixture_is_adapted_shadow",
        },
        "negative": {
            "path": "harness/test_artifact_evidence_adapter_check.py",
            "contains": "test_projection_drift_is_rejected",
        },
    },
}

COMMAND_SUCCESS = (
    "ADAPTED_SHADOW (observation mapping only; no execution, pass, completion, "
    "truth, authority, claim, atom, persistence, or effect attestation)"
)
COMMAND_SKILL_MARKERS = (
    "### Command observation adapter 分支",
    ("python3 -B harness/command_observation_evidence_adapter_check.py --golden "
     "<repo-root>"),
    ("python3 -B harness/command_observation_evidence_adapter_check.py <repo-root> "
     "<request.json> <evidence-record.json>"),
    COMMAND_SUCCESS,
    "不会执行命令",
    "不会验证 stream preimage",
    "也不把 exit=0 当 PASS",
    "不会创建 Claim/Atom、不会 append journal",
)
COMMAND_OBSERVATION_EVIDENCE_ADAPTER = {
    "api_version": "forgeos.governance.command-observation-evidence-adapter/v1",
    "mode": "deterministic_command_observation_to_evidence_shadow",
    "source_kind": "forgeos.command-observation/v1",
    "output_kind": "EvidenceRecord",
    "source_fields": [
        "api_version", "canonicalization", "command", "ended_at_unix_ms",
        "evidence_type", "producer", "source", "started_at_unix_ms", "streams",
        "termination",
    ],
    "binding_fields": [
        "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope",
        "sensitivity", "sequence", "subjects", "supersedes_record_ids",
    ],
    "exact_request": {
        "canonical_required": True,
        "compact_utf8_required": True,
        "observation_anchor": "#CommandObservationV1",
        "observation_terminations": ["cancelled", "exited", "timed_out"],
        "projectable_termination": "exited",
        "executes_command": False,
        "reads_current_process": False,
        "digest_preimage_profile": "not_frozen_opaque_producer_declaration",
    },
    "identity": {
        "command_digest_domain":
            "forgeos.governance.command-observation.command.v1\0",
        "source_digest_domain":
            "forgeos.governance.command-observation-source.v1\0",
        "request_digest_domain":
            "forgeos.governance.command-observation-evidence-adapter.request.v1\0",
        "evidence_record_digest_domain": "forgeos.governance.evidence-record.v1\0",
        "record_id_prefix": "command-evidence-",
        "snapshot_id_prefix": "command-observation-",
        "principal_run_id_prefix": "command-adaptation-",
    },
    "constants": {
        "principal_id": "forgeos.command-observation-evidence-adapter",
        "principal_type": "tool",
        "authority_domain": "shadow",
        "role": "evidence-adapter",
        "collector_from": "producer_declaration",
        "collector_parameters": "command_sha256",
        "artifact_sha256": "source_snapshot_sha256",
        "evidence_types": ["gate_result", "test_run"],
        "directness": "direct",
        "source_trust": "observed",
        "content_role": "untrusted_data",
        "source_snapshot_type": "runtime",
        "status": "valid",
    },
    "limits": {
        "max_request_bytes": 131072,
        "max_depth": 16,
        "max_object_fields": 64,
        "max_array_items": 256,
        "max_argv_items": 64,
        "max_text_scalars": 4096,
        "max_string_bytes": 16384,
        "max_timeout_ms": 86400000,
        "integer_domain": "signed_int64",
        "exit_code_domain": "portable_nonnegative_signed_int32",
    },
    "positive_result": COMMAND_SUCCESS,
    "attests": [],
    "persistence": "none",
}
COMMAND_SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "command_digest_domain":
        "forgeos.governance.command-observation.command.v1\0",
    "source_digest_domain":
        "forgeos.governance.command-observation-source.v1\0",
    "request_digest_domain":
        "forgeos.governance.command-observation-evidence-adapter.request.v1\0",
    "evidence_record_digest_domain": "forgeos.governance.evidence-record.v1\0",
}
COMMAND_SCHEMA_LIMITS = COMMAND_OBSERVATION_EVIDENCE_ADAPTER["limits"]
COMMAND_SCHEMA_SEMANTIC_VALIDATION = {
    "schema_alone_sufficient": False,
    "observation_ref": "#CommandObservationV1",
    "observation_terminations": ["cancelled", "exited", "timed_out"],
    "request_projectable_terminations": ["exited"],
    "authoritative_checks": [
        "exact_compact_canonical_utf8", "bounded_unicode_and_utf8_bytes",
        "sorted_unique_identifier_arrays", "nondecreasing_time",
        "stream_count_and_digest_relations", "empty_stdin_and_stream_digests",
    ],
}
COMMAND_SCHEMA_MAPPING = {
    "result": COMMAND_SUCCESS,
    "input_kind": "forgeos.command-observation/v1",
    "output_kind": "EvidenceRecord",
    "projectable_termination": "exited",
    "record_id_prefix": "command-evidence-",
    "snapshot_id_prefix": "command-observation-",
    "principal_id": "forgeos.command-observation-evidence-adapter",
    "principal_type": "tool",
    "authority_domain": "shadow",
    "role": "evidence-adapter",
    "evidence_types": ["gate_result", "test_run"],
    "directness": "direct",
    "source_trust": "observed",
    "content_role": "untrusted_data",
    "source_snapshot_type": "runtime",
    "artifact_sha256": "source_snapshot_sha256",
    "principal_run_id": "command-adaptation-<request_sha256>",
    "status": "valid",
    "persistence": "none",
    "attestations": [],
}
COMMAND_CANONICAL_REFS = {
    "command_observation_evidence_adapter_schema":
        "docs/contracts/command-observation-evidence-adapter-v1.schema.json",
    "command_observation_evidence_adapter_golden_fixture":
        "docs/contracts/fixtures/command-observation-evidence-adapter-v1.json",
    "command_observation_evidence_adapter_checker":
        "harness/command_observation_evidence_adapter_check.py",
    "command_observation_evidence_adapter_decision":
        "docs/adr/0049-command-observation-evidence-adapter-v1.md",
}
COMMAND_EVIDENCE_DETECTOR = {
    "id": "governance.command_observation_evidence_adapter",
    "version": "1.0.0",
    "state": "shadow",
    "rule_refs": ["TRUTH-002"],
    "implementation": {
        "argv": [
            "python3", "harness/command_observation_evidence_adapter_check.py",
            "repo_root", "command_observation_evidence_request", "evidence_record",
        ],
        "cwd": "repo_root",
        "shell": False,
    },
    "invocation": {
        "owner": "operator",
        "adapter": "standalone.commandObservationEvidenceAdapter",
        "acceptance_criterion": None,
        "load_bearing": False,
    },
    "fail_closed": True,
    "tests": {
        "positive": {
            "path": "harness/test_command_observation_evidence_adapter_check.py",
            "contains": "test_golden_fixture_is_adapted_shadow",
        },
        "negative": {
            "path": "harness/test_command_observation_evidence_adapter_check.py",
            "contains": "test_projection_drift_is_rejected",
        },
    },
}


def artifact_skill_marker_issues(text, path):
    """Return the historical Skill error when Artifact guidance drifts."""
    if any(marker not in text for marker in ARTIFACT_SKILL_MARKERS):
        return [
            f"{path}: artifact Evidence adapter guidance or non-capability boundary is missing"
        ]
    return []


def artifact_adapter_registry_issues(data, path):
    """Validate the frozen Artifact adapter registry entry and references."""
    adapter = data.get("artifact_evidence_adapter") if isinstance(data, dict) else None
    issues = []
    if adapter != ARTIFACT_EVIDENCE_ADAPTER:
        issues.append(f"{path}: artifact_evidence_adapter contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in ARTIFACT_CANONICAL_REFS.items():
            if refs.get(field) != expected:
                issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    return issues


def artifact_adapter_schema_issues(repo_root):
    """Validate Artifact adapter Schema extension fields without changing errors."""
    relative = "docs/contracts/artifact-evidence-adapter-v1.schema.json"
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate artifact Evidence adapter Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": ARTIFACT_SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": ARTIFACT_SCHEMA_LIMITS,
        "x-forgeos-mapping": ARTIFACT_SCHEMA_MAPPING,
    }
    return [f"{path}: {field} drifted" for field, value in expected.items()
            if schema.get(field) != value]


def artifact_detector_issues(detectors):
    """Validate the Artifact detector while preserving historical issue text."""
    artifact = detectors.get("governance.artifact_evidence_adapter")
    artifact_argv = [
        "python3", "harness/artifact_evidence_adapter_check.py", "repo_root",
        "artifact_evidence_request", "evidence_record",
    ]
    if not isinstance(artifact, dict):
        return ["artifact Evidence adapter detector is missing"]
    issues = []
    implementation = artifact.get("implementation")
    if not isinstance(implementation, dict) or implementation.get("argv") != artifact_argv:
        issues.append("artifact Evidence adapter detector requires exact adapter arguments")
    if artifact != ARTIFACT_EVIDENCE_DETECTOR:
        issues.append("artifact Evidence adapter detector requires the exact shadow binding")
    return issues


def command_skill_marker_issues(text, path):
    """Require exact command-observation guidance and non-capabilities."""
    if any(marker not in text for marker in COMMAND_SKILL_MARKERS):
        return [
            f"{path}: command observation Evidence adapter guidance or "
            "non-capability boundary is missing"
        ]
    return []


def command_adapter_registry_issues(data, path):
    """Validate the frozen command-observation adapter and references."""
    key = "command_observation_evidence_adapter"
    adapter = data.get(key) if isinstance(data, dict) else None
    issues = []
    if adapter != COMMAND_OBSERVATION_EVIDENCE_ADAPTER:
        issues.append(f"{path}: {key} contract drifted")
    refs = data.get("canonical_refs") if isinstance(data, dict) else None
    if not isinstance(refs, dict):
        issues.append(f"{path}: canonical_refs must be a mapping")
    else:
        for field, expected in COMMAND_CANONICAL_REFS.items():
            if refs.get(field) != expected:
                issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    return issues


def command_adapter_schema_issues(repo_root):
    """Validate command adapter Schema extensions and semantic-validator notice."""
    relative = "docs/contracts/command-observation-evidence-adapter-v1.schema.json"
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate command observation adapter Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": COMMAND_SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": COMMAND_SCHEMA_LIMITS,
        "x-forgeos-semantic-validation": COMMAND_SCHEMA_SEMANTIC_VALIDATION,
        "x-forgeos-mapping": COMMAND_SCHEMA_MAPPING,
    }
    return [f"{path}: {field} drifted" for field, value in expected.items()
            if schema.get(field) != value]


def command_detector_issues(detectors):
    """Keep the command adapter standalone, shadow and non-load-bearing."""
    command = detectors.get("governance.command_observation_evidence_adapter")
    argv = [
        "python3", "harness/command_observation_evidence_adapter_check.py",
        "repo_root", "command_observation_evidence_request", "evidence_record",
    ]
    if not isinstance(command, dict):
        return ["command observation Evidence adapter detector is missing"]
    issues = []
    implementation = command.get("implementation")
    if not isinstance(implementation, dict) or implementation.get("argv") != argv:
        issues.append(
            "command observation Evidence adapter detector requires exact adapter arguments"
        )
    if command != COMMAND_EVIDENCE_DETECTOR:
        issues.append(
            "command observation Evidence adapter detector requires the exact shadow binding"
        )
    return issues
