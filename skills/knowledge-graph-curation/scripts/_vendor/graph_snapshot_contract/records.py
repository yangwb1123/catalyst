"""ADR-0065 identity-first record sealing and structured set digests."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import domain_digest, self_digest
from .constants import (
    EDGE_DOMAIN, EDGE_IDENTITY_DOMAIN, EXTRACTOR_DOMAIN,
    EXTRACTOR_IDENTITY_DOMAIN, IDENTITY_FIELDS, MAX_SNAPSHOT_BYTES, NODE_DOMAIN,
    NODE_IDENTITY_DOMAIN, SOURCE_DOMAIN, SOURCE_IDENTITY_DOMAIN,
    UNRESOLVED_EDGE_DOMAIN, UNRESOLVED_EDGE_IDENTITY_DOMAIN,
    UNRESOLVED_NODE_DOMAIN, UNRESOLVED_NODE_IDENTITY_DOMAIN,
)

_KINDS = {
    "source": (SOURCE_IDENTITY_DOMAIN, SOURCE_DOMAIN, "graph-source-",
               "source_id", "source_identity_sha256", "source_sha256"),
    "extractor": (EXTRACTOR_IDENTITY_DOMAIN, EXTRACTOR_DOMAIN, "graph-extractor-",
                  "extractor_id", "extractor_identity_sha256", "extractor_sha256"),
    "node": (NODE_IDENTITY_DOMAIN, NODE_DOMAIN, "graph-node-",
             "node_id", "node_identity_sha256", "node_sha256"),
    "edge": (EDGE_IDENTITY_DOMAIN, EDGE_DOMAIN, "graph-edge-",
             "edge_id", "edge_identity_sha256", "edge_sha256"),
    "unresolved_node": (
        UNRESOLVED_NODE_IDENTITY_DOMAIN, UNRESOLVED_NODE_DOMAIN,
        "graph-unresolved-node-", "unresolved_node_id",
        "unresolved_node_identity_sha256", "unresolved_node_sha256"),
    "unresolved_edge": (
        UNRESOLVED_EDGE_IDENTITY_DOMAIN, UNRESOLVED_EDGE_DOMAIN,
        "graph-unresolved-edge-", "unresolved_edge_id",
        "unresolved_edge_identity_sha256", "unresolved_edge_sha256"),
}


def seal_record(kind: str, body: dict[str, object]) -> dict[str, object]:
    """Derive independent identity, final ID, then the ID-bound self digest."""
    identity_domain, record_domain, prefix, id_field, identity_field, self_field = _KINDS[kind]
    try:
        identity = {field: body[field] for field in IDENTITY_FIELDS[kind]}
    except KeyError as error:
        raise ContractError(f"ADR-0065 {kind} identity field missing: {error}") from error
    identity_sha = domain_digest(identity_domain, identity, max_bytes=MAX_SNAPSHOT_BYTES)
    record = dict(body)
    record[id_field] = prefix + identity_sha
    record[identity_field] = identity_sha
    record[self_field] = ""
    record[self_field] = self_digest(
        record_domain, record, self_field, max_bytes=MAX_SNAPSHOT_BYTES)
    return record


def structured_set_digest(
        domain: bytes, items: list[dict[str, object]],
        *, max_items: int | None = None) -> str:
    preimage = {"item_count": len(items), "items": items}
    if max_items is None:
        return domain_digest(domain, preimage, max_bytes=MAX_SNAPSHOT_BYTES)
    return domain_digest(
        domain, preimage, max_bytes=MAX_SNAPSHOT_BYTES,
        array_limit_overrides={("items",): max_items})


def ensure_unique_ids(items: list[dict[str, object]], field: str, label: str) -> None:
    identifiers = [item[field] for item in items]
    if len(identifiers) != len(set(identifiers)):
        raise ContractError(f"ADR-0065 {label} identity collision")


def ensure_global_identity_uniqueness(
        groups: list[tuple[list[dict[str, object]], str]],
        snapshot_identity_sha256: str | None = None) -> None:
    """Reject a digest collision across every identity kind, not just each set."""
    seen: dict[str, str] = {}
    for records, field in groups:
        for record in records:
            digest = record[field]
            previous = seen.get(digest)
            if previous is not None:
                raise ContractError(
                    f"ADR-0065 global identity digest collision: {previous}/{field}")
            seen[digest] = field
    if snapshot_identity_sha256 is not None:
        previous = seen.get(snapshot_identity_sha256)
        if previous is not None:
            raise ContractError(
                f"ADR-0065 global identity digest collision: {previous}/snapshot")


def knowledge_defaults() -> dict[str, object]:
    return {
        "claim_record_ids": [], "data_classification": "unknown",
        "epistemic_status": "derived", "evidence_record_ids": [],
        "freshness_status": "unknown", "lifecycle_status": "unknown",
        "owner_node_ids": [], "owner_status": "unknown",
        "provenance_status": "unknown", "validity_status": "unknown",
    }
