"""Frozen identifiers and resource bounds for KnowledgeUpdateProposal v1."""

PROPOSAL_API = "forgeos.knowledge-update-proposal/v1"
REQUEST_API = "forgeos.knowledge-update-proposal-declared-assessment-request/v1"
ASSESSMENT_API = "forgeos.knowledge-update-proposal-declared-assessment/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
PROPOSAL_KIND = "KnowledgeUpdateProposal"
MODE = "authority_neutral_declared_knowledge_update_only"

RECORD_SET_DOMAIN = b"forgeos.governance.record-set.v1\0"
PROPOSAL_DOMAIN = b"forgeos.knowledge-update-proposal.v1\0"
TARGET_DOMAIN = b"forgeos.knowledge-update-declared-target.v1\0"
REQUEST_DOMAIN = b"forgeos.knowledge-update-proposal-declared-assessment-request.v1\0"
ASSESSMENT_DOMAIN = b"forgeos.knowledge-update-proposal-declared-assessment.v1\0"

RESULT = (
    "ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no proposer, Grant, Context, "
    "evidence, current-knowledge, conflict, freshness, policy or authority "
    "evaluation; no truth, adoption, authorization, permission, persistence, "
    "apply, receipt, execution or effect attestation)"
)
GRANT_RESULT = (
    "ASSESSED_GRANT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no issuer, policy, Approval, "
    "revocation, usage, authorization, permission, persistence, apply, receipt or "
    "effect attestation)"
)
CONTEXT_RESULT = (
    "ASSESSED_CONTEXT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no source authentication, "
    "freshness, truth, instruction, permission, adoption, persistence, apply or "
    "effect attestation)"
)

MAX_PROPOSAL_BYTES = 2_097_152
MAX_TARGET_BYTES = 1_048_576
MAX_REQUEST_BYTES = 4_194_304
MAX_ASSESSMENT_BYTES = 262_144
MAX_GOLDEN_BYTES = 8_388_608
MAX_RECORD_BYTES = 131_072
MAX_RECORD_SET_BYTES = 1_048_576
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY = 256
MAX_STRING_BYTES = 16_384
MAX_SHORT_BYTES = 160
MAX_REFERENCE_BYTES = 4_096
MAX_RECORDS = 256
MAX_MUTATIONS = 64
MAX_ARTIFACTS = 32
MAX_MUTATION_REASONS = 16

STABLE_CLAIM_FIELDS = (
    "claim_type", "subject", "predicate", "object_type", "object_value", "owner",
)
SHADOW_TRANSITIONS = {
    "fact": {("candidate", "candidate"), ("candidate", "contested"),
             ("contested", "candidate"), ("contested", "contested")},
    "constraint": {("candidate", "candidate")},
    "decision": {("proposed", "proposed")},
    "inference": {("candidate", "candidate")},
    "assumption": {("open", "open"), ("open", "testing"),
                   ("testing", "testing")},
    "hypothesis": {("open", "open"), ("open", "testing"),
                   ("testing", "testing")},
    "lesson": {("candidate", "candidate")},
    "proposal": {("draft", "draft"), ("draft", "submitted"),
                 ("submitted", "submitted")},
    "unknown": {("open", "open"), ("open", "investigating"),
                ("investigating", "investigating")},
}
