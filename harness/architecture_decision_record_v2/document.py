"""Strict Markdown framing, body, and digest validation for ADR v2."""

from __future__ import annotations

from pathlib import Path

from governance_contract import ContractError, read_bounded_file

from .codec import body_digest, decode_canonical, forbidden_scalar, self_digest
from .constants import BODY_SECTIONS, MAX_BODY_BYTES, MAX_DOCUMENT_BYTES
from .shape import validate_metadata


def split_document(raw: bytes) -> tuple[bytes, bytes]:
    if len(raw) > MAX_DOCUMENT_BYTES:
        raise ContractError(f"ADR v2 document exceeds {MAX_DOCUMENT_BYTES} bytes")
    if not raw.startswith(b"---\n"):
        raise ContractError("ADR v2 document must start with exact LF frontmatter framing")
    if b"\r" in raw or raw.startswith(b"\xef\xbb\xbf"):
        raise ContractError("ADR v2 document forbids BOM and CR bytes")
    newline = raw.find(b"\n", 4)
    if newline < 0:
        raise ContractError("ADR v2 frontmatter JSON line is unterminated")
    metadata_raw = raw[4:newline]
    if not metadata_raw:
        raise ContractError("ADR v2 frontmatter JSON line must not be empty")
    if not raw[newline + 1:].startswith(b"---\n\n"):
        raise ContractError("ADR v2 frontmatter must contain exactly one JSON line")
    body = raw[newline + 6:]
    if len(metadata_raw) > 64 * 1024:
        raise ContractError("ADR v2 frontmatter exceeds 65536 bytes")
    if not body or len(body) > MAX_BODY_BYTES:
        raise ContractError(f"ADR v2 body must contain 1..{MAX_BODY_BYTES} bytes")
    return metadata_raw, body


def _decode_body(body: bytes) -> str:
    try:
        text = body.decode("utf-8")
    except UnicodeError as error:
        raise ContractError("ADR v2 body is not valid UTF-8") from error
    for character in text:
        if character == "\n":
            continue
        if forbidden_scalar(character):
            raise ContractError("ADR v2 body contains a forbidden Unicode scalar")
    if not text.endswith("\n") or text.endswith("\n\n"):
        raise ContractError("ADR v2 body must end with exactly one LF")
    if any(line.endswith((" ", "\t")) for line in text.splitlines()):
        raise ContractError("ADR v2 body lines must not contain trailing whitespace")
    return text


def _extract_sections(text: str, heading: str) -> list[str]:
    if not text.startswith(heading):
        raise ContractError("ADR v2 body heading does not match adr_id and title")
    remaining = text[len(heading):]
    sections = []
    for index, name in enumerate(BODY_SECTIONS):
        prefix = f"## {name}\n"
        if not remaining.startswith(prefix):
            raise ContractError(f"ADR v2 body requires exact section {name!r} in order")
        remaining = remaining[len(prefix):]
        if index + 1 < len(BODY_SECTIONS):
            marker = f"\n\n## {BODY_SECTIONS[index + 1]}\n"
            boundary = remaining.find(marker)
            if boundary < 0:
                raise ContractError(f"ADR v2 body cannot find section after {name!r}")
            content, remaining = remaining[:boundary], remaining[boundary + 2:]
        else:
            content, remaining = remaining[:-1], ""
        if not content or content.strip() != content:
            raise ContractError(f"ADR v2 body section {name!r} must contain canonical nonempty text")
        sections.append(content)
    if remaining or any(_contains_level_two_heading(part) for part in sections):
        raise ContractError("ADR v2 body contains an extra level-two section")
    return sections


def _contains_level_two_heading(section: str) -> bool:
    lines = section.splitlines()
    for index, line in enumerate(lines):
        candidate = line.lstrip(" ")
        indent = len(line) - len(candidate)
        if indent <= 3 and (candidate == "##" or candidate.startswith("## ")):
            return True
        if (index > 0 and lines[index - 1].strip() and indent <= 3
                and candidate and set(candidate) == {"-"}):
            return True
    return False


def validate_document_bytes(raw: bytes, document_name: str) -> dict[str, object]:
    metadata_raw, body = split_document(raw)
    metadata = validate_metadata(decode_canonical(metadata_raw), document_name)
    text = _decode_body(body)
    heading = f"# {metadata['adr_id']}: {metadata['title']}\n\n"
    _extract_sections(text, heading)
    if metadata["body_sha256"] != body_digest(body):
        raise ContractError("body_sha256 does not bind the exact ADR v2 body")
    if metadata["self_sha256"] != self_digest(metadata, body):
        raise ContractError("self_sha256 does not bind the frontmatter and exact body")
    return metadata


def validate_document_file(path: Path) -> dict[str, object]:
    raw = read_bounded_file(path, label=str(path), max_bytes=MAX_DOCUMENT_BYTES)
    return validate_document_bytes(raw, path.name)
