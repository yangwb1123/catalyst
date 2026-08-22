"""Recursive parser for the frozen flow-map and flow-sequence subset."""

from __future__ import annotations

from .codec import ContractError
from .yaml_resources import Resources, bounded_mapping, bounded_sequence
from .yaml_scalars import mapping_key, scalar


class FlowParser:
    def __init__(self, text: str, resources: Resources, depth: int):
        self.text, self.resources, self.depth, self.at = text, resources, depth, 0

    def parse(self) -> object:
        value = self._value(self.depth)
        self._spaces()
        if self.at != len(self.text):
            raise ContractError("trailing YAML flow content")
        return value

    def _spaces(self) -> None:
        while self.at < len(self.text) and self.text[self.at] == " ":
            self.at += 1

    def _take(self, expected: str) -> None:
        self._spaces()
        if self.at >= len(self.text) or self.text[self.at] != expected:
            raise ContractError(f"expected YAML flow token {expected!r}")
        self.at += 1

    def _value(self, depth: int) -> object:
        self.resources.depth(depth)
        self._spaces()
        if self.at >= len(self.text):
            raise ContractError("missing YAML flow value")
        if self.text[self.at] == "[":
            return self._sequence(depth)
        if self.text[self.at] == "{":
            return self._mapping(depth)
        return scalar(self._scalar_text(",]}"), self.resources)

    def _scalar_text(self, delimiters: str) -> str:
        start, quoted = self.at, False
        while self.at < len(self.text):
            character = self.text[self.at]
            if character == '"':
                quoted = not quoted
            if not quoted and character in delimiters:
                break
            self.at += 1
        if quoted:
            raise ContractError("unterminated YAML flow quote")
        return self.text[start:self.at].strip()

    def _sequence(self, depth: int) -> list[object]:
        self.resources.collection(depth)
        self._take("[")
        values: list[object] = []
        self._spaces()
        if self.at < len(self.text) and self.text[self.at] == "]":
            self.at += 1
            return values
        while True:
            values.append(self._value(depth + 1))
            bounded_sequence(values)
            self._spaces()
            if self.at < len(self.text) and self.text[self.at] == "]":
                self.at += 1
                return values
            self._take(",")
            self._spaces()
            if self.at < len(self.text) and self.text[self.at] == "]":
                raise ContractError("trailing YAML flow comma forbidden")

    def _flow_key(self) -> str:
        self._spaces()
        start = self.at
        while self.at < len(self.text) and self.text[self.at] not in ":,{}[]":
            self.at += 1
        raw = self.text[start:self.at]
        if raw != raw.strip():
            raise ContractError("YAML flow mapping key has padding")
        return mapping_key(raw, self.resources)

    def _mapping(self, depth: int) -> dict[str, object]:
        self.resources.collection(depth)
        self._take("{")
        result: dict[str, object] = {}
        self._spaces()
        if self.at < len(self.text) and self.text[self.at] == "}":
            self.at += 1
            return result
        while True:
            key = self._flow_key()
            if key in result:
                raise ContractError(f"duplicate YAML key {key!r}")
            if self.at >= len(self.text) or self.text[self.at] != ":":
                raise ContractError("missing YAML flow mapping colon")
            self.at += 1
            if (self.at + 1 >= len(self.text) or self.text[self.at] != " " or
                    self.text[self.at + 1] == " "):
                raise ContractError("YAML flow colon requires exactly one space and a value")
            self.at += 1
            result[key] = self._value(depth + 1)
            bounded_mapping(result)
            self._spaces()
            if self.at < len(self.text) and self.text[self.at] == "}":
                self.at += 1
                return result
            self._take(",")
            self._spaces()
            if self.at < len(self.text) and self.text[self.at] == "}":
                raise ContractError("trailing YAML flow comma forbidden")


def parse_flow(text: str, resources: Resources, depth: int) -> object:
    return FlowParser(text, resources, depth).parse()
