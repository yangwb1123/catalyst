"""Strict Evolve repository locator to shadow EvidenceRecord adapter."""

from .adapter import (adapt_request, check_projection_bytes,
                      compute_locator_digest, compute_request_digest,
                      compute_source_digest, decode_evidence_record,
                      validate_evidence_record, validate_observation,
                      validate_projection, validate_request)
from .codec import (canonical_json, canonical_locator, canonical_observation,
                    decode_request)
from .constants import SUCCESS
from .fixture import validate_golden_fixture

__all__ = [
    "SUCCESS", "adapt_request", "canonical_json", "canonical_locator",
    "canonical_observation", "check_projection_bytes", "compute_locator_digest",
    "compute_request_digest", "compute_source_digest", "decode_evidence_record",
    "decode_request", "validate_evidence_record", "validate_golden_fixture",
    "validate_observation", "validate_projection", "validate_request",
]
