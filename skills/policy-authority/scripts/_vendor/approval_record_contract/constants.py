"""Frozen identifiers and resource ceilings for ApprovalRecord v1."""

APPROVAL_API = "forgeos.approval-record/v1"
REQUEST_API = "forgeos.approval-record-declared-assessment-request/v1"
ASSESSMENT_API = "forgeos.approval-record-declared-assessment/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
KIND = "ApprovalRecord"
MODE = "authority_neutral_declared_approval_only"

APPROVAL_DOMAIN = b"forgeos.approval-record.v1\0"
TARGET_DOMAIN = b"forgeos.approval-declared-target.v1\0"
REQUEST_DOMAIN = b"forgeos.approval-record-declared-assessment-request.v1\0"
ASSESSMENT_DOMAIN = b"forgeos.approval-record-declared-assessment.v1\0"

EFFECT_VOCABULARY_SHA256 = (
    "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"
)
EFFECTS = (
    "approval.decide", "approval.request", "knowledge.apply", "knowledge.propose",
    "migration.apply", "migration.generate", "network.read", "network.write",
    "placement.plan", "policy.propose", "policy.write", "process.exec",
    "release.execute", "release.plan", "repo.read", "repo.write", "secrets.read",
    "target.execute", "target.inventory", "target.probe", "target.reserve",
)

DISTINCTIONS = (
    "approver_not_implementer", "approver_not_requester", "approver_not_subject",
)
RESULT = (
    "ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority authentication, "
    "attestation or SoD proof verification, condition or RiskAcceptance validation, "
    "revocation evaluation, policy decision, effective approval, authorization, "
    "permission, persistence, transition, execution, or effect attestation)"
)

RELATION_REASONS = {
    "approver_mismatch": "approver_mismatch",
    "authority_binding_mismatch": "authority_binding_mismatch",
    "binding_mismatch": "binding_mismatch",
    "conditions_mismatch": "conditions_mismatch",
    "declared_revocation_time_reached": "declared_revocation_time_reached",
    "decision_mismatch": "decision_mismatch",
    "risk_acceptance_mismatch": "risk_acceptance_mismatch",
    "scope_mismatch": "scope_mismatch",
    "separation_of_duty_mismatch": "separation_of_duty_mismatch",
    "subject_mismatch": "subject_mismatch",
    "outside_declared_window": "temporal_window_mismatch",
}
POSITIVE_RELATIONS = {
    "approver": "same_declared_approver",
    "authority_binding": "same_declared_authority_binding",
    "binding": "same_declared_binding",
    "conditions": "same_declared_conditions",
    "decision": "same_declared_decision",
    "revocation": "declared_revocation_time_not_reached",
    "risk_acceptance": "same_declared_risk_acceptance_refs",
    "scope": "same_declared_scope",
    "separation_of_duty": "same_declared_separation_of_duty",
    "subject": "same_declared_subject",
    "temporal": "inside_declared_window",
}

MAX_RECORD_BYTES = 1_048_576
MAX_TARGET_BYTES = 1_048_576
MAX_REQUEST_BYTES = 2_097_152
MAX_ASSESSMENT_BYTES = 262_144
MAX_GOLDEN_BYTES = 4_194_304
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY = 256
MAX_STRING_BYTES = 16_384
MAX_SHORT_BYTES = 160
MAX_PROOF_BYTES = 16_384
MAX_TTL_MS = 86_400_000

