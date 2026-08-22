"""Public pure structural API for authenticated ADR lifecycle v1."""

from .canonical import ContractError, bounded_canonical_json, signature_message
from .constants import SUCCESS_MARKER
from .contract import decode_document, validate_document
from .documents import (acceptance_sha256, entry_sha256, record_key_sha256,
                        request_sha256, supersession_sha256)
from .fixture import golden_bytes, golden_fixture, load_golden
from .ledger import (current_head_set_sha256, ledger_sha256,
                     rebuild_materialized_view, view_sha256)
from .prerequisite import prerequisite_sha256
from .proposal import derive_proposal_binding
from .state import state_sha256

__all__ = [
    "ContractError", "SUCCESS_MARKER", "acceptance_sha256",
    "bounded_canonical_json", "current_head_set_sha256", "decode_document",
    "derive_proposal_binding", "entry_sha256", "golden_bytes", "golden_fixture",
    "ledger_sha256", "load_golden",
    "prerequisite_sha256", "rebuild_materialized_view", "record_key_sha256",
    "request_sha256", "signature_message", "state_sha256",
    "supersession_sha256", "validate_document", "view_sha256",
]
