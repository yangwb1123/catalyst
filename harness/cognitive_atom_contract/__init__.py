"""Public API for deterministic, non-authoritative CognitiveAtom projection."""

from .fixture import validate_golden_fixture
from .projection import (canonical_atom_payload, check_projection_bytes,
                         compute_atom_digest, compute_atom_id,
                         compute_atom_set_digest, decode_atom_set,
                         project_atom_set, project_atom_set_bytes,
                         project_record_set, source_closure, validate_atom_set,
                         validate_projection)

SUCCESS = ("PROJECTED_SHADOW (structural projection only; no truth attestation; "
           "no authority attestation; no instruction attestation; no hard-guard attestation; "
           "no transition attestation; no completion attestation; no effect attestation)")

__all__ = [
    "SUCCESS", "canonical_atom_payload", "check_projection_bytes",
    "compute_atom_digest", "compute_atom_id", "compute_atom_set_digest",
    "decode_atom_set", "project_atom_set", "project_atom_set_bytes",
    "project_record_set", "source_closure", "validate_atom_set",
    "validate_golden_fixture", "validate_projection",
]
