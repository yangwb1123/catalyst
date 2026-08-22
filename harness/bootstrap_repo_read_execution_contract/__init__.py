"""Structural ADR-0058 contract; load-bearing authentication belongs to Kernel."""

from .authority import root_sha256, validate_execution_root
from .canonical import ContractError, canonical_json, signature_message
from .contract import (decode_document, decode_usage_ledger, load_fixture,
                       terminal_replay, terminal_replay_from_documents,
                       validate_document)
from .documents import invocation_sha256, policy_sha256
from .ledger import ledger_sha256, lookup_usage_group, receipt_sha256
from .manifest import manifest_sha256
from .results import metadata_sha256, result_sha256
from .shape import action_sha256, record_key_sha256

__all__ = [
    "ContractError", "action_sha256", "canonical_json", "decode_document",
    "decode_usage_ledger", "invocation_sha256", "ledger_sha256", "load_fixture",
    "lookup_usage_group", "manifest_sha256", "metadata_sha256", "policy_sha256",
    "receipt_sha256", "record_key_sha256", "result_sha256", "root_sha256",
    "signature_message", "terminal_replay", "terminal_replay_from_documents",
    "validate_document", "validate_execution_root",
]
