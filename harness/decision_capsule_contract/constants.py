"""Frozen vocabulary for Decision Capsule structural replay core v1."""

from __future__ import annotations

CANONICALIZATION = "forgeos.canonical-json/v1"

MANIFEST_API = "forgeos.aadm.structural-replay-manifest/v1"
MANIFEST_KIND = "StructuralReplayManifest"
MANIFEST_DOMAIN = b"forgeos.aadm.structural-replay-manifest.v1\0"
MANIFEST_PREFIX = "structural-replay-manifest-"
MANIFEST_MODE = "structural_validate_reseal_compare_only"

CAPSULE_API = "forgeos.aadm.decision-capsule/v1"
CAPSULE_KIND = "DecisionCapsule"
CAPSULE_DOMAIN = b"forgeos.aadm.decision-capsule.v1\0"
CAPSULE_PREFIX = "decision-capsule-"
CAPSULE_MODE = "structural_replay_manifest_only"

BRANCH_API = "forgeos.aadm.evaluation-branch/v1"
BRANCH_KIND = "EvaluationBranch"
BRANCH_DOMAIN = b"forgeos.aadm.evaluation-branch.v1\0"
BRANCH_PREFIX = "evaluation-branch-"
BRANCH_MODE = "structural_validate_reseal_compare_only"
COMPARISON_RESULT = "EXACT_STRUCTURAL_REFERENCE_MATCH_ONLY"

CLOSURE_API = "forgeos.aadm.structural-replay-closure/v1"
CLOSURE_KIND = "StructuralReplayClosure"
CLOSURE_DOMAIN = b"forgeos.aadm.structural-replay-closure.v1\0"
CLOSURE_PREFIX = "structural-replay-closure-"

CAPSULE_RESULT = (
    "STRUCTURALLY_VALID_DECISION_CAPSULE_V1 (exact caller-supplied ADR-0090 "
    "closure and complete projection of the embedded caller-supplied closure only; "
    "replay is validate/reseal/compare only; no effect replay or history rewrite; "
    "all thirty-two replay attestations are false)"
)
SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_DECISION_CAPSULE_REPLAY_CLOSURE_V1 (exact caller-supplied "
    "DecisionCapsule and separately sealed deterministic structural comparison only; "
    "dedicated ReflectionReport ArtifactRefs are unresolved and attached only by the "
    "outer closure; upstream ArtifactRefs remain opaque and uninterpreted; no model, "
    "rule or world-state evaluation, effect replay, history rewrite, authorization, "
    "persistence, PDP or controller; all thirty-two replay attestations are false)"
)

MAX_MANIFEST_BYTES = 4_194_304
MAX_CAPSULE_BYTES = 27_262_976
MAX_BRANCH_BYTES = 65_536
MAX_CLOSURE_BYTES = 29_360_128
MAX_ATOMS = 256
MAX_ARTIFACTS = 256
MAX_ARTIFACT_RECEIPTS = 64
MAX_INVOCATIONS = 64
MAX_EVENTS = 256
MAX_EXECUTION_RECEIPTS = 64
MAX_REFLECTION_REPORT_REFS = 32

MANIFEST_FIELDS = {
    "api_version", "artifact_receipt_refs", "artifact_refs", "attestations",
    "canonicalization", "capability_invocation_refs", "decision_closure_ref",
    "decision_transaction_ref", "effect_replay_allowed", "execution_receipt_refs",
    "history_rewrite_allowed", "interaction_event_refs", "kind", "manifest_id",
    "manifest_sha256", "operational_closure_ref", "postdecision_atom_refs",
    "predecision_atom_refs", "replay_mode",
}
CAPSULE_FIELDS = {
    "api_version", "attestations", "canonicalization", "capsule_id",
    "capsule_mode", "capsule_sha256", "decision_closure", "kind",
    "replay_manifest", "result",
}
BRANCH_FIELDS = {
    "api_version", "attestations", "branch_id", "branch_mode", "branch_sha256",
    "canonicalization", "capsule_ref", "comparison_result", "decision_closure_ref",
    "effect_replay_allowed", "history_rewrite_allowed", "kind", "manifest_ref",
}
CLOSURE_FIELDS = {
    "api_version", "attestations", "canonicalization", "closure_id",
    "closure_sha256", "decision_capsule", "evaluation_branch", "kind",
    "reflection_report_artifact_refs", "result",
}

DECISION_CLOSURE_REF_FIELDS = {"closure_id", "closure_sha256"}
TRANSACTION_REF_FIELDS = {
    "decision_transaction_id", "decision_transaction_sha256",
}
OPERATIONAL_CLOSURE_REF_FIELDS = {"closure_id", "closure_sha256"}
MANIFEST_REF_FIELDS = {"manifest_id", "manifest_sha256"}
CAPSULE_REF_FIELDS = {"capsule_id", "capsule_sha256"}

ATTESTATION_FIELDS = {
    "approval_authentication_attestation",
    "attempt_history_completeness_attestation",
    "authority_attestation",
    "authorization_attestation",
    "binding_authentication_attestation",
    "capsule_completeness_attestation",
    "cas_attestation",
    "completion_attestation",
    "content_provenance_attestation",
    "effect_attestation",
    "evaluation_execution_attestation",
    "evaluator_independence_attestation",
    "event_append_attestation",
    "execution_attestation",
    "external_history_resolution_attestation",
    "grant_authentication_attestation",
    "hard_guard_attestation",
    "instruction_attestation",
    "outcome_attestation",
    "permission_attestation",
    "persistence_attestation",
    "principal_authentication_attestation",
    "reflection_completeness_attestation",
    "replay_equivalence_attestation",
    "result_authentication_attestation",
    "rule_evaluation_attestation",
    "source_resolution_attestation",
    "transition_attestation",
    "truth_attestation",
    "usage_measurement_attestation",
    "verifier_independence_attestation",
    "world_state_resolution_attestation",
}

FIXTURE_PATH = "docs/contracts/fixtures/decision-capsule-structural-replay-v1.json"
