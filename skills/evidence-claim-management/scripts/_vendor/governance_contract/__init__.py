"""Public, pure Governance Evidence/Claim v1 shadow validation API."""

from .codec import (ContractError, canonical_json, canonical_record_payload,
                    compute_record_digest, decode_record_set, read_bounded_file)
from .fixture import validate_golden_fixture
from .record_set import check_record_set_bytes, validate_record_set

SUCCESS = "STRUCTURALLY_VALID (shadow; no truth or authority attestation)"

__all__ = [
    "SUCCESS", "ContractError", "canonical_json", "canonical_record_payload",
    "check_record_set_bytes", "compute_record_digest", "decode_record_set",
    "read_bounded_file",
    "validate_golden_fixture", "validate_record_set",
]
