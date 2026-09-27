# Forge execution lease release v1

`forge.execution-lease-release/v1` is the metadata-only response produced by
the owner-authenticated scheduler lease release candidate. It binds a
terminal release observation to one owner, Conversation, Run, Attempt, device,
Runner instance, fencing epoch, and release time.

The value is an interoperability envelope. A receiver may validate the
binding, but it must not release a lease, select a device, reserve capacity,
authorize execution, contact or dispatch a Runner, persist a receipt, or
publish Audit. `replayed` may be either value because an exact idempotent
release replay is a valid response. All six authority flags are always false
in this contract.

Receivers must reject unknown, duplicate, missing, or trailing JSON fields,
non-canonical owner and identifier values, unsafe numeric values, zero epoch
or release time, and any binding or authority drift. The canonical fixture is
`fixtures/forge-execution-lease-release-v1.json`.
