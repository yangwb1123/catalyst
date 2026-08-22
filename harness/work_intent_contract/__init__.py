"""Authority-neutral WorkIntent v1 Proposed candidate semantic core."""

from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import SUCCESS_MARKER
from .fixture import golden_bytes, golden_fixture, load_golden
from .record import (canonical_work_intent, decode_work_intent, seal_work_intent,
                     validate_work_intent, work_intent_digest)

__all__ = [
    "ContractError", "SUCCESS_MARKER", "canonical_json", "canonical_work_intent",
    "decode_canonical_json", "decode_work_intent", "golden_bytes", "golden_fixture",
    "load_golden", "seal_work_intent", "validate_work_intent", "work_intent_digest",
]
