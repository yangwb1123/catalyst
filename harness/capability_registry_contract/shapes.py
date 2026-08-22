"""Closed nested shapes and relations for Capability Registry v1."""

from __future__ import annotations

import re
from typing import Callable

from capability_grant_contract.constants import EFFECTS, EFFECT_SPECS

from .codec import ContractError, canonical_json
from .constants import MAX_IDENTIFIER_BYTES, MAX_NARRATIVE_BYTES, MAX_PATH_BYTES
from .digests import require_digest

HASH_RE = re.compile(r"[a-f0-9]{64}")
IDENTIFIER_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]*")
VERSION_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
PATH_SEGMENT_RE = re.compile(r"[A-Za-z0-9._-]+")
MEDIA_TYPES = {
    "application/json", "application/schema+json", "text/markdown",
    "text/x-go", "text/x-python",
}


def exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label}: expected object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(f"{label}: unknown={sorted(unknown)} missing={sorted(missing)}")
    return value


def string(value: object, label: str, *, nonempty: bool = True,
           maximum: int | None = None) -> str:
    if not isinstance(value, str) or nonempty and not value:
        raise ContractError(f"{label}: expected {'non-empty ' if nonempty else ''}string")
    if maximum is not None and len(value.encode("utf-8")) > maximum:
        raise ContractError(f"{label}: exceeds {maximum} UTF-8 bytes")
    return value


def integer(value: object, label: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{label}: expected integer in {low}..{high}")
    return value


def array(value: object, label: str, low: int, high: int) -> list[object]:
    if not isinstance(value, list) or not low <= len(value) <= high:
        raise ContractError(f"{label}: expected array with {low}..{high} items")
    return value


def enum(value: object, choices: set[str], label: str) -> str:
    if not isinstance(value, str) or value not in choices:
        raise ContractError(f"{label}: unsupported value {value!r}")
    return value


def hash_value(value: object, label: str) -> str:
    text = string(value, label)
    if HASH_RE.fullmatch(text) is None:
        raise ContractError(f"{label}: expected lowercase SHA-256")
    return text


def identifier(value: object, label: str) -> str:
    text = string(value, label)
    if (len(text.encode("utf-8")) > MAX_IDENTIFIER_BYTES or
            IDENTIFIER_RE.fullmatch(text) is None):
        raise ContractError(f"{label}: invalid bounded ASCII identifier")
    return text


def version(value: object, label: str) -> str:
    text = string(value, label)
    if (len(text.encode("utf-8")) > MAX_IDENTIFIER_BYTES or
            VERSION_RE.fullmatch(text) is None):
        raise ContractError(f"{label}: invalid opaque ASCII version")
    return text


def repo_path(value: object, label: str) -> str:
    text = string(value, label)
    parts = text.split("/")
    if (len(text.encode("utf-8")) > MAX_PATH_BYTES or text.startswith("/") or
            "\\" in text or any(part in {"", ".", ".."} or
                                  PATH_SEGMENT_RE.fullmatch(part) is None
                                  for part in parts)):
        raise ContractError(f"{label}: unsafe repository-relative path")
    return text


def selector(value: object, label: str) -> str | None:
    if value is None:
        return None
    text = string(value, label)
    try:
        text.encode("ascii")
    except UnicodeError as error:
        raise ContractError(f"{label}: selector must be ASCII") from error
    if not text.startswith("#") or len(text.encode()) > MAX_PATH_BYTES:
        raise ContractError(f"{label}: invalid JSON Pointer fragment")
    if text != "#" and not text.startswith("#/"):
        raise ContractError(f"{label}: invalid JSON Pointer fragment")
    for token in text[2:].split("/") if text.startswith("#/") else []:
        if re.search(r"~(?![01])", token):
            raise ContractError(f"{label}: invalid JSON Pointer escape")
    return text


def sorted_unique(values: list[object], label: str,
                  key: Callable[[object], bytes] = canonical_json) -> None:
    keys = [key(item) for item in values]
    if keys != sorted(keys) or len(keys) != len(set(keys)):
        raise ContractError(f"{label}: must already be sorted and unique")


def unique_ref_targets(values: list[object], label: str) -> None:
    targets = [(item["path"], item["selector"]) for item in values]
    if len(targets) != len(set(targets)):
        raise ContractError(f"{label}: duplicate (path, selector) content ref")


def string_set(value: object, label: str, low: int = 0,
               choices: set[str] | None = None) -> list[str]:
    values = array(value, label, low, 64)
    result = [identifier(item, f"{label}[{index}]")
              for index, item in enumerate(values)]
    if choices is not None and any(item not in choices for item in result):
        raise ContractError(f"{label}: item outside frozen vocabulary")
    sorted_unique(result, label, lambda item: item.encode("utf-8"))
    return result


def validate_content_ref(value: object, label: str,
                         schema_only: bool = False) -> dict[str, object]:
    fields = {"content_bytes", "content_sha256", "media_type", "path", "selector"}
    node = exact_object(value, fields, label)
    integer(node["content_bytes"], f"{label}.content_bytes", 0, 16 * 1024 * 1024)
    hash_value(node["content_sha256"], f"{label}.content_sha256")
    media = enum(node["media_type"], MEDIA_TYPES, f"{label}.media_type")
    if schema_only and media != "application/schema+json":
        raise ContractError(f"{label}.media_type: expected application/schema+json")
    repo_path(node["path"], f"{label}.path")
    selector(node["selector"], f"{label}.selector")
    return node


def validate_selection(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"mode", "root", "suffixes"}, label)
    mode = enum(node["mode"], {
        "all_regular_files_recursive_with_suffixes", "explicit_files",
    }, f"{label}.mode")
    suffixes = array(node["suffixes"], f"{label}.suffixes", 0, 16)
    for index, suffix in enumerate(suffixes):
        text = string(suffix, f"{label}.suffixes[{index}]")
        if re.fullmatch(r"\.[a-z0-9]+", text) is None:
            raise ContractError(f"{label}.suffixes[{index}]: invalid suffix")
    sorted_unique(suffixes, f"{label}.suffixes", lambda item: item.encode())
    if mode == "explicit_files" and (node["root"] is not None or suffixes):
        raise ContractError(f"{label}: explicit selection requires null root and no suffixes")
    if mode != "explicit_files":
        repo_path(node["root"], f"{label}.root")
        if not suffixes:
            raise ContractError(f"{label}: recursive selection requires suffixes")
    return node


