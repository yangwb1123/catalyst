"""Pure unverified-legacy projection and closed view validation."""

from __future__ import annotations

from collections import defaultdict
from typing import Any

from .canonical import (
    ContractError, canonical_json, decode_canonical, digest, require_exact_fields,
    self_digest, sha256_bytes, validate_text,
)
from .constants import (
    ADR_KIND, CANDIDATE_DOMAIN, CANDIDATE_ID_DOMAIN, CANONICALIZATION,
    CONFLICT_DOMAIN, MAX_ADR_BYTES, MAX_ADR_SOURCES, MAX_MEMORY_BYTES,
    MAX_MEMORY_ENTRIES, MAX_RAW_BYTES, MAX_REQUEST_BYTES, MAX_SOURCE_REF_BYTES,
    MAX_VIEW_BYTES, MEMORY_KIND, RESULT, SOURCE_SET_DOMAIN, SUPERSESSION_DOMAIN,
    VIEW_API, VIEW_DOMAIN, VIEW_KIND,
)
from .memory import parse_jsonl
from .source import (
    decode_base64url, decode_request, encode_base64url, require_digest,
    source_descriptors, validate_binding,
)

VIEW_FIELDS = {
    "api_version", "attestations", "binding", "candidates", "canonicalization",
    "conflict_sets", "declared_supersessions", "kind", "request_sha256", "result",
    "source_set_sha256", "sources", "view_sha256",
}
ATTESTATION_FIELDS = {
    "acceptance", "authority", "confidence_interpretation", "conflict_resolution",
    "currentness", "instruction_eligibility", "persistence", "runtime_effect",
    "source_authentication", "source_completeness", "status_interpretation", "truth",
    "winner_selection",
}
SOURCE_DESCRIPTOR_FIELDS = {"byte_count", "content_sha256", "source_kind", "source_ref"}
COMMON_CANDIDATE_FIELDS = {
    "authority", "candidate_id", "candidate_sha256", "current", "hardness",
    "instruction_allowed", "ordinal", "raw_byte_count", "raw_bytes_base64url",
    "raw_sha256", "request_sha256", "source_kind", "source_ref", "trust_state",
}
MEMORY_CANDIDATE_FIELDS = COMMON_CANDIDATE_FIELDS | {
    "confidence", "created_at_unix", "declared_kind", "declared_source",
    "declared_supersedes", "declared_topic", "detail", "iteration", "legacy_format",
}
ADR_CANDIDATE_FIELDS = COMMON_CANDIDATE_FIELDS | {"document_name", "parsing"}
CONFIDENCE_FIELDS = {"presence", "raw_number_lexeme"}
CONFLICT_FIELDS = {
    "conflict_set_id", "declared_kind", "declared_topic", "member_candidate_ids",
}
SUPERSESSION_FIELDS = {
    "declaration_id", "declared_supersedes", "declaring_candidate_id", "relation_state",
}


def _candidate_base(request: dict[str, Any], source: dict[str, Any], ordinal: int,
                    raw: bytes) -> dict[str, Any]:
    locator = {
        "ordinal": ordinal,
        "request_sha256": request["request_sha256"],
        "source_kind": source["source_kind"],
        "source_ref": source["source_ref"],
    }
    return {
        "authority": None,
        "candidate_id": digest(CANDIDATE_ID_DOMAIN, locator, MAX_REQUEST_BYTES,
                               "candidate locator"),
        "candidate_sha256": "",
        "current": False,
        "hardness": "none",
        "instruction_allowed": False,
        "ordinal": ordinal,
        "raw_byte_count": len(raw),
        "raw_bytes_base64url": encode_base64url(raw),
        "raw_sha256": sha256_bytes(raw),
        "request_sha256": request["request_sha256"],
        "source_kind": source["source_kind"],
        "source_ref": source["source_ref"],
        "trust_state": "unverified_legacy",
    }


def _seal(candidate: dict[str, Any]) -> dict[str, Any]:
    candidate["candidate_sha256"] = self_digest(
        CANDIDATE_DOMAIN, candidate, "candidate_sha256", MAX_VIEW_BYTES, "candidate")
    return candidate


def _memory_candidates(request: dict[str, Any], source: dict[str, Any],
                       raw: bytes) -> list[dict[str, Any]]:
    candidates = []
    for ordinal, line, entry in parse_jsonl(raw):
        candidate = _candidate_base(request, source, ordinal, line)
        candidate.update(entry)
        candidates.append(_seal(candidate))
    return candidates


