#!/usr/bin/env python3
"""Resolve frontend artifacts separately from typed proof claims."""
import hashlib
import re
import struct
import zlib

from engineering_check_support import repo_path_issue, unique_id_issues, unknown_field_issues
from .contract import (
    CLAIM_CLASS_ARTIFACT_KINDS,
    CLAIM_CLASS_CLAIMANTS,
    CLAIM_CLASS_PROOF_TYPES,
    CLAIM_CLASS_RESULTS,
    MAX_ARTIFACT_BYTES,
    PROOF_TYPES,
)

DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
GOOD_RESULTS = {"observed", "passed"}
ARTIFACT_FIELDS = {
    "id", "kind", "media_type", "locator", "content_sha256", "bytes",
    "source_revision", "integrity", "provenance", "producer", "producer_id",
}
CLAIM_FIELDS = {
    "id", "claim_class", "proof_types", "subject_type", "subject_id",
    "artifact_ids", "claimant", "claimant_id", "result",
}
ARTIFACT_KINDS = {
    "source", "tool_output", "review_output", "screenshot", "trace",
    "visual_diff", "accessibility_report",
}
SUBJECT_TYPES = {
    "classification", "decision", "readiness", "flow", "state_model",
    "verification_case", "assumption", "risk", "review", "applicability",
    "profile_override",
}
PROOF_KIND_REQUIREMENTS = {
    "interaction_execution_receipts": {"trace"},
    "capture_receipt": {"screenshot"},
    "visual_diff_receipts": {"visual_diff"},
    "accessibility_execution_receipts": {"accessibility_report"},
    "static_execution_receipts": {"tool_output"},
    "performance_measurement": {"tool_output", "trace"},
    "geometry_measurement_receipts": {"tool_output"},
    "independent_visual_review": {"review_output"},
    "independent_review": {"review_output"},
    "permission_action_review": {"review_output"},
    "applicability_assessment": {"review_output"},
    "profile_override_review": {"review_output"},
    "architecture_decision_record": {"source"},
}
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
PNG_MAX_DECODED_BYTES = 64 * 1024 * 1024
PNG_CHANNELS = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
PNG_DEPTHS = {0: {1, 2, 4, 8, 16}, 2: {8, 16}, 3: {1, 2, 4, 8}, 4: {8, 16}, 6: {8, 16}}
ADAM7_PASSES = ((0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
                 (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2))


def _shape(record, fields, label):
    if not isinstance(record, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(record, fields, label)
    missing = fields - set(record)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _strings(value, label, *, non_empty=False):
    if not isinstance(value, list) or (non_empty and not value):
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list"]
    if not all(isinstance(item, str) and item.strip() for item in value):
        return [f"{label}: values must be non-empty strings"]
    return [f"{label}: values must be unique"] if len(value) != len(set(value)) else []


def _bounded_string(record, field, maximum, label):
    value = record.get(field)
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        return [f"{label}.{field}: must be non-empty string <= {maximum} characters"]
    return []


def _stream_sha256(path):
    digest, total = hashlib.sha256(), 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            total += len(chunk)
            if total > MAX_ARTIFACT_BYTES:
                raise ValueError(f"artifact exceeds {MAX_ARTIFACT_BYTES} bytes while reading")
            digest.update(chunk)
    return "sha256:" + digest.hexdigest(), total


def _resolved_file_issues(record, label, repo_root):
    digest, locator = record.get("content_sha256"), record.get("locator")
    issues = []
    if not isinstance(digest, str) or not DIGEST.fullmatch(digest):
        issues.append(f"{label}.content_sha256: requires sha256:<64 lowercase hex>")
    issue = repo_path_issue(repo_root, locator, f"{label}.locator")
    if issue:
        return issues + [issue]
    target = repo_root / locator
    try:
        if not target.is_file():
            return issues + [f"{label}.locator: must resolve to a regular file"]
        actual, size = _stream_sha256(target)
    except (OSError, ValueError) as exc:
        return issues + [f"{label}.locator: cannot read artifact ({exc})"]
    if record.get("bytes") != size:
        issues.append(f"{label}.bytes: does not match current file size")
    if isinstance(digest, str) and DIGEST.fullmatch(digest) and digest != actual:
        issues.append(f"{label}.content_sha256: does not match current file bytes")
    if record.get("kind") == "screenshot":
        issues.extend(_png_issues(target, record, label))
    return issues


def _pass_size(width, height, x0, y0, dx, dy, bits_per_pixel):
    pass_width = max(0, (width - x0 + dx - 1) // dx)
    pass_height = max(0, (height - y0 + dy - 1) // dy)
    return pass_height * (((pass_width * bits_per_pixel + 7) // 8) + 1) if pass_width else 0


def _decoded_size(width, height, bits_per_pixel, interlace):
    if interlace == 0:
        return height * (((width * bits_per_pixel + 7) // 8) + 1)
    return sum(_pass_size(width, height, *item, bits_per_pixel) for item in ADAM7_PASSES)


def _inflate_pixels(chunks, expected):
    decoder, pixels = zlib.decompressobj(), bytearray()
    for compressed in chunks:
        pending = compressed
        while pending:
            output = decoder.decompress(pending, min(1024 * 1024, expected - len(pixels) + 1))
            pixels.extend(output)
            if len(pixels) > expected:
                raise ValueError("decoded pixels exceed IHDR dimensions")
            remainder = decoder.unconsumed_tail
            if remainder == pending and not output:
                raise ValueError("PNG zlib stream made no progress")
            pending = remainder
    if not decoder.eof or decoder.unused_data or len(pixels) != expected:
        raise ValueError("PNG pixel stream is truncated or inconsistent with IHDR")
    return pixels


def _scanline_shapes(width, height, bits_per_pixel, interlace):
    if interlace == 0:
        return [(height, (width * bits_per_pixel + 7) // 8)]
    shapes = []
    for x0, y0, dx, dy in ADAM7_PASSES:
        pass_width = max(0, (width - x0 + dx - 1) // dx)
        pass_height = max(0, (height - y0 + dy - 1) // dy)
        if pass_width and pass_height:
            shapes.append((pass_height, (pass_width * bits_per_pixel + 7) // 8))
    return shapes


def _validate_filter_bytes(pixels, shapes):
    offset = 0
    for rows, row_bytes in shapes:
        for _ in range(rows):
            if pixels[offset] > 4:
                raise ValueError("PNG scanline uses an invalid filter type")
            offset += row_bytes + 1
    if offset != len(pixels):
        raise ValueError("PNG scanline layout does not match IHDR")


def _png_chunks(data):
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError("missing PNG signature")
    position, ihdr, idat, plte = 8, None, [], None
    seen_idat, ended_idat, saw_iend = False, False, False
    while position < len(data):
        if len(data) - position < 12:
            raise ValueError("truncated PNG chunk")
        length = struct.unpack(">I", data[position:position + 4])[0]
        chunk_type = data[position + 4:position + 8]
        end = position + 12 + length
        if end > len(data):
            raise ValueError("truncated PNG chunk payload")
        payload, stored_crc = data[position + 8:position + 8 + length], data[position + 8 + length:end]
        if zlib.crc32(chunk_type + payload) & 0xffffffff != struct.unpack(">I", stored_crc)[0]:
            raise ValueError("PNG chunk CRC mismatch")
        if ihdr is None and chunk_type != b"IHDR":
            raise ValueError("IHDR must be the first PNG chunk")
        if chunk_type == b"IHDR":
            if ihdr is not None or length != 13:
                raise ValueError("PNG must contain one 13-byte IHDR")
            ihdr = payload
        elif chunk_type == b"IDAT":
            if ended_idat:
                raise ValueError("PNG IDAT chunks must be contiguous")
            seen_idat, idat = True, idat + [payload]
        elif chunk_type == b"PLTE":
            if plte is not None or seen_idat or not 0 < length <= 768 or length % 3:
                raise ValueError("PNG PLTE is duplicate, misplaced, or malformed")
            plte = payload
        elif chunk_type == b"IEND":
            if length != 0 or end != len(data):
                raise ValueError("IEND must be empty and final")
            saw_iend = True
        elif chunk_type[:1].isalpha() and chunk_type[:1].isupper():
            raise ValueError(f"unsupported critical PNG chunk {chunk_type!r}")
        elif seen_idat:
            ended_idat = True
        position = end
        if saw_iend:
            break
    if ihdr is None or not idat or not saw_iend:
        raise ValueError("PNG requires IHDR, IDAT and IEND chunks")
    return ihdr, idat, plte


def _parse_png(path):
    ihdr, idat, plte = _png_chunks(path.read_bytes())
    width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", ihdr)
    if width <= 0 or height <= 0 or width * height > 32_000_000:
        raise ValueError("PNG dimensions are invalid or exceed 32MP")
    if depth not in PNG_DEPTHS.get(color, set()) or (compression, filtering) != (0, 0) or interlace not in {0, 1}:
        raise ValueError("unsupported or invalid PNG IHDR encoding")
    if color == 3 and (plte is None or len(plte) // 3 > 2 ** depth):
        raise ValueError("indexed PNG requires a valid PLTE")
    if color in {0, 4} and plte is not None:
        raise ValueError("grayscale PNG cannot contain PLTE")
    expected = _decoded_size(width, height, PNG_CHANNELS[color] * depth, interlace)
    if expected > PNG_MAX_DECODED_BYTES:
        raise ValueError("decoded PNG exceeds the 64MiB verification budget")
    pixels = _inflate_pixels(idat, expected)
    _validate_filter_bytes(pixels, _scanline_shapes(width, height,
                                                    PNG_CHANNELS[color] * depth, interlace))
    return width, height


def _png_issues(path, record, label):
    if record.get("media_type") != "image/png":
        return [f"{label}.media_type: screenshot artifacts must be image/png"]
    try:
        _parse_png(path)
    except (OSError, ValueError, struct.error, zlib.error) as exc:
        return [f"{label}.locator: screenshot is not a structurally decodable PNG ({exc})"]
    return []


def png_dimensions(path):
    """Return dimensions after bounded chunk, CRC, and zlib validation."""
    return _parse_png(path)


def artifact_issues(record, label, repo_root, source_revision):
    issues = _shape(record, ARTIFACT_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    for field, maximum in (("id", 128), ("media_type", 128), ("locator", 512),
                           ("source_revision", 256), ("producer", 64),
                           ("producer_id", 128)):
        issues.extend(_bounded_string(record, field, maximum, label))
    kind = record.get("kind")
    if not isinstance(kind, str) or kind not in ARTIFACT_KINDS:
        issues.append(f"{label}.kind: invalid")
    if record.get("integrity") != "digest_bound" or record.get("provenance") != "declarative":
        issues.append(f"{label}: v1 integrity/provenance claims were overstated")
    if record.get("source_revision") != source_revision:
        issues.append(f"{label}.source_revision: does not match package source_revision")
    size = record.get("bytes")
    if not isinstance(size, int) or isinstance(size, bool) or size < 0 or size > MAX_ARTIFACT_BYTES:
        issues.append(f"{label}.bytes: invalid or exceeds {MAX_ARTIFACT_BYTES}")
    issues.extend(_resolved_file_issues(record, label, repo_root))
    return issues


def claim_issues(record, label, artifacts):
    issues = _shape(record, CLAIM_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    for field, maximum in (("id", 128), ("subject_type", 64), ("subject_id", 128),
                           ("claimant", 64), ("claimant_id", 128)):
        issues.extend(_bounded_string(record, field, maximum, label))
    claim_class = record.get("claim_class")
    if not isinstance(claim_class, str) or claim_class not in CLAIM_CLASS_PROOF_TYPES:
        issues.append(f"{label}.claim_class: invalid")
    proof_types = record.get("proof_types")
    issues.extend(_strings(proof_types, f"{label}.proof_types", non_empty=True))
    known = set(proof_types) if isinstance(proof_types, list) and all(isinstance(x, str) for x in proof_types) else set()
    if known - PROOF_TYPES:
        issues.append(f"{label}.proof_types: contains unknown proof type")
    allowed_proofs = CLAIM_CLASS_PROOF_TYPES.get(claim_class, set()) \
        if isinstance(claim_class, str) else set()
    if not known or not known <= allowed_proofs:
        issues.append(f"{label}: proof types do not match claim_class")
    claimant = record.get("claimant")
    allowed_claimants = CLAIM_CLASS_CLAIMANTS.get(claim_class, set()) \
        if isinstance(claim_class, str) else set()
    if not isinstance(claimant, str) or claimant not in allowed_claimants:
        issues.append(f"{label}.claimant: does not match claim_class")
    result = record.get("result")
    allowed_results = CLAIM_CLASS_RESULTS.get(claim_class, set()) \
        if isinstance(claim_class, str) else set()
    if not isinstance(result, str) or result not in allowed_results:
        issues.append(f"{label}.result: does not match claim_class")
    subject_type = record.get("subject_type")
    if not isinstance(subject_type, str) or subject_type not in SUBJECT_TYPES:
        issues.append(f"{label}.subject_type: invalid")
    issues.extend(_claim_artifact_issues(record, label, artifacts))
    return issues


def _claim_artifact_issues(record, label, artifacts):
    refs = record.get("artifact_ids")
    issues = _strings(refs, f"{label}.artifact_ids", non_empty=True)
    if not isinstance(refs, list) or not all(isinstance(ref, str) for ref in refs):
        return issues
    missing = set(refs) - set(artifacts)
    if missing:
        issues.append(f"{label}.artifact_ids: unknown artifact ids {sorted(missing)}")
    claim_class = record.get("claim_class")
    allowed = CLAIM_CLASS_ARTIFACT_KINDS.get(claim_class, set()) \
        if isinstance(claim_class, str) else set()
    kinds = {artifacts[ref].get("kind") for ref in refs
             if ref in artifacts and isinstance(artifacts[ref], dict)
             and isinstance(artifacts[ref].get("kind"), str)}
    if not kinds or not kinds <= allowed:
        issues.append(f"{label}.artifact_ids: artifact kinds do not match claim_class")
    for proof_type in record.get("proof_types") if isinstance(record.get("proof_types"), list) else []:
        required = PROOF_KIND_REQUIREMENTS.get(proof_type) if isinstance(proof_type, str) else None
        if required and not kinds & required:
            issues.append(f"{label}: proof type {proof_type!r} requires artifact kind {sorted(required)}")
    if isinstance(claim_class, str) and claim_class in {"execution_observation", "review_observation"}:
        for ref in refs:
            artifact = artifacts.get(ref)
            if isinstance(artifact, dict) and (artifact.get("producer") != record.get("claimant")
                                               or artifact.get("producer_id") != record.get("claimant_id")):
                issues.append(f"{label}: claimant identity does not match artifact producer for {ref!r}")
    return issues


def build_evidence_indexes(package, repo_root, label="FrontendDesignPackage"):
    artifacts, issues = package.get("evidence_artifacts"), []
    id_issues, _ = unique_id_issues(artifacts, label, "evidence artifact")
    issues.extend(id_issues)
    if not isinstance(artifacts, list) or not artifacts or len(artifacts) > 512:
        return issues + [f"{label}.evidence_artifacts: expected 1..512 records"], {}, {}
    artifact_index = {}
    for index, record in enumerate(artifacts):
        issues.extend(artifact_issues(record, f"{label}.evidence_artifacts[{index}]", repo_root,
                                      package.get("source_revision")))
        if isinstance(record, dict) and isinstance(record.get("id"), str):
            artifact_index[record["id"]] = record
    claims = package.get("proof_claims")
    id_issues, _ = unique_id_issues(claims, label, "proof claim")
    issues.extend(id_issues)
    if not isinstance(claims, list) or not claims or len(claims) > 512:
        return issues + [f"{label}.proof_claims: expected 1..512 records"], artifact_index, {}
    claim_index = {}
    for index, record in enumerate(claims):
        issues.extend(claim_issues(record, f"{label}.proof_claims[{index}]", artifact_index))
        if isinstance(record, dict) and isinstance(record.get("id"), str):
            claim_index[record["id"]] = record
    used = {ref for record in claims if isinstance(record, dict)
            for ref in record.get("artifact_ids", []) if isinstance(ref, str)}
    orphaned = set(artifact_index) - used
    if orphaned:
        issues.append(f"{label}.evidence_artifacts: orphaned artifact ids {sorted(orphaned)}")
    return issues, artifact_index, claim_index


def claim_refs_issues(refs, label, claims, *, positive=False, non_empty=False):
    issues = _strings(refs, label, non_empty=non_empty)
    if not isinstance(refs, list) or not all(isinstance(ref, str) for ref in refs):
        return issues
    missing = set(refs) - set(claims)
    if missing:
        issues.append(f"{label}: unknown proof claim ids {sorted(missing)}")
    if positive:
        bad = [ref for ref in refs if claims.get(ref, {}).get("result") not in GOOD_RESULTS]
        if bad:
            issues.append(f"{label}: requires observed/passed claims, got {sorted(bad)}")
    return issues


def subject_claim_issues(refs, label, claims, subject_type, subject_id, required_types,
                         *, allowed_negative_types=frozenset()):
    issues = claim_refs_issues(refs, label, claims, non_empty=True)
    if not isinstance(refs, list) or not all(isinstance(ref, str) for ref in refs):
        return issues
    matched, disallowed_negative = set(), []
    for ref in refs:
        record = claims.get(ref)
        if not isinstance(record, dict):
            continue
        if record.get("subject_type") != subject_type or record.get("subject_id") != subject_id:
            issues.append(f"{label}: proof claim {ref!r} is bound to a different subject")
            continue
        values = record.get("proof_types")
        if isinstance(values, list) and all(isinstance(item, str) for item in values):
            typed_values = set(values)
            if record.get("result") in GOOD_RESULTS:
                matched.update(typed_values)
            elif not typed_values or not typed_values <= set(allowed_negative_types):
                disallowed_negative.append(ref)
    if disallowed_negative:
        issues.append(f"{label}: requires observed/passed claims, got {sorted(disallowed_negative)}")
    missing = set(required_types) - matched
    if missing:
        issues.append(f"{label}: missing required proof types {sorted(missing)}")
    return issues
