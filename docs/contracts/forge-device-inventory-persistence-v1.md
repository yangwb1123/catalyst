# Pure persisted inventory compare-and-swap and projection v1

`forge-device-inventory-persistence-v1.json` freezes the value-level state
needed before an authoritative inventory store. One state joins an owner tuple,
device approval/cordon/reservation declarations, and a bounded Runner instance
with a monotonic revision. Restore requires the device and Runner IDs to match;
replacement uses an exact revision and detects overflow. Projection evaluates
the fixed caller-supplied time and owner, returning only display status,
freshness, and declared eligibility.

Go Forge Core and Rust Runtime consume the same strict twelve-case fixture for
online, stale, pending, cordoned, offline, revoked, owner, binding, revision,
and overflow behavior. Capabilities are canonicalized and remain observations;
the owner tuple is still unverified input at this boundary.

This is a pure restart/CAS contract. It performs no database write, clock read,
credential verification, enrollment, heartbeat listener, inventory publication,
reservation, target selection, scheduling, transport, Runner execution,
artifact transfer, or Audit publication. All authority bits are `false`. The
contract does not amend ADR-0039 or Proposed-only ADR-0114; a live registry and
execution path require separate accepted architecture and security decisions.
