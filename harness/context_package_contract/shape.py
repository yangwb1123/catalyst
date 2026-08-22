"""Exact request/package shape and scalar validation."""

from __future__ import annotations

from .codec import ContractError, forbidden_scalar, plain_sha256
from .constants import (ACCOUNTING_FIELDS, ASSEMBLY_MODE, AVAILABILITY, BUDGET_FIELDS,
                        CANONICALIZATION, CATEGORIES, DECLARED_LANES, DECLARED_TRUST,
                        DELIMITER, DISPOSITIONS, FRESHNESS, FRESHNESS_FIELDS, HASH_RE,
                        INJECTION_RISKS, LANES, MAX_CONTENT_BYTES, MAX_I64,
                        MAX_REDACTION_RANGES, MAX_SELECTED, MAX_SOURCE_BYTES,
                        MAX_SOURCES, MAX_TOKENS, NORMALIZATION, OMISSION_FIELDS,
                        OMISSION_REASONS, PACKAGE_FIELDS, PACKAGE_VERSION, RANGE_FIELDS,
                        REDACTION_FIELDS, REQUEST_FIELDS, REQUEST_VERSION, RESULT,
                        SNIPPET_FIELDS, SOURCE_BINDING_FIELDS, SOURCE_CLASSES,
                        SOURCE_FIELDS, TASK_FIELDS, TRUNCATION_FIELDS,
                        TRUNCATION_POLICIES, UNTRUSTED_SOURCE_CLASSES)


