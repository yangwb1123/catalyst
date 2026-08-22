"""Pinned, injectable token counting boundary."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from .codec import ContractError
from .constants import UTF8_COUNTER_ID, UTF8_COUNTER_SHA256


class TokenCounter(Protocol):
    tokenizer_id: str
    tokenizer_sha256: str

    def count(self, projection_bytes: bytes) -> int:
        """Count tokens for exact projection canonical bytes."""


@dataclass(frozen=True)
class Utf8ByteTokenCounter:
    """Golden-only deterministic counter: one token per UTF-8 byte."""

    tokenizer_id: str = UTF8_COUNTER_ID
    tokenizer_sha256: str = UTF8_COUNTER_SHA256

    def count(self, projection_bytes: bytes) -> int:
        return len(projection_bytes)


def checked_count(counter: TokenCounter, budget: dict[str, object], payload: bytes) -> int:
    if (counter.tokenizer_id != budget["tokenizer_id"] or
            counter.tokenizer_sha256 != budget["tokenizer_sha256"]):
        raise ContractError("token counter identity does not match request budget")
    try:
        result = counter.count(payload)
    except Exception as error:
        raise ContractError(f"token counter failed: {error}") from error
    if isinstance(result, bool) or not isinstance(result, int) or result < 0:
        raise ContractError("token counter returned an invalid count")
    return result
