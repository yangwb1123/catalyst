"""Bundled methodology-document routes for deterministic system types.

The classifier emits one of these closed values.  Keeping the route table in
the analysis package makes ``rules`` and ``assess`` self-contained while the
referenced catalogue remains readable by a later implementation agent.
"""

from .paths import bundled_reference

METHODOLOGY_CATALOG = bundled_reference("methodologies/system-types.md")

SYSTEM_TYPES = (
    "state-machine",
    "event-driven",
    "realtime",
    "search",
    "optimization",
    "knowledge",
    "batch",
    "adaptive",
    "collaboration",
    "deterministic",
)

SYSTEM_TYPE_METHODOLOGY = {
    system_type: [METHODOLOGY_CATALOG] for system_type in SYSTEM_TYPES
}