def exact(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(f"{label} fields mismatch: unknown={sorted(unknown)}, missing={sorted(missing)}")
    return value


def text(value: object, label: str, *, maximum: int = 4096, content: bool = False) -> str:
    if not isinstance(value, str) or not value:
        raise ContractError(f"{label} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ContractError(f"{label} exceeds {maximum} UTF-8 bytes")
    if any(forbidden_scalar(character, content=content) for character in value):
        raise ContractError(f"{label} contains a forbidden Unicode scalar")
    return value


def integer(value: object, label: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ContractError(f"{label} must be an integer in [{minimum}, {maximum}]")
    return value


def enum(value: object, allowed: set[str] | tuple[str, ...], label: str) -> str:
    if not isinstance(value, str) or value not in allowed:
        raise ContractError(f"{label} must be one of {sorted(allowed)}")
    return value


def sha(value: object, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must be a lowercase bare SHA-256")
    return value


def _validate_binding(request: dict[str, object]) -> None:
    task = exact(request["task_binding"], TASK_FIELDS, "task_binding")
    for field in TASK_FIELDS:
        text(task[field], f"task_binding.{field}", maximum=160)
    source = exact(request["source_binding"], SOURCE_BINDING_FIELDS, "source_binding")
    integer(source["as_of_unix_ms"], "source_binding.as_of_unix_ms", 0, MAX_I64)
    for field in {"policy_sha256", "routes_sha256", "source_tree_sha256"}:
        sha(source[field], f"source_binding.{field}")
    text(source["source_revision"], "source_binding.source_revision", maximum=160)


def _validate_budget(value: object) -> None:
    budget = exact(value, BUDGET_FIELDS, "budget")
    integer(budget["max_content_bytes"], "budget.max_content_bytes", 1, MAX_CONTENT_BYTES)
    integer(budget["max_snippets"], "budget.max_snippets", 1, MAX_SELECTED)
    integer(budget["max_tokens"], "budget.max_tokens", 1, MAX_TOKENS)
    text(budget["tokenizer_id"], "budget.tokenizer_id", maximum=160)
    sha(budget["tokenizer_sha256"], "budget.tokenizer_sha256")


def _validate_source(value: object, index: int) -> dict[str, object]:
    label = f"sources[{index}]"
    source = exact(value, SOURCE_FIELDS, label)
    enum(source["availability"], AVAILABILITY, f"{label}.availability")
    enum(source["category"], CATEGORIES, f"{label}.category")
    enum(source["declared_lane"], DECLARED_LANES, f"{label}.declared_lane")
    enum(source["declared_trust"], DECLARED_TRUST, f"{label}.declared_trust")
    enum(source["disposition"], DISPOSITIONS, f"{label}.disposition")
    enum(source["freshness"], FRESHNESS, f"{label}.freshness")
    enum(source["injection_risk"], INJECTION_RISKS, f"{label}.injection_risk")
    enum(source["source_class"], SOURCE_CLASSES, f"{label}.source_class")
    enum(source["truncation"], TRUNCATION_POLICIES, f"{label}.truncation")
    integer(source["max_bytes"], f"{label}.max_bytes", 1, MAX_SOURCE_BYTES)
    integer(source["priority"], f"{label}.priority", 0, 1000)
    if not isinstance(source["required"], bool):
        raise ContractError(f"{label}.required must be boolean")
    for field, maximum in (("source_id", 160), ("source_ref", 4096), ("source_revision", 160)):
        text(source[field], f"{label}.{field}", maximum=maximum)
    _validate_source_content(source, label)
    return source


def _validate_source_content(source: dict[str, object], label: str) -> None:
    content, content_hash = source["content"], source["content_sha256"]
    expires = source["expires_at_unix_ms"]
    if expires is not None:
        integer(expires, f"{label}.expires_at_unix_ms", 0, MAX_I64)
    if source["availability"] == "missing":
        if content is not None or content_hash is not None:
            raise ContractError(f"{label}: missing source requires null content and digest")
    else:
        content = text(content, f"{label}.content", maximum=MAX_SOURCE_BYTES, content=True)
        sha(content_hash, f"{label}.content_sha256")
        if plain_sha256(content.encode("utf-8")) != content_hash:
            raise ContractError(f"{label}.content_sha256 mismatch")
    if (source["source_class"] in UNTRUSTED_SOURCE_CLASSES and
            source["declared_lane"] in {"instruction", "trusted_context"}):
        raise ContractError(f"{label}: untrusted source class cannot escalate its lane")
    if (source["source_class"] in UNTRUSTED_SOURCE_CLASSES and
            source["declared_trust"] != "untrusted"):
        raise ContractError(f"{label}: untrusted source class cannot escalate declared trust")


def _validate_redactions(request: dict[str, object], sources: list[dict[str, object]]) -> None:
    plans = request["redactions"]
    if not isinstance(plans, list) or len(plans) > MAX_SOURCES:
        raise ContractError("redactions must be an array bounded by source count")
    source_map = {source["source_id"]: source for source in sources}
    ids: list[str] = []
    total = 0
    for index, value in enumerate(plans):
        plan = exact(value, REDACTION_FIELDS, f"redactions[{index}]")
        source_id = text(plan["source_id"], f"redactions[{index}].source_id", maximum=160)
        ids.append(source_id)
        total += _validate_ranges(plan["ranges"], source_map.get(source_id), index)
    if ids != sorted(set(ids), key=lambda item: item.encode("utf-8")):
        raise ContractError("redactions must be sorted by unique source_id")
    if total > MAX_REDACTION_RANGES:
        raise ContractError(f"redactions exceed {MAX_REDACTION_RANGES} total ranges")


def _validate_ranges(value: object, source: dict[str, object] | None, plan_index: int) -> int:
    if source is None or source["availability"] != "available":
        raise ContractError(f"redactions[{plan_index}] must reference an available source")
    if not isinstance(value, list) or not value or len(value) > MAX_REDACTION_RANGES:
        raise ContractError(f"redactions[{plan_index}].ranges must be a non-empty bounded array")
    content = source["content"].encode("utf-8")
    boundaries = _utf8_boundaries(content)
    previous = -1
    for index, item in enumerate(value):
        label = f"redactions[{plan_index}].ranges[{index}]"
        current = exact(item, RANGE_FIELDS, label)
        start = integer(current["start_byte"], f"{label}.start_byte", 0, len(content))
        end = integer(current["end_byte"], f"{label}.end_byte", 1, len(content))
        text(current["rule_id"], f"{label}.rule_id", maximum=160)
        if start >= end or start < previous or start not in boundaries or end not in boundaries:
            raise ContractError(f"{label} must be ordered, non-overlapping UTF-8 boundaries")
        previous = end
    return len(value)


def _utf8_boundaries(content: bytes) -> set[int]:
    result, offset = {0}, 0
    for character in content.decode("utf-8"):
        offset += len(character.encode("utf-8"))
        result.add(offset)
    return result


def validate_request(request: dict[str, object]) -> None:
    exact(request, REQUEST_FIELDS, "request")
    if request["api_version"] != REQUEST_VERSION:
        raise ContractError("unsupported build request api_version")
    if request["canonicalization"] != CANONICALIZATION:
        raise ContractError("unsupported canonicalization")
    _validate_binding(request)
    _validate_budget(request["budget"])
    values = request["sources"]
    if not isinstance(values, list) or not 1 <= len(values) <= MAX_SOURCES:
        raise ContractError(f"sources must contain 1..{MAX_SOURCES} entries")
    sources = [_validate_source(value, index) for index, value in enumerate(values)]
    ids = [source["source_id"] for source in sources]
    if ids != sorted(ids, key=lambda item: item.encode("utf-8")):
        raise ContractError("sources must be sorted by source_id UTF-8 byte order")
    for field in ("source_id", "source_ref"):
        members = [source[field] for source in sources]
        if len(members) != len(set(members)):
            raise ContractError(f"sources require unique {field}")
    _validate_redactions(request, sources)


def _validate_accounting(value: object) -> None:
    accounting = exact(value, ACCOUNTING_FIELDS, "package.accounting")
    bounds = {"actual_tokens": MAX_TOKENS, "candidate_count": MAX_SOURCES,
              "content_bytes": MAX_CONTENT_BYTES, "omitted_source_count": MAX_SOURCES,
              "redacted_range_count": MAX_REDACTION_RANGES,
              "selected_snippet_count": MAX_SELECTED, "truncated_snippet_count": MAX_SELECTED}
    for field, maximum in bounds.items():
        integer(accounting[field], f"package.accounting.{field}", 0, maximum)
    if accounting["candidate_count"] < 1:
        raise ContractError("package.accounting.candidate_count must be at least one")
    if accounting["truncated_snippet_count"] > accounting["selected_snippet_count"]:
        raise ContractError("truncated snippet count exceeds selected snippet count")


def _validate_truncation(value: object, content: str, required: bool, label: str) -> None:
    if value is None:
        return
    if required:
        raise ContractError(f"{label}: required snippet cannot be truncated")
    receipt = exact(value, TRUNCATION_FIELDS, label)
    original = integer(receipt["original_redacted_bytes"], f"{label}.original_redacted_bytes",
                       1, MAX_CONTENT_BYTES)
    retained = integer(receipt["retained_bytes"], f"{label}.retained_bytes",
                       1, MAX_SOURCE_BYTES)
    if receipt["reason"] != "source_max_bytes" or retained >= original:
        raise ContractError(f"{label} is not an actual source-max truncation")
    if retained != len(content.encode("utf-8")):
        raise ContractError(f"{label}.retained_bytes does not match content")


def _validate_snippet(value: object, lane: str, index: int) -> None:
    label = f"package.lanes.{lane}[{index}]"
    snippet = exact(value, SNIPPET_FIELDS, label)
    enum(snippet["category"], CATEGORIES, f"{label}.category")
    content = text(snippet["content"], f"{label}.content", maximum=MAX_SOURCE_BYTES, content=True)
    enum(snippet["declared_lane"], DECLARED_LANES, f"{label}.declared_lane")
    enum(snippet["declared_trust"], DECLARED_TRUST, f"{label}.declared_trust")
    enum(snippet["source_class"], SOURCE_CLASSES, f"{label}.source_class")
    if (snippet["source_class"] in UNTRUSTED_SOURCE_CLASSES and
            (snippet["declared_lane"] != "untrusted_data" or
             snippet["declared_trust"] != "untrusted")):
        raise ContractError(f"{label}: untrusted source class escalates lane or trust")
    expected_lane = "trusted_context"
    if (snippet["source_class"] in {"system_policy", "user_instruction"} and
            snippet["declared_lane"] == "instruction"):
        expected_lane = "instruction_candidates"
    elif snippet["declared_lane"] == "untrusted_data":
        expected_lane = "untrusted_data"
    if snippet["delimiter"] != DELIMITER or snippet["normalization"] != NORMALIZATION:
        raise ContractError(f"{label}: unsupported delimiter or normalization")
    if (snippet["instruction_allowed"] is not False or snippet["lane"] != lane or
            lane != expected_lane):
        raise ContractError(f"{label}: lane boundary or instruction flag mismatch")
    if not isinstance(snippet["required"], bool):
        raise ContractError(f"{label}.required must be boolean")
    expected_reason = "required_source" if snippet["required"] else "priority_selection"
    if snippet["selection_reason"] != expected_reason:
        raise ContractError(f"{label}.selection_reason contradicts required")
    for field in {"projected_content_sha256", "snippet_sha256", "source_content_sha256"}:
        sha(snippet[field], f"{label}.{field}")
    for field, maximum in (("source_id", 160), ("source_ref", 4096), ("source_revision", 160)):
        text(snippet[field], f"{label}.{field}", maximum=maximum)
    _validate_truncation(
        snippet["truncation"], content, snippet["required"], f"{label}.truncation"
    )


def _validate_lanes(value: object) -> list[dict[str, object]]:
    lanes = exact(value, set(LANES), "package.lanes")
    selected: list[dict[str, object]] = []
    for lane in LANES:
        if not isinstance(lanes[lane], list) or len(lanes[lane]) > MAX_SELECTED:
            raise ContractError(f"package.lanes.{lane} must contain at most {MAX_SELECTED} snippets")
        for index, snippet in enumerate(lanes[lane]):
            _validate_snippet(snippet, lane, index)
            selected.append(snippet)
    if len(selected) > MAX_SELECTED:
        raise ContractError(f"package lanes exceed {MAX_SELECTED} total snippets")
    ids = [snippet["source_id"] for snippet in selected]
    if len(ids) != len(set(ids)):
        raise ContractError("package snippets require unique source_id")
    return selected


def _validate_omissions(value: object) -> list[dict[str, object]]:
    if not isinstance(value, list) or len(value) > MAX_SOURCES:
        raise ContractError("package.omissions exceeds source bound")
    for index, item in enumerate(value):
        omission = exact(item, OMISSION_FIELDS, f"package.omissions[{index}]")
        enum(omission["reason"], OMISSION_REASONS, f"package.omissions[{index}].reason")
        text(omission["source_id"], f"package.omissions[{index}].source_id", maximum=160)
        text(omission["source_ref"], f"package.omissions[{index}].source_ref", maximum=4096)
    ids = [item["source_id"] for item in value]
    if ids != sorted(set(ids), key=lambda item: item.encode("utf-8")):
        raise ContractError("package omissions must be sorted by unique source_id")
    return value


def _validate_receipt_ranges(value: object, label: str) -> int:
    if not isinstance(value, list) or not value or len(value) > MAX_REDACTION_RANGES:
        raise ContractError(f"{label} must be a non-empty bounded array")
    previous = -1
    for index, item in enumerate(value):
        current = exact(item, RANGE_FIELDS, f"{label}[{index}]")
        start = integer(current["start_byte"], f"{label}[{index}].start_byte", 0,
                        MAX_SOURCE_BYTES - 1)
        end = integer(current["end_byte"], f"{label}[{index}].end_byte", 1,
                      MAX_SOURCE_BYTES)
        text(current["rule_id"], f"{label}[{index}].rule_id", maximum=160)
        if start >= end or start < previous:
            raise ContractError(f"{label} must be ordered and non-overlapping")
        previous = end
    return len(value)


def _validate_receipts(value: object) -> None:
    if not isinstance(value, list) or len(value) > MAX_SOURCES:
        raise ContractError("package.redaction_receipts exceeds source bound")
    ids, total = [], 0
    for index, item in enumerate(value):
        receipt = exact(item, REDACTION_FIELDS, f"package.redaction_receipts[{index}]")
        ids.append(text(receipt["source_id"], f"package.redaction_receipts[{index}].source_id",
                        maximum=160))
        total += _validate_receipt_ranges(receipt["ranges"],
                                          f"package.redaction_receipts[{index}].ranges")
    if ids != sorted(set(ids), key=lambda item: item.encode("utf-8")):
        raise ContractError("package redaction receipts must be sorted by unique source_id")
    if total > MAX_REDACTION_RANGES:
        raise ContractError("package redaction receipts exceed total range bound")


def _validate_package_nested(package: dict[str, object]) -> None:
    _validate_budget(package["budget"])
    _validate_binding({"task_binding": package["task_binding"],
                       "source_binding": package["source_binding"]})
    _validate_accounting(package["accounting"])
    freshness = exact(package["freshness"], FRESHNESS_FIELDS, "package.freshness")
    integer(freshness["evaluated_at_unix_ms"], "package.freshness.evaluated_at_unix_ms", 0,
            MAX_I64)
    if freshness["expires_at_unix_ms"] is not None:
        integer(freshness["expires_at_unix_ms"], "package.freshness.expires_at_unix_ms", 0,
                MAX_I64)
    selected = _validate_lanes(package["lanes"])
    omissions = _validate_omissions(package["omissions"])
    _validate_receipts(package["redaction_receipts"])
    if {item["source_id"] for item in selected} & {item["source_id"] for item in omissions}:
        raise ContractError("a package source cannot be both selected and omitted")
    _validate_package_accounting(package, selected, omissions)


def _validate_package_accounting(package: dict[str, object], selected: list[dict[str, object]],
                                 omissions: list[dict[str, object]]) -> None:
    accounting, budget = package["accounting"], package["budget"]
    content_bytes = sum(len(item["content"].encode("utf-8")) for item in selected)
    redacted_ranges = sum(len(item["ranges"]) for item in package["redaction_receipts"])
    truncated = sum(item["truncation"] is not None for item in selected)
    expected = {
        "candidate_count": len(selected) + len(omissions),
        "content_bytes": content_bytes,
        "omitted_source_count": len(omissions),
        "redacted_range_count": redacted_ranges,
        "selected_snippet_count": len(selected),
        "truncated_snippet_count": truncated,
    }
    for field, value in expected.items():
        if accounting[field] != value:
            raise ContractError(f"package.accounting.{field} does not match package content")
    if (accounting["actual_tokens"] > budget["max_tokens"] or
            content_bytes > budget["max_content_bytes"] or
            len(selected) > budget["max_snippets"]):
        raise ContractError("package accounting exceeds the declared budget")
    freshness = package["freshness"]
    if freshness["evaluated_at_unix_ms"] != package["source_binding"]["as_of_unix_ms"]:
        raise ContractError("package freshness does not match source binding")
    expiry = freshness["expires_at_unix_ms"]
    if expiry is not None and expiry <= freshness["evaluated_at_unix_ms"]:
        raise ContractError("package selected expiry must follow evaluation time")


def validate_package_shape(package: dict[str, object]) -> None:
    exact(package, PACKAGE_FIELDS, "package")
    if package["api_version"] != PACKAGE_VERSION or package["assembly_mode"] != ASSEMBLY_MODE:
        raise ContractError("unsupported package identity")
    if package["canonicalization"] != CANONICALIZATION or package["result"] != RESULT:
        raise ContractError("unsupported package semantics")
    for field in {"cache_key_sha256", "context_sha256", "projection_sha256", "request_sha256"}:
        sha(package[field], f"package.{field}")
    _validate_package_nested(package)
