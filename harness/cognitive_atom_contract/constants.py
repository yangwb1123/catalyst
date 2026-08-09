"""Frozen constants for the authority-free CognitiveAtom v1 projection."""

import re

from governance_contract.constants import (HASH_RE, ID_RE, MAX_DEPTH, MAX_FIELDS,
                                           MAX_I64, MAX_ITEMS, MAX_RECORD_BYTES,
                                           MAX_SET_BYTES, MAX_STRING_BYTES, MIN_I64,
                                           SHADOW_STATES)

API_VERSION = "forgeos.aadm.cognitive-atom/v1"
KIND = "CognitiveAtom"
CANONICALIZATION = "forgeos.canonical-json/v1"
ATOM_DOMAIN = b"forgeos.aadm.cognitive-atom.v1\0"
ATOM_ID_DOMAIN = b"forgeos.aadm.cognitive-atom-id.v1\0"
ATOM_SET_DOMAIN = b"forgeos.aadm.cognitive-atom-set.v1\0"
SOURCE_SET_DOMAIN = b"forgeos.governance.record-set.v1\0"
MAX_ATOM_BYTES = MAX_RECORD_BYTES
MAX_ATOM_SET_BYTES = MAX_SET_BYTES
MAX_ATOMS = MAX_ITEMS
ATOM_TYPES = frozenset({
    "assumption", "constraint", "decision", "fact", "hypothesis", "inference", "unknown",
})
ATOM_ID_RE = re.compile(r"atom-[a-f0-9]{64}\Z")

TOP_FIELDS = {"api_version", "integrity", "kind", "metadata", "source", "spec"}
INTEGRITY_FIELDS = {"canonical_sha256", "canonicalization"}
METADATA_FIELDS = {
    "atom_id", "context_sha256", "policy_sha256", "project_id", "scope",
    "source_revision", "source_tree_sha256", "task_id",
}
SOURCE_FIELDS = {
    "canonical_sha256", "claim_aggregate_id", "claim_record_id", "claim_sequence",
    "closure_byte_count", "closure_record_count", "closure_sha256", "record_kind",
}
SPEC_FIELDS = {
    "atom_type", "authority_ref", "contradicting_evidence_record_ids",
    "derived_from_claim_record_ids", "epistemic_state", "hardness",
    "instruction_allowed", "projection_confidence_micros", "projection_mode",
    "proposition", "supporting_evidence_record_ids", "validity",
}
PROPOSITION_FIELDS = {"object_type", "object_value", "predicate", "subject"}
VALIDITY_FIELDS = {"valid_from_unix_ms", "valid_until_unix_ms"}
