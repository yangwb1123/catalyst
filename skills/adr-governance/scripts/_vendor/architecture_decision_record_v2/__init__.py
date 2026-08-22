"""Public API for the proposed-only ArchitectureDecisionRecord v2 checker."""

from governance_contract import ContractError

from .codec import body_digest, canonical_json, decode_canonical, self_digest
from .constants import GOLDEN, SUCCESS
from .document import validate_document_bytes, validate_document_file
from .fixture import validate_golden

__all__ = [
    "ContractError", "GOLDEN", "SUCCESS", "body_digest", "canonical_json",
    "decode_canonical", "self_digest", "validate_document_bytes",
    "validate_document_file", "validate_golden",
]