def _adr_candidate(request: dict[str, Any], source: dict[str, Any],
                   raw: bytes) -> dict[str, Any]:
    candidate = _candidate_base(request, source, 1, raw)
    candidate.update({"document_name": source["source_ref"], "parsing": "not_performed"})
    return _seal(candidate)


def _conflicts(request_sha: str, candidates: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: dict[tuple[str, str], list[str]] = defaultdict(list)
    for candidate in candidates:
        if candidate["source_kind"] == MEMORY_KIND:
            groups[(candidate["declared_kind"], candidate["declared_topic"])].append(
                candidate["candidate_id"])
    result = []
    for (kind, topic), members in sorted(groups.items()):
        if len(members) < 2:
            continue
        unique_members = sorted(set(members))
        if len(unique_members) != len(members):
            raise ContractError("conflict member candidate IDs must be unique")
        members = unique_members
        preimage = {
            "declared_kind": kind,
            "declared_topic": topic,
            "member_candidate_ids": members,
            "request_sha256": request_sha,
        }
        result.append({
            "conflict_set_id": digest(CONFLICT_DOMAIN, preimage, MAX_VIEW_BYTES,
                                      "conflict set"),
            "declared_kind": kind,
            "declared_topic": topic,
            "member_candidate_ids": members,
        })
    return result


def _supersessions(request_sha: str,
                   candidates: list[dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    for candidate in candidates:
        target = candidate.get("declared_supersedes")
        if target is None:
            continue
        preimage = {
            "declared_supersedes": target,
            "declaring_candidate_id": candidate["candidate_id"],
            "request_sha256": request_sha,
        }
        result.append({
            "declaration_id": digest(SUPERSESSION_DOMAIN, preimage, MAX_VIEW_BYTES,
                                     "supersession declaration"),
            "declared_supersedes": target,
            "declaring_candidate_id": candidate["candidate_id"],
            "relation_state": "unresolved_unverified_legacy",
        })
    return result


def project_request(request_raw: bytes) -> bytes:
    request, raws = decode_request(request_raw)
    candidates: list[dict[str, Any]] = []
    for source, raw in zip(request["sources"], raws, strict=True):
        if source["source_kind"] == MEMORY_KIND:
            candidates.extend(_memory_candidates(request, source, raw))
        else:
            candidates.append(_adr_candidate(request, source, raw))
    candidate_ids = [candidate["candidate_id"] for candidate in candidates]
    if len(set(candidate_ids)) != len(candidate_ids):
        raise ContractError("projected candidate IDs must be unique")
    descriptors = source_descriptors(request)
    view = {
        "api_version": VIEW_API,
        "attestations": {field: False for field in sorted(ATTESTATION_FIELDS)},
        "binding": request["binding"],
        "candidates": candidates,
        "canonicalization": CANONICALIZATION,
        "conflict_sets": _conflicts(request["request_sha256"], candidates),
        "declared_supersessions": _supersessions(request["request_sha256"], candidates),
        "kind": VIEW_KIND,
        "request_sha256": request["request_sha256"],
        "result": RESULT,
        "source_set_sha256": digest(SOURCE_SET_DOMAIN, descriptors, MAX_VIEW_BYTES,
                                    "source descriptor set"),
        "sources": descriptors,
        "view_sha256": "",
    }
    view["view_sha256"] = self_digest(
        VIEW_DOMAIN, view, "view_sha256", MAX_VIEW_BYTES, "view")
    return canonical_json(view, MAX_VIEW_BYTES, "view")


def _validate_candidate(candidate: Any, index: int, request_sha: str) -> dict[str, Any]:
    if not isinstance(candidate, dict):
        raise ContractError(f"candidates[{index}] must be an object")
    candidate_kind = candidate.get("source_kind")
    if not isinstance(candidate_kind, str) or candidate_kind not in {MEMORY_KIND, ADR_KIND}:
        raise ContractError("candidate source_kind is unsupported")
    fields = MEMORY_CANDIDATE_FIELDS if candidate_kind == MEMORY_KIND \
        else ADR_CANDIDATE_FIELDS
    require_exact_fields(candidate, fields, f"candidates[{index}]")
    if candidate["request_sha256"] != request_sha:
        raise ContractError("candidate is not bound to the view request")
    if (candidate["authority"] is not None or candidate["current"] is not False or
            candidate["hardness"] != "none" or
            candidate["instruction_allowed"] is not False or
            candidate["trust_state"] != "unverified_legacy"):
        raise ContractError("candidate authority/trust constants are not frozen")
    validate_text(candidate["source_ref"], "candidate source_ref", MAX_SOURCE_REF_BYTES)
    if not isinstance(candidate["ordinal"], int) or isinstance(candidate["ordinal"], bool) \
            or candidate["ordinal"] < 1:
        raise ContractError("candidate ordinal must be positive")
    raw = decode_base64url(candidate["raw_bytes_base64url"], "candidate raw bytes")
    if (isinstance(candidate["raw_byte_count"], bool) or
            candidate["raw_byte_count"] != len(raw) or
            candidate["raw_sha256"] != sha256_bytes(raw)):
        raise ContractError("candidate raw byte pins do not match")
    locator = {field: candidate[field] for field in
               ("ordinal", "request_sha256", "source_kind", "source_ref")}
    if candidate["candidate_id"] != digest(
            CANDIDATE_ID_DOMAIN, locator, MAX_REQUEST_BYTES, "candidate locator"):
        raise ContractError("candidate_id does not match its request-bound locator")
    _validate_candidate_raw(candidate, raw)
    require_digest(candidate["candidate_id"], "candidate_id")
    expected = self_digest(CANDIDATE_DOMAIN, candidate, "candidate_sha256",
                           MAX_VIEW_BYTES, "candidate")
    if candidate["candidate_sha256"] != expected:
        raise ContractError("candidate_sha256 does not match")
    return candidate


def _validate_candidate_raw(candidate: dict[str, Any], raw: bytes) -> None:
    try:
        raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ContractError("candidate raw bytes are not strict UTF-8") from error
    if candidate["source_kind"] == MEMORY_KIND:
        if candidate["ordinal"] > MAX_MEMORY_ENTRIES:
            raise ContractError("memory candidate ordinal exceeds its entry bound")
        if b"\n" in raw or b"\r" in raw:
            raise ContractError("memory candidate raw bytes must be one unframed line")
        parsed = parse_jsonl(raw + b"\n")[0][2]
        if any(candidate[field] != value for field, value in parsed.items()):
            raise ContractError("memory candidate projection does not match raw line")
    elif (candidate["ordinal"] != 1 or
          candidate["document_name"] != candidate["source_ref"] or
          candidate["parsing"] != "not_performed" or b"\r" in raw or
          not raw.endswith(b"\n") or len(raw) > MAX_ADR_BYTES):
        raise ContractError("ADR candidate framing or no-parse constants do not match")


def decode_view(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_VIEW_BYTES, "legacy import view")
    view = require_exact_fields(value, VIEW_FIELDS, "view")
    if (view["api_version"] != VIEW_API or view["kind"] != VIEW_KIND or
            view["canonicalization"] != CANONICALIZATION or view["result"] != RESULT):
        raise ContractError("view frozen identity constants do not match")
    require_digest(view["request_sha256"], "view.request_sha256")
    validate_binding(view["binding"])
    attestations = require_exact_fields(view["attestations"], ATTESTATION_FIELDS,
                                        "view.attestations")
    if any(value is not False for value in attestations.values()):
        raise ContractError("every view attestation must be false")
    if not isinstance(view["candidates"], list):
        raise ContractError("view candidates must be an array")
    candidates = [_validate_candidate(item, index, view["request_sha256"])
                  for index, item in enumerate(view["candidates"])]
    keys = [(item["source_kind"], item["source_ref"], item["ordinal"])
            for item in candidates]
    if keys != sorted(keys) or len(set(keys)) != len(keys):
        raise ContractError("view candidates are not uniquely source/ordinal ordered")
    candidate_ids = [item["candidate_id"] for item in candidates]
    if len(set(candidate_ids)) != len(candidate_ids):
        raise ContractError("view candidate IDs must be unique")
    _validate_view_sources(view)
    if view["conflict_sets"] != _conflicts(view["request_sha256"], candidates):
        raise ContractError("view conflict sets are not the complete deterministic grouping")
    if view["declared_supersessions"] != _supersessions(
            view["request_sha256"], candidates):
        raise ContractError("view supersessions do not preserve every declaration")
    _reconstruct(view)
    expected_sha = self_digest(VIEW_DOMAIN, view, "view_sha256", MAX_VIEW_BYTES, "view")
    if view["view_sha256"] != expected_sha:
        raise ContractError("view_sha256 does not match")
    return view


def _validate_view_sources(view: dict[str, Any]) -> None:
    if (not isinstance(view["sources"], list) or not view["sources"] or
            len(view["sources"]) > MAX_ADR_SOURCES + 1):
        raise ContractError("view sources must be a nonempty array")
    source_keys = []
    for descriptor in view["sources"]:
        require_exact_fields(descriptor, SOURCE_DESCRIPTOR_FIELDS, "view source descriptor")
        require_digest(descriptor["content_sha256"], "source content_sha256")
        if (not isinstance(descriptor["byte_count"], int) or
                isinstance(descriptor["byte_count"], bool) or descriptor["byte_count"] < 1):
            raise ContractError("source descriptor byte_count must be positive int64")
        if (not isinstance(descriptor["source_kind"], str) or
                descriptor["source_kind"] not in {MEMORY_KIND, ADR_KIND}):
            raise ContractError("source descriptor kind is unsupported")
        validate_text(descriptor["source_ref"], "source descriptor ref",
                      MAX_SOURCE_REF_BYTES)
        source_keys.append((descriptor["source_kind"], descriptor["source_ref"]))
    if source_keys != sorted(source_keys) or len(set(source_keys)) != len(source_keys):
        raise ContractError("view sources are not uniquely ordered")
    expected_source_set = digest(SOURCE_SET_DOMAIN, view["sources"], MAX_VIEW_BYTES,
                                 "source descriptor set")
    if view["source_set_sha256"] != expected_source_set:
        raise ContractError("view source_set_sha256 does not match")


def _reconstruct(view: dict[str, Any]) -> dict[tuple[str, str], bytes]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for candidate in view["candidates"]:
        grouped[(candidate["source_kind"], candidate["source_ref"])].append(candidate)
    result = {}
    memory_count = 0
    adr_count = 0
    total = 0
    for descriptor in view["sources"]:
        require_exact_fields(descriptor, SOURCE_DESCRIPTOR_FIELDS, "view source descriptor")
        key = (descriptor["source_kind"], descriptor["source_ref"])
        members = grouped.pop(key, [])
        if descriptor["source_kind"] == MEMORY_KIND:
            if not 1 <= len(members) <= MAX_MEMORY_ENTRIES:
                raise ContractError("memory source candidate count is outside 1..4096")
            if [item["ordinal"] for item in members] != list(range(1, len(members) + 1)):
                raise ContractError("memory candidate ordinals are not contiguous")
            raw = b"\n".join(decode_base64url(item["raw_bytes_base64url"], "memory line")
                             for item in members) + b"\n"
            memory_count += 1
            if len(raw) > MAX_MEMORY_BYTES:
                raise ContractError("reconstructed memory source exceeds bound")
        elif descriptor["source_kind"] == ADR_KIND and len(members) == 1:
            raw = decode_base64url(members[0]["raw_bytes_base64url"], "ADR document")
            adr_count += 1
            if len(raw) > MAX_ADR_BYTES:
                raise ContractError("reconstructed ADR source exceeds bound")
        else:
            raise ContractError("view source candidate cardinality is invalid")
        if len(raw) != descriptor["byte_count"] or sha256_bytes(raw) != descriptor["content_sha256"]:
            raise ContractError("reconstructed source does not match its descriptor")
        result[key] = raw
        total += len(raw)
    if grouped:
        raise ContractError("view has candidates outside declared sources")
    if memory_count > 1 or adr_count > MAX_ADR_SOURCES or total > MAX_RAW_BYTES:
        raise ContractError("reconstructed sources exceed contract cardinality or size")
    return result


def validate_view_against_request(view_raw: bytes, request_raw: bytes) -> dict[str, Any]:
    request, supplied = decode_request(request_raw)
    view = decode_view(view_raw)
    if view["binding"] != request["binding"] or view["request_sha256"] != request["request_sha256"]:
        raise ContractError("view binding does not match request")
    reconstructed = _reconstruct(view)
    expected_sources = {(source["source_kind"], source["source_ref"]): raw
                        for source, raw in zip(request["sources"], supplied, strict=True)}
    if reconstructed != expected_sources:
        raise ContractError("view does not reconstruct the exact supplied request bytes")
    expected = project_request(request_raw)
    if canonical_json(view, MAX_VIEW_BYTES, "view") != expected:
        raise ContractError("view is not the unique deterministic projection of request")
    return view
