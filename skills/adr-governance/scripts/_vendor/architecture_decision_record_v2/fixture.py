"""Byte-authoritative golden fixture validation."""

from __future__ import annotations

import hashlib
from pathlib import Path

from governance_contract import ContractError, read_bounded_file

from .constants import GOLDEN, GOLDEN_SHA256, MAX_DOCUMENT_BYTES
from .document import validate_document_bytes


def validate_golden(repo_root: Path) -> dict[str, object]:
    path = repo_root / GOLDEN
    raw = read_bounded_file(path, label=str(GOLDEN), max_bytes=MAX_DOCUMENT_BYTES)
    actual = hashlib.sha256(raw).hexdigest()
    if actual != GOLDEN_SHA256:
        raise ContractError("ADR v2 golden fixture bytes drifted from the authoritative pin")
    return validate_document_bytes(raw, path.name)
