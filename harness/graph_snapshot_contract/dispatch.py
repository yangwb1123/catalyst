"""Explicit GraphSnapshot projector profile dispatch without fallback."""

from __future__ import annotations

from governance_contract import ContractError

from .constants import PROFILE_ID, TEST_SOURCE_PROFILE_ID
from .derive import derive_envelope
from .validation import validate_envelope_bytes


def derive_profile_envelope(graph_json: bytes, graph_sha256: object,
                            run_id: object, project_id: object,
                            projector_profile_id: object):
    """Select one exact projector contract; aliases and fallback are forbidden."""
    if projector_profile_id == PROFILE_ID:
        return derive_envelope(graph_json, graph_sha256, run_id, project_id)
    if projector_profile_id == TEST_SOURCE_PROFILE_ID:
        from .lexical_test_source_derive import derive_test_source_envelope
        return derive_test_source_envelope(
            graph_json, graph_sha256, run_id, project_id)
    raise ContractError("unsupported_profile: unsupported graph snapshot projector profile")


def validate_profile_envelope_bytes(raw: bytes,
                                    projector_profile_id: object) -> list[str]:
    """Validate bytes against the caller-selected exact single-profile endpoint."""
    if projector_profile_id == PROFILE_ID:
        return validate_envelope_bytes(raw)
    if projector_profile_id == TEST_SOURCE_PROFILE_ID:
        from .lexical_test_source_validation import validate_test_source_envelope_bytes
        return validate_test_source_envelope_bytes(raw)
    return ["unsupported_profile: unsupported graph snapshot projector profile"]
