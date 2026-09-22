# Forge Attempt request v1

`forge.attempt-request/v1` freezes the authority-neutral Attempt request
declaration shared by Forge Core and forge-runtime. The value binds one full
project/work-item/Attempt scope, exact entity references, control aggregate
versions, an executor declaration, optional context and capability records,
bounded requested effects, resource budgets, a timeout, and an idempotency
key. Approval and effect sets are validated as bounded unique values and are
stored in deterministic order.

Go Forge Core and Rust Runtime consume the same strict fixture. Valid requests
start in `requested`; invalid values return `invalid_value`, while broken
declared relations return `reference_mismatch`. Construction defensively
copies caller values and exposes read-only accessors; it does not resolve a
reference or create durable state.

This is a pure request contract. It performs no database write, clock read,
authentication, enrollment, inventory publication, reservation, target
selection, scheduling, dispatch, Runner transport, process execution,
artifact transfer, or Audit publication. Every authority bit in the fixture
is `false`. The contract does not change ADR-0039 or Proposed ADR-0114; a
future live Attempt admission and execution path requires the separately
accepted P4 execution and security decision.
