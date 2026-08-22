"""Frozen vocabulary for the Kernel decision reference core v1."""

from __future__ import annotations

import re

CANONICALIZATION = "forgeos.canonical-json/v1"

ATOM_API = "forgeos.aadm.cognitive-atom/v2"
ATOM_KIND = "CognitiveAtom"
ATOM_DOMAIN = b"forgeos.aadm.cognitive-atom.v2\0"
ATOM_PREFIX = "cognitive-atom-"

TRANSACTION_API = "forgeos.aadm.decision-transaction/v1"
TRANSACTION_KIND = "DecisionTransaction"
TRANSACTION_DOMAIN = b"forgeos.aadm.decision-transaction.v1\0"
TRANSACTION_PREFIX = "decision-transaction-"
TRANSACTION_MODE = "structural_proposal_only"

CLOSURE_API = "forgeos.kernel-decision-reference-closure/v1"
CLOSURE_KIND = "KernelDecisionReferenceClosure"
CLOSURE_DOMAIN = b"forgeos.kernel-decision-reference-closure.v1\0"
CLOSURE_PREFIX = "kernel-decision-reference-closure-"

SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_KERNEL_DECISION_REFERENCE_CLOSURE_V1 "
    "(exact caller-supplied cognitive, transaction and operational reference relations "
    "only; declared authority and hardness are ineffective; all twenty-two attestations "
    "are false: no Approval, principal, Grant or binding authentication; no source "
    "resolution, authority, authorization, CAS, completion, content provenance, effect, "
    "event append, execution, hard guard, instruction, outcome, permission, persistence, "
    "transition, truth, usage measurement or verifier independence attestation)"
)

MAX_ATOM_BYTES = 131_072
MAX_ATOM_SET_BYTES = 1_048_576
MAX_TRANSACTION_BYTES = 1_048_576
MAX_CLOSURE_BYTES = 20_971_520
MAX_ATOMS = 256
MAX_OPTIONS = 16
MAX_ATOM_REFS = 64
MAX_IO_ITEMS = 32
MAX_PROOFS = 32
MAX_EVIDENCE_KINDS = 16
MAX_SELECTOR_BYTES = 4_096
MAX_TIMEOUT_MS = 86_400_000
MAX_CONFIDENCE_MICROS = 1_000_000

HASH_RE = re.compile(r"[0-9a-f]{64}\Z")
ATOM_ID_RE = re.compile(r"cognitive-atom-[0-9a-f]{64}\Z")
LEGACY_ATOM_ID_RE = re.compile(r"atom-[0-9a-f]{64}\Z")
TRANSACTION_ID_RE = re.compile(r"decision-transaction-[0-9a-f]{64}\Z")
WORK_INTENT_ID_RE = re.compile(r"work-intent-[0-9a-f]{64}\Z")
ADR_ID_RE = re.compile(r"ADR-[0-9]{4,}\Z")
JSON_POINTER_RE = re.compile(r"(?:/(?:[^~/]|~[01])*)+\Z")

ATOM_FIELDS = {
    "api_version", "atom_id", "atom_sha256", "atom_type", "attestations",
    "bindings", "canonicalization", "confidence_micros", "declared_authority",
    "declared_hardness", "effective_hardness", "epistemic_state",
    "instruction_allowed", "kind", "proposition", "scope", "source",
    "task_binding", "validity",
}
SOURCE_FIELDS = {"source_kind", "source_phase", "source_ref", "source_selector"}
AUTHORITY_FIELDS = {"authority_kind", "authority_ref"}
PROPOSITION_FIELDS = {"object_type", "object_value", "predicate", "subject"}
VALIDITY_FIELDS = {"valid_from_unix_ms", "valid_until_unix_ms"}
SCOPE_FIELDS = {"module", "object", "project"}
ATOM_REF_FIELDS = {"atom_id", "atom_sha256"}
LEGACY_ATOM_REF_FIELDS = {"atom_id", "canonical_sha256"}
EVIDENCE_REF_FIELDS = {"canonical_sha256", "record_id"}
WORK_INTENT_REF_FIELDS = {"work_intent_id", "work_intent_sha256"}
ADR_REF_FIELDS = {"adr_id", "adr_self_sha256"}
APPROVAL_REF_FIELDS = {"approval_id", "approval_sha256", "authority_domain"}

