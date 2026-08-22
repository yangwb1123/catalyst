"""Frozen constants for ADR-0056."""

CANONICALIZATION = "forgeos.canonical-json/v1"
VOCABULARY_API = "forgeos.governance.effect-vocabulary/v1"
GRANT_API = "forgeos.capability-grant/v1"
REQUEST_API = "forgeos.capability-grant-declared-assessment-request/v1"
ASSESSMENT_API = "forgeos.capability-grant-declared-assessment/v1"
VOCABULARY_KIND = "EffectVocabulary"
GRANT_KIND = "CapabilityGrant"
MODE = "authority_neutral_declared_envelope_only"
RESULT = (
    "ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, "
    "approval, revocation, usage, preflight, authorization, permission, "
    "persistence, execution, or effect attestation)"
)

VOCABULARY_DOMAIN = b"forgeos.governance.effect-vocabulary.v1\0"
GRANT_DOMAIN = b"forgeos.capability-grant.v1\0"
ACTION_DOMAIN = b"forgeos.capability-requested-action.v1\0"
REQUEST_DOMAIN = b"forgeos.capability-grant-declared-assessment-request.v1\0"
ASSESSMENT_DOMAIN = b"forgeos.capability-grant-declared-assessment.v1\0"
VOCABULARY_SHA256 = "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"

MAX_VOCABULARY_BYTES = 128 * 1024
MAX_GRANT_BYTES = 1024 * 1024
MAX_REQUEST_BYTES = 2 * 1024 * 1024
MAX_ASSESSMENT_BYTES = 256 * 1024
MAX_GOLDEN_BYTES = 4 * 1024 * 1024
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY = 256
MAX_STRING_BYTES = 16_384
MAX_TTL_MS = 86_400_000
MAX_USAGE = 1_000_000_000
MAX_BYTES = 1_073_741_824
MAX_COST_MICROS = 1_000_000_000_000_000
MAX_TIMEOUT_MS = 86_400_000

EFFECTS = (
    "approval.decide", "approval.request", "knowledge.apply", "knowledge.propose",
    "migration.apply", "migration.generate", "network.read", "network.write",
    "placement.plan", "policy.propose", "policy.write", "process.exec",
    "release.execute", "release.plan", "repo.read", "repo.write", "secrets.read",
    "target.execute", "target.inventory", "target.probe", "target.reserve",
)

# allowed kinds, required kinds, production restriction, scope profile
EFFECT_SPECS = {
    "approval.decide": (("governance_object",), ("governance_object",), "policy_controlled_default_deny", "approval_object"),
    "approval.request": (("governance_object",), ("governance_object",), "policy_controlled_default_deny", "approval_object"),
    "knowledge.apply": (("governance_object",), ("governance_object",), "policy_controlled_default_deny", "knowledge_object"),
    "knowledge.propose": (("governance_object",), ("governance_object",), "policy_controlled_default_deny", "knowledge_object"),
    "migration.apply": (("artifact", "environment"), ("artifact", "environment"), "external_operator_only", "artifact_environment"),
    "migration.generate": (("environment", "repo_path"), ("repo_path",), "policy_controlled_default_deny", "repo_emit_optional_environment"),
    "network.read": (("network_origin",), ("network_origin",), "policy_controlled_default_deny", "network_origin"),
    "network.write": (("network_origin",), ("network_origin",), "policy_controlled_default_deny", "network_origin"),
    "placement.plan": (("target_query",), ("target_query",), "policy_controlled_default_deny", "target_query"),
    "policy.propose": (("governance_object",), ("governance_object",), "policy_controlled_default_deny", "policy_object"),
    "policy.write": (("governance_object",), ("governance_object",), "policy_controlled_default_deny", "policy_object"),
    "process.exec": (("command",), ("command",), "policy_controlled_default_deny", "command"),
    "release.execute": (("artifact", "environment"), ("artifact", "environment"), "external_operator_only", "artifact_environment"),
    "release.plan": (("environment", "repo_path"), ("environment", "repo_path"), "policy_controlled_default_deny", "environment_repo_emit"),
    "repo.read": (("repo_path",), ("repo_path",), "policy_controlled_default_deny", "repo_read"),
    "repo.write": (("repo_path",), ("repo_path",), "policy_controlled_default_deny", "repo_write_exact"),
    "secrets.read": (("secret_ref",), ("secret_ref",), "policy_controlled_default_deny", "secret_ref"),
    "target.execute": (("target",), ("target",), "policy_controlled_default_deny", "target"),
    "target.inventory": (("target_query",), ("target_query",), "policy_controlled_default_deny", "target_query"),
    "target.probe": (("target",), ("target",), "policy_controlled_default_deny", "target"),
    "target.reserve": (("target",), ("target",), "policy_controlled_default_deny", "target"),
}

POSITIVE_RELATIONS = {
    "binding": "same_declared_binding",
    "budget": "at_or_below_declared_ceiling",
    "capability": "same_declared_capability",
    "effect": "same_declared_effect",
    "scope": "covered_by_declaration",
    "subject": "same_declared_subject",
    "task": "same_declared_task",
    "temporal": "inside_declared_window",
}

RELATION_REASONS = {
    "binding_mismatch": "binding_mismatch",
    "exceeds_declared_ceiling": "budget_exceeded",
    "capability_mismatch": "capability_mismatch",
    "effect_mismatch": "effect_mismatch",
    "denied_by_declaration": "deny_matched",
    "outside_declared_scope": "scope_not_covered",
    "subject_mismatch": "subject_mismatch",
    "task_mismatch": "task_mismatch",
    "outside_declared_window": "temporal_window_mismatch",
}
