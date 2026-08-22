"""Package-local strict parser for ADR-0069's deliberately narrow YAML subset."""

from __future__ import annotations

from dataclasses import dataclass

from .codec import ContractError
from .constants import MAX_CATALOG_BYTES, MAX_SCALAR_BYTES
from .yaml_flow import parse_flow
from .yaml_resources import Resources, bounded_mapping, bounded_sequence
from .yaml_scalars import mapping_key, outside_double, scalar, top_level_colon


@dataclass(frozen=True)
class Line:
    number: int
    indent: int
    text: str


def _sequence_line(text: str) -> bool:
    return text == "-" or text.startswith("- ")


def _screen_line(text: str, number: int) -> None:
    if text.endswith(" "):
        raise ContractError(f"YAML line {number} has trailing whitespace")
    if outside_double(text, "#"):
        raise ContractError(f"YAML line {number} contains a comment")
    if outside_double(text, "'") or outside_double(text, "\\"):
        raise ContractError(f"YAML line {number} uses forbidden quoting or escape syntax")
    stripped = text.lstrip(" ")
    if stripped in {"---", "..."} or stripped.startswith("%"):
        raise ContractError(f"YAML line {number} uses a document marker or directive")
    if outside_double(text, "&") or outside_double(text, "*") or outside_double(text, "!"):
        raise ContractError(f"YAML line {number} uses anchor, alias, or tag syntax")
    if stripped.startswith("<<:"):
        raise ContractError(f"YAML line {number} uses a merge key")


def _lines(raw: bytes, maximum: int) -> list[Line]:
    if not isinstance(raw, bytes) or not 1 <= len(raw) <= maximum:
        raise ContractError(f"YAML byte length must be 1..{maximum}")
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("YAML must have exactly one terminal LF")
    if any(byte < 0x20 and byte != 0x0A or byte == 0x7F for byte in raw):
        raise ContractError("YAML C0 controls other than LF and DEL are forbidden")
    try:
        text = raw.decode("ascii")
    except UnicodeDecodeError as error:
        raise ContractError("YAML must use the ASCII subset of UTF-8") from error
    result = []
    for number, content in enumerate(text[:-1].split("\n"), 1):
        _screen_line(content, number)
        indent = len(content) - len(content.lstrip(" "))
        if indent % 2:
            raise ContractError(f"YAML line {number} indentation is not a multiple of two")
        result.append(Line(number, indent, content[indent:]))
    return result


class BlockParser:
    def __init__(self, raw: bytes, maximum: int = MAX_CATALOG_BYTES):
        self.lines, self.at, self.resources = _lines(raw, maximum), 0, Resources()

    def parse(self) -> object:
        self._blank()
        if self.at >= len(self.lines):
            raise ContractError("empty YAML document")
        value = self._block(0, 1)
        self._blank()
        if self.at != len(self.lines):
            raise ContractError(f"trailing YAML content at line {self.lines[self.at].number}")
        return value

    def _blank(self) -> None:
        while self.at < len(self.lines) and not self.lines[self.at].text:
            self.at += 1

    def _block(self, indent: int, depth: int) -> object:
        self._blank()
        if self.at >= len(self.lines) or self.lines[self.at].indent != indent:
            raise ContractError("missing or over-indented YAML block")
        if _sequence_line(self.lines[self.at].text):
            return self._sequence(indent, depth)
        return self._mapping(indent, depth)

    def _inline(self, text: str, depth: int) -> object:
        self.resources.depth(depth)
        if text.startswith("[") or text.startswith("{"):
            return parse_flow(text, self.resources, depth)
        if text in {">", "|", "|-", ">+", "|+"}:
            raise ContractError("unsupported YAML block scalar style")
        return scalar(text, self.resources)

    def _folded(self, indent: int, depth: int) -> str:
        self.resources.depth(depth)
        values, total = [], 0
        while self.at < len(self.lines):
            line = self.lines[self.at]
            if line.indent < indent:
                break
            if not line.text:
                break
            if line.indent != indent:
                raise ContractError("folded YAML content must use exact indentation")
            if any(indicator in line.text for indicator in "#&*!'\\"):
                raise ContractError("folded YAML content contains a forbidden syntax byte")
            total += len(line.text.encode("utf-8")) + bool(values)
            if total > MAX_SCALAR_BYTES:
                raise ContractError("folded YAML scalar byte limit exceeded")
            values.append(line.text)
            self.at += 1
        if not values:
            raise ContractError("folded YAML scalar must have content")
        value = " ".join(values)
        self.resources.token(value)
        return value

    def _entry(self, text: str, indent: int, depth: int) -> tuple[str, object]:
        colon = top_level_colon(text)
        if colon <= 0:
            raise ContractError("YAML mapping entry requires a key and colon")
        key = mapping_key(text[:colon], self.resources)
        tail = text[colon + 1:]
        if tail:
            if len(tail) < 2 or tail[0] != " " or tail[1] == " ":
                raise ContractError("YAML mapping colon requires exactly one space and a value")
            tail = tail[1:]
        if tail == ">-":
            return key, self._folded(indent + 2, depth + 1)
        if tail:
            return key, self._inline(tail, depth + 1)
        self._blank()
        if self.at >= len(self.lines) or self.lines[self.at].indent != indent + 2:
            raise ContractError(f"YAML key {key!r} is missing a nested value")
        return key, self._block(indent + 2, depth + 1)

    def _put(self, result: dict[str, object], entry: tuple[str, object]) -> None:
        key, value = entry
        if key in result:
            raise ContractError(f"duplicate YAML key {key!r}")
        result[key] = value
        bounded_mapping(result)

    def _mapping(self, indent: int, depth: int,
                 initial: tuple[str, object] | None = None) -> dict[str, object]:
        self.resources.collection(depth)
        result: dict[str, object] = {}
        if initial is not None:
            self._put(result, initial)
        while self.at < len(self.lines):
            self._blank()
            if self.at >= len(self.lines) or self.lines[self.at].indent < indent:
                break
            line = self.lines[self.at]
            if line.indent != indent or _sequence_line(line.text):
                break
            self.at += 1
            self._put(result, self._entry(line.text, indent, depth))
        return result

    def _sequence_item(self, line: Line, indent: int, depth: int) -> object:
        tail = "" if line.text == "-" else line.text[2:]
        colon = top_level_colon(tail)
        if colon > 0:
            self.at += 1
            first = self._entry(tail, indent + 2, depth + 1)
            return self._mapping(indent + 2, depth + 1, first)
        self.at += 1
        if not tail:
            self._blank()
            return self._block(indent + 2, depth + 1)
        return self._inline(tail, depth + 1)

    def _sequence(self, indent: int, depth: int) -> list[object]:
        self.resources.collection(depth)
        result = []
        while self.at < len(self.lines):
            self._blank()
            if self.at >= len(self.lines) or self.lines[self.at].indent < indent:
                break
            line = self.lines[self.at]
            if line.indent != indent or not _sequence_line(line.text):
                break
            result.append(self._sequence_item(line, indent, depth))
            bounded_sequence(result)
        return result


def parse_yaml(raw: bytes, maximum: int = MAX_CATALOG_BYTES) -> object:
    """Parse all exact source bytes with no ambient reads or YAML library defaults."""
    try:
        return BlockParser(raw, maximum).parse()
    except ContractError:
        raise
    except (MemoryError, RecursionError) as error:
        raise ContractError(f"YAML resource exhaustion: {error}") from error