TRANSACTION_FIELDS = {
    "accountable_owner", "actor", "api_version", "attestations", "bindings",
    "budget", "canonicalization", "completion_condition", "compensation",
    "created_at_unix_ms", "decision_transaction_id", "decision_transaction_sha256",
    "goal_atom_ref", "guard_atom_refs", "idempotency_key", "kind", "options",
    "proof_obligations", "read_artifact_receipt_refs", "selected_option_id",
    "selection_basis_sha256", "task_binding", "transaction_mode", "trigger_atom_refs",
    "verifier", "write_preconditions", "write_slots",
}
OPTION_FIELDS = {"capability", "option_id", "requested_action_sha256"}
CONDITION_FIELDS = {"condition_ref", "condition_sha256"}
COMPENSATION_FIELDS = {"applicability", "capability", "requested_action_sha256"}
PROOF_FIELDS = {"obligation_id", "predicate_sha256", "required_evidence_kinds"}
VERIFIER_FIELDS = {"capability", "independence_basis_sha256", "principal", "timeout_ms"}
PRECONDITION_FIELDS = {"expected_sha256", "precondition_id", "resource_ref"}
BUDGET_FIELDS = {
    "max_calls", "max_cost_usd_micros", "max_input_tokens", "max_network_bytes",
    "max_output_bytes", "max_output_tokens", "timeout_ms",
}
CLOSURE_FIELDS = {
    "api_version", "attestations", "canonicalization", "closure_id",
    "closure_sha256", "cognitive_atoms", "decision_transaction", "kind",
    "operational_closure", "result",
}

ATTESTATION_FIELDS = {
    "approval_authentication_attestation", "authority_attestation",
    "authorization_attestation", "binding_authentication_attestation",
    "cas_attestation", "completion_attestation", "content_provenance_attestation",
    "effect_attestation", "event_append_attestation", "execution_attestation",
    "grant_authentication_attestation", "hard_guard_attestation",
    "instruction_attestation", "outcome_attestation", "permission_attestation",
    "persistence_attestation", "principal_authentication_attestation",
    "source_resolution_attestation", "transition_attestation", "truth_attestation",
    "usage_measurement_attestation", "verifier_independence_attestation",
}

ATOM_TYPES = {
    "acceptance", "actor", "assumption", "constraint", "decision", "evidence",
    "fact", "goal", "hypothesis", "inference", "object", "observation",
    "operation", "preference", "risk", "unknown",
}
LEGACY_TYPES = {
    "assumption", "constraint", "decision", "fact", "hypothesis", "inference",
    "unknown",
}
LEGACY_STATES = {
    "assumption": {"open", "testing"}, "constraint": {"candidate"},
    "decision": {"proposed"}, "fact": {"candidate", "contested"},
    "hypothesis": {"open", "testing"}, "inference": {"candidate"},
    "unknown": {"investigating", "open"},
}
SOURCE_TYPES = {
    "artifact": ATOM_TYPES,
    "artifact_receipt": {"evidence", "object", "observation"},
    "capability_invocation": {"actor", "operation"},
    "cognitive_atom_v1": LEGACY_TYPES,
    "evidence_record": {"evidence", "observation"},
    "execution_receipt": {"actor", "evidence", "observation"},
    "interaction_event": {"actor", "evidence", "object", "observation", "operation"},
    "work_intent": {"acceptance", "constraint", "goal", "preference", "risk", "unknown"},
}
PREDECISION_SOURCES = {"artifact", "cognitive_atom_v1", "evidence_record", "work_intent"}
POSTDECISION_SOURCES = set(SOURCE_TYPES) - PREDECISION_SOURCES

HARDNESS_BY_TYPE = {
    "acceptance": {"advisory", "preferred", "required"},
    "actor": {"none"}, "assumption": {"advisory", "none"},
    "constraint": {"advisory", "contract", "invariant", "preferred", "required"},
    "decision": {"advisory", "required"}, "evidence": {"none"},
    "fact": {"none"}, "goal": {"advisory", "preferred", "required"},
    "hypothesis": {"advisory", "none"}, "inference": {"advisory", "none"},
    "object": {"none"}, "observation": {"none"}, "operation": {"none"},
    "preference": {"advisory", "preferred"}, "risk": {"advisory", "none"},
    "unknown": {"advisory", "none"},
}
AUTHORITY_KINDS = {"approval_record", "architecture_decision", "contract_artifact", "none"}
OBJECT_TYPES = {"artifact_ref", "boolean", "integer", "null", "string"}