def validate_content_set(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"files", "selection", "set_sha256"}, label)
    files = array(node["files"], f"{label}.files", 1, 256)
    for index, ref in enumerate(files):
        validate_content_ref(ref, f"{label}.files[{index}]")
    sorted_unique(files, f"{label}.files")
    unique_ref_targets(files, f"{label}.files")
    validate_selection(node["selection"], f"{label}.selection")
    hash_value(node["set_sha256"], f"{label}.set_sha256")
    require_digest("content_set", node)
    return node


def validate_predicate(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"document", "json_pointer", "operator", "value"}, label)
    enum(node["document"], {"input", "output"}, f"{label}.document")
    pointer = string(node["json_pointer"], f"{label}.json_pointer", nonempty=False)
    selector("#" + pointer, f"{label}.json_pointer")
    operator = enum(node["operator"], {
        "absent", "equals", "not_equals", "present",
    }, f"{label}.operator")
    if operator in {"absent", "present"} and node["value"] is not None:
        raise ContractError(f"{label}.value: {operator} requires null")
    if operator in {"equals", "not_equals"}:
        string(node["value"], f"{label}.value", nonempty=False,
               maximum=MAX_NARRATIVE_BYTES)
    return node


def validate_predicate_set(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"mode", "predicates"}, label)
    mode = enum(node["mode"], {"all", "any", "never"}, f"{label}.mode")
    predicates = array(node["predicates"], f"{label}.predicates", 0, 64)
    for index, predicate in enumerate(predicates):
        validate_predicate(predicate, f"{label}.predicates[{index}]")
    sorted_unique(predicates, f"{label}.predicates")
    if (mode == "never") != (not predicates):
        raise ContractError(f"{label}: never must be empty; all/any must be non-empty")
    return node


def validate_rule(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"enforcement_mode", "rule_id", "statement"}, label)
    enum(node["enforcement_mode"], {"guidance", "hard_gate", "review_trigger"},
         f"{label}.enforcement_mode")
    identifier(node["rule_id"], f"{label}.rule_id")
    string(node["statement"], f"{label}.statement", maximum=MAX_NARRATIVE_BYTES)
    return node


def validate_gate(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"gate_id", "required_test_ids"}, label)
    identifier(node["gate_id"], f"{label}.gate_id")
    string_set(node["required_test_ids"], f"{label}.required_test_ids", 1)
    return node


def validate_failure(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"disposition", "failure_id", "result"}, label)
    enum(node["disposition"], {"fail_closed_no_output", "structured_negative_assessment"},
         f"{label}.disposition")
    identifier(node["failure_id"], f"{label}.failure_id")
    string(node["result"], f"{label}.result", maximum=MAX_NARRATIVE_BYTES)
    return node


def validate_signal(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"signal_id", "signal_kind"}, label)
    identifier(node["signal_id"], f"{label}.signal_id")
    enum(node["signal_kind"], {"artifact", "event", "log", "metric", "trace"},
         f"{label}.signal_kind")
    return node


def validate_permission(value: object, label: str) -> dict[str, object]:
    fields = {"effect_id", "requirement_id", "scope_profile"}
    node = exact_object(value, fields, label)
    effect = enum(node["effect_id"], set(EFFECTS), f"{label}.effect_id")
    identifier(node["requirement_id"], f"{label}.requirement_id")
    profile = identifier(node["scope_profile"], f"{label}.scope_profile")
    if profile != EFFECT_SPECS[effect][3]:
        raise ContractError(f"{label}.scope_profile: not the frozen effect profile")
    return node


def validate_rollback(value: object, label: str) -> dict[str, object]:
    node = exact_object(value, {"description", "mode"}, label)
    string(node["description"], f"{label}.description", maximum=MAX_NARRATIVE_BYTES)
    enum(node["mode"], {
        "compensation_declared", "external_operator_only", "not_required_no_effects",
        "rollback_declared",
    }, f"{label}.mode")
    return node
