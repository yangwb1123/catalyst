"""Authority-free deterministic ContextPackage v1 assembly."""

from __future__ import annotations

import copy

from .codec import ContractError, canonical_json, digest
from .constants import (CACHE_DOMAIN, CATEGORY_RANK, CONTENT_DOMAIN, CONTEXT_DOMAIN,
                        DELIMITER, LANES, NORMALIZATION, PACKAGE_VERSION, PROJECTION_DOMAIN,
                        REDACTION_REPLACEMENT, REQUEST_DOMAIN, RESULT, SNIPPET_DOMAIN,
                        ASSEMBLY_MODE, CANONICALIZATION)
from .shape import validate_package_shape, validate_request
from .token_counter import TokenCounter, checked_count


def _projection(lanes: dict[str, list[dict[str, object]]]) -> dict[str, object]:
    return {
        lane: [{"content": item["content"], "instruction_allowed": False,
                "source_id": item["source_id"]} for item in lanes[lane]]
        for lane in LANES
    }


def _lane(source: dict[str, object]) -> str:
    if (source["source_class"] in {"system_policy", "user_instruction"} and
            source["declared_lane"] == "instruction"):
        return "instruction_candidates"
    if source["declared_lane"] == "untrusted_data":
        return "untrusted_data"
    return "trusted_context"


def _ineligible_reason(source: dict[str, object], as_of: int) -> str | None:
    if source["availability"] == "missing":
        return "missing"
    if source["disposition"] == "deny":
        return "denied"
    if source["freshness"] == "stale":
        return "stale"
    if source["freshness"] == "contested":
        return "contested"
    if source["freshness"] == "unknown":
        return "unknown_freshness"
    expires = source["expires_at_unix_ms"]
    if expires is not None and as_of >= expires:
        return "expired"
    if source["injection_risk"] == "suspected":
        return "quarantined_prompt_injection"
    return None


def _redaction_map(request: dict[str, object]) -> dict[str, list[dict[str, object]]]:
    return {plan["source_id"]: plan["ranges"] for plan in request["redactions"]}


def _apply_redactions(content: str, ranges: list[dict[str, object]]) -> str:
    raw = content.encode("utf-8")
    result, cursor = bytearray(), 0
    for selected in ranges:
        result.extend(raw[cursor:selected["start_byte"]])
        result.extend(REDACTION_REPLACEMENT)
        cursor = selected["end_byte"]
    result.extend(raw[cursor:])
    return bytes(result).decode("utf-8")


def _utf8_prefix(content: str, maximum: int) -> str:
    raw = content.encode("utf-8")
    retained = raw[:maximum]
    while retained:
        try:
            return retained.decode("utf-8")
        except UnicodeDecodeError:
            retained = retained[:-1]
    return ""


def _snippet(source: dict[str, object], content: str,
             truncation: dict[str, object] | None) -> dict[str, object]:
    item: dict[str, object] = {
        "category": source["category"], "content": content,
        "declared_lane": source["declared_lane"],
        "declared_trust": source["declared_trust"], "delimiter": DELIMITER,
        "instruction_allowed": False,
        "lane": _lane(source), "normalization": NORMALIZATION,
        "projected_content_sha256": digest(CONTENT_DOMAIN, content.encode("utf-8")),
        "required": source["required"],
        "selection_reason": "required_source" if source["required"] else "priority_selection",
        "snippet_sha256": "", "source_class": source["source_class"],
        "source_content_sha256": source["content_sha256"], "source_id": source["source_id"],
        "source_ref": source["source_ref"], "source_revision": source["source_revision"],
        "truncation": truncation,
    }
    item["snippet_sha256"] = digest(SNIPPET_DOMAIN, canonical_json(item))
    return item


def _prepare(source: dict[str, object], ranges: list[dict[str, object]]) -> dict[str, object] | None:
    content = _apply_redactions(source["content"], ranges)
    size = len(content.encode("utf-8"))
    if size <= source["max_bytes"]:
        return _snippet(source, content, None)
    if source["required"]:
        raise ContractError(f"required source {source['source_id']} exceeds source max_bytes")
    if source["truncation"] == "forbidden":
        return None
    retained = _utf8_prefix(content, source["max_bytes"])
    if not retained:
        return None
    receipt = {
        "original_redacted_bytes": size, "reason": "source_max_bytes",
        "retained_bytes": len(retained.encode("utf-8")),
    }
    return _snippet(source, retained, receipt)


def _sort_key(item: dict[str, object]) -> tuple[int, int, bytes]:
    return (CATEGORY_RANK[item["category"]], -item["priority"],
            item["source_id"].encode("utf-8"))


def _omit(source: dict[str, object], reason: str) -> dict[str, object]:
    return {"reason": reason, "source_id": source["source_id"],
            "source_ref": source["source_ref"]}


def _collect(request: dict[str, object]) -> tuple[list[tuple[dict[str, object], dict[str, object]]],
                                                    list[dict[str, object]], list[dict[str, object]]]:
    as_of = request["source_binding"]["as_of_unix_ms"]
    plans = _redaction_map(request)
    candidates, omissions = [], []
    sources = sorted(request["sources"], key=_sort_key)
    for source in sources:
        if source["availability"] == "available" and source["source_id"] in plans:
            _apply_redactions(source["content"], plans[source["source_id"]])
        reason = _ineligible_reason(source, as_of)
        if reason is not None:
            if source["required"]:
                raise ContractError(f"required source {source['source_id']} is ineligible: {reason}")
            omissions.append(_omit(source, reason))
            continue
        snippet = _prepare(source, plans.get(source["source_id"], []))
        if snippet is None:
            omissions.append(_omit(source, "source_limit_exceeded"))
        else:
            candidates.append((source, snippet))
    receipts = [{"ranges": copy.deepcopy(plan["ranges"]), "source_id": plan["source_id"]}
                for plan in request["redactions"]]
    return candidates, omissions, receipts


