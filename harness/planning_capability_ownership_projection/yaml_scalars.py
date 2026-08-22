"""Scalar typing and lexical screening for the ADR-0069 YAML subset."""

from __future__ import annotations

import re

from .codec import ContractError
from .constants import MAX_I64, MIN_I64
from .yaml_resources import Resources

KEY_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._/-]*")
INTEGER_RE = re.compile(r"0|-[1-9][0-9]*|[1-9][0-9]*")
NUMERIC_LIKE_RE = re.compile(r"[+-]?(?:[0-9].*|\.[0-9].*)")
TIMESTAMP_RE = re.compile(
    r"(?:[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}(?:[Tt ].*)?|"
    r"[0-9]{1,2}:[0-9]{2}(?::[0-9]{2})?(?:\.[0-9]+)?(?:[Zz]|[+-][0-9:]+)?)"
)
LEGACY_RE = re.compile(
    r"(?:y|yes|n|no|on|off|true|false|null|~|"
    r"[+-]?(?:\.?(?:inf|nan)|infinity))",
    re.IGNORECASE,
)


def mapping_key(text: str, resources: Resources) -> str:
    value = _quoted(text) if text.startswith('"') else text
    if KEY_RE.fullmatch(value) is None or value == "<<":
        raise ContractError(f"unsupported YAML mapping key {text!r}")
    resources.token(value)
    return value


def _quoted(text: str) -> str:
    if len(text) < 2 or text[-1] != '"':
        raise ContractError("unterminated double-quoted YAML scalar")
    value = text[1:-1]
    if '"' in value or "\\" in value:
        raise ContractError("YAML quoted escapes or embedded quote forbidden")
    return value


def _plain(text: str) -> object:
    if not text or text != text.strip():
        raise ContractError("empty or padded YAML scalar")
    if text == "true":
        return True
    if text == "false":
        return False
    if text == "null":
        return None
    if INTEGER_RE.fullmatch(text):
        if len(text) > 20:
            raise ContractError("YAML integer outside signed int64")
        value = int(text)
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("YAML integer outside signed int64")
        return value
    if LEGACY_RE.fullmatch(text) or NUMERIC_LIKE_RE.fullmatch(text) or TIMESTAMP_RE.fullmatch(text):
        raise ContractError(f"unsupported implicit YAML scalar {text!r}")
    if text[0] in "-?:,[]{}#&*!|>'%@`" or text[-1] == ":":
        raise ContractError(f"unsupported plain YAML scalar {text!r}")
    return text


def scalar(text: str, resources: Resources) -> object:
    if text.startswith('"'):
        value = _quoted(text)
        resources.token(value)
        return value
    if '"' in text:
        raise ContractError("double quote may only delimit a complete scalar")
    if "'" in text or "\\" in text:
        raise ContractError("single quotes and escape bytes are forbidden")
    resources.token(text)
    return _plain(text)


def outside_double(text: str, target: str) -> bool:
    quoted = False
    for character in text:
        if character == '"':
            quoted = not quoted
        elif character == target and not quoted:
            return True
    if quoted:
        raise ContractError("unterminated double-quoted YAML scalar")
    return False


def top_level_colon(text: str) -> int:
    quoted, square, curly = False, 0, 0
    for index, character in enumerate(text):
        if character == '"':
            quoted = not quoted
        elif not quoted and character == "[":
            square += 1
        elif not quoted and character == "]":
            square = max(0, square - 1)
        elif not quoted and character == "{":
            curly += 1
        elif not quoted and character == "}":
            curly = max(0, curly - 1)
        elif not quoted and not square and not curly and character == ":":
            return index
    return -1
