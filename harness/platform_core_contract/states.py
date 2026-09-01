"""Pure Platform Core state vocabulary and edge validation."""

from __future__ import annotations

from types import MappingProxyType
from typing import AbstractSet, Mapping

from .codec import ContractError
from .constants import STATE_INVALID, TRANSITION_INVALID

WORK_ITEM_EDGES = MappingProxyType({
    "draft": frozenset({"planned", "cancelled"}),
    "planned": frozenset({"awaiting_approval", "ready", "cancelled"}),
    "awaiting_approval": frozenset({"ready", "blocked", "cancelled"}),
    "ready": frozenset({"dispatched", "blocked", "cancelled"}),
    "dispatched": frozenset({"running", "blocked", "failed", "uncertain", "cancelled"}),
    "running": frozenset({"verifying", "blocked", "failed", "uncertain", "cancelled"}),
    "verifying": frozenset({"completed", "ready", "blocked", "failed", "uncertain", "cancelled"}),
    "blocked": frozenset({"ready", "failed", "cancelled"}),
    "failed": frozenset({"ready", "cancelled"}),
    "uncertain": frozenset({"ready", "failed", "cancelled"}),
    "completed": frozenset(),
    "cancelled": frozenset(),
})

ATTEMPT_EDGES = MappingProxyType({
    "requested": frozenset({"accepted"}),
    "accepted": frozenset({"starting", "interrupted", "failed", "uncertain"}),
    "starting": frozenset({"running", "interrupted", "failed", "uncertain"}),
    "running": frozenset({"interrupted", "completed", "failed", "uncertain"}),
    "interrupted": frozenset(),
    "completed": frozenset(),
    "failed": frozenset(),
    "uncertain": frozenset(),
})

ACTION_EDGES = MappingProxyType({
    "requested": frozenset({"awaiting_approval", "approved", "rejected", "cancelled"}),
    "awaiting_approval": frozenset({"approved", "rejected", "cancelled"}),
    "approved": frozenset({"started", "cancelled"}),
    "started": frozenset({"finished", "failed", "cancelled", "uncertain"}),
    "finished": frozenset(),
    "rejected": frozenset(),
    "failed": frozenset(),
    "cancelled": frozenset(),
    "uncertain": frozenset(),
})


def validate_work_item_transition(source: object, target: object) -> None:
    """Validate one WorkItem edge without changing state."""
    _validate_transition(source, target, WORK_ITEM_EDGES)


def validate_attempt_transition(source: object, target: object) -> None:
    """Validate one Attempt edge without changing state."""
    _validate_transition(source, target, ATTEMPT_EDGES)


def validate_action_transition(source: object, target: object) -> None:
    """Validate one Action edge without changing state."""
    _validate_transition(source, target, ACTION_EDGES)


def _validate_transition(
        source: object, target: object,
        graph: Mapping[str, AbstractSet[str]]) -> None:
    if type(source) is not str or source not in graph:
        raise ContractError("source state is unsupported", STATE_INVALID)
    if type(target) is not str or target not in graph:
        raise ContractError("target state is unsupported", STATE_INVALID)
    if target not in graph[source]:
        raise ContractError(
            f"transition {source} -> {target} is not declared", TRANSITION_INVALID)