def _try_add(lanes: dict[str, list[dict[str, object]]], snippet: dict[str, object],
             counter: TokenCounter, budget: dict[str, object], content_bytes: int,
             selected_count: int) -> tuple[str | None, int]:
    if selected_count + 1 > budget["max_snippets"]:
        return "snippet_budget_exceeded", 0
    size = len(snippet["content"].encode("utf-8"))
    if content_bytes + size > budget["max_content_bytes"]:
        return "content_budget_exceeded", 0
    tentative = {lane: list(values) for lane, values in lanes.items()}
    tentative[snippet["lane"]].append(snippet)
    tokens = checked_count(counter, budget, canonical_json(_projection(tentative)))
    if tokens > budget["max_tokens"]:
        return "token_budget_exceeded", tokens
    return None, tokens


def _select(candidates: list[tuple[dict[str, object], dict[str, object]]],
            omissions: list[dict[str, object]], counter: TokenCounter,
            budget: dict[str, object]) -> tuple[dict[str, list[dict[str, object]]], int, int]:
    lanes = {lane: [] for lane in LANES}
    baseline = checked_count(counter, budget, canonical_json(_projection(lanes)))
    if baseline > budget["max_tokens"]:
        raise ContractError("token budget cannot represent the empty projection")
    content_bytes, actual_tokens = 0, baseline
    ordered = ([item for item in candidates if item[0]["required"]] +
               [item for item in candidates if not item[0]["required"]])
    for source, snippet in ordered:
        selected_count = sum(len(values) for values in lanes.values())
        reason, tokens = _try_add(lanes, snippet, counter, budget, content_bytes, selected_count)
        if reason is not None:
            if source["required"]:
                raise ContractError(f"required source {source['source_id']} exceeds {reason}")
            omissions.append(_omit(source, reason))
            continue
        lanes[snippet["lane"]].append(snippet)
        content_bytes += len(snippet["content"].encode("utf-8"))
        actual_tokens = tokens
    return lanes, content_bytes, actual_tokens


def assemble(request: dict[str, object], counter: TokenCounter) -> dict[str, object]:
    """Validate and assemble one deterministic authority-free package."""
    validate_request(request)
    request_bytes = canonical_json(request)
    candidates, omissions, receipts = _collect(request)
    lanes, content_bytes, actual_tokens = _select(
        candidates, omissions, counter, request["budget"])
    projection_bytes = canonical_json(_projection(lanes))
    selected = [item for lane in LANES for item in lanes[lane]]
    expiries = [item[0]["expires_at_unix_ms"] for item in candidates
                if item[0]["source_id"] in {entry["source_id"] for entry in selected}
                and item[0]["expires_at_unix_ms"] is not None]
    omissions.sort(key=lambda item: item["source_id"].encode("utf-8"))
    package = _package_body(request, request_bytes, lanes, omissions, receipts,
                            candidates, content_bytes, actual_tokens, expiries)
    package["context_sha256"] = digest(CONTEXT_DOMAIN, canonical_json(package))
    validate_package_shape(package)
    return package


def _package_body(request: dict[str, object], request_bytes: bytes,
                  lanes: dict[str, list[dict[str, object]]], omissions: list[dict[str, object]],
                  receipts: list[dict[str, object]], candidates: list[tuple[dict[str, object], dict[str, object]]],
                  content_bytes: int, actual_tokens: int, expiries: list[int]) -> dict[str, object]:
    selected = [item for lane in LANES for item in lanes[lane]]
    return {
        "accounting": {"actual_tokens": actual_tokens,
                       "candidate_count": len(request["sources"]),
                       "content_bytes": content_bytes, "omitted_source_count": len(omissions),
                       "redacted_range_count": sum(len(item["ranges"]) for item in receipts),
                       "selected_snippet_count": len(selected),
                       "truncated_snippet_count": sum(item["truncation"] is not None for item in selected)},
        "api_version": PACKAGE_VERSION, "assembly_mode": ASSEMBLY_MODE,
        "budget": copy.deepcopy(request["budget"]),
        "cache_key_sha256": digest(CACHE_DOMAIN, request_bytes),
        "canonicalization": CANONICALIZATION, "context_sha256": "",
        "freshness": {"evaluated_at_unix_ms": request["source_binding"]["as_of_unix_ms"],
                      "expires_at_unix_ms": min(expiries) if expiries else None},
        "lanes": lanes, "omissions": omissions,
        "projection_sha256": digest(PROJECTION_DOMAIN, canonical_json(_projection(lanes))),
        "redaction_receipts": receipts,
        "request_sha256": digest(REQUEST_DOMAIN, request_bytes), "result": RESULT,
        "source_binding": copy.deepcopy(request["source_binding"]),
        "task_binding": copy.deepcopy(request["task_binding"]),
    }


def validate_package(request: dict[str, object], package: dict[str, object],
                     counter: TokenCounter) -> None:
    """Reassemble and demand an exact package match."""
    validate_package_shape(package)
    expected = assemble(request, counter)
    if canonical_json(package) != canonical_json(expected):
        raise ContractError("context package does not exactly match deterministic reassembly")


def validate_cache_hit(request: dict[str, object], package: dict[str, object],
                       counter: TokenCounter) -> None:
    """Accept a cache hit only for the same request key after full revalidation."""
    request_key = digest(CACHE_DOMAIN, canonical_json(request))
    if package.get("cache_key_sha256") != request_key:
        raise ContractError("cached package key does not match request")
    validate_package(request, package, counter)
