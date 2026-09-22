# Forge device heartbeat contract v1

`forge-device-heartbeat-contract-v1.json` is a pure transition fixture for
the future Runner heartbeat boundary. Go, Rust, and Flutter consume the same
twelve cases.

The transition receives an already-bound device declaration, an optional prior
Runner instance, one heartbeat signal, a caller-supplied server observation
time, and a bounded lease TTL. It checks the bounded non-empty device and
Runner instance identifiers, known approval states (`pending`, `approved`, or
`revoked`), nonzero incarnation counters, device binding, revoked approval,
initial generation and sequence, generation restart, instance continuity,
monotonic sequence, server-time monotonicity, TTL bounds, and uint64 lease
expiry overflow. Unknown approval states and malformed identifiers fail before
capability validation; a pending declaration remains a pure input and is
blocked by the owner-approval lifecycle boundary.

Capabilities, tenant values, and authority fields are declarations in this
fixture. The Go, Rust, and Flutter pure consumers validate the bounded
capability value and retain it on an accepted observed instance; this remains
declaration handling rather than hardware proof. The consumer does not
authenticate a Runner, read a clock, persist or publish a heartbeat, register
a device, expose a route, reserve capacity, schedule, dispatch, or execute a
process. Every authority bit must remain `false`. ADR-0039 remains
planning-only, ADR-0114 remains Proposed/null, and P4 still requires a
separately Accepted execution/security decision.
