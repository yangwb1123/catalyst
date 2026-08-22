"""Resource accounting for the frozen ADR-0069 YAML subset."""

from dataclasses import dataclass

from .codec import ContractError
from .constants import (
    MAX_COLLECTIONS, MAX_DEPTH, MAX_FIELDS, MAX_ITEMS, MAX_SCALAR_BYTES, MAX_TOKENS,
)


@dataclass
class Resources:
    """Reject inputs before any parser resource bound is exceeded."""

    tokens: int = 0
    collections: int = 0

    @staticmethod
    def depth(value: int) -> None:
        if value > MAX_DEPTH:
            raise ContractError("YAML depth limit exceeded")

    def token(self, value: str | None = None) -> None:
        self.tokens += 1
        if self.tokens > MAX_TOKENS:
            raise ContractError("YAML token limit exceeded")
        if value is not None and len(value.encode("utf-8")) > MAX_SCALAR_BYTES:
            raise ContractError("YAML scalar byte limit exceeded")

    def collection(self, depth: int) -> None:
        self.depth(depth)
        if self.collections >= MAX_COLLECTIONS:
            raise ContractError("YAML collection limit exceeded")
        self.collections += 1
        self.token()


def bounded_mapping(value: dict[str, object]) -> dict[str, object]:
    if len(value) > MAX_FIELDS:
        raise ContractError("YAML mapping field limit exceeded")
    return value


def bounded_sequence(value: list[object]) -> list[object]:
    if len(value) > MAX_ITEMS:
        raise ContractError("YAML sequence item limit exceeded")
    return value
